// SPDX-License-Identifier: GPL-3.0-or-later
//! Central logic controller for Task operations.
//! This is the single source of truth for background persistence orchestration.
use crate::cache::{Cache, SettingsMeta};
use crate::client::RustyClient;
use crate::client::core::{ProbeResult, PropWriteError};
use crate::config::{
    CFAIT_SETTINGS_PROP, CFAIT_SETTINGS_REV_PROP, Config, PROBE_TTL_SECS, SETTINGS_CATEGORY,
    SETTINGS_SUMMARY, SETTINGS_UID, SettingsPayload, SyncableConfig,
};
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::{PENDING_REFRESH_ETAG, Task, TaskStatus};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use crate::store::TaskStore;
use chrono::{DateTime, Utc};
use serde_json;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

static PERSIST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn get_persist_lock() -> &'static Mutex<()> {
    PERSIST_LOCK.get_or_init(|| Mutex::new(()))
}

// -----------------------------
// Settings synchronization helpers
//
// Settings are synced either as custom WebDAV properties on the default
// calendar collection (preferred; standard CalDAV clients never request or
// display them) or, as a fallback for servers without custom-property
// support, as a CANCELLED VEVENT carrier anchored at the Unix epoch.
// -----------------------------

/// Outcome of comparing the local settings against the remote ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsDecision {
    /// Remote and local agree; nothing to do.
    InSync,
    /// Remote is newer; apply it to the local config.
    SyncDown,
    /// Local is newer (or there is no remote); push it to the remote.
    SyncUp,
}

/// Outcome of a property-based settings sync attempt.
enum PropertySync {
    /// The property path finished. `true` when a remote write happened.
    Done(bool),
    /// The server rejected the property write; fall back to object storage.
    Demoted,
}

/// Decide which way the settings need to flow.
fn decide_settings_sync(
    local_updated_at: i64,
    local_config: &SyncableConfig,
    remote: Option<&SettingsPayload>,
) -> SettingsDecision {
    let Some(remote) = remote else {
        return SettingsDecision::SyncUp;
    };
    if remote.updated_at > local_updated_at
        || (local_updated_at == 0 && remote.config != *local_config)
    {
        SettingsDecision::SyncDown
    } else if local_updated_at > remote.updated_at
        || (local_updated_at > 0 && *local_config != remote.config)
    {
        SettingsDecision::SyncUp
    } else {
        SettingsDecision::InSync
    }
}

/// Pick the payload with the higher `updated_at`. Used to reconcile the
/// property payload against a leftover object carrier during migration.
fn newer_payload(
    a: Option<&SettingsPayload>,
    b: Option<&SettingsPayload>,
) -> Option<SettingsPayload> {
    match (a, b) {
        (Some(x), Some(y)) => {
            if y.updated_at > x.updated_at {
                Some(y.clone())
            } else {
                Some(x.clone())
            }
        }
        (Some(x), None) => Some(x.clone()),
        (None, Some(y)) => Some(y.clone()),
        (None, None) => None,
    }
}

/// Parse a JSON `SettingsPayload`, returning `None` on any error.
fn parse_settings_payload(json: &str) -> Option<SettingsPayload> {
    serde_json::from_str::<SettingsPayload>(json).ok()
}

/// Determine the calendar that should host the settings: the configured
/// default calendar when it is a remote collection, otherwise the first
/// cached remote calendar, otherwise the local default calendar.
fn settings_target_calendar(config: &Config, ctx: &dyn AppContext) -> String {
    let first_remote = || {
        Cache::load_calendars(ctx)
            .unwrap_or_default()
            .into_iter()
            .find(|c| !c.href.starts_with("local://"))
            .map(|c| c.href)
    };
    match &config.default_calendar {
        Some(def) if !def.starts_with("local://") => def.clone(),
        _ => first_remote().unwrap_or_else(|| LOCAL_CALENDAR_HREF.to_string()),
    }
}

/// Load the last known settings carrier from the in-memory store and the
/// disk cache, preferring the higher `sequence`.
fn load_settings_task(ctx: &dyn AppContext, store: &TaskStore) -> Option<Task> {
    let mut from_disk = None;
    if let Ok(cals) = Cache::load_calendars(ctx) {
        for cal in cals {
            if let Ok((tasks, _)) = Cache::load(ctx, &cal.href)
                && let Some(t) = tasks.into_iter().find(|t| t.uid == SETTINGS_UID)
            {
                from_disk = Some(t);
                break;
            }
        }
    }
    let from_store = store.get_task_ref(SETTINGS_UID).cloned();
    match (from_store, from_disk) {
        (Some(mut s), Some(d)) => {
            if d.sequence > s.sequence {
                s = d;
            }
            Some(s)
        }
        (Some(s), None) => Some(s),
        (None, d) => d,
    }
}

/// Build a fresh settings carrier task for the given calendar.
fn settings_carrier_task(href: &str, json: &str) -> Task {
    let mut task = Task::new(SETTINGS_SUMMARY, &std::collections::HashMap::new(), None);
    task.uid = SETTINGS_UID.to_string();
    task.status = TaskStatus::Cancelled;
    task.description = json.to_string();
    task.categories = vec![SETTINGS_CATEGORY.to_string()];
    task.calendar_href = href.to_string();
    task.is_event = true;
    task
}

