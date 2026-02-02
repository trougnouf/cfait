// File: ./src/tui/network.rs
// Manages background network operations for the TUI.
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::context::AppContext;
use crate::storage::{LocalCalendarRegistry, LocalStorage};
use crate::tui::action::{Action, AppEvent};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct NetworkActorConfig {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub allow_insecure: bool,
    pub default_cal: Option<String>,
}

pub async fn run_network_actor(
    ctx: Arc<dyn AppContext>,
    config: NetworkActorConfig,
    mut action_rx: Receiver<Action>,
    event_tx: Sender<AppEvent>,
) {
    let NetworkActorConfig {
        url,
        user,
        pass,
        allow_insecure,
        default_cal: _default_cal,
    } = config;
    // ------------------------------------------------------------------
    // 0. LOAD CACHE IMMEDIATELY
    // ------------------------------------------------------------------
    if let Ok(mut cached_cals) = Cache::load_calendars(ctx.as_ref()) {
        // Load local registry and merge
        if let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
        }

        let _ = event_tx
            .send(AppEvent::CalendarsLoaded(cached_cals.clone()))
            .await;

        let mut cached_tasks = Vec::new();
        // Load tasks for all local calendars
        for cal in &cached_cals {
            if cal.href.starts_with("local://") {
                if let Ok(tasks) = LocalStorage::load_for_href(ctx.as_ref(), &cal.href) {
                    cached_tasks.push((cal.href.clone(), tasks));
                }
            } else if let Ok((tasks, _)) = Cache::load(ctx.as_ref(), &cal.href) {
                cached_tasks.push((cal.href.clone(), tasks));
            }
        }

        if !cached_tasks.is_empty() {
            let _ = event_tx.send(AppEvent::TasksLoaded(cached_tasks)).await;
        }
    }

    // ------------------------------------------------------------------
    // 1. CONNECT & SYNC
    // ------------------------------------------------------------------
    let client: RustyClient =
        match RustyClient::new(ctx.clone(), &url, &user, &pass, allow_insecure, Some("TUI")) {
            Ok(c) => c,
            Err(e) => {
                let _ = event_tx.send(AppEvent::Error(e)).await;
                return;
            }
        };
    let _ = event_tx
        .send(AppEvent::Status("Connecting...".to_string()))
        .await;

    let mut calendars = match client.get_calendars().await {
        Ok((cals, _)) => cals,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("InvalidCertificate") {
                let mut helpful_msg =
                    "Connection failed: The server presented an invalid TLS/SSL certificate."
                        .to_string();
                let config_advice = format!(
                    "\n\nTo fix this, please edit your config file:\n  {}",
                    crate::config::Config::get_path_string(ctx.as_ref())
                        .unwrap_or_else(|_| "path unknown".to_string())
                );
                if !allow_insecure {
                    helpful_msg.push_str(
                        "\nIf this is a self-hosted server, set 'allow_insecure_certs = true'.",
                    );
                }
                helpful_msg.push_str(&config_advice);
                let _ = event_tx.send(AppEvent::Error(helpful_msg)).await;
                return;
            } else {
                let _ = event_tx
                    .send(AppEvent::Status(format!("Sync warning: {}", err_str)))
                    .await;
                vec![]
            }
        }
    };

    // Merge locals again after network discovery
    if let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
        for loc in locals {
            if !calendars.iter().any(|c| c.href == loc.href) {
                calendars.push(loc);
            }
        }
    }

    let _ = event_tx
        .send(AppEvent::CalendarsLoaded(calendars.clone()))
        .await;

    let _ = event_tx
        .send(AppEvent::Status("Syncing...".to_string()))
        .await;

    // Load tasks again with validated calendars list
    let mut cached_results = Vec::new();
    for cal in &calendars {
        if cal.href.starts_with("local://") {
            if let Ok(tasks) = LocalStorage::load_for_href(ctx.as_ref(), &cal.href) {
                cached_results.push((cal.href.clone(), tasks));
            }
        } else if let Ok((tasks, _)) = Cache::load(ctx.as_ref(), &cal.href) {
            cached_results.push((cal.href.clone(), tasks));
        }
    }
    if !cached_results.is_empty() {
        let _ = event_tx.send(AppEvent::TasksLoaded(cached_results)).await;
    }

    match client.get_all_tasks(&calendars).await {
        Ok(results) => {
            let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;
            let _ = event_tx.send(AppEvent::Status("Ready.".to_string())).await;
        }
        Err(e) => {
            let _ = event_tx
                .send(AppEvent::Status(format!("Sync warning: {}", e)))
                .await;
        }
    }

    // ------------------------------------------------------------------
    // 2. ACTION LOOP
    // ------------------------------------------------------------------
    while let Some(action) = action_rx.recv().await {
        match action {
            Action::Quit => break,
            Action::SwitchCalendar(href) => match client.get_tasks(&href).await {
                Ok(t) => {
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e)).await;
                }
            },
            Action::IsolateCalendar(href) => match client.get_tasks(&href).await {
                Ok(t) => {
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e)).await;
                }
            },
            Action::ToggleCalendarVisibility(href) => match client.get_tasks(&href).await {
                Ok(t) => {
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(AppEvent::Error(format!("Fetch failed: {}", e)))
                        .await;
                }
            },
            Action::CreateTask(mut new_task) => {
                let href = new_task.calendar_href.clone();
                match client.create_task(&mut new_task).await {
                    Ok(msgs) => {
                        if let Ok(t) = client.get_tasks(&href).await {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                        let s: String = if msgs.is_empty() {
                            "Created.".to_string()
                        } else {
                            msgs.join("; ")
                        };
                        let _ = event_tx.send(AppEvent::Status(s)).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }
            Action::UpdateTask(mut task) => {
                let href = task.calendar_href.clone();
                match client.update_task(&mut task).await {
                    Ok(msgs) => {
                        let s: String = if msgs.is_empty() {
                            "Saved.".to_string()
                        } else {
                            msgs.join("; ")
                        };
                        let _ = event_tx.send(AppEvent::Status(s)).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        // On error, reload to revert
                        if let Ok(t) = client.get_tasks(&href).await {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                    }
                }
            }
            Action::ToggleTask(mut task) => {
                let href = task.calendar_href.clone();

                match client.toggle_task(&mut task).await {
                    Ok((_, _, msgs)) => {
                        let s: String = if msgs.is_empty() {
                            "Synced.".to_string()
                        } else {
                            msgs.join("; ")
                        };
                        let _ = event_tx.send(AppEvent::Status(s)).await;
                        if let Ok(t) = client.get_tasks(&href).await {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        if let Ok(t) = client.get_tasks(&href).await {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                    }
                }
            }
            Action::DeleteTask(task) => {
                let href = task.calendar_href.clone();
                match client.delete_task(&task).await {
                    Ok(msgs) => {
                        let s: String = if msgs.is_empty() {
                            "Deleted.".to_string()
                        } else {
                            msgs.join("; ")
                        };
                        let _ = event_tx.send(AppEvent::Status(s)).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        if let Ok(t) = client.get_tasks(&href).await {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                    }
                }
            }
            Action::Refresh => {
                let _ = event_tx
                    .send(AppEvent::Status("Refreshing...".to_string()))
                    .await;

                let mut calendars = match client.get_calendars().await {
                    Ok((c, _)) => c,
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        vec![]
                    }
                };

                // Merge local calendars from registry
                if let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
                    for loc in locals {
                        if !calendars.iter().any(|c| c.href == loc.href) {
                            calendars.push(loc);
                        }
                    }
                }

                let _ = event_tx
                    .send(AppEvent::CalendarsLoaded(calendars.clone()))
                    .await;

                match client.get_all_tasks(&calendars).await {
                    Ok(results) => {
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;
                        let _ = event_tx
                            .send(AppEvent::Status("Refreshed.".to_string()))
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }
            Action::MarkInProcess(mut task) => match client.update_task(&mut task).await {
                Ok(msgs) => {
                    let s: String = if msgs.is_empty() {
                        "Saved.".to_string()
                    } else {
                        msgs.join("; ")
                    };
                    let _ = event_tx.send(AppEvent::Status(s)).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e)).await;
                }
            },
            Action::MarkCancelled(mut task) => match client.update_task(&mut task).await {
                Ok(msgs) => {
                    let s: String = if msgs.is_empty() {
                        "Saved.".to_string()
                    } else {
                        msgs.join("; ")
                    };
                    let _ = event_tx.send(AppEvent::Status(s)).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e)).await;
                }
            },
            Action::MoveTask(task, new_href) => {
                let old_href = task.calendar_href.clone();
                match client.move_task(&task, &new_href).await {
                    Ok((_, msgs)) => {
                        let s: String = if msgs.is_empty() {
                            "Moved.".to_string()
                        } else {
                            msgs.join("; ")
                        };
                        let _ = event_tx.send(AppEvent::Status(s)).await;
                        if let Ok(t1) = client.get_tasks(&old_href).await {
                            let _ = event_tx
                                .send(AppEvent::TasksLoaded(vec![(old_href, t1)]))
                                .await;
                        }
                        if let Ok(t2) = client.get_tasks(&new_href).await {
                            let _ = event_tx
                                .send(AppEvent::TasksLoaded(vec![(new_href, t2)]))
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(AppEvent::Error(format!("Move failed: {}", e)))
                            .await;
                    }
                }
            }
            Action::MigrateLocal(source_href, target_href) => {
                // Export from specified local calendar to target CalDAV calendar
                if let Ok(local_tasks) = LocalStorage::load_for_href(ctx.as_ref(), &source_href) {
                    let _ = event_tx
                        .send(AppEvent::Status(format!(
                            "Exporting {} tasks from {}...",
                            local_tasks.len(),
                            source_href
                        )))
                        .await;
                    match client.migrate_tasks(local_tasks, &target_href).await {
                        Ok(count) => {
                            let _ = event_tx
                                .send(AppEvent::Status(format!("Exported {} tasks.", count)))
                                .await;
                            // Reload source local calendar
                            if let Ok(t1) = client.get_tasks(&source_href).await {
                                let _ = event_tx
                                    .send(AppEvent::TasksLoaded(vec![(source_href.clone(), t1)]))
                                    .await;
                            }
                            if let Ok(t2) = client.get_tasks(&target_href).await {
                                let _ = event_tx
                                    .send(AppEvent::TasksLoaded(vec![(target_href, t2)]))
                                    .await;
                            }
                        }
                        Err(e) => {
                            let _ = event_tx
                                .send(AppEvent::Error(format!("Export failed: {}", e)))
                                .await;
                        }
                    }
                }
            }
            Action::StartCreateChild(_parent_uid) => {
                // UI logic only
            }
        }
    }
}
