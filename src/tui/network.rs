 // Manages background network operations for the TUI.
use crate::cache::Cache;
use crate::client::ClientManager;
use crate::config::Config;
use crate::storage::{LocalCalendarRegistry, LocalStorage};
use crate::tui::action::{Action, AppEvent};
use tokio::sync::mpsc::{Receiver, Sender};

pub async fn run_network_actor(
    mut action_rx: Receiver<Action>,
    event_tx: Sender<AppEvent>,
) {
    // 0. Load cached calendars immediately
    if let Ok(mut cached_cals) = Cache::load_calendars() {
        // Merge locals from registry
        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
        }
        let _ = event_tx.send(AppEvent::CalendarsLoaded(cached_cals.clone())).await;

        // Load cached tasks for UI fast-start
        let mut cached_tasks = Vec::new();
        for cal in &cached_cals {
            if cal.href.starts_with("local://") {
                if let Ok(tasks) = LocalStorage::load_for_href(&cal.href) {
                    cached_tasks.push((cal.href.clone(), tasks));
                }
            } else if let Ok((tasks, _)) = Cache::load(&cal.href) {
                cached_tasks.push((cal.href.clone(), tasks));
            }
        }
        if !cached_tasks.is_empty() {
            let _ = event_tx.send(AppEvent::TasksLoaded(cached_tasks)).await;
        }
    }

    // 1. Load Config and initialize ClientManager
    let config = Config::load().unwrap_or_default();
    let manager = ClientManager::new(&config.accounts, Some("TUI")).await;

    let _ = event_tx.send(AppEvent::Status("Syncing...".to_string())).await;

    // Get Calendars via Manager
    let mut calendars = manager.get_all_calendars().await;

    // Merge Locals
    if let Ok(locals) = LocalCalendarRegistry::load() {
        for loc in locals {
            if !calendars.iter().any(|c| c.href == loc.href) {
                calendars.push(loc);
            }
        }
    }

    let _ = event_tx.send(AppEvent::CalendarsLoaded(calendars.clone())).await;

    // Fetch Tasks via Manager
    match manager.get_all_tasks(&calendars).await {
        Ok(results) => {
            let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;
            let _ = event_tx.send(AppEvent::Status("Ready.".to_string())).await;
        }
        Err(e) => {
            let _ = event_tx.send(AppEvent::Status(format!("Sync partial error: {}", e))).await;
        }
    }

    // 2. Action Loop
    while let Some(action) = action_rx.recv().await {
        match action {
            Action::Quit => break,

            // Switch/Isolate Calendar: we can still fetch tasks per href using manager by resolving account_id
            Action::SwitchCalendar(href) => {
                // Attempt to find calendar and associated account
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.get_tasks(&href).await {
                        Ok(t) => {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::IsolateCalendar(href) => {
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.get_tasks(&href).await {
                        Ok(t) => {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::ToggleCalendarVisibility(href) => {
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.get_tasks(&href).await {
                        Ok(t) => {
                            let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(format!("Fetch failed: {}", e))).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::CreateTask(mut new_task) => {
                let href = new_task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.create_task(&mut new_task).await {
                        Ok(msgs) => {
                            if let Ok(t) = client.get_tasks(&href).await {
                                let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                            }
                            let s: String = if msgs.is_empty() { "Created.".to_string() } else { msgs.join("; ") };
                            let _ = event_tx.send(AppEvent::Status(s)).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::UpdateTask(mut task) => {
                let href = task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.update_task(&mut task).await {
                        Ok(msgs) => {
                            let s: String = if msgs.is_empty() { "Saved.".to_string() } else { msgs.join("; ") };
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
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::ToggleTask(mut task) => {
                let href = task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.toggle_task(&mut task).await {
                        Ok((_, _, msgs)) => {
                            let s: String = if msgs.is_empty() { "Synced.".to_string() } else { msgs.join("; ") };
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
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::DeleteTask(task) => {
                let href = task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.delete_task(&task).await {
                        Ok(msgs) => {
                            let s: String = if msgs.is_empty() { "Deleted.".to_string() } else { msgs.join("; ") };
                            let _ = event_tx.send(AppEvent::Status(s)).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                            if let Ok(t) = client.get_tasks(&href).await {
                                let _ = event_tx.send(AppEvent::TasksLoaded(vec![(href, t)])).await;
                            }
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::Refresh => {
                let _ = event_tx.send(AppEvent::Status("Refreshing...".to_string())).await;
                // Re-fetch calendars from all connected accounts
                calendars = manager.get_all_calendars().await;
                // Merge local registry
                if let Ok(locals) = LocalCalendarRegistry::load() {
                    for loc in locals {
                        if !calendars.iter().any(|c| c.href == loc.href) {
                            calendars.push(loc);
                        }
                    }
                }
                let _ = event_tx.send(AppEvent::CalendarsLoaded(calendars.clone())).await;
                match manager.get_all_tasks(&calendars).await {
                    Ok(results) => {
                        let _ = event_tx.send(AppEvent::TasksLoaded(results)).await;
                        let _ = event_tx.send(AppEvent::Status("Refreshed.".to_string())).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(AppEvent::Error(e)).await;
                    }
                }
            }

            Action::MarkInProcess(mut task) => {
                let href = task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.update_task(&mut task).await {
                        Ok(msgs) => {
                            let s: String = if msgs.is_empty() { "Saved.".to_string() } else { msgs.join("; ") };
                            let _ = event_tx.send(AppEvent::Status(s)).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::MarkCancelled(mut task) => {
                let href = task.calendar_href.clone();
                let acc_id = calendars.iter().find(|c| c.href == href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                if let Some(client) = manager.get_client(&acc_id) {
                    match client.update_task(&mut task).await {
                        Ok(msgs) => {
                            let s: String = if msgs.is_empty() { "Saved.".to_string() } else { msgs.join("; ") };
                            let _ = event_tx.send(AppEvent::Status(s)).await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(AppEvent::Error(e)).await;
                        }
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                }
            }

            Action::MoveTask(task, new_href) => {
                let old_href = task.calendar_href.clone();
                // Determine account for old_href and new_href
                let old_acc = calendars.iter().find(|c| c.href == old_href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());
                let new_acc = calendars.iter().find(|c| c.href == new_href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());

                if old_acc == new_acc {
                    if let Some(client) = manager.get_client(&old_acc) {
                        match client.move_task(&task, &new_href).await {
                            Ok((_, msgs)) => {
                                let s: String = if msgs.is_empty() { "Moved.".to_string() } else { msgs.join("; ") };
                                let _ = event_tx.send(AppEvent::Status(s)).await;
                                if let Ok(t1) = client.get_tasks(&old_href).await {
                                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(old_href, t1)])).await;
                                }
                                if let Ok(t2) = client.get_tasks(&new_href).await {
                                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(new_href, t2)])).await;
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(AppEvent::Error(format!("Move failed: {}", e))).await;
                            }
                        }
                    } else {
                        let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                    }
                } else {
                    let _ = event_tx.send(AppEvent::Error("Cross-account move not supported".to_string())).await;
                }
            }

            Action::MigrateLocal(source_href, target_href) => {
                if let Ok(local_tasks) = LocalStorage::load_for_href(&source_href) {
                    let _ = event_tx.send(AppEvent::Status(format!("Exporting {} tasks from {}...", local_tasks.len(), source_href))).await;

                    let acc_id = calendars.iter().find(|c| c.href == target_href).map(|c| c.account_id.clone()).unwrap_or_else(|| "default".to_string());

                    if let Some(client) = manager.get_client(&acc_id) {
                        match client.migrate_tasks(local_tasks, &target_href).await {
                            Ok(count) => {
                                let _ = event_tx.send(AppEvent::Status(format!("Exported {} tasks.", count))).await;
                                if let Ok(t1) = client.get_tasks(&source_href).await {
                                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(source_href.clone(), t1)])).await;
                                }
                                if let Ok(t2) = client.get_tasks(&target_href).await {
                                    let _ = event_tx.send(AppEvent::TasksLoaded(vec![(target_href, t2)])).await;
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(AppEvent::Error(format!("Export failed: {}", e))).await;
                            }
                        }
                    } else {
                        let _ = event_tx.send(AppEvent::Error("Offline".to_string())).await;
                    }
                }
            }

            Action::StartCreateChild(_) => {}
        }
    }
}
