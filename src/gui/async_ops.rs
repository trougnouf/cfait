// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/async_ops.rs
// Asynchronous operations wrapper bridging sync GUI and async client.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::controller::TaskController;
use crate::journal::Action;
use crate::model::{CalendarListEntry, Task as TodoTask};
use crate::store::TaskStore;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender as IcedSender;
use iced::stream as iced_stream;

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

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
                Some(rust_i18n::t!("error_timeout").to_string()),
            ))
        }
    }
}

pub async fn async_fetch_wrapper(
    client: RustyClient,
    href: String,
) -> anyhow::Result<(String, Vec<TodoTask>)> {
    match tokio::time::timeout(std::time::Duration::from_secs(60), client.get_tasks(&href)).await {
        Ok(res) => {
            let tasks = res?;
            Ok((href, tasks))
        }
        Err(_) => Err(anyhow::anyhow!("Fetch timed out for calendar {}", href)),
    }
}

pub async fn async_create_remote_calendar_wrapper(
    client: RustyClient,
    name: String,
    color: Option<String>,
) -> anyhow::Result<String> {
    client.create_calendar(&name, color.as_deref()).await
}

pub async fn async_update_remote_calendar_wrapper(
    client: RustyClient,
    href: String,
    name: String,
    color: Option<String>,
) -> anyhow::Result<()> {
    client.update_calendar(&href, &name, color.as_deref()).await
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

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    UpdateClient(Option<RustyClient>),
    Batch(Vec<Action>),
    SyncNow,
}

pub fn spawn_background_worker(
    ui_tx: mpsc::Sender<crate::gui::message::Message>,
    ctx: Arc<dyn AppContext>,
) -> mpsc::Sender<WorkerCommand> {
    let (tx, mut rx) = mpsc::channel::<WorkerCommand>(100);

    tokio::spawn(async move {
        // Initialize an isolated TaskController for background persistence handling
        let store = Arc::new(tokio::sync::Mutex::new(TaskStore::new(ctx.clone())));
        let client_container = Arc::new(tokio::sync::Mutex::new(None));
        let controller = TaskController::new(store, client_container.clone(), ctx.clone());
        let mut sync_pending = false;

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(WorkerCommand::UpdateClient(c)) => {
                            *client_container.lock().await = c;
                        }
                        Some(WorkerCommand::Batch(actions)) => {
                            let _ = controller.persist_changes(actions).await;
                            sync_pending = true;
                            let _ = ui_tx.send(crate::gui::message::Message::JournalSaved).await;
                        }
                        Some(WorkerCommand::SyncNow) => {
                            sync_pending = true;
                        }
                        None => break,
                    }
                }
                // Debounce network synchronization by 500ms
                _ = sleep(Duration::from_millis(500)), if sync_pending => {
                    sync_pending = false;
                    match controller.sync_and_update_store().await {
                        Ok((_warns, synced_tasks)) => {
                            // Always send the success message to allow the GUI to update the unsynced badge
                            let _ = ui_tx.send(crate::gui::message::Message::BackgroundSyncComplete(synced_tasks)).await;
                        }
                        Err(_) => {
                            let _ = ui_tx.send(crate::gui::message::Message::BackgroundSyncFailed).await;
                        }
                    }
                }
            }
        }
    });

    tx
}

#[derive(Clone)]
struct WorkerData(Arc<dyn AppContext>);

impl std::hash::Hash for WorkerData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::any::TypeId::of::<Self>().hash(state);
    }
}

impl PartialEq for WorkerData {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for WorkerData {}

pub fn worker_subscription(
    ctx: Arc<dyn AppContext>,
) -> iced::Subscription<crate::gui::message::Message> {
    iced::Subscription::run_with(WorkerData(ctx), |data| {
        let ctx = data.0.clone();
        iced_stream::channel(
            100,
            move |mut output: IcedSender<crate::gui::message::Message>| {
                let ctx = ctx.clone();
                async move {
                    let (gui_tx, mut gui_rx) = tokio::sync::mpsc::channel(100);
                    let worker_tx = spawn_background_worker(gui_tx, ctx);

                    let _ = output
                        .send(crate::gui::message::Message::InitBackgroundWorker(
                            worker_tx,
                        ))
                        .await;

                    while let Some(msg) = gui_rx.recv().await {
                        let _ = output.send(msg).await;
                    }
                    std::future::pending::<()>().await;
                }
            },
        )
    })
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
    delete_on_completion: bool,
) -> Result<usize, String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(120),
        client.sync_multiple_companion_events(&tasks, global_enabled, delete_on_completion)
    )
    .await {
        Ok(Ok(count)) => Ok(count),
        _ => Err("Batch creation timed out or failed".to_string()),
    }
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
