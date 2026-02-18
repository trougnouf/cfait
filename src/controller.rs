// File: ./src/controller.rs
//! Central logic controller for Task operations.
//! This is the single source of truth for all business logic (create, update, delete, etc.).
//! All UI layers (TUI, GUI, Mobile) must delegate actions to this controller to ensure
//! consistent behavior for both online and offline operations.
use crate::client::RustyClient;
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::Task;
use crate::storage::LocalStorage;
use crate::store::TaskStore;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Central logic controller for Task operations.
/// Handles the "Optimistic UI" pattern:
/// 1. Mutate Memory (Store) -> Instant UI feedback
/// 2. Attempt Network Call (Client)
/// 3. Fallback to journaling and trigger background sync when possible
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

    /// Internal helper to handle the "Save to Network OR Disk" logic.
    /// This will attempt to perform the action via the network client. If it fails
    /// or if the client is offline, it falls back to journaling the action.
    async fn persist_change(&self, action: Action) -> Result<Vec<String>, String> {
        let client_guard = self.client.lock().await;

        if let Some(client) = &*client_guard {
            // Push action to journal *before* sync attempt for safety.
            Journal::push(self.ctx.as_ref(), action).map_err(|e| e.to_string())?;
            // sync_journal will attempt to clear the queue and return any warnings/logs.
            client.sync_journal().await
        } else {
            // Offline: just push to journal.
            Journal::push(self.ctx.as_ref(), action).map_err(|e| e.to_string())?;
            Ok(vec!["Offline: Changes queued.".to_string()])
        }
    }

    // --- Public API ---

    pub async fn create_task(&self, mut task: Task) -> Result<String, String> {
        // 1. Update Store (optimistic)
        self.store.lock().await.add_task(task.clone());

        // 2. Try to perform network create if client available
        let client_guard = self.client.lock().await;
        if let Some(client) = &*client_guard
            && client.create_task(&mut task).await.is_ok()
        {
            // Update store with server-assigned fields (ETag, href)
            self.store.lock().await.update_or_add_task(task.clone());
            return Ok(task.uid);
        }

        // Offline / fallback handling
        if task.calendar_href.starts_with("local://") {
            let mut store = self.store.lock().await;
            if let Some(map) = store.calendars.get_mut(&task.calendar_href) {
                let list: Vec<Task> = map.values().cloned().collect();
                LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &list)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            Journal::push(self.ctx.as_ref(), Action::Create(task.clone()))
                .map_err(|e| e.to_string())?;
        }

        Ok(task.uid)
    }

    pub async fn update_task(&self, task: Task) -> Result<Vec<String>, String> {
        let mut store = self.store.lock().await;

        // FIX: Check for Recurrence Completion Transition via Smart Input.
        // If the user updated a recurring task to "Completed" (e.g. via "done:" syntax),
        // we must trigger the recycle logic to create history and advance the next instance,
        // rather than just overwriting the task in place (which breaks the recurrence chain).
        let is_recurring_completion = if let Some(existing) = store.get_task_ref(&task.uid) {
            task.rrule.is_some() && task.status.is_done() && !existing.status.is_done()
        } else {
            false
        };

        if is_recurring_completion {
            // Apply recycle logic using the NEW task state as the base.
            // This ensures any other edits (notes, tags) made alongside the completion are preserved.
            let (history, next_opt) = task.recycle(task.status);

            // 1. Optimistic UI Update
            store.add_task(history.clone()); // Add history item (new UID)

            if let Some(next) = &next_opt {
                store.update_or_add_task(next.clone()); // Update main item (same UID)
            } else {
                // Fallback: just update the task if recycle failed to produce next (e.g. malformed rrule)
                store.update_or_add_task(task.clone());
            }

            // Drop lock before network ops
            drop(store);

            // 2. Persist Changes (History + Next)
            // We must persist both the history creation and the update to the next instance.
            let mut logs = self.persist_change(Action::Create(history)).await?;

            if let Some(next) = next_opt {
                let next_logs = self.persist_change(Action::Update(next)).await?;
                logs.extend(next_logs);
            }

            return Ok(logs);
        }

        // Standard Update Path (Non-recurring or no status change)
        store.update_or_add_task(task.clone());
        drop(store);

        self.persist_change(Action::Update(task)).await
    }

    pub async fn delete_task(&self, uid: &str) -> Result<Vec<String>, String> {
        let (task, _) = self
            .store
            .lock()
            .await
            .delete_task(uid)
            .ok_or("Task not found".to_string())?;

        self.persist_change(Action::Delete(task)).await
    }

    pub async fn toggle_task(&self, uid: &str) -> Result<Vec<String>, String> {
        // 1. Acquire store lock and validate existence + derive next status
        let mut store = self.store.lock().await;

        let (primary_ref, _) = store
            .get_task_mut(uid)
            .ok_or("Task not found".to_string())?;
        let current_status = primary_ref.status;

        let next_status = if current_status.is_done() {
            crate::model::TaskStatus::NeedsAction
        } else {
            crate::model::TaskStatus::Completed
        };

        // 2. Apply business logic inside the store (recycle/advance/reset children).
        // This returns the primary (history or updated), optional secondary (next instance),
        // and any child tasks that were reset as part of a recurring completion.
        // Unwrap is safe because we validated task existence above.
        let (primary, secondary, children) = store
            .set_status(uid, next_status)
            .ok_or("Failed to set status".to_string())?;
        // Drop the store lock before performing async network/disk operations.
        drop(store);

        // 3. Persist all resulting mutations via persist_change.
        // Aggregate warnings/messages from each persistence attempt.
        let mut all_warnings: Vec<String> = Vec::new();

        if let Some(sec) = secondary {
            // Recurrence advanced: primary is a history snapshot (Create), secondary is updated (Update)
            if let Ok(w) = self.persist_change(Action::Create(primary)).await {
                all_warnings.extend(w);
            }
            if let Ok(w) = self.persist_change(Action::Update(sec)).await {
                all_warnings.extend(w);
            }
        } else {
            // Simple toggle: primary is the updated task (Update)
            if let Ok(w) = self.persist_change(Action::Update(primary)).await {
                all_warnings.extend(w);
            }
        }

        // 4. Persist any children that were auto-reset by the store
        for child in children {
            if let Ok(w) = self.persist_change(Action::Update(child)).await {
                all_warnings.extend(w);
            }
        }

        Ok(all_warnings)
    }

    pub async fn move_task(&self, uid: &str, new_cal_href: &str) -> Result<Vec<String>, String> {
        let mut store = self.store.lock().await;
        let (original, _) = store
            .move_task(uid, new_cal_href.to_string())
            .ok_or("Task not found".to_string())?;
        drop(store);

        self.persist_change(Action::Move(original, new_cal_href.to_string()))
            .await
    }
}
