// File: ./src/controller.rs
//! Central logic controller for Task operations.
//! This is the single source of truth for all business logic (create, update, delete, etc.).
//! All UI layers (TUI, GUI, Mobile) must delegate actions to this controller to ensure
//! consistent behavior for both online and offline operations.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::{RawProperty, Task};
use crate::storage::{LOCAL_TRASH_HREF, LocalCalendarRegistry, LocalStorage};
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

    /// Internal helper: persist an Action.
    ///
    /// Strategy:
    /// 1. If the action targets a local calendar (`local://...`), write directly to `LocalStorage`
    ///    and skip the `Journal` entirely.
    /// 2. If a network client is available, push the action to the local Journal
    ///    (so the action is durable) before attempting an immediate sync. Pushing
    ///    to the journal first ensures that a crash during sync does not lose the
    ///    user's intent.
    /// 3. If the client is present, trigger `client.sync_journal()` to attempt
    ///    delivering queued actions to the server and return any warnings.
    /// 4. If no client is available (offline), keep the action in the journal and
    ///    return a quiet success indicating the change is queued.
    async fn persist_change(&self, action: Action) -> Result<Vec<String>, String> {
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

        // Flag as pending in memory to prevent Precondition Failed loops
        {
            let mut store = self.store.lock().await;
            if let Some((existing, _)) = store.get_task_mut(&uid) {
                if existing.etag.is_empty() {
                    existing.etag = "pending_refresh".to_string();
                }
            }
        }

        // Durable push to journal BEFORE network attempt
        Journal::push(self.ctx.as_ref(), action).map_err(|e| e.to_string())?;

        let client_opt = self.client.lock().await.clone();
        if let Some(client) = client_opt {
            let store_ref = self.store.clone();
            // Fire and forget the sync to keep the UI instant
            tokio::spawn(async move {
                if let Ok((_warns, synced)) = client.sync_journal().await {
                    let mut s = store_ref.lock().await;
                    for sync_task in synced {
                        if let Some((existing, _)) = s.get_task_mut(&sync_task.uid) {
                            existing.etag = sync_task.etag;
                            existing.href = sync_task.href;
                        }
                    }
                }
            });
            Ok(vec![])
        } else {
            Ok(vec!["Offline: Changes queued.".to_string()])
        }
    }

    /// Create a task.
    ///
    /// Optimistic update pattern:
    /// 1. Insert into in-memory store for instant UI feedback.
    /// 2. If the network client is available, attempt to create remotely and then
    ///    update the store with any server-assigned metadata (etag/href).
    /// 3. If offline or remote failure, journal the Create action for background sync.
    pub async fn create_task(&self, mut task: Task) -> Result<String, String> {
        // Pre-compute full_href for remote tasks so it's ready for the UI instantly
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

    /// Update an existing task.
    ///
    /// Special handling for recurring completions:
    /// - If the update marks a recurring task as completed, we must:
    ///   a) create a history snapshot (new UID) that represents the completed instance,
    ///   b) advance the recurring item to its next occurrence (if possible),
    ///   c) perform optimistic UI updates for both history and next instance,
    ///   d) persist both mutations (history create + next update) via journaling/sync.
    ///
    /// For non-recurring updates, perform optimistic store update and persist the Update.
    pub async fn update_task(&self, mut task: Task) -> Result<Vec<String>, String> {
        task.sequence += 1;

        let mut store = self.store.lock().await;

        // Detect a recurring task being completed (transition from not-done -> done).
        let is_recurring_completion = if let Some(existing) = store.get_task_ref(&task.uid) {
            task.rrule.is_some() && task.status.is_done() && !existing.status.is_done()
        } else {
            false
        };

        if is_recurring_completion {
            // Recycle produces a history snapshot and optionally a next-instance.
            let (history, next_opt) = task.recycle(task.status);

            // 1. Optimistic UI: insert history (new UID) and update next/main item.
            store.add_task(history.clone());

            if let Some(next) = &next_opt {
                store.update_or_add_task(next.clone());
            } else {
                // Fallback: update the task in place if no next-instance produced.
                store.update_or_add_task(task.clone());
            }

            // Drop the lock before performing network/disk operations.
            drop(store);

            // 2. Persist changes: history create and next update (if present).
            let mut logs = self.persist_change(Action::Create(history)).await?;

            if let Some(next) = next_opt {
                let next_logs = self.persist_change(Action::Update(next)).await?;
                logs.extend(next_logs);
            } else {
                let next_logs = self.persist_change(Action::Update(task)).await?;
                logs.extend(next_logs);
            }

            return Ok(logs);
        }

        // Standard update path
        store.update_or_add_task(task.clone());
        drop(store);

        self.persist_change(Action::Update(task)).await
    }

    /// Delete a task.
    ///
    /// Soft-delete behavior:
    /// - If trash retention is enabled (>0) and the task is not already in the trash,
    ///   move the task to the local trash calendar and stamp X-TRASHED-DATE.
    /// - If retention is 0 or the task is already in trash, perform a hard delete.
    ///
    /// Steps for soft delete:
    /// 1. Ensure trash calendar exists in registry/disk.
    /// 2. Ensure an in-memory entry for the trash calendar.
    /// 3. Move the task in the store to the trash calendar.
    /// 4. Stamp deletion date on the moved item.
    /// 5. Persist the trash item to disk (local storage).
    /// 6. If original was remote, push a Delete action for background sync.
    pub async fn delete_task(&self, uid: &str) -> Result<Vec<String>, String> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();

        let mut store = self.store.lock().await;
        let task_ref = store
            .get_task_ref(uid)
            .ok_or("Task not found".to_string())?;

        let is_already_trash = task_ref.calendar_href == LOCAL_TRASH_HREF;
        let retention = config.trash_retention_days;

        // Hard delete if retention is disabled or item already in trash.
        if retention == 0 || is_already_trash {
            let (task, _) = store.delete_task(uid).ok_or("Task not found".to_string())?;
            drop(store);
            return self.persist_change(Action::Delete(task)).await;
        }

        // Soft delete path: move to trash.
        let target_href = LOCAL_TRASH_HREF.to_string();

        // Ensure trash calendar is registered on disk/registry.
        let _ = LocalCalendarRegistry::ensure_trash_calendar_exists(self.ctx.as_ref());
        // Ensure in-memory store has an entry for trash.
        store.calendars.entry(target_href.clone()).or_default();

        // Move in store; move_task returns (original, updated)
        let (original, mut updated) = store
            .move_task(uid, target_href.clone())
            .ok_or("Task not found".to_string())?;

        // Stamp deletion date so pruning can determine age.
        let now_str = Utc::now().to_rfc3339();
        updated
            .unmapped_properties
            .retain(|p| p.key != "X-TRASHED-DATE");
        updated.unmapped_properties.push(RawProperty {
            key: "X-TRASHED-DATE".to_string(),
            value: now_str,
            params: vec![],
        });

        store.update_or_add_task(updated.clone());
        drop(store);

        // Save the updated task to the trash local collection
        let _ = self.persist_change(Action::Create(updated)).await;

        // Delete the original
        self.persist_change(Action::Delete(original)).await
    }

    /// Prune items from the trash that have exceeded retention.
    ///
    /// Algorithm:
    /// - Load configured retention days.
    /// - If retention == 0, nothing to do.
    /// - Walk the trash calendar, parse X-TRASHED-DATE and collect UIDs older than retention.
    /// - Delete each found UID from the store and push a Delete action to keep journal consistent.
    pub async fn prune_trash(&self) -> Result<usize, String> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let retention_days = config.trash_retention_days as i64;

        if retention_days == 0 {
            return Ok(0);
        }

        let now = Utc::now();
        let mut tasks_to_purge = Vec::new();

        let mut store = self.store.lock().await;

        if let Some(trash_map) = store.calendars.get(LOCAL_TRASH_HREF) {
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

    /// Toggle a task's done state.
    ///
    /// Workflow:
    /// 1. Acquire store read/modify lock and determine current and next status.
    /// 2. Apply the store-level set_status which returns:
    ///    - primary: the history or updated task that represents the immediate change
    ///    - optional secondary: for recurring tasks, the advanced next-instance
    ///    - children: any descendant tasks that were auto-reset (need persistence)
    /// 3. Drop the lock and persist the produced mutations via `persist_change`.
    /// 4. Persist any children mutations as well.
    pub async fn toggle_task(&self, uid: &str) -> Result<Vec<String>, String> {
        let mut store = self.store.lock().await;

        let primary_ref = store
            .get_task_ref(uid)
            .ok_or("Task not found".to_string())?;
        let current_status = primary_ref.status;

        let next_status = if current_status.is_done() {
            crate::model::TaskStatus::NeedsAction
        } else {
            crate::model::TaskStatus::Completed
        };

        let (primary, secondary, children) = store
            .set_status(uid, next_status)
            .ok_or("Failed to set status".to_string())?;

        drop(store);

        // Persist all produced mutations, aggregating any warnings/messages.
        let mut all_warnings: Vec<String> = Vec::new();

        if let Some(sec) = secondary {
            // Recurrence advanced: persist the created history and the updated next instance.
            if let Ok(w) = self.persist_change(Action::Create(primary)).await {
                all_warnings.extend(w);
            }
            if let Ok(w) = self.persist_change(Action::Update(sec)).await {
                all_warnings.extend(w);
            }
        } else if let Ok(w) = self.persist_change(Action::Update(primary)).await {
            // Simple toggle: persist the single Update action.
            all_warnings.extend(w);
        }

        // Persist any children that were auto-reset by the store's logic.
        for child in children {
            if let Ok(w) = self.persist_change(Action::Update(child)).await {
                all_warnings.extend(w);
            }
        }

        Ok(all_warnings)
    }

    /// Move a task between calendars.
    ///
    /// This returns the original task state so callers can persist a Move action
    /// to the journal/network indicating the source calendar and target.
    pub async fn move_task(&self, uid: &str, new_cal_href: &str) -> Result<Vec<String>, String> {
        let mut store = self.store.lock().await;
        let (original, _) = store
            .move_task(uid, new_cal_href.to_string())
            .ok_or("Task not found".to_string())?;
        drop(store);

        if !original.calendar_href.starts_with("local://") && !new_cal_href.starts_with("local://")
        {
            self.persist_change(Action::Move(original, new_cal_href.to_string()))
                .await
        } else if !original.calendar_href.starts_with("local://")
            && new_cal_href.starts_with("local://")
        {
            let _ = self.persist_change(Action::Delete(original.clone())).await;
            let mut moved = original.clone();
            moved.calendar_href = new_cal_href.to_string();
            self.persist_change(Action::Create(moved)).await
        } else if original.calendar_href.starts_with("local://")
            && !new_cal_href.starts_with("local://")
        {
            let _ = self.persist_change(Action::Delete(original.clone())).await;
            let mut moved = original.clone();
            moved.calendar_href = new_cal_href.to_string();
            moved.href = String::new();
            moved.etag = String::new();
            self.persist_change(Action::Create(moved)).await
        } else {
            self.persist_change(Action::Move(original, new_cal_href.to_string()))
                .await
        }
    }
}
