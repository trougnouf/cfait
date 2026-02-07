// File: ./src/controller.rs
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
/// 3. Fallback to Disk (Journal/LocalStorage) on failure
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

    /// Internal helper to handle the "Save to Network OR Disk" logic
    async fn persist_change(&self, action: Action) -> Result<(), String> {
        let client_guard = self.client.lock().await;

        let network_success = if let Some(client) = &*client_guard {
            match action.clone() {
                Action::Create(mut t) => client.create_task(&mut t).await.is_ok(),
                Action::Update(mut t) => client.update_task(&mut t).await.is_ok(),
                Action::Delete(t) => client.delete_task(&t).await.is_ok(),
                Action::Move(t, new_cal) => client.move_task(&t, &new_cal).await.is_ok(),
            }
        } else {
            false
        };

        if !network_success {
            // Fallback to offline storage
            match action {
                Action::Create(t) | Action::Update(t) => self.save_offline(t, false).await?,
                Action::Delete(t) => self.save_offline(t, true).await?,
                Action::Move(t, target) => {
                    if !t.calendar_href.starts_with("local://") {
                        Journal::push(self.ctx.as_ref(), Action::Move(t, target))
                            .map_err(|e| e.to_string())?;
                    } else {
                        // Local moves are atomic in memory store; explicit file save happens via Store triggers or manual saves usually.
                        // In this simplified controller, we rely on the caller/Store to handle local file persistence
                        // or we could implement specific local file saving logic here if needed.
                    }
                }
            }
        }
        Ok(())
    }

    async fn save_offline(&self, task: Task, is_delete: bool) -> Result<(), String> {
        if task.calendar_href.starts_with("local://") {
            // Local Calendar: Persist entire list to local_X.json
            let store = self.store.lock().await;

            if let Some(map) = store.calendars.get(&task.calendar_href) {
                let list: Vec<Task> = map.values().cloned().collect();
                LocalStorage::save_for_href(self.ctx.as_ref(), &task.calendar_href, &list)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            // Remote Calendar: Push to Journal
            let action = if is_delete {
                Action::Delete(task)
            } else if task.etag.is_empty() {
                Action::Create(task)
            } else {
                Action::Update(task)
            };
            Journal::push(self.ctx.as_ref(), action).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // --- Public API ---

    pub async fn create_task(&self, mut task: Task) -> Result<String, String> {
        // 1. Update Store
        self.store.lock().await.add_task(task.clone());

        // 2. Persist
        let client_guard = self.client.lock().await;
        if let Some(client) = &*client_guard
            && client.create_task(&mut task).await.is_ok()
        {
            // Update store with server-assigned fields (ETag, href)
            self.store.lock().await.update_or_add_task(task.clone());
            return Ok(task.uid);
        }

        // Offline Fallback
        self.save_offline(task.clone(), false).await?;
        // Ensure store has the placeholder ETag
        if task.etag.is_empty() {
            let mut t = task.clone();
            t.etag = "pending_refresh".to_string();
            self.store.lock().await.update_or_add_task(t);
        }

        Ok(task.uid)
    }

    pub async fn update_task(&self, task: Task) -> Result<(), String> {
        self.store.lock().await.update_or_add_task(task.clone());
        self.persist_change(Action::Update(task)).await
    }

    pub async fn delete_task(&self, uid: String) -> Result<(), String> {
        let (task, _) = self
            .store
            .lock()
            .await
            .delete_task(&uid)
            .ok_or("Task not found".to_string())?;

        self.persist_change(Action::Delete(task)).await
    }

    pub async fn toggle_task(&self, uid: String) -> Result<(), String> {
        let mut store = self.store.lock().await;

        let (task, _) = store
            .get_task_mut(&uid)
            .ok_or("Task not found".to_string())?;
        let current_status = task.status;

        let next_status = if current_status.is_done() {
            crate::model::TaskStatus::NeedsAction
        } else {
            crate::model::TaskStatus::Completed
        };

        // 3. Store logic (Recycle/Snapshot)
        let (primary, secondary) = store.set_status(&uid, next_status).unwrap();
        drop(store); // release lock before async persistence

        // 4. Network/Disk logic
        if let Some(sec) = secondary {
            self.persist_change(Action::Create(primary)).await?;
            self.persist_change(Action::Update(sec)).await?;
        } else {
            self.persist_change(Action::Update(primary)).await?;
        }

        Ok(())
    }

    pub async fn move_task(&self, uid: String, new_cal_href: String) -> Result<(), String> {
        let mut store = self.store.lock().await;
        // Use atomic store API to move in memory and get pre/post states
        let (original, new_task) = store
            .move_task(&uid, new_cal_href.clone())
            .ok_or("Task not found".to_string())?;
        drop(store);

        let client_guard = self.client.lock().await;
        let mut network_success = false;

        // Attempt network move
        if let Some(client) = &*client_guard {
            // RustyClient::move_task handles complex logic (e.g. Local->Remote migration)
            if client.move_task(&original, &new_cal_href).await.is_ok() {
                network_success = true;
            }
        }

        // Fallback to offline Journaling if network unavailable
        if !network_success {
            if original.calendar_href.starts_with("local://") {
                // Case 1: Source was Local.
                // If Target is Remote, we must queue a Create for the *new* task on the remote cal.
                // (If Target is Local, Store::move_task already saved it to disk).
                if !new_cal_href.starts_with("local://") {
                    Journal::push(self.ctx.as_ref(), Action::Create(new_task))
                        .map_err(|e| e.to_string())?;
                }
            } else {
                // Case 2: Source was Remote.
                if new_cal_href.starts_with("local://") {
                    // Remote -> Local (Offline): Just delete from remote later.
                    Journal::push(self.ctx.as_ref(), Action::Delete(original))
                        .map_err(|e| e.to_string())?;
                } else {
                    // Remote -> Remote: Queue a Move action.
                    Journal::push(self.ctx.as_ref(), Action::Move(original, new_cal_href))
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        Ok(())
    }
}
