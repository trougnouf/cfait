// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/async_ops.rs
// Asynchronous operations wrapper bridging sync GUI and async client.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::model::{CalendarListEntry, Task as TodoTask};
use futures::stream::{self, StreamExt};
use std::sync::Arc;

// --- WRAPPERS ---

pub async fn connect_and_fetch_wrapper(
    ctx: Arc<dyn AppContext>,
    config: Config,
) -> anyhow::Result<(
    RustyClient,
    Vec<CalendarListEntry>,
    Vec<TodoTask>,
    Option<String>,
    Option<String>,
)> {
    let ctx_clone = ctx.clone();
    let config_clone = config.clone();
    match tokio::time::timeout(
        std::time::Duration::from_secs(120),
        RustyClient::connect_with_fallback(ctx, config, Some("GUI")),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => {
            // Timeout occurred. Return offline fallback to avoid kicking user out.
            let client = RustyClient::new(
                ctx_clone.clone(),
                &config_clone.url,
                &config_clone.username,
                &config_clone.password,
                config_clone.allow_insecure_certs,
                Some("GUI"),
            )
            .unwrap_or_else(|_| RustyClient {
                client: None,
                ctx: ctx_clone.clone(),
            });

            let cals = crate::cache::Cache::load_calendars(ctx_clone.as_ref()).unwrap_or_default();
            let active_href = config_clone.default_calendar.clone();
            let tasks = if let Some(ref h) = active_href {
                let (mut t, _) =
                    crate::cache::Cache::load(ctx_clone.as_ref(), h).unwrap_or((vec![], None));
                crate::journal::Journal::apply_to_tasks(ctx_clone.as_ref(), &mut t, h);
                t
            } else {
                vec![]
            };

            Ok((
                client,
                cals,
                tasks,
                active_href,
                Some("Connection timed out. Check your network or server URL.".to_string()),
            ))
        }
    }
}

pub async fn async_fetch_wrapper(
    client: RustyClient,
    href: String,
) -> anyhow::Result<(String, Vec<TodoTask>)> {
    match tokio::time::timeout(std::time::Duration::from_secs(30), client.get_tasks(&href)).await {
        Ok(res) => {
            let tasks = res?;
            Ok((href, tasks))
        }
        Err(_) => Err(anyhow::anyhow!("Fetch timed out for calendar {}", href)),
    }
}

pub async fn async_fetch_all_wrapper(
    client: RustyClient,
    cals: Vec<CalendarListEntry>,
) -> anyhow::Result<Vec<(String, Vec<TodoTask>)>> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(180),
        client.get_all_tasks(&cals),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!("Fetch all timed out")),
    }
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
) -> anyhow::Result<(Vec<TodoTask>, Vec<String>)> {
    let store_arc = Arc::new(Mutex::new(store.clone()));
    let client_arc = Arc::new(Mutex::new(client));
    let controller = TaskController::new(store_arc.clone(), client_arc, ctx);

    // Record UIDs present before the action executes to detect deletions
    let mut before_uids = std::collections::HashSet::new();
    for map in store.calendars.values() {
        for uid in map.keys() {
            before_uids.insert(uid.clone());
        }
    }

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

    let _ = tokio::time::timeout(std::time::Duration::from_secs(60), action_future).await;

    let updated_store = store_arc.lock().await;

    let mut updated_tasks = Vec::new();
    let mut after_uids = std::collections::HashSet::new();

    // Only return tasks that have been mutated (e.g. given an ETag by the server)
    for map in updated_store.calendars.values() {
        for (uid, task) in map {
            after_uids.insert(uid.clone());

            let mut changed = false;
            if !before_uids.contains(uid) {
                changed = true; // Task is entirely new
            } else if let Some(old_task) = store.get_task_ref(uid)
                && (old_task.etag != task.etag
                    || old_task.href != task.href
                    || old_task.calendar_href != task.calendar_href)
                {
                    changed = true; // Metadata changed over the network
                }

            if changed {
                updated_tasks.push(task.clone());
            }
        }
    }

    let mut deleted_uids = Vec::new();
    for uid in before_uids {
        if !after_uids.contains(&uid) {
            deleted_uids.push(uid);
        }
    }

    Ok((updated_tasks, deleted_uids))
}

pub async fn async_migrate_wrapper(
    client: RustyClient,
    tasks: Vec<TodoTask>,
    target: String,
) -> anyhow::Result<usize> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(45),
        client.migrate_tasks(tasks, &target),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!("Migration timed out")),
    }
}

/// Backfill calendar events for all tasks when the global setting is enabled.
/// This is called when the user toggles the setting from OFF to ON.
pub async fn async_backfill_events_wrapper(
    client: RustyClient,
    tasks: Vec<TodoTask>,
    global_enabled: bool,
) -> Result<usize, String> {
    let futures = tasks
        .into_iter()
        .filter(|task| task.due.is_some() || task.dtstart.is_some() || !task.sessions.is_empty())
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
}

pub async fn async_delete_all_events_wrapper(
    client: RustyClient,
    calendars: Vec<String>,
) -> anyhow::Result<usize> {
    match tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut total = 0;
        for cal_href in calendars {
            if let Ok(count) = client.delete_all_companion_events(&cal_href).await {
                total += count;
            }
        }
        Ok::<usize, anyhow::Error>(total)
    })
    .await
    {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!("Deleting events timed out")),
    }
}
