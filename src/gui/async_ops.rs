// Asynchronous operations wrapper bridging sync GUI and async client.
use crate::client::RustyClient;
use crate::config::Config;
use crate::model::{CalendarListEntry, Task as TodoTask};
use futures::stream::{self, StreamExt};
use std::sync::OnceLock;
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
    rt.spawn(async { RustyClient::connect_with_fallback(config, Some("GUI")).await })
        .await
        .map_err(|e| e.to_string())?
}

pub async fn async_fetch_wrapper(
    client: RustyClient,
    href: String,
) -> Result<(String, Vec<TodoTask>), String> {
    let rt = get_runtime();
    rt.spawn(async move {
        let tasks = client.get_tasks(&href).await.map_err(|e: String| e)?;
        Ok((href, tasks))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_fetch_all_wrapper(
    client: RustyClient,
    cals: Vec<CalendarListEntry>,
) -> Result<Vec<(String, Vec<TodoTask>)>, String> {
    let rt = get_runtime();
    rt.spawn(async move { client.get_all_tasks(&cals).await })
        .await
        .map_err(|e| e.to_string())?
}

pub async fn async_create_wrapper(
    client: RustyClient,
    mut task: TodoTask,
) -> Result<TodoTask, String> {
    let _ = client.create_task(&mut task).await?;
    Ok(task)
}

pub async fn async_update_wrapper(
    client: RustyClient,
    mut task: TodoTask,
) -> Result<TodoTask, String> {
    let _ = client.update_task(&mut task).await?;
    Ok(task)
}

pub async fn async_delete_wrapper(client: RustyClient, task: TodoTask) -> Result<(), String> {
    let _ = client.delete_task(&task).await?;
    Ok(())
}

pub async fn async_toggle_wrapper(
    client: RustyClient,
    mut task: TodoTask,
) -> Result<(TodoTask, Option<TodoTask>), String> {
    let (_, next, _) = client.toggle_task(&mut task).await?;
    Ok((task, next))
}

pub async fn async_move_wrapper(
    client: RustyClient,
    task: TodoTask,
    new_href: String,
) -> Result<TodoTask, String> {
    let (t, _) = client.move_task(&task, &new_href).await?;
    Ok(t)
}

pub async fn async_migrate_wrapper(
    client: RustyClient,
    tasks: Vec<TodoTask>,
    target: String,
) -> Result<usize, String> {
    let rt = get_runtime();
    rt.spawn(async move { client.migrate_tasks(tasks, &target).await })
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
        // NEW CONCURRENT LOGIC
        let futures = tasks.into_iter().map(|task| {
            let c = client.clone();
            async move {
                match c.sync_task_companion_event(&task, global_enabled).await {
                    Ok(true) => 1,
                    Ok(false) => 0,
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to backfill event for task {}: {}",
                            task.uid, e
                        );
                        0
                    }
                }
            }
        });

        // Run 8 concurrent requests
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
