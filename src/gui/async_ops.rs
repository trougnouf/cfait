use crate::client::{RustyClient, ClientManager};
use crate::config::Config;
use crate::model::{CalendarListEntry, Task as TodoTask};
use futures::stream::{self, StreamExt};
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use std::collections::HashMap;

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

pub async fn connect_and_fetch_wrapper(
    config: Config,
) -> Result<
    (
        ClientManager,
        Vec<CalendarListEntry>,
        Vec<TodoTask>,
        Option<String>,
        Option<String>,
    ),
    String,
> {
    let rt = get_runtime();

    // Clone accounts outside the async block to satisfy lifetimes
    let accounts = config.accounts.clone();
    let default_calendar = config.default_calendar.clone();
    let has_accounts = !config.accounts.is_empty();

    rt.spawn(async move {
        // 1. Initialize Manager
        let manager = ClientManager::new(&accounts, Some("GUI")).await;

        // 2. Fetch Calendars
        let calendars = manager.get_all_calendars().await;

        // 3. Check for Cache & Warnings
        let warning = if calendars.is_empty() && has_accounts {
            Some("No calendars found (Offline or Auth failed)".to_string())
        } else {
            None
        };

        Ok((manager, calendars, vec![], default_calendar, warning))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_fetch_wrapper(
    manager: ClientManager,
    href: String,
    account_id: String,
) -> Result<(String, Vec<TodoTask>), String> {
    let rt = get_runtime();
    rt.spawn(async move {
        let client = manager.get_client(&account_id).ok_or("Account offline")?;
        let tasks = client.get_tasks(&href).await.map_err(|e: String| e)?;
        Ok((href, tasks))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn async_fetch_all_wrapper(
    manager: ClientManager,
    cals: Vec<CalendarListEntry>,
) -> Result<Vec<(String, Vec<TodoTask>)>, String> {
    let rt = get_runtime();
    rt.spawn(async move { manager.get_all_tasks(&cals).await })
        .await
        .map_err(|e| e.to_string())?
}

fn get_client_for_task(manager: &ClientManager, _task: &TodoTask, account_id: &str) -> Result<RustyClient, String> {
    manager.get_client(account_id)
        .cloned()
        .ok_or_else(|| format!("Account {} not connected", account_id))
}

pub async fn async_create_wrapper(
    manager: ClientManager,
    mut task: TodoTask,
    account_id: String,
) -> Result<TodoTask, String> {
    let client = get_client_for_task(&manager, &task, &account_id)?;
    let _ = client.create_task(&mut task).await?;
    Ok(task)
}

pub async fn async_update_wrapper(
    manager: ClientManager,
    mut task: TodoTask,
    account_id: String,
) -> Result<TodoTask, String> {
    let client = get_client_for_task(&manager, &task, &account_id)?;
    let _ = client.update_task(&mut task).await?;
    Ok(task)
}

pub async fn async_delete_wrapper(manager: ClientManager, task: TodoTask, account_id: String) -> Result<(), String> {
    let client = get_client_for_task(&manager, &task, &account_id)?;
    let _ = client.delete_task(&task).await?;
    Ok(())
}

pub async fn async_toggle_wrapper(
    manager: ClientManager,
    mut task: TodoTask,
    account_id: String,
) -> Result<(TodoTask, Option<TodoTask>), String> {
    let client = get_client_for_task(&manager, &task, &account_id)?;
    let (_, next, _) = client.toggle_task(&mut task).await?;
    Ok((task, next))
}

pub async fn async_move_wrapper(
    manager: ClientManager,
    task: TodoTask,
    new_href: String,
    account_id: String,
) -> Result<TodoTask, String> {
    let client = get_client_for_task(&manager, &task, &account_id)?;
    let (t, _) = client.move_task(&task, &new_href).await?;
    Ok(t)
}

pub async fn async_migrate_wrapper(
    manager: ClientManager,
    tasks: Vec<TodoTask>,
    target_href: String,
    target_account_id: String,
) -> Result<usize, String> {
    let client = manager.get_client(&target_account_id)
        .cloned()
        .ok_or("Target account offline")?;

    let rt = get_runtime();
    rt.spawn(async move { client.migrate_tasks(tasks, &target_href).await })
        .await
        .map_err(|e| e.to_string())?
}

pub async fn async_backfill_events_wrapper(
    manager: ClientManager,
    tasks: Vec<TodoTask>,
    // Map calendar_href -> account_id to route tasks correctly
    calendar_account_map: HashMap<String, String>,
    global_enabled: bool,
) -> Result<usize, String> {
    let rt = get_runtime();
    rt.spawn(async move {
        // Group tasks by account
        let mut tasks_by_account: HashMap<String, Vec<TodoTask>> = HashMap::new();

        for task in tasks {
            if let Some(acc_id) = calendar_account_map.get(&task.calendar_href) {
                tasks_by_account.entry(acc_id.clone()).or_default().push(task);
            }
        }

        let mut total_count = 0;

        for (acc_id, acc_tasks) in tasks_by_account {
            if let Some(client) = manager.get_client(&acc_id) {
                let futures = acc_tasks.into_iter().map(|task| {
                    let c = client.clone();
                    async move {
                        match c.sync_task_companion_event(&task, global_enabled).await {
                            Ok(true) => 1,
                            _ => 0,
                        }
                    }
                });

                let count: usize = stream::iter(futures)
                    .buffer_unordered(8)
                    .collect::<Vec<usize>>()
                    .await
                    .iter()
                    .sum();
                total_count += count;
            }
        }

        Ok(total_count)
    })
    .await
    .map_err(|e| e.to_string())?
}
