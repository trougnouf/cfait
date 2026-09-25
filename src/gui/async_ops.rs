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
    /// Reload the disk state (config + calendars + tasks) and replace the
    /// worker's store with it. Used when another cfait instance changed files.
    FlushAndLoad,
}

pub fn spawn_background_worker(
    ui_tx: mpsc::Sender<crate::gui::message::Message>,
    ctx: Arc<dyn AppContext>,
) -> mpsc::Sender<WorkerCommand> {
    let (tx, mut rx) = mpsc::channel::<WorkerCommand>(1000);

    tokio::spawn(async move {
        // Initialize an isolated TaskController for background persistence handling
        let store = Arc::new(tokio::sync::Mutex::new(TaskStore::new(ctx.clone())));
        let client_container = Arc::new(tokio::sync::Mutex::new(None));
        let controller = TaskController::new(store, client_container.clone(), ctx.clone());
        let mut sync_pending = false;

        // Watch the data and cache directories for changes made by other cfait
        // instances (e.g. a `cfait sync` in another terminal). Events for files
        // this process just wrote are suppressed per-file in atomic_write, so a
        // quick external edit right after our own persistence is not lost.
        let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel(100);
        let mut watching = false;
        let mut _watcher = None;
        {
            use notify::{EventKind, RecursiveMode, Watcher};
            if let Ok(mut w) =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res
                        && matches!(
                            event.kind,
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                        )
                    {
                        // A batched event may carry paths from both our own write
                        // and an external one, so check each path individually
                        // (the suppression is per-file and non-consuming).
                        let is_relevant = event.paths.iter().any(|p| {
                            let is_watched_file =
                                p.file_name().and_then(|n| n.to_str()).is_some_and(|name| {
                                    name.ends_with(".json") && name != "alarm_index.json"
                                });
                            is_watched_file && !crate::storage::is_suppressed_local_write(p)
                        });
                        if is_relevant {
                            // Best-effort: a full channel already guarantees a reload
                            // is pending, so dropping a redundant signal is safe and
                            // keeps the notify thread from ever blocking.
                            let _ = watch_tx.try_send(());
                        }
                    }
                })
            {
                if let Ok(data_dir) = ctx.get_data_dir() {
                    let _ = w.watch(&data_dir, RecursiveMode::NonRecursive);
                }
                if let Ok(cache_dir) = ctx.get_cache_dir() {
                    let _ = w.watch(&cache_dir, RecursiveMode::NonRecursive);
                }
                _watcher = Some(w);
                watching = true;
            }
        }

        let mut external_change_pending = false;

        loop {
            tokio::select! {
                // External file change; the guard disables this branch once the
                // watcher channel closes so a dead watcher can't hot-spin the loop.
                res = watch_rx.recv(), if watching => {
                    if res.is_some() {
                        external_change_pending = true;
                    } else {
                        watching = false;
                    }
                }
                // Debounce external file changes by 200ms
                _ = sleep(Duration::from_millis(200)), if external_change_pending => {
                    external_change_pending = false;
                    let _ = ui_tx
                        .send(crate::gui::message::Message::ExternalChangeDetected)
                        .await;
                }
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
                        Some(WorkerCommand::FlushAndLoad) => {
                            // Channel FIFO guarantees any earlier Batch (disk writes)
                            // is fully persisted before this reload reads the disk.
                            let ctx_clone = ctx.clone();
                            let res = tokio::task::spawn_blocking(move || {
                                crate::config::Config::invalidate_cache();
                                let cfg = crate::config::Config::load_with_credentials(
                                    ctx_clone.as_ref(),
                                )
                                .unwrap_or_default();
                                let (cals, tasks) = crate::cache::Cache::load_all_disk_state(
                                    ctx_clone.as_ref(),
                                    cfg.enable_local_mode,
                                );
                                (Box::new(cfg), cals, tasks)
                            })
                            .await;
                            match res {
                                Ok((cfg, cals, tasks)) => {
                                    // Replace the worker's store with the fresh disk
                                    // state (under a single lock) so later Batch
                                    // actions operate on current data.
                                    let mut s = controller.store.lock().await;
                                    s.clear();
                                    s.insert_many(tasks.clone());
                                    drop(s);
                                    let _ = ui_tx
                                        .send(crate::gui::message::Message::ExternalReloaded(
                                            cfg, cals, tasks,
                                        ))
                                        .await;
                                }
                                Err(e) => {
                                    // Route the failure through LocalLoaded so the UI
                                    // releases its loading state instead of waiting
                                    // for an ExternalReloaded that never comes.
                                    let _ = ui_tx
                                        .send(crate::gui::message::Message::LocalLoaded(Err(
                                            format!("external reload failed: {}", e),
                                        )))
                                        .await;
                                }
                            }
                        }
                        None => break,
                    }
                }
                // Debounce network synchronization by 500ms
                _ = sleep(Duration::from_millis(500)), if sync_pending => {
                    sync_pending = false;
                    match controller.sync_and_update_store().await {
                        Ok((_warns, synced_tasks, config_changed)) => {
                            // Always send the success message to allow the GUI to update the unsynced badge
                            let _ = ui_tx.send(crate::gui::message::Message::BackgroundSyncComplete(synced_tasks)).await;

                            if config_changed {
                                let ctx_ref = ctx.clone();
                                let ui_tx_clone = ui_tx.clone();
                                tokio::spawn(async move {
                                    if let Ok(Ok(cfg)) = tokio::task::spawn_blocking(move || Config::load_with_credentials(ctx_ref.as_ref())).await {
                                        let _ = ui_tx_clone.send(crate::gui::message::Message::ConfigUpdated(Box::new(cfg))).await;
                                    }
                                });
                            }
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
            1000,
            move |mut output: IcedSender<crate::gui::message::Message>| {
                let ctx = ctx.clone();
                async move {
                    let (gui_tx, mut gui_rx) = tokio::sync::mpsc::channel(1000);
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
        client.sync_multiple_companion_events(&tasks, global_enabled, delete_on_completion),
    )
    .await
    {
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
