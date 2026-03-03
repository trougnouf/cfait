// File: ./src/tui/network.rs
// Manages background network operations for the TUI.
//
// This actor no longer accepts high-level "intent" actions like ToggleTask or
// MarkCancelled. Those behaviors are computed by the TUI using the in-memory
// store, and the TUI now sends explicit CRUD actions (CreateTask/UpdateTask/etc.)
// for persistence. The network actor therefore acts as a dumb persistence
// pipeline: attempt online operation, otherwise journal the action and return
// status events so the UI can react.
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::context::AppContext;
use crate::controller::TaskController;
use crate::storage::{LocalCalendarRegistry, LocalStorage};
use crate::store::TaskStore;
use crate::tui::action::{Action, AppEvent};
use std::sync::Arc;
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};

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
    // 0. INITIALIZE CONTROLLER (Lightweight reconstruction for the actor context)
    // ------------------------------------------------------------------
    // Create a minimal controller instance for background tasks like pruning local trash.
    let store = Arc::new(Mutex::new(TaskStore::new(ctx.clone())));
    let client_container = Arc::new(Mutex::new(None));
    let controller = TaskController::new(store.clone(), client_container.clone(), ctx.clone());

    // PRUNE TRASH on startup (best-effort; report error to event channel)
    if let Err(e) = controller.prune_trash().await {
        let _ = event_tx
            .send(AppEvent::Error(format!("Trash prune failed: {}", e)))
            .await;
    }

    // ------------------------------------------------------------------
    // 1. LOAD CACHE IMMEDIATELY
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
        .send(AppEvent::Status {
            key: "connecting".to_string(),
            human: "Connecting...".to_string(),
        })
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
                    .send(AppEvent::Status {
                        key: "sync_warning".to_string(),
                        human: format!("Sync warning: {}", err_str),
                    })
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
        .send(AppEvent::Status {
            key: "syncing".to_string(),
            human: "Syncing...".to_string(),
        })
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
            let _ = event_tx
                .send(AppEvent::Status {
                    key: "ready".to_string(),
                    human: "Ready.".to_string(),
                })
                .await;
        }
        Err(e) => {
            let _ = event_tx
                .send(AppEvent::Status {
                    key: "sync_warning".to_string(),
                    human: format!("Sync warning: {}", e),
                })
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

            Action::CreateTask(new_task) => {
                // Delegate to TaskController (single source of truth).
                // Build a controller instance that has access to the same in-memory store
                // and a client wrapper so background syncs can be triggered.
                let client_container = Arc::new(Mutex::new(Some(client.clone())));
                let controller = TaskController::new(store.clone(), client_container, ctx.clone());

                match controller.create_task(new_task).await {
                    Ok(_uid) => {
                        // Re-read the controller store and emit a TasksLoaded event so UI refreshes.
                        let s = controller.store.lock().await;
                        let mut results: Vec<(String, Vec<crate::model::Task>)> = Vec::new();
                        for (href, map) in &s.calendars {
                            results.push((href.clone(), map.values().cloned().collect()));
                        }
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

                        let _ = event_tx
                            .send(AppEvent::Status {
                                key: "status_created".to_string(),
                                human: rust_i18n::t!("status_created").to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }

            Action::UpdateTask(task) => {
                // Use controller to perform update (handles recurrence, history, children).
                let client_container = Arc::new(Mutex::new(Some(client.clone())));
                let controller = TaskController::new(store.clone(), client_container, ctx.clone());

                match controller.update_task(task.clone()).await {
                    Ok(_warnings) => {
                        // Re-read the controller store and emit TasksLoaded so UI refreshes.
                        let s = controller.store.lock().await;
                        let mut results: Vec<(String, Vec<crate::model::Task>)> = Vec::new();
                        for (href, map) in &s.calendars {
                            results.push((href.clone(), map.values().cloned().collect()));
                        }
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

                        let _ = event_tx
                            .send(AppEvent::Status {
                                key: "status_saved".to_string(),
                                human: rust_i18n::t!("status_saved").to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        // Attempt best-effort reload from client to revert local view
                        if let Ok(t) = client.get_tasks(&task.calendar_href).await {
                            let _ = event_tx
                                .send(AppEvent::TasksLoaded(vec![(task.calendar_href.clone(), t)]))
                                .await;
                        }
                    }
                }
            }

            Action::DeleteTask(uid) => {
                // Controller will handle soft-vs-hard delete, journaling and local trash logic.
                let client_container = Arc::new(Mutex::new(Some(client.clone())));
                let controller = TaskController::new(store.clone(), client_container, ctx.clone());

                match controller.delete_task(&uid).await {
                    Ok(_warnings) => {
                        // Re-read store and emit TasksLoaded for updated UI.
                        let s = controller.store.lock().await;
                        let mut results: Vec<(String, Vec<crate::model::Task>)> = Vec::new();
                        for (href, map) in &s.calendars {
                            results.push((href.clone(), map.values().cloned().collect()));
                        }
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

                        let _ = event_tx
                            .send(AppEvent::Status {
                                key: "status_deleted".to_string(),
                                human: rust_i18n::t!("status_deleted").to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                        // Best-effort: reload calendar of interest if possible.
                        // We don't know the original href here; rely on client refresh if needed.
                    }
                }
            }

            Action::Refresh => {
                let _ = event_tx
                    .send(AppEvent::Status {
                        key: "syncing".to_string(),
                        human: "Syncing...".to_string(),
                    })
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
                            .send(AppEvent::Status {
                                key: "refreshed".to_string(),
                                human: "Refreshed.".to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }

            Action::MoveTask(uid, new_href) => {
                // Use TaskController to perform the move (handles local<->remote transitions).
                let client_container = Arc::new(Mutex::new(Some(client.clone())));
                let controller = TaskController::new(store.clone(), client_container, ctx.clone());

                match controller.move_task(&uid, &new_href).await {
                    Ok(_warnings) => {
                        // Re-read the controller store and emit TasksLoaded so the UI reflects both calendars.
                        let s = controller.store.lock().await;
                        let mut results: Vec<(String, Vec<crate::model::Task>)> = Vec::new();
                        for (href, map) in &s.calendars {
                            results.push((href.clone(), map.values().cloned().collect()));
                        }
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

                        let _ = event_tx
                            .send(AppEvent::Status {
                                key: "status_moved".to_string(),
                                human: rust_i18n::t!("status_moved").to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(AppEvent::Error(format!("Move failed: {}", e)))
                            .await;
                    }
                }
            }

            Action::MigrateLocal(source_href, target_href) => {
                let _ = event_tx
                    .send(AppEvent::Status {
                        key: "migrating_local".to_string(),
                        human: "Migrating local...".to_string(),
                    })
                    .await;

                // FIX: Load tasks from disk. The client is dumb; we must provide the data.
                if let Ok(local_tasks) = LocalStorage::load_for_href(ctx.as_ref(), &source_href) {
                    match client.migrate_tasks(local_tasks, &target_href).await {
                        Ok(count) => {
                            let _ = event_tx
                                .send(AppEvent::Status {
                                    key: "migration_complete".to_string(),
                                    human: format!("Migration complete. Moved {}.", count),
                                })
                                .await;

                            // Trigger refresh to show moved tasks
                            let _ = event_tx
                                .send(AppEvent::Status {
                                    key: "refreshing".to_string(),
                                    human: "Refreshing...".to_string(),
                                })
                                .await;
                            // (Existing refresh logic usually follows here or user presses 'r')
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx
                        .send(AppEvent::Error("Failed to load local tasks".to_string()))
                        .await;
                }
            }

            // Any other actions are ignored by the network actor; the TUI/store
            // layer is responsible for computing state changes and emitting explicit
            // persistence commands.
            _ => {}
        }
    }
}