/// Apply the carrier markers that keep the settings object hidden and
/// recognizable: cancelled status, VEVENT serialization, internal category.
fn normalize_carrier(task: &mut Task) {
    task.uid = SETTINGS_UID.to_string();
    task.status = TaskStatus::Cancelled;
    task.is_event = true;
    task.is_journal = false;
    if !task.summary.starts_with("⚙ Cfait Settings") {
        task.summary = SETTINGS_SUMMARY.to_string();
    }
    if !task.categories.iter().any(|c| c == SETTINGS_CATEGORY) {
        task.categories.push(SETTINGS_CATEGORY.to_string());
    }
}

/// Build the carrier task to push and whether it is a fresh Create.
///
/// Prefers the remote object (carries the server href/etag) as the base.
/// Without one, falls back to the local store copy when it lives in the
/// target calendar; `remote_known_absent` (an online fetch that found
/// nothing) forces a fresh Create, while an offline call keeps the existing
/// href/etag so the update can use If-Match (a 404 self-heals to a Create).
fn build_carrier_task(
    existing_task: Option<&Task>,
    remote_object: Option<&Task>,
    remote_known_absent: bool,
    target: &str,
    json: &str,
) -> (Task, bool) {
    let (mut t, is_new) = match remote_object {
        Some(remote) => (remote.clone(), false),
        None => match existing_task {
            Some(existing) if existing.calendar_href == target => {
                if remote_known_absent || existing.href.is_empty() {
                    let mut fresh = existing.clone();
                    fresh.href = String::new();
                    fresh.etag = String::new();
                    (fresh, true)
                } else {
                    (existing.clone(), false)
                }
            }
            _ => (settings_carrier_task(target, json), true),
        },
    };
    t.uid = SETTINGS_UID.to_string();
    t.description = json.to_string();
    t.sequence += 1;
    normalize_carrier(&mut t);
    (t, is_new)
}

/// Resolve whether the target calendar supports custom properties, using the
/// cached probe result when fresh and probing otherwise. Returns
/// `Some(bool)` for a definitive answer and `None` when the probe was
/// transient and no prior information exists (caller should skip the cycle).
async fn resolve_property_support(
    client: &RustyClient,
    meta: &mut SettingsMeta,
    target: &str,
    now: i64,
) -> Option<bool> {
    let cached = meta.probed.get(target).copied();
    let fresh = match (cached, meta.probed_at.get(target)) {
        (Some(_), Some(&at)) => now - at < PROBE_TTL_SECS,
        _ => false,
    };
    if let Some(c) = cached
        && fresh
    {
        return Some(c);
    }

    match client.probe_custom_property_support(target).await {
        ProbeResult::Supported => {
            meta.probed.insert(target.to_string(), true);
            meta.probed_at.insert(target.to_string(), now);
            Some(true)
        }
        ProbeResult::Unsupported(reason) => {
            log::info!(
                "Settings sync: custom properties unsupported on {}: {}",
                target,
                reason
            );
            meta.probed.insert(target.to_string(), false);
            meta.probed_at.insert(target.to_string(), now);
            Some(false)
        }
        ProbeResult::Transient(reason) => {
            log::warn!("Settings sync: property probe transient error: {}", reason);
            cached // fall back to the (stale) cache, if any
        }
    }
}

/// Central logic controller for Task operations.
/// Handles business workflows and coordinates in-memory store mutations,
/// network client interactions and the journaling fallback used for offline-safe writes.
#[derive(Clone)]
pub struct TaskController {
    pub store: Arc<Mutex<TaskStore>>,
    pub client: Arc<Mutex<Option<RustyClient>>>,
    pub ctx: Arc<dyn AppContext>,
    pub undo_history: Arc<Mutex<crate::journal::UndoHistory>>,
}

impl TaskController {
    pub fn new(
        store: Arc<Mutex<TaskStore>>,
        client: Arc<Mutex<Option<RustyClient>>>,
        ctx: Arc<dyn AppContext>,
    ) -> Self {
        Self {
            store,
            client,
            ctx,
            undo_history: Arc::new(Mutex::new(crate::journal::UndoHistory::new())),
        }
    }

