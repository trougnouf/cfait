// SPDX-License-Identifier: GPL-3.0-or-later
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
    pub enable_local_mode: bool,
    pub default_cal: Option<String>,
}

fn apply_local_mode_filter(
    calendars: &mut Vec<crate::model::CalendarListEntry>,
    enable_local_mode: bool,
) {
    if !enable_local_mode {
        calendars.retain(|c| !c.href.starts_with("local://"));
    }
}

async fn merge_results_into_store(
    store: &Arc<Mutex<TaskStore>>,
    results: &[(String, Vec<crate::model::Task>)],
) {
    let mut s = store.lock().await;
    for (href, tasks) in results {
        s.insert(href.clone(), tasks.clone());
    }
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
        enable_local_mode,
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
            .send(AppEvent::Error(
                rust_i18n::t!("error_trash_prune", error = e.to_string()).to_string(),
            ))
            .await;
    }

    // ------------------------------------------------------------------
    // 1. LOAD CACHE IMMEDIATELY
    // ------------------------------------------------------------------
    if let Ok(mut cached_cals) = Cache::load_calendars(ctx.as_ref()) {
        // Load local registry and merge
        if enable_local_mode && let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
        }
        apply_local_mode_filter(&mut cached_cals, enable_local_mode);

        let _ = event_tx
            .send(AppEvent::CalendarsLoaded(cached_cals.clone()))
            .await;

        let mut cached_tasks = Vec::new();
        // Load tasks for all local calendars
        for cal in &cached_cals {
            if cal.href.starts_with("local://") {
                if let Ok(mut tasks) = LocalStorage::load_for_href(ctx.as_ref(), &cal.href) {
                    crate::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &cal.href);
                    cached_tasks.push((cal.href.clone(), tasks));
                }
            } else if let Ok((mut tasks, _)) = Cache::load(ctx.as_ref(), &cal.href) {
                crate::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &cal.href);
                cached_tasks.push((cal.href.clone(), tasks));
            }
        }

        if !cached_tasks.is_empty() {
            merge_results_into_store(&store, &cached_tasks).await;
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
                let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                return;
            }
        };
    let _ = event_tx
        .send(AppEvent::Status {
            key: "connecting".to_string(),
            human: rust_i18n::t!("connecting").to_string(),
        })
        .await;

    let mut calendars = match client.get_calendars().await {
        Ok((cals, _)) => cals,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("InvalidCertificate") {
                let mut helpful_msg = rust_i18n::t!("error_invalid_tls_detailed")
                    .trim()
                    .to_string();
                let config_path = crate::config::Config::get_path_string(ctx.as_ref())
                    .unwrap_or_else(|_| "path unknown".to_string());
                let config_advice = rust_i18n::t!("error_tls_config_advice").trim().to_string();
                if !allow_insecure {
                    helpful_msg.push('\n');
                    helpful_msg.push_str(rust_i18n::t!("error_tls_self_hosted").trim());
                }
                helpful_msg.push_str("\n\n");
                helpful_msg.push_str(&config_advice);
                helpful_msg.push_str("\n  ");
                helpful_msg.push_str(&config_path);
                let _ = event_tx.send(AppEvent::Error(helpful_msg)).await;
                return;
            } else {
                let _ = event_tx
                    .send(AppEvent::Status {
                        key: "sync_warning".to_string(),
                        human: rust_i18n::t!("sync_warning", msg = err_str).to_string(),
                    })
                    .await;
                vec![]
            }
        }
    };
    apply_local_mode_filter(&mut calendars, enable_local_mode);

    // Merge locals again after network discovery
    if enable_local_mode && let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
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
            human: rust_i18n::t!("syncing").to_string(),
        })
        .await;

    // Load tasks again with validated calendars list
    let mut cached_results = Vec::new();
    for cal in &calendars {
        if cal.href.starts_with("local://") {
            if let Ok(mut tasks) = LocalStorage::load_for_href(ctx.as_ref(), &cal.href) {
                crate::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &cal.href);
                cached_results.push((cal.href.clone(), tasks));
            }
        } else if let Ok((mut tasks, _)) = Cache::load(ctx.as_ref(), &cal.href) {
            crate::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &cal.href);
            cached_results.push((cal.href.clone(), tasks));
        }
    }
    if !cached_results.is_empty() {
        merge_results_into_store(&store, &cached_results).await;
        let _ = event_tx.send(AppEvent::TasksLoaded(cached_results)).await;
    }

    match client.get_all_tasks(&calendars).await {
        Ok(results) => {
            merge_results_into_store(&store, &results).await;
            let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

            let client_container = Arc::new(tokio::sync::Mutex::new(Some(client.clone())));
            let controller = TaskController::new(store.clone(), client_container, ctx.clone());

            // Trigger a full sync pass which handles settings first, then uploads any pending changes.
            if let Ok((_warns, _synced, config_changed)) = controller.sync_and_update_store().await
                && config_changed
                && let Ok(cfg) = crate::config::Config::load(ctx.as_ref())
            {
                let _ = event_tx.send(AppEvent::ConfigUpdated(Box::new(cfg))).await;
            }

            let _ = event_tx
                .send(AppEvent::Status {
                    key: "ready".to_string(),
                    human: rust_i18n::t!("ready").to_string(),
                })
                .await;
        }
        Err(e) => {
            let _ = event_tx
                .send(AppEvent::Status {
                    key: "sync_warning".to_string(),
                    human: rust_i18n::t!("sync_warning", msg = e).to_string(),
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
                    merge_results_into_store(&store, &[(href.clone(), t.clone())]).await;
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                }
            },

            Action::IsolateCalendar(href) => match client.get_tasks(&href).await {
                Ok(t) => {
                    merge_results_into_store(&store, &[(href.clone(), t.clone())]).await;
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                }
            },

            Action::ToggleCalendarVisibility(href) => match client.get_tasks(&href).await {
                Ok(t) => {
                    merge_results_into_store(&store, &[(href.clone(), t.clone())]).await;
                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(AppEvent::Error(
                            rust_i18n::t!("error_fetch_failed", error = e.to_string()).to_string(),
                        ))
                        .await;
                }
            },

            Action::PersistBatch(actions) => {
                // To keep the network actor's store in sync, apply actions here too
                let mut s = store.lock().await;
                for action in &actions {
                    match action {
                        crate::journal::Action::Create(t) | crate::journal::Action::Update(t) => {
                            s.update_or_add_task(t.clone());
                        }
                        crate::journal::Action::Delete(t) => {
                            let _ = s.delete_task(&t.uid);
                        }
                        crate::journal::Action::Move(t, target) => {
                            let _ = s.move_task(&t.uid, target.clone());
                        }
                    }
                }
                drop(s);

                let client_container = Arc::new(Mutex::new(Some(client.clone())));
                let controller = TaskController::new(store.clone(), client_container, ctx.clone());

                match controller.persist_changes(actions).await {
                    Ok(_) => {
                        let controller_clone = controller.clone();
                        let event_tx_clone = event_tx.clone();
                        let ctx_clone = ctx.clone();

                        tokio::spawn(async move {
                            if let Ok((_warns, synced_tasks, config_changed)) =
                                controller_clone.sync_and_update_store().await
                            {
                                if config_changed
                                    && let Ok(cfg) = crate::config::Config::load(ctx_clone.as_ref())
                                {
                                    let _ = event_tx_clone
                                        .send(AppEvent::ConfigUpdated(Box::new(cfg)))
                                        .await;
                                }

                                // Send TaskSynced events instead of TasksLoaded to update metadata
                                // without overwriting the UI's optimistic state!
                                for sync_task in synced_tasks {
                                    let _ = event_tx_clone
                                        .send(AppEvent::TaskSynced {
                                            uid: sync_task.uid,
                                            href: sync_task.href,
                                            etag: sync_task.etag,
                                            sequence: sync_task.sequence,
                                        })
                                        .await;
                                }

                                let _ = event_tx_clone
                                    .send(AppEvent::Status {
                                        key: "status_saved".to_string(),
                                        human: rust_i18n::t!("status_saved").to_string(),
                                    })
                                    .await;
                            }
                        });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }

            Action::Refresh => {
                let _ = event_tx
                    .send(AppEvent::Status {
                        key: "syncing".to_string(),
                        human: rust_i18n::t!("syncing").to_string(),
                    })
                    .await;

                let mut calendars = match client.get_calendars().await {
                    Ok((c, _)) => c,
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                        vec![]
                    }
                };
                apply_local_mode_filter(&mut calendars, enable_local_mode);

                // Merge local calendars from registry
                if enable_local_mode && let Ok(locals) = LocalCalendarRegistry::load(ctx.as_ref()) {
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
                        merge_results_into_store(&store, &results).await;
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;

                        let client_container =
                            Arc::new(tokio::sync::Mutex::new(Some(client.clone())));
                        let controller =
                            TaskController::new(store.clone(), client_container, ctx.clone());

                        // Trigger a full sync pass which handles settings first, then uploads any pending changes.
                        if let Ok((_warns, _synced, config_changed)) =
                            controller.sync_and_update_store().await
                            && config_changed
                            && let Ok(cfg) = crate::config::Config::load(ctx.as_ref())
                        {
                            let _ = event_tx.send(AppEvent::ConfigUpdated(Box::new(cfg))).await;
                        }

                        let _ = event_tx
                            .send(AppEvent::Status {
                                key: "refreshed".to_string(),
                                human: rust_i18n::t!("refreshed").to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                    }
                }
            }

            Action::MigrateLocal(source_href, target_href) => {
                let _ = event_tx
                    .send(AppEvent::Status {
                        key: "migrating_local".to_string(),
                        human: rust_i18n::t!("migrating_local").to_string(),
                    })
                    .await;

                // FIX: Load tasks from disk. The client is dumb; we must provide the data.
                if let Ok(local_tasks) = LocalStorage::load_for_href(ctx.as_ref(), &source_href) {
                    match client.migrate_tasks(local_tasks, &target_href).await {
                        Ok(count) => {
                            let human = if count == 1 {
                                rust_i18n::t!("migration_complete_moved.one").to_string()
                            } else {
                                rust_i18n::t!("migration_complete_moved.other", count = count)
                                    .to_string()
                            };
                            let _ = event_tx
                                .send(AppEvent::Status {
                                    key: "migration_complete".to_string(),
                                    human,
                                })
                                .await;

                            // Trigger refresh to show moved tasks
                            let _ = event_tx
                                .send(AppEvent::Status {
                                    key: "refreshing".to_string(),
                                    human: rust_i18n::t!("refreshing").to_string(),
                                })
                                .await;
                            // (Existing refresh logic usually follows here or user presses 'r')
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e.to_string())).await;
                        }
                    }
                } else {
                    let _ = event_tx
                        .send(AppEvent::Error(
                            rust_i18n::t!("failed_to_load_local_tasks").to_string(),
                        ))
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
