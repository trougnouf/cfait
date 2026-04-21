// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/async_ops.rs
// Asynchronous operations wrapper bridging sync GUI and async client.
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::AppContext;
use crate::journal::{Action, Journal};
use crate::model::{CalendarListEntry, Task as TodoTask};
use crate::storage::LocalStorage;
use futures::stream::{self, StreamExt};
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
        let mut client: Option<RustyClient> = None;
        let mut sync_pending = false;

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(WorkerCommand::UpdateClient(c)) => {
                            client = c;
                        }
                        Some(WorkerCommand::Batch(actions)) => {
                            let mut local_actions = Vec::new();
                            let mut remote_actions = Vec::new();

                            for action in actions {
                                match action {
                                    Action::Create(t) | Action::Update(t) => {
                                        if t.calendar_href.starts_with("local://") {
                                            local_actions.push(Action::Create(t));
                                        } else {
                                            remote_actions.push(Action::Update(t));
                                        }
                                    }
                                    Action::Delete(t) => {
                                        if t.calendar_href.starts_with("local://") {
                                            local_actions.push(Action::Delete(t));
                                        } else {
                                            remote_actions.push(Action::Delete(t));
                                        }
                                    }
                                    Action::Move(t, target) => {
                                        if t.calendar_href.starts_with("local://") && target.starts_with("local://") {
                                            local_actions.push(Action::Move(t, target));
                                        } else if t.calendar_href.starts_with("local://") && !target.starts_with("local://") {
                                            local_actions.push(Action::Delete(t.clone()));
                                            let mut moved = t.clone();
                                            moved.calendar_href = target.clone();
                                            moved.href = String::new();
                                            moved.etag = String::new();
                                            remote_actions.push(Action::Create(moved));
                                        } else if !t.calendar_href.starts_with("local://") && target.starts_with("local://") {
                                            remote_actions.push(Action::Delete(t.clone()));
                                            let mut moved = t.clone();
                                            moved.calendar_href = target.clone();
                                            local_actions.push(Action::Create(moved));
                                        } else {
                                            remote_actions.push(Action::Move(t, target));
                                        }
                                    }
                                }
                            }

                            // Process local actions immediately
                            for action in local_actions {
                                match action {
                                    Action::Create(t) | Action::Update(t) => {
                                        let _ = LocalStorage::modify_for_href(ctx.as_ref(), &t.calendar_href, |all| {
                                            if let Some(idx) = all.iter().position(|item| item.uid == t.uid) {
                                                all[idx] = t.clone();
                                            } else {
                                                all.push(t.clone());
                                            }
                                        });
                                    }
                                    Action::Delete(t) => {
                                        let _ = LocalStorage::modify_for_href(ctx.as_ref(), &t.calendar_href, |all| {
                                            all.retain(|item| item.uid != t.uid);
                                        });
                                    }
                                    Action::Move(t, target) => {
                                        let _ = LocalStorage::modify_for_href(ctx.as_ref(), &t.calendar_href, |all| {
                                            all.retain(|item| item.uid != t.uid);
                                        });
                                        let mut moved = t.clone();
                                        moved.calendar_href = target.clone();
                                        let _ = LocalStorage::modify_for_href(ctx.as_ref(), &target, |all| {
                                            all.push(moved);
                                        });
                                    }
                                }
                            }

                            // Process remote actions via Journal with automatic compaction
                            if !remote_actions.is_empty() {
                                let _ = Journal::modify(ctx.as_ref(), |queue| {
                                    queue.extend(remote_actions);
                                    let mut tmp_j = Journal { queue: std::mem::take(queue) };
                                    tmp_j.compact();
                                    *queue = tmp_j.queue;
                                });
                                sync_pending = true;
                            }
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
                    if let Some(c) = &client {
                        if let Ok((_warns, synced_tasks)) = c.sync_journal().await {
                            if !synced_tasks.is_empty() {
                                let _ = ui_tx.send(crate::gui::message::Message::BackgroundSyncComplete(synced_tasks)).await;
                            }
                        } else {
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
