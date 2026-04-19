// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/async_ops.rs
// Asynchronous operations wrapper bridging sync GUI and async client.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::model::{CalendarListEntry, Task as TodoTask};
use futures::stream::{self, StreamExt};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

// Global runtime instance for bridging Iced (sync) and Client (async)
static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn init_runtime() {
    if TOKIO_RUNTIME.get().is_none() {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        TOKIO_RUNTIME
            .set(runtime)
            .expect("Failed to set global runtime");
    }
}

pub fn get_runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get().expect("Runtime not initialized")
}

// --- WRAPPERS ---

pub async fn connect_and_fetch_wrapper(
    ctx: Arc<dyn AppContext>,
    config: Config,
) -> Result<
    (
        RustyClient,
        Vec<CalendarListEntry>,
        Vec<TodoTask>,
        Option<String>,
        Option<String>,
    ),
    String,
> {
    let rt = get_runtime();
    rt.spawn(async {
        match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            RustyClient::connect_with_fallback(ctx, config, Some("GUI")),
        )
        .await
        {
            Ok(res) => res.map_err(|e| e.to_string()),
            Err(_) => Err("Connection timed out. Check your network or server URL.".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_fetch_wrapper(
    client: RustyClient,
    href: String,
) -> Result<(String, Vec<TodoTask>), String> {
    let rt = get_runtime();
    rt.spawn(async move {
        match tokio::time::timeout(std::time::Duration::from_secs(30), client.get_tasks(&href))
            .await
        {
            Ok(res) => {
                let tasks = res.map_err(|e: String| e)?;
                Ok((href, tasks))
            }
            Err(_) => Err(format!("Fetch timed out for calendar {}", href)),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_fetch_all_wrapper(
    client: RustyClient,
    cals: Vec<CalendarListEntry>,
) -> Result<Vec<(String, Vec<TodoTask>)>, String> {
    let rt = get_runtime();
    rt.spawn(async move {
        match tokio::time::timeout(
            std::time::Duration::from_secs(180),
            client.get_all_tasks(&cals),
        )
        .await
        {
            Ok(res) => res.map_err(|e| e.to_string()),
            Err(_) => Err("Fetch all timed out".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

use crate::controller::TaskController;
use crate::store::TaskStore;
use tokio::sync::Mutex;

pub enum ControllerAction {
    Create(TodoTask),
    Update(TodoTask),
    Delete(String),
    DeleteTree(String),
    Toggle(String),
    Move(String, String),
    DuplicateTree(String),
}

pub async fn async_controller_dispatch(
    ctx: Arc<dyn AppContext>,
    client: Option<RustyClient>,
    store: TaskStore,
    action: ControllerAction,
) -> Result<TaskStore, String> {
    let store_arc = Arc::new(Mutex::new(store));
    let client_arc = Arc::new(Mutex::new(client));
    let controller = TaskController::new(store_arc.clone(), client_arc, ctx);

    let action_future = async move {
        match action {
            ControllerAction::Create(t) => {
                let _ = controller.create_task(t).await;
            }
            ControllerAction::Update(t) => {
                let _ = controller.update_task(t).await;
            }
            ControllerAction::Delete(uid) => {
                let _ = controller.delete_task(&uid).await;
            }
            ControllerAction::DeleteTree(uid) => {
                let _ = controller.delete_task_tree(&uid).await;
            }
            ControllerAction::Toggle(uid) => {
                let _ = controller.toggle_task(&uid).await;
            }
            ControllerAction::Move(uid, href) => {
                let _ = controller.move_task(&uid, &href).await;
            }
            ControllerAction::DuplicateTree(uid) => {
                let _ = controller.duplicate_task_tree(&uid).await;
            }
        }
    };

    // Even if network sync times out, the local store memory modifications
    // are synchronously committed prior to yielding.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(60), action_future).await;

    let updated_store = store_arc.lock().await.clone();
    Ok(updated_store)
}

pub async fn async_migrate_wrapper(
    client: RustyClient,
    tasks: Vec<TodoTask>,
    target: String,
) -> Result<usize, String> {
    let rt = get_runtime();
    rt.spawn(async move {
        match tokio::time::timeout(
            std::time::Duration::from_secs(45),
            client.migrate_tasks(tasks, &target),
        )
        .await
        {
            Ok(res) => res.map_err(|e| e.to_string()),
            Err(_) => Err("Migration timed out".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Backfill calendar events for all tasks when the global setting is enabled.
/// This is called when the user toggles the setting from OFF to ON.
pub async fn async_backfill_events_wrapper(
    client: RustyClient,
    tasks: Vec<TodoTask>,
    global_enabled: bool,
) -> Result<usize, String> {
    let rt = get_runtime();
    rt.spawn(async move {
        let futures = tasks
            .into_iter()
            .filter(|task| {
                task.due.is_some() || task.dtstart.is_some() || !task.sessions.is_empty()
            })
            .map(|task| {
                let c = client.clone();
                async move {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        c.sync_task_companion_event(&task, global_enabled),
                    )
                    .await
                    {
                        Ok(Ok(true)) => 1,
                        _ => 0,
                    }
                }
            });

        let count = stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<usize>>()
            .await
            .iter()
            .sum();

        Ok(count)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_delete_all_events_wrapper(
    client: RustyClient,
    calendars: Vec<String>,
) -> Result<usize, String> {
    let rt = get_runtime();
    rt.spawn(async move {
        match tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let mut total = 0;
            for cal_href in calendars {
                if let Ok(count) = client.delete_all_companion_events(&cal_href).await {
                    total += count;
                }
            }
            Ok::<usize, String>(total)
        })
        .await
        {
            Ok(res) => res,
            Err(_) => Err("Deleting events timed out".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