    /// Process a batch of actions atomically to ensure proper journal queueing.
    /// This is an instantaneous operation that saves to disk and returns without hitting the network.
    pub async fn persist_changes(&self, actions: Vec<Action>) -> Result<(), String> {
        let _persist_guard = get_persist_lock().lock().await;
        let mut remote_actions = Vec::new();

        enum LocalOp {
            Upsert(Box<Task>),
            Delete(String),
        }

        let mut local_ops_by_href: std::collections::HashMap<String, Vec<LocalOp>> =
            std::collections::HashMap::new();

        for action in actions {
            // Prevent Data-Loss: Ensure Trash calendar is registered on disk during a trash-create event
            if let Action::Create(ref t) | Action::Update(ref t) = action
                && t.calendar_href == crate::storage::LOCAL_TRASH_HREF
            {
                let _ = LocalCalendarRegistry::ensure_trash_calendar_exists(self.ctx.as_ref());
            }

            match action {
                Action::Create(t) => {
                    if t.calendar_href.starts_with("local://") {
                        local_ops_by_href
                            .entry(t.calendar_href.clone())
                            .or_default()
                            .push(LocalOp::Upsert(Box::new(t)));
                    } else {
                        remote_actions.push(Action::Create(t));
                    }
                }
                Action::Update(t) => {
                    if t.calendar_href.starts_with("local://") {
                        local_ops_by_href
                            .entry(t.calendar_href.clone())
                            .or_default()
                            .push(LocalOp::Upsert(Box::new(t)));
                    } else {
                        remote_actions.push(Action::Update(t));
                    }
                }
                Action::Delete(t) => {
                    if t.calendar_href.starts_with("local://") {
                        local_ops_by_href
                            .entry(t.calendar_href.clone())
                            .or_default()
                            .push(LocalOp::Delete(t.uid.clone()));
                    } else {
                        remote_actions.push(Action::Delete(t));
                    }
                }
                Action::Move(t, target_href) => {
                    if t.calendar_href.starts_with("local://") {
                        local_ops_by_href
                            .entry(t.calendar_href.clone())
                            .or_default()
                            .push(LocalOp::Delete(t.uid.clone()));
                    } else {
                        remote_actions.push(Action::Move(t.clone(), target_href.clone()));
                    }

                    if target_href.starts_with("local://") {
                        let mut moved = t.clone();
                        moved.calendar_href = target_href.clone();
                        local_ops_by_href
                            .entry(target_href)
                            .or_default()
                            .push(LocalOp::Upsert(Box::new(moved)));
                    }
                }
            }
        }

        let mut first_local_err: Option<String> = None;
        for (href, ops) in local_ops_by_href {
            if let Err(e) = LocalStorage::modify_for_href(self.ctx.as_ref(), &href, |all| {
                for op in ops {
                    match op {
                        LocalOp::Upsert(task) => {
                            if let Some(idx) = all.iter().position(|item| item.uid == task.uid) {
                                all[idx] = *task;
                            } else {
                                all.push(*task);
                            }
                        }
                        LocalOp::Delete(uid) => {
                            all.retain(|item| item.uid != uid);
                        }
                    }
                }
            }) && first_local_err.is_none()
            {
                first_local_err = Some(e.to_string());
            }
        }

        if remote_actions.is_empty() {
            return first_local_err.map_or(Ok(()), Err);
        }

        {
            let mut store = self.store.lock().await;
            for action in &remote_actions {
                let uid = match action {
                    Action::Create(t) | Action::Update(t) | Action::Delete(t) => &t.uid,
                    Action::Move(t, _) => &t.uid,
                };
                if let Some((existing, _)) = store.get_task_mut(uid)
                    && existing.etag.is_empty()
                {
                    existing.etag = PENDING_REFRESH_ETAG.to_string();
                }
            }
        }

        Journal::modify(self.ctx.as_ref(), |journal| {
            journal.queue.extend(remote_actions);
            let mut tmp_j = Journal {
                queue: std::mem::take(&mut journal.queue),
                ..Default::default()
            };
            tmp_j.compact();
            journal.queue = tmp_j.queue;
        })
        .map_err(|e| e.to_string())?;

        // Local disk failures are reported after the remote actions have been
        // journaled, so a failed local write never loses queued remote work.
        first_local_err.map(Err).unwrap_or(Ok(()))
    }

    /// Synchronizes the configuration and aliases.
    ///
    /// Preferred: custom WebDAV properties on the default calendar collection
    /// (standard CalDAV clients never request or display them). Fallback for
    /// servers without custom-property support: a 1970 CANCELLED VEVENT
    /// carrier that most clients filter out by time range. The target
    /// calendar is re-derived on every call, so a deleted or changed default
    /// collection heals itself on the next cycle.
    pub async fn sync_settings(&self) -> Result<bool, String> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        if !config.sync_settings {
            return Ok(false);
        }

        let mut meta = Cache::load_settings_meta(self.ctx.as_ref());
        let target = settings_target_calendar(&config, self.ctx.as_ref());
        let client = self.client.lock().await.clone();
        let now = Utc::now().timestamp();

        let existing_task = {
            let store = self.store.lock().await;
            load_settings_task(self.ctx.as_ref(), &store)
        };

        // Local-only target: keep the settings in the local carrier object.
        if target.starts_with("local://") {
            return self
                .push_local_settings_to_carrier(&mut config, existing_task)
                .await;
        }

        // Offline: in property mode the settings already live on the server;
        // otherwise keep the carrier object up to date so it is journaled on
        // reconnect.
        let Some(client) = client else {
            let property_mode = meta.property_href.as_deref() == Some(target.as_str())
                || meta.probed.get(&target) == Some(&true);
            if property_mode {
                return Ok(false);
            }
            return self
                .push_local_settings_to_carrier(&mut config, existing_task)
                .await;
        };

        // The settings property may live on a calendar that is no longer the
        // target (collection deleted, default changed): remove it best-effort.
        if let Some(old) = meta.property_href.clone()
            && old != target
        {
            let _ = client.remove_property(&old, CFAIT_SETTINGS_PROP).await;
            let _ = client.remove_property(&old, CFAIT_SETTINGS_REV_PROP).await;
            meta.property_href = None;
        }

        let mut use_property = meta.property_href.as_deref() == Some(target.as_str());
        if !use_property {
            match resolve_property_support(&client, &mut meta, &target, now).await {
                Some(supported) => use_property = supported,
                None => {
                    // Transient probe with no prior information: skip the cycle.
                    return Ok(false);
                }
            }
        }

        let changed = if use_property {
            match self
                .sync_settings_property(
                    &client,
                    &mut config,
                    existing_task.clone(),
                    &target,
                    &mut meta,
                )
                .await
            {
                PropertySync::Done(did_write) => did_write,
                PropertySync::Demoted => {
                    // The server rejected the property write: fall back to the
                    // carrier object within this same cycle.
                    meta.property_href = None;
                    meta.probed.insert(target.clone(), false);
                    meta.probed_at.insert(target.clone(), now);
                    self.sync_settings_object(&client, &mut config, existing_task, &target)
                        .await?
                }
            }
        } else {
            self.sync_settings_object(&client, &mut config, existing_task, &target)
                .await?
        };

