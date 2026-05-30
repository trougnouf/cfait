// SPDX-License-Identifier: GPL-3.0-or-later
//! Central logic controller for Task operations.
//! This is the single source of truth for background persistence orchestration.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::Task;
use crate::storage::{LocalCalendarRegistry, LocalStorage};
use crate::store::TaskStore;
use chrono::{DateTime, Utc};
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Central logic controller for Task operations.
/// Handles business workflows and coordinates in-memory store mutations,
/// network client interactions and the journaling fallback used for offline-safe writes.
#[derive(Clone)]
pub struct TaskController {
    pub store: Arc<Mutex<TaskStore>>,
    pub client: Arc<Mutex<Option<RustyClient>>>,
    pub ctx: Arc<dyn AppContext>,
}

impl TaskController {
    pub fn new(
        store: Arc<Mutex<TaskStore>>,
        client: Arc<Mutex<Option<RustyClient>>>,
        ctx: Arc<dyn AppContext>,
    ) -> Self {
        Self { store, client, ctx }
    }

    /// Process a batch of actions atomically to ensure proper journal queueing.
    /// This is an instantaneous operation that saves to disk and returns without hitting the network.
    pub async fn persist_changes(&self, actions: Vec<Action>) -> Result<(), String> {
        let mut remote_actions = Vec::new();

        for action in actions {
            // Prevent Data-Loss: Ensure Trash calendar is registered on disk during a trash-create event
            if let Action::Create(ref t) | Action::Update(ref t) = action
                && t.calendar_href == crate::storage::LOCAL_TRASH_HREF
            {
                let _ = LocalCalendarRegistry::ensure_trash_calendar_exists(self.ctx.as_ref());
            }

            let is_local = match &action {
                Action::Create(t) | Action::Update(t) | Action::Delete(t) => {
                    t.calendar_href.starts_with("local://")
                }
                Action::Move(t, _) => t.calendar_href.starts_with("local://"),
            };

            if is_local {
                match &action {
                    Action::Create(t) | Action::Update(t) => {
                        let task_clone = t.clone();
                        let _ = LocalStorage::modify_for_href(
                            self.ctx.as_ref(),
                            &t.calendar_href,
                            |all| {
                                if let Some(idx) =
                                    all.iter().position(|item| item.uid == task_clone.uid)
                                {
                                    all[idx] = task_clone;
                                } else {
                                    all.push(task_clone);
                                }
                            },
                        );
                    }
                    Action::Delete(t) => {
                        let _ = LocalStorage::modify_for_href(
                            self.ctx.as_ref(),
                            &t.calendar_href,
                            |all| {
                                all.retain(|item| item.uid != t.uid);
                            },
                        );
                    }
                    Action::Move(t, target_href) => {
                        let _ = LocalStorage::modify_for_href(
                            self.ctx.as_ref(),
                            &t.calendar_href,
                            |all| {
                                all.retain(|item| item.uid != t.uid);
                            },
                        );
                        if target_href.starts_with("local://") {
                            let mut moved = t.clone();
                            moved.calendar_href = target_href.clone();
                            let _ = LocalStorage::modify_for_href(
                                self.ctx.as_ref(),
                                target_href,
                                |all| {
                                    all.push(moved);
                                },
                            );
                        }
                    }
                }
            } else {
                remote_actions.push(action);
            }
        }

        if remote_actions.is_empty() {
            return Ok(());
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
                    existing.etag = "pending_refresh".to_string();
                }
            }
        }

        Journal::modify(self.ctx.as_ref(), |queue| {
            queue.extend(remote_actions);
            let mut tmp_j = Journal {
                queue: std::mem::take(queue),
            };
            tmp_j.compact();
            *queue = tmp_j.queue;
        })
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Synchronizes the configuration and aliases via a hidden CalDAV VTODO.
    pub async fn sync_settings(&self) -> Result<bool, String> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        if !config.sync_settings {
            return Ok(false);
        }

        let settings_uid = "cfait-global-settings-v1";
        let mut store = self.store.lock().await;
        let existing_task = store.get_task_ref(settings_uid).cloned();

        let local_syncable = config.get_syncable();
        let local_payload = crate::config::SettingsPayload {
            updated_at: config.settings_updated_at,
            config: local_syncable,
        };
        let local_json = serde_json::to_string_pretty(&local_payload).unwrap_or_default();

        let config_changed = false;

        match existing_task {
            Some(mut task) => {
                if let Ok(remote_payload) = serde_json::from_str::<crate::config::SettingsPayload>(&task.description) {
                    if remote_payload.updated_at > config.settings_updated_at {
                        // Remote is newer! Sync down.
                        config.apply_syncable(remote_payload.config.clone());
                        config.settings_updated_at = remote_payload.updated_at;
                        let _ = config.save(self.ctx.as_ref());

                        let mut modified_tasks = Vec::new();
                        for (key, values) in &remote_payload.config.tag_aliases {
                            modified_tasks.extend(store.apply_alias_retroactively(key, values));
                        }
                        drop(store);

                        if !modified_tasks.is_empty() {
                            let actions = modified_tasks.into_iter().map(Action::Update).collect();
                            let _ = self.persist_changes(actions).await;
                        }

                        return Ok(true);
                    } else if config.settings_updated_at > remote_payload.updated_at {
                        // Local is newer! Sync up.
                        task.description = local_json;
                        task.sequence += 1;
                        store.update_or_add_task(task.clone());
                        drop(store);
                        let _ = self.persist_changes(vec![Action::Update(task)]).await;
                    }
                } else {
                    // Invalid JSON in task, overwrite with local
                    task.description = local_json;
                    task.sequence += 1;
                    store.update_or_add_task(task.clone());
                    drop(store);
                    let _ = self.persist_changes(vec![Action::Update(task)]).await;
                }
            }
            None => {
                // Task doesn't exist, deploy local settings upstream
                let target_href = if let Some(def) = &config.default_calendar {
                    if !def.starts_with("local://") {
                        def.clone()
                    } else {
                        let cals = crate::cache::Cache::load_calendars(self.ctx.as_ref()).unwrap_or_default();
                        cals.into_iter().find(|c| !c.href.starts_with("local://")).map(|c| c.href).unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string())
                    }
                } else {
                    let cals = crate::cache::Cache::load_calendars(self.ctx.as_ref()).unwrap_or_default();
                    cals.into_iter().find(|c| !c.href.starts_with("local://")).map(|c| c.href).unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string())
                };
                let mut new_task = Task::new("⚙ Cfait Settings (Do not delete)", &std::collections::HashMap::new(), None);
                new_task.uid = settings_uid.to_string();
                new_task.status = crate::model::TaskStatus::Cancelled; // Hides it in standard clients
                new_task.description = local_json;
                new_task.categories.push("cfait-internal".to_string());
                new_task.calendar_href = target_href;

                store.add_task(new_task.clone());
                drop(store);
                let _ = self.persist_changes(vec![Action::Create(new_task)]).await;
            }
        }
        Ok(config_changed)
    }

    /// Synchronize the journal with the remote server and update the in-memory store
    /// with the resulting ETags and URLs.
    pub async fn sync_and_update_store(&self) -> Result<(Vec<String>, Vec<Task>, bool), String> {
        let client_opt = self.client.lock().await.clone();
        
        let (warns, actual_synced) = if let Some(client) = client_opt {
            match client.sync_journal().await {
                Ok((w, s)) => {
                    let mut st = self.store.lock().await;
                    let mut actual = Vec::new();
                    let mut to_delete = Vec::new();

                    for sync_task in &s {
                        if sync_task.summary.starts_with("⚙ Cfait Settings") && sync_task.summary.ends_with("(Conflict Copy)") {
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
                        }
                    }
                    drop(st);

                    if !to_delete.is_empty() {
                        let actions = to_delete.into_iter().map(Action::Delete).collect();
                        let _ = self.persist_changes(actions).await;
                    }

                    (w, actual)
                }
                Err(e) => return Err(e),
            }
        } else {
            (vec!["Offline: Changes queued.".to_string()], vec![])
        };

        // Inject the settings synchronization cycle
        let config_changed = self.sync_settings().await.unwrap_or(false);

        Ok((warns, actual_synced, config_changed))
    }

    pub async fn create_task(&self, mut task: Task) -> Result<String, String> {
        if task.calendar_href == crate::storage::LOCAL_TRASH_HREF
            || task.calendar_href == "local://recovery"
        {
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
        self.store.lock().await.add_task(task.clone());
        let _ = self
            .persist_changes(vec![Action::Create(task.clone())])
            .await;
        Ok(task.uid)
    }

    pub async fn update_task(&self, mut task: Task) -> Result<Vec<String>, String> {
        task.sequence += 1;
        let mut store = self.store.lock().await;
        store.update_or_add_task(task.clone());
        drop(store);
        let _ = self.persist_changes(vec![Action::Update(task)]).await;
        Ok(vec![])
    }

    pub async fn prune_trash(&self) -> Result<usize, String> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let retention_days = config.trash_retention_days as i64;
        if retention_days == 0 {
            return Ok(0);
        }

        let now = Utc::now();
        let mut tasks_to_purge = Vec::new();

        let mut store = self.store.lock().await;

        if let Some(trash_map) = store.calendars.get(crate::storage::LOCAL_TRASH_HREF) {
            for task in trash_map.values() {
                if let Some(prop) = task
                    .unmapped_properties
                    .iter()
                    .find(|p| p.key == "X-TRASHED-DATE")
                    && let Ok(dt) = DateTime::parse_from_rfc3339(&prop.value)
                {
                    let age_days = (now - dt.with_timezone(&Utc)).num_days();
                    if age_days >= retention_days {
                        tasks_to_purge.push(task.uid.clone());
                    }
                }
            }
        }

        let mut purged_tasks = Vec::new();
        for uid in tasks_to_purge {
            if let Some((task, _)) = store.delete_task(&uid) {
                purged_tasks.push(task);
            }
        }
        drop(store);

        let count = purged_tasks.len();
        let actions = purged_tasks.into_iter().map(Action::Delete).collect();
        let _ = self.persist_changes(actions).await;
        Ok(count)
    }
}
