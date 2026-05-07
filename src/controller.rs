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

    /// Persist an Action to LocalStorage or Journal, auto-initializing system calendars if needed.
    pub async fn persist_change(&self, action: Action) -> Result<Vec<String>, String> {
        // Prevent Data-Loss: Ensure Trash calendar is registered on disk during a trash-create event
        if let Action::Create(t) | Action::Update(t) = &action
            && t.calendar_href == crate::storage::LOCAL_TRASH_HREF {
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
                    let _ =
                        LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            if let Some(idx) =
                                all.iter().position(|item| item.uid == task_clone.uid)
                            {
                                all[idx] = task_clone;
                            } else {
                                all.push(task_clone);
                            }
                        });
                }
                Action::Delete(t) => {
                    let _ =
                        LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            all.retain(|item| item.uid != t.uid);
                        });
                }
                Action::Move(t, target_href) => {
                    let _ =
                        LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            all.retain(|item| item.uid != t.uid);
                        });
                    if target_href.starts_with("local://") {
                        let mut moved = t.clone();
                        moved.calendar_href = target_href.clone();
                        let _ =
                            LocalStorage::modify_for_href(self.ctx.as_ref(), target_href, |all| {
                                all.push(moved);
                            });
                    }
                }
            }
            return Ok(vec![]);
        }

        let uid = match &action {
            Action::Create(t) | Action::Update(t) | Action::Delete(t) => t.uid.clone(),
            Action::Move(t, _) => t.uid.clone(),
        };

        {
            let mut store = self.store.lock().await;
            if let Some((existing, _)) = store.get_task_mut(&uid)
                && existing.etag.is_empty()
            {
                existing.etag = "pending_refresh".to_string();
            }
        }

        Journal::push(self.ctx.as_ref(), action).map_err(|e| e.to_string())?;

        let client_opt = self.client.lock().await.clone();
        if let Some(client) = client_opt {
            if let Ok((warns, synced)) = client.sync_journal().await {
                let mut s = self.store.lock().await;
                for sync_task in synced {
                    if let Some((existing, _)) = s.get_task_mut(&sync_task.uid) {
                        existing.etag = sync_task.etag;
                        existing.href = sync_task.href;
                    }
                }
                Ok(warns)
            } else {
                Ok(vec!["Network error: Changes queued for next sync.".to_string()])
            }
        } else {
            Ok(vec!["Offline: Changes queued.".to_string()])
        }
    }

    /// Process a batch of actions atomically to ensure proper journal queueing,
    /// avoiding excessive network calls by batching all remote actions into a single sync.
    pub async fn persist_changes(&self, actions: Vec<Action>) -> Result<(Vec<String>, Vec<Task>), String> {
        let mut warnings = Vec::new();
        let mut remote_actions = Vec::new();

        for action in actions {
            // Prevent Data-Loss: Ensure Trash calendar is registered on disk during a trash-create event
            if let Action::Create(ref t) | Action::Update(ref t) = action
                && t.calendar_href == crate::storage::LOCAL_TRASH_HREF {
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
                        let _ = LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            if let Some(idx) = all.iter().position(|item| item.uid == task_clone.uid) {
                                all[idx] = task_clone;
                            } else {
                                all.push(task_clone);
                            }
                        });
                    }
                    Action::Delete(t) => {
                        let _ = LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            all.retain(|item| item.uid != t.uid);
                        });
                    }
                    Action::Move(t, target_href) => {
                        let _ = LocalStorage::modify_for_href(self.ctx.as_ref(), &t.calendar_href, |all| {
                            all.retain(|item| item.uid != t.uid);
                        });
                        if target_href.starts_with("local://") {
                            let mut moved = t.clone();
                            moved.calendar_href = target_href.clone();
                            let _ = LocalStorage::modify_for_href(self.ctx.as_ref(), target_href, |all| {
                                all.push(moved);
                            });
                        }
                    }
                }
            } else {
                remote_actions.push(action);
            }
        }

        if remote_actions.is_empty() {
            return Ok((warnings, vec![]));
        }

        {
            let mut store = self.store.lock().await;
            for action in &remote_actions {
                let uid = match action {
                    Action::Create(t) | Action::Update(t) | Action::Delete(t) => &t.uid,
                    Action::Move(t, _) => &t.uid,
                };
                if let Some((existing, _)) = store.get_task_mut(uid)
                    && existing.etag.is_empty() {
                        existing.etag = "pending_refresh".to_string();
                    }
            }
        }

        Journal::modify(self.ctx.as_ref(), |queue| {
            queue.extend(remote_actions);
            let mut tmp_j = Journal { queue: std::mem::take(queue) };
            tmp_j.compact();
            *queue = tmp_j.queue;
        }).map_err(|e| e.to_string())?;

        let client_opt = self.client.lock().await.clone();
        if let Some(client) = client_opt {
            if let Ok((warns, synced)) = client.sync_journal().await {
                let mut s = self.store.lock().await;
                for sync_task in &synced {
                    if let Some((existing, _)) = s.get_task_mut(&sync_task.uid) {
                        existing.etag = sync_task.etag.clone();
                        existing.href = sync_task.href.clone();
                    } else {
                        // Task doesn't exist in store yet (newly created task), add it
                        s.add_task(sync_task.clone());
                    }
                }
                warnings.extend(warns);
                Ok((warnings, synced))
            } else {
                warnings.push("Network error: Changes queued for next sync.".to_string());
                Ok((warnings, vec![]))
            }
        } else {
            warnings.push("Offline: Changes queued.".to_string());
            Ok((warnings, vec![]))
        }
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
        let _ = self.persist_change(Action::Create(task.clone())).await;
        Ok(task.uid)
    }

    pub async fn update_task(&self, mut task: Task) -> Result<Vec<String>, String> {
        task.sequence += 1;
        let mut store = self.store.lock().await;
        store.update_or_add_task(task.clone());
        drop(store);
        self.persist_change(Action::Update(task)).await
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
        for task in purged_tasks {
            let _ = self.persist_change(Action::Delete(task)).await;
        }
        Ok(count)
    }
}