        let _ = Cache::save_settings_meta(self.ctx.as_ref(), &meta);
        Ok(changed)
    }

    /// Settings sync via custom WebDAV properties on the target calendar.
    async fn sync_settings_property(
        &self,
        client: &RustyClient,
        config: &mut Config,
        existing_task: Option<Task>,
        target: &str,
        meta: &mut SettingsMeta,
    ) -> PropertySync {
        // Read the current property payload (None when the calendar or the
        // property is gone).
        let remote_payload = match client
            .propfind_custom_props(target, &[CFAIT_SETTINGS_PROP, CFAIT_SETTINGS_REV_PROP])
            .await
        {
            Err(e) => {
                log::warn!("Settings sync: PROPFIND failed on {}: {}", target, e);
                return PropertySync::Done(false);
            }
            Ok(None) => None,
            Ok(Some(props)) => props
                .get(CFAIT_SETTINGS_PROP)
                .and_then(|json| parse_settings_payload(json)),
        };

        // Migration guard: a leftover carrier object (legacy VTODO or a
        // VEVENT from an earlier version) may hold a newer payload; take the
        // newer of the two.
        let remote = match client.fetch_settings_object(target).await {
            Ok(Some(obj)) => {
                let obj_payload = parse_settings_payload(&obj.description);
                newer_payload(remote_payload.as_ref(), obj_payload.as_ref())
            }
            Ok(None) => remote_payload,
            Err(e) => {
                log::warn!("Settings sync: failed to fetch settings object: {}", e);
                remote_payload
            }
        };

        let local_syncable = config.get_syncable();
        let decision =
            decide_settings_sync(config.settings_updated_at, &local_syncable, remote.as_ref());

        match decision {
            SettingsDecision::InSync => {
                meta.property_href = Some(target.to_string());
                self.cleanup_stale_carrier(existing_task).await;
                PropertySync::Done(false)
            }
            SettingsDecision::SyncDown => {
                let remote_payload = remote.expect("SyncDown implies a remote payload");
                config.apply_syncable(remote_payload.config.clone());
                config.settings_updated_at = if remote_payload.updated_at == 0 {
                    Utc::now().timestamp()
                } else {
                    remote_payload.updated_at
                };
                let _ = config.save(self.ctx.as_ref());

                // Re-anchor the property so the server copy carries the
                // stamped revision.
                let json = serde_json::to_string(&SettingsPayload {
                    updated_at: config.settings_updated_at,
                    config: remote_payload.config.clone(),
                })
                .unwrap_or_default();
                if self
                    .write_settings_property(client, target, &json, config.settings_updated_at)
                    .await
                    .is_err()
                {
                    meta.property_href = None;
                    return PropertySync::Demoted;
                }
                meta.property_href = Some(target.to_string());
                self.cleanup_stale_carrier(existing_task).await;
                self.apply_aliases_and_persist(&remote_payload.config).await;
                PropertySync::Done(true)
            }
            SettingsDecision::SyncUp => {
                if config.settings_updated_at == 0 {
                    config.settings_updated_at = Utc::now().timestamp();
                    let _ = config.save(self.ctx.as_ref());
                }
                let json = serde_json::to_string(&SettingsPayload {
                    updated_at: config.settings_updated_at,
                    config: local_syncable,
                })
                .unwrap_or_default();
                if self
                    .write_settings_property(client, target, &json, config.settings_updated_at)
                    .await
                    .is_err()
                {
                    meta.property_href = None;
                    return PropertySync::Demoted;
                }
                meta.property_href = Some(target.to_string());
                self.cleanup_stale_carrier(existing_task).await;
                PropertySync::Done(true)
            }
        }
    }

    /// PROPPATCH the settings payload and revision onto the calendar,
    /// classifying the failure so the caller can demote to object storage.
    async fn write_settings_property(
        &self,
        client: &RustyClient,
        target: &str,
        json: &str,
        rev: i64,
    ) -> Result<(), PropWriteError> {
        let res = client
            .proppatch_custom_props(
                target,
                &[
                    (CFAIT_SETTINGS_PROP.to_string(), json.to_string()),
                    (CFAIT_SETTINGS_REV_PROP.to_string(), rev.to_string()),
                ],
                &[],
            )
            .await;
        if let Err(e) = &res {
            log::warn!("Settings sync: PROPPATCH failed on {}: {:?}", target, e);
        }
        res
    }

    /// Remove a leftover settings carrier object (legacy VTODO or a VEVENT
    /// from fallback mode) from the store and, when it was previously synced,
    /// from the server.
    async fn cleanup_stale_carrier(&self, existing_task: Option<Task>) {
        let Some(task) = existing_task else {
            return;
        };
        let mut store = self.store.lock().await;
        store.delete_task(&task.uid);
        drop(store);
        if !task.href.is_empty() {
            let _ = self.persist_changes(vec![Action::Delete(task)]).await;
        }
    }

    /// Apply tag aliases to existing tasks and persist the resulting updates.
    async fn apply_aliases_and_persist(&self, sync: &SyncableConfig) {
        let modified = {
            let mut store = self.store.lock().await;
            let mut modified = Vec::new();
            for (key, values) in &sync.tag_aliases {
                modified.extend(store.apply_alias_retroactively(key, values));
            }
            modified
        };
        if !modified.is_empty() {
            let actions = modified.into_iter().map(Action::Update).collect();
            let _ = self.persist_changes(actions).await;
        }
    }

    /// Settings sync via the carrier object (1970 CANCELLED VEVENT) in the
    /// target calendar. Used when the server does not support custom
    /// properties.
    async fn sync_settings_object(
        &self,
        client: &RustyClient,
        config: &mut Config,
        existing_task: Option<Task>,
        target: &str,
    ) -> Result<bool, String> {
        // Best-effort: drop a carrier left in a calendar that is no longer the
        // target (collection deleted or default changed).
        let stale_in_other_cal = existing_task
            .as_ref()
            .filter(|t| !t.calendar_href.starts_with("local://") && t.calendar_href != target)
            .cloned();
        if let Some(stale) = &stale_in_other_cal
            && !stale.href.is_empty()
        {
            let mut store = self.store.lock().await;
            store.delete_task(&stale.uid);
            drop(store);
            let _ = self
                .persist_changes(vec![Action::Delete(stale.clone())])
                .await;
        }

        let remote_object = match client.fetch_settings_object(target).await {
            Ok(obj) => obj,
            Err(e) => {
                log::warn!("Settings sync: settings object fetch failed: {}", e);
                return Ok(false);
            }
        };

        let remote_payload = remote_object
            .as_ref()
            .and_then(|t| parse_settings_payload(&t.description));
        let local_syncable = config.get_syncable();
        let decision = decide_settings_sync(
            config.settings_updated_at,
            &local_syncable,
            remote_payload.as_ref(),
        );

        match decision {
            SettingsDecision::InSync => Ok(false),
            SettingsDecision::SyncDown => {
                let remote_payload = remote_payload.expect("SyncDown implies a remote payload");
                config.apply_syncable(remote_payload.config.clone());
                config.settings_updated_at = if remote_payload.updated_at == 0 {
                    Utc::now().timestamp()
                } else {
                    remote_payload.updated_at
                };
                let _ = config.save(self.ctx.as_ref());
                self.apply_aliases_and_persist(&remote_payload.config).await;

                // Re-anchor the carrier when it lacks a revision or still uses
                // the legacy VTODO format (migrate it to the invisible VEVENT).
                let needs_reanchor = remote_payload.updated_at == 0
                    || remote_object.as_ref().is_some_and(|t| !t.is_event);
                if needs_reanchor {
                    let json = serde_json::to_string(&SettingsPayload {
                        updated_at: config.settings_updated_at,
                        config: remote_payload.config,
                    })
                    .unwrap_or_default();
                    self.push_carrier(existing_task, remote_object.as_ref(), false, target, &json)
                        .await?;
                }
                Ok(true)
            }
            SettingsDecision::SyncUp => {
                if config.settings_updated_at == 0 {
                    config.settings_updated_at = Utc::now().timestamp();
                    let _ = config.save(self.ctx.as_ref());
                }
                let json = serde_json::to_string(&SettingsPayload {
                    updated_at: config.settings_updated_at,
                    config: local_syncable,
                })
                .unwrap_or_default();
                self.push_carrier(
                    existing_task,
                    remote_object.as_ref(),
                    remote_object.is_none(),
                    target,
                    &json,
                )
                .await?;
                Ok(true)
            }
        }
    }

    /// Build the carrier task to push (Create or Update) and persist it.
    async fn push_carrier(
        &self,
        existing_task: Option<Task>,
        remote_object: Option<&Task>,
        remote_known_absent: bool,
        target: &str,
        json: &str,
    ) -> Result<bool, String> {
        let (task, is_new) = build_carrier_task(
            existing_task.as_ref(),
            remote_object,
            remote_known_absent,
            target,
            json,
        );
        let mut store = self.store.lock().await;
        store.update_or_add_task(task.clone());
        drop(store);
        let action = if is_new {
            Action::Create(task)
        } else {
            Action::Update(task)
        };
        self.persist_changes(vec![action]).await?;
        Ok(true)
    }

    /// Keep the settings in the local carrier object: used for the local-only
    /// target and for offline cycles in fallback (object) mode.
    async fn push_local_settings_to_carrier(
        &self,
        config: &mut Config,
        existing_task: Option<Task>,
    ) -> Result<bool, String> {
        let remote_payload = existing_task
            .as_ref()
            .and_then(|t| parse_settings_payload(&t.description));
        let local_syncable = config.get_syncable();
        let target = settings_target_calendar(config, self.ctx.as_ref());

        match decide_settings_sync(
            config.settings_updated_at,
            &local_syncable,
            remote_payload.as_ref(),
        ) {
            SettingsDecision::InSync => Ok(false),
            SettingsDecision::SyncDown => {
                let remote_payload = remote_payload.expect("SyncDown implies a remote payload");
                config.apply_syncable(remote_payload.config.clone());
                config.settings_updated_at = if remote_payload.updated_at == 0 {
                    Utc::now().timestamp()
                } else {
                    remote_payload.updated_at
                };
                let _ = config.save(self.ctx.as_ref());
                self.apply_aliases_and_persist(&remote_payload.config).await;

                if remote_payload.updated_at == 0 {
                    // Re-anchor the carrier with a stamped revision.
                    let json = serde_json::to_string(&SettingsPayload {
                        updated_at: config.settings_updated_at,
                        config: remote_payload.config,
                    })
                    .unwrap_or_default();
                    self.push_carrier(existing_task, None, false, &target, &json)
                        .await?;
                }
                Ok(true)
            }
            SettingsDecision::SyncUp => {
                if config.settings_updated_at == 0 {
                    config.settings_updated_at = Utc::now().timestamp();
                    let _ = config.save(self.ctx.as_ref());
                }
                let json = serde_json::to_string(&SettingsPayload {
                    updated_at: config.settings_updated_at,
                    config: config.get_syncable(),
                })
                .unwrap_or_default();
                self.push_carrier(existing_task, None, false, &target, &json)
                    .await?;
                Ok(true)
            }
        }
    }

    /// Synchronize the journal with the remote server and update the in-memory store
    /// with the resulting ETags and URLs.
    pub async fn sync_and_update_store(&self) -> Result<(Vec<String>, Vec<Task>, bool), String> {
        // 1. Inject the settings synchronization cycle FIRST, so if it creates a settings task,
        // it gets pushed to the journal before we upload the journal to the server!
        let mut config_changed = self.sync_settings().await.unwrap_or(false);

        let client_opt = self.client.lock().await.clone();

        let (warns, actual_synced) = if let Some(ref client) = client_opt {
            match client.sync_journal().await {
                Ok((w, s)) => {
                    let mut st = self.store.lock().await;
                    let mut actual = Vec::new();
                    let mut to_delete = Vec::new();

                    for sync_task in &s {
                        if sync_task.summary.starts_with("⚙ Cfait Settings")
                            && sync_task.summary.ends_with("(Conflict Copy)")
                        {
                            to_delete.push(sync_task.clone());
                            continue; // Prevent it from entering the store
                        }

                        if let Some((existing, _)) = st.get_task_mut(&sync_task.uid) {
                            existing.etag = sync_task.etag.clone();
                            existing.href = sync_task.href.clone();
                            actual.push(sync_task.clone());
                        } else if sync_task.summary.ends_with("(Conflict Copy)") {
                            // Safe to resurrect because it is a new server-generated conflict resolution
                            st.add_task(sync_task.clone());
                            actual.push(sync_task.clone());
                        } else {
                            // Catch-all: Ensure entirely new tasks fetched from server (like settings) enter the store
                            st.add_task(sync_task.clone());
                            actual.push(sync_task.clone());
                        }
                    }
                    drop(st);

                    if !to_delete.is_empty() {
                        let actions = to_delete.into_iter().map(Action::Delete).collect();
                        let _ = self.persist_changes(actions).await;
                    }

                    // Update Cache to reflect successful uploads, preventing 3-way merge failures
                    // if the user edits the task again before a full sync.
                    let mut by_calendar: std::collections::HashMap<String, Vec<Task>> =
                        std::collections::HashMap::new();
                    for t in &actual {
                        if !t.calendar_href.starts_with("local://") {
                            by_calendar
                                .entry(t.calendar_href.clone())
                                .or_default()
                                .push(t.clone());
                        }
                    }

                    for (href, tasks) in by_calendar {
                        if let Ok((mut cached, token)) =
                            crate::cache::Cache::load(self.ctx.as_ref(), &href)
                        {
                            let mut changed = false;
                            for t in tasks {
                                if let Some(idx) = cached.iter().position(|x| x.uid == t.uid) {
                                    cached[idx] = t;
                                    changed = true;
                                } else {
                                    cached.push(t);
                                    changed = true;
                                }
                            }
                            if changed {
                                let _ = crate::cache::Cache::save(
                                    self.ctx.as_ref(),
                                    &href,
                                    &cached,
                                    token,
                                );
                            }
                        }
                    }

                    (w, actual)
                }
                Err(e) => return Err(e),
            }
        } else {
            (
                vec![rust_i18n::t!("offline_changes_queued").to_string()],
                vec![],
            )
        };

        // 2. Run settings synchronization AGAIN so we instantly pick up any remote changes
        // downloaded during the sync_journal pass.
        let q_len_before = Journal::load(self.ctx.as_ref()).queue.len();

        if self.sync_settings().await.unwrap_or(false) {
            config_changed = true;
        }

        let q_len_after = Journal::load(self.ctx.as_ref()).queue.len();
        if q_len_after > q_len_before {
            // sync_settings pushed a new action (likely Action::Update for the settings task).
            // We must flush the journal again immediately so it doesn't get stuck!
            if let Some(client) = client_opt
                && let Ok((_w, s)) = client.sync_journal().await
            {
                let mut st = self.store.lock().await;
                for sync_task in &s {
                    if let Some((existing, _)) = st.get_task_mut(&sync_task.uid) {
                        existing.etag = sync_task.etag.clone();
                        existing.href = sync_task.href.clone();
                    }
                }
            }
        }

        Ok((warns, actual_synced, config_changed))
    }

    pub async fn create_task(&self, mut task: Task) -> Result<String, String> {
        if crate::storage::is_system_calendar(&task.calendar_href) {
            task.calendar_href = crate::storage::LOCAL_CALENDAR_HREF.to_string();
        }
        if !task.calendar_href.starts_with("local://") {
            let cal_path = task.calendar_href.clone();
            let filename = format!("{}.ics", task.uid);
            let full_href = if cal_path.ends_with('/') {
                format!("{}{}", cal_path, filename)
            } else {
                format!("{}/{}", cal_path, filename)
            };
            task.href = full_href;
        }

        // Persist to disk FIRST to guarantee data integrity. If this fails,
        // we return an error and the UI will NOT clear the text input.
        self.persist_changes(vec![Action::Create(task.clone())])
            .await?;

        self.store.lock().await.add_task(task.clone());
        Ok(task.uid)
    }

    pub async fn update_task(&self, mut task: Task) -> Result<Vec<String>, String> {
        task.sequence += 1;

        // Persist to disk FIRST to guarantee data integrity.
        self.persist_changes(vec![Action::Update(task.clone())])
            .await?;

        let mut store = self.store.lock().await;
        store.update_or_add_task(task);
        drop(store);
        Ok(vec![])
    }

    /// Drop `tasks` from the in-memory store and persist their deletion to
    /// disk, returning how many were purged.
    async fn purge_tasks(&self, tasks: Vec<Task>) -> Result<usize, String> {
        let count = tasks.len();
        if count == 0 {
            return Ok(0);
        }

        // Drop them from the in-memory store too, so the UI reflects the purge.
        {
            let mut store = self.store.lock().await;
            for task in &tasks {
                let _ = store.delete_task(&task.uid);
            }
        }

        let actions = tasks.into_iter().map(Action::Delete).collect();
        self.persist_changes(actions).await?;
        Ok(count)
    }

    pub async fn empty_trash(&self) -> Result<usize, String> {
        // Enumerate from disk so items created by another instance (CLI,
        // background daemon) are purged even if our in-memory store never
        // loaded them.
        let disk_trash =
            LocalStorage::load_for_href(self.ctx.as_ref(), crate::storage::LOCAL_TRASH_HREF)
                .map_err(|e| e.to_string())?;
        self.purge_tasks(disk_trash).await
    }

    pub async fn prune_trash(&self) -> Result<usize, String> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let retention_days = config.trash_retention_days as i64;
        if retention_days == 0 {
            return Ok(0);
        }

        // Enumerate from disk so retention pruning also works when the
        // in-memory store is empty (e.g. TUI startup before the first load).
        let disk_trash =
            LocalStorage::load_for_href(self.ctx.as_ref(), crate::storage::LOCAL_TRASH_HREF)
                .map_err(|e| e.to_string())?;

        let now = Utc::now();
        let mut purged_tasks = Vec::new();
        for task in disk_trash {
            // Tasks without a parseable X-TRASHED-DATE are kept, so a
            // missing property never causes data loss.
            if let Some(prop) = task
                .unmapped_properties
                .iter()
                .find(|p| p.key == "X-TRASHED-DATE")
                && let Ok(dt) = DateTime::parse_from_rfc3339(&prop.value)
            {
                let age_days = (now - dt.with_timezone(&Utc)).num_days();
                if age_days >= retention_days {
                    purged_tasks.push(task);
                }
            }
        }

        self.purge_tasks(purged_tasks).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::TestContext;
    use crate::model::CalendarListEntry;

    fn payload(updated_at: i64, calendar: Option<&str>) -> SettingsPayload {
        let config = SyncableConfig {
            default_calendar: calendar.map(|c| c.to_string()),
            ..Default::default()
        };
        SettingsPayload { updated_at, config }
    }

    #[test]
    fn decide_no_remote_is_sync_up() {
        let local = SyncableConfig::default();
        assert_eq!(
            decide_settings_sync(5, &local, None),
            SettingsDecision::SyncUp
        );
    }

    #[test]
    fn decide_remote_newer_is_sync_down() {
        let local = SyncableConfig::default();
        let remote = payload(10, None);
        assert_eq!(
            decide_settings_sync(5, &local, Some(&remote)),
            SettingsDecision::SyncDown
        );
    }

    #[test]
    fn decide_local_newer_is_sync_up() {
        let local = SyncableConfig::default();
        let remote = payload(5, None);
        assert_eq!(
            decide_settings_sync(10, &local, Some(&remote)),
            SettingsDecision::SyncUp
        );
    }

    #[test]
    fn decide_equal_rev_equal_config_is_in_sync() {
        let local = SyncableConfig::default();
        let remote = payload(5, None);
        assert_eq!(
            decide_settings_sync(5, &local, Some(&remote)),
            SettingsDecision::InSync
        );
    }

    #[test]
    fn decide_tie_with_diff_content_and_unstamped_local_prefers_remote() {
        let local = SyncableConfig::default();
        let remote = payload(5, Some("/calendars/u/other"));
        assert_eq!(
            decide_settings_sync(0, &local, Some(&remote)),
            SettingsDecision::SyncDown
        );
    }

    #[test]
    fn decide_tie_with_diff_content_and_stamped_local_prefers_local() {
        let local = SyncableConfig::default();
        let remote = payload(5, Some("/calendars/u/other"));
        assert_eq!(
            decide_settings_sync(5, &local, Some(&remote)),
            SettingsDecision::SyncUp
        );
    }

    #[test]
    fn newer_payload_picks_higher_revision() {
        let a = payload(5, None);
        let b = payload(9, None);
        assert_eq!(newer_payload(Some(&a), Some(&b)).unwrap().updated_at, 9);
        assert_eq!(newer_payload(Some(&b), Some(&a)).unwrap().updated_at, 9);
    }

    #[test]
    fn newer_payload_tie_prefers_first() {
        let a = payload(5, Some("/a"));
        let b = payload(5, Some("/b"));
        assert_eq!(
            newer_payload(Some(&a), Some(&b))
                .unwrap()
                .config
                .default_calendar
                .as_deref(),
            Some("/a")
        );
        assert_eq!(
            newer_payload(Some(&b), Some(&a))
                .unwrap()
                .config
                .default_calendar
                .as_deref(),
            Some("/b")
        );
    }

    #[test]
    fn newer_payload_handles_none() {
        assert!(newer_payload(None, None).is_none());
        let a = payload(1, None);
        assert_eq!(newer_payload(Some(&a), None).unwrap().updated_at, 1);
        assert_eq!(newer_payload(None, Some(&a)).unwrap().updated_at, 1);
    }

    #[test]
    fn parse_settings_payload_round_trips_and_rejects_garbage() {
        let p = payload(42, Some("/calendars/u/cal"));
        let json = serde_json::to_string(&p).unwrap();
        let parsed = parse_settings_payload(&json).expect("valid json parses");
        assert_eq!(parsed.updated_at, 42);
        assert_eq!(
            parsed.config.default_calendar.as_deref(),
            Some("/calendars/u/cal")
        );
        assert!(parse_settings_payload("not json").is_none());
        assert!(parse_settings_payload("").is_none());
    }

    #[test]
    fn target_calendar_prefers_remote_default() {
        let ctx = Arc::new(TestContext::new());
        let config = Config {
            default_calendar: Some("/calendars/u/main".into()),
            ..Default::default()
        };
        assert_eq!(
            settings_target_calendar(&config, ctx.as_ref()),
            "/calendars/u/main"
        );
    }

    #[test]
    fn target_calendar_falls_back_to_first_cached_remote() {
        let ctx = Arc::new(TestContext::new());
        let config = Config {
            default_calendar: Some(LOCAL_CALENDAR_HREF.to_string()),
            ..Default::default()
        };
        let cals = vec![
            CalendarListEntry {
                name: "Trash".into(),
                href: crate::storage::LOCAL_TRASH_HREF.to_string(),
                color: None,
                supports_vjournal: None,
            },
            CalendarListEntry {
                name: "Work".into(),
                href: "/calendars/u/work".into(),
                color: None,
                supports_vjournal: None,
            },
        ];
        Cache::save_calendars(ctx.as_ref(), &cals).unwrap();
        assert_eq!(
            settings_target_calendar(&config, ctx.as_ref()),
            "/calendars/u/work"
        );
    }

    #[test]
    fn target_calendar_local_default_when_no_remote_cals() {
        let ctx = Arc::new(TestContext::new());
        let config = Config::default();
        assert_eq!(
            settings_target_calendar(&config, ctx.as_ref()),
            LOCAL_CALENDAR_HREF.to_string()
        );
    }

    #[test]
    fn carrier_task_has_hidden_markers() {
        let t = settings_carrier_task("/calendars/u/cal", "{\"updated_at\":1}");
        assert_eq!(t.uid, SETTINGS_UID);
        assert_eq!(t.status, TaskStatus::Cancelled);
        assert!(t.is_event);
        assert!(!t.is_journal);
        assert_eq!(t.summary, SETTINGS_SUMMARY);
        assert!(t.categories.contains(&SETTINGS_CATEGORY.to_string()));
        assert_eq!(t.calendar_href, "/calendars/u/cal");
        assert_eq!(t.description, "{\"updated_at\":1}");
    }

    #[test]
    fn normalize_carrier_repairs_tampered_object() {
        let mut t = Task::new("whatever", &std::collections::HashMap::new(), None);
        t.uid = "attacker-uid".into();
        t.status = TaskStatus::NeedsAction;
        t.is_event = false;
        t.is_journal = true;
        t.summary = "random event".into();
        t.categories = vec!["work".into()];
        normalize_carrier(&mut t);
        assert_eq!(t.uid, SETTINGS_UID);
        assert_eq!(t.status, TaskStatus::Cancelled);
        assert!(t.is_event);
        assert!(!t.is_journal);
        assert!(t.summary.starts_with("⚙ Cfait Settings"));
        assert!(t.categories.contains(&SETTINGS_CATEGORY.to_string()));
    }

    #[test]
    fn normalize_carrier_is_idempotent() {
        let mut t = settings_carrier_task("/calendars/u/cal", "{}");
        normalize_carrier(&mut t);
        normalize_carrier(&mut t);
        assert_eq!(
            t.categories
                .iter()
                .filter(|c| c == &SETTINGS_CATEGORY)
                .count(),
            1
        );
    }

    fn existing_in(target: &str, with_href: bool) -> Task {
        let mut t = settings_carrier_task(target, "{}");
        if with_href {
            t.href = format!("{}/cfait-global-settings-v1.ics", target);
            t.etag = "etag-1".into();
        }
        t
    }

    #[test]
    fn build_carrier_prefers_remote_object() {
        let remote = existing_in("/calendars/u/cal", true);
        let (t, is_new) = build_carrier_task(None, Some(&remote), false, "/calendars/u/cal", "{}");
        assert!(!is_new);
        assert_eq!(t.uid, SETTINGS_UID);
        assert_eq!(t.href, remote.href);
        assert_eq!(t.sequence, remote.sequence + 1);
    }

    #[test]
    fn build_carrier_recreates_when_remote_known_absent() {
        let existing = existing_in("/calendars/u/cal", true);
        let (t, is_new) = build_carrier_task(Some(&existing), None, true, "/calendars/u/cal", "{}");
        assert!(is_new);
        assert!(t.href.is_empty());
        assert!(t.etag.is_empty());
    }

    #[test]
    fn build_carrier_updates_when_remote_not_checked() {
        let existing = existing_in("/calendars/u/cal", true);
        let (t, is_new) =
            build_carrier_task(Some(&existing), None, false, "/calendars/u/cal", "{}");
        assert!(!is_new);
        assert_eq!(t.href, existing.href);
    }

    #[test]
    fn build_carrier_fresh_when_existing_in_other_calendar() {
        let existing = existing_in("/calendars/u/old", true);
        let (t, is_new) =
            build_carrier_task(Some(&existing), None, false, "/calendars/u/new", "{}");
        assert!(is_new);
        assert_eq!(t.calendar_href, "/calendars/u/new");
        assert!(t.href.is_empty());
    }

    #[test]
    fn build_carrier_fresh_when_nothing_known() {
        let (t, is_new) = build_carrier_task(None, None, false, "/calendars/u/cal", "{}");
        assert!(is_new);
        assert_eq!(t.calendar_href, "/calendars/u/cal");
    }
}
