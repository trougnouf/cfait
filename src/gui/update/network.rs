// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/update/network.rs
use crate::cache::Cache;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::update::common::{refresh_filtered_tasks, scroll_to_selected};
use crate::journal::Journal;
use crate::model::CalendarListEntry;
use crate::storage::{LOCAL_CALENDAR_HREF, LOCAL_CALENDAR_NAME, LocalCalendarRegistry};
use crate::system::SystemEvent;
use iced::Task;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => {
            app.loading = true;
            app.error_msg = None;

            if app.client.is_some() {
                let mut cfg = app.core_config.clone();
                cfg.password = app.ob_pass.clone(); // Re-use the securely loaded password
                return Task::perform(connect_and_fetch_wrapper(app.ctx.clone(), cfg), |res| {
                    Message::Loaded(res.map_err(|e| e.to_string()))
                });
            }
            Task::none()
        }
        Message::InitBackgroundWorker(tx) => {
            app.bg_tx = Some(tx.clone());
            if let Some(client) = &app.client {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::UpdateClient(Some(
                    client.clone(),
                )));
            }
            Task::none()
        }
        Message::BackgroundSyncComplete(synced_tasks) => {
            app.last_sync_failed = false;
            crate::gui::update::common::update_journal_state(app);

            // The TaskController updated the shared TaskStore in the background.
            // We only need to trigger a heavy UI rebuild if a completely new task
            // was introduced (like a Conflict Copy), otherwise ETags updating in the
            // background are invisible to the user.
            if synced_tasks
                .iter()
                .any(|t| t.summary.ends_with("(Conflict Copy)"))
            {
                crate::gui::update::common::refresh_filtered_tasks(app);
            }

            Task::none()
        }
        Message::BackgroundSyncFailed => {
            app.last_sync_failed = true;
            crate::gui::update::common::update_journal_state(app);
            Task::none()
        }
        Message::Loaded(Ok((client, mut cals, mut tasks, active, warning))) => {
            app.client = Some(client.clone());

            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::UpdateClient(Some(
                    client.clone(),
                )));
            }

            if let Some(w) = warning {
                app.error_msg = Some(w);
                app.last_sync_failed = true;
            } else {
                app.error_msg = None;
                app.last_sync_failed = false;
            }

            crate::gui::update::common::update_journal_state(app);

            let local_cals = LocalCalendarRegistry::load(app.ctx.as_ref()).unwrap_or_default();

            for local_cal in local_cals {
                if !cals.iter().any(|c| c.href == local_cal.href) {
                    cals.push(local_cal);
                }
            }

            if !cals.iter().any(|c| c.href == LOCAL_CALENDAR_HREF) {
                let local_entry = CalendarListEntry {
                    name: LOCAL_CALENDAR_NAME.to_string(),
                    href: LOCAL_CALENDAR_HREF.to_string(),
                    color: None,
                };
                cals.push(local_entry);
            }

            app.calendars = cals.clone();
            app.sort_calendars();

            app.store.clear();

            for cal in &app.calendars {
                if cal.href.starts_with("local://")
                    && let Ok(mut local_t) =
                        crate::storage::LocalStorage::load_for_href(app.ctx.as_ref(), &cal.href)
                {
                    Journal::apply_to_tasks(app.ctx.as_ref(), &mut local_t, &cal.href);
                    app.store.insert(cal.href.clone(), local_t);
                }
            }

            for cal in &app.calendars {
                if cal.href.starts_with("local://") {
                    continue;
                }
                if let Ok((mut cached_tasks, _)) = Cache::load(app.ctx.as_ref(), &cal.href) {
                    Journal::apply_to_tasks(app.ctx.as_ref(), &mut cached_tasks, &cal.href);
                    app.store.insert(cal.href.clone(), cached_tasks);
                }
            }

            let net_active = active;

            let mut valid_active = None;
            if let Some(current) = &app.active_cal_href
                && app.calendars.iter().any(|c| c.href == *current)
                && !app.hidden_calendars.contains(current)
            {
                valid_active = Some(current.clone());
            }

            if valid_active.is_none()
                && let Some(ref net_active_href) = net_active
                && !app.hidden_calendars.contains(net_active_href)
            {
                valid_active = Some(net_active_href.clone());
            }

            if valid_active.is_none() {
                valid_active = Some(LOCAL_CALENDAR_HREF.to_string());
            }

            app.active_cal_href = valid_active.clone();

            if let Some(href) = net_active
                && href != LOCAL_CALENDAR_HREF
                && app.error_msg.is_none()
            {
                Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
                app.store.insert(href, tasks);
            }

            let cfg = &app.core_config;
            app.hide_completed = cfg.hide_completed;
            app.hide_fully_completed_tags = cfg.hide_fully_completed_tags;
            app.tag_aliases = cfg.tag_aliases.clone();
            app.disabled_calendars = cfg.disabled_calendars.iter().cloned().collect();

            app.state = AppState::Active;
            refresh_filtered_tasks(app);
            app.loading = false;

            if let Some(tx) = &app.alarm_tx {
                let _ = tx.try_send(SystemEvent::EnableAlarms);
            }

            // FIXED: Set focus to false so it doesn't steal focus from text inputs
            // when loading completes in the background.
            let scroll_cmd = scroll_to_selected(app, false);

            if app.error_msg.is_none() {
                app.loading = true;
                Task::batch(vec![
                    Task::perform(async_fetch_all_wrapper(client, cals), |res| {
                        Message::RefreshedAll(res.map_err(|e| e.to_string()))
                    }),
                    scroll_cmd,
                ])
            } else {
                scroll_cmd
            }
        }
        Message::Loaded(Err(e)) => {
            log::error!("Connection Failed: {}", e);
            app.error_msg = Some(rust_i18n::t!("connection_failed", error = e).to_string());
            app.last_sync_failed = true;
            crate::gui::update::common::update_journal_state(app);

            if let Some(tx) = &app.alarm_tx {
                let _ = tx.try_send(SystemEvent::EnableAlarms);
            }

            if app.state != AppState::Active && app.state != AppState::Settings {
                let cfg = &app.core_config;
                if !cfg.url.is_empty() {
                    app.state = AppState::Active;
                    let cals = Cache::load_calendars(app.ctx.as_ref()).unwrap_or_default();
                    app.calendars = cals;
                    app.sort_calendars();
                    app.store.clear();
                    for cal in &app.calendars {
                        if cal.href.starts_with("local://") {
                            if let Ok(mut tasks) = crate::storage::LocalStorage::load_for_href(
                                app.ctx.as_ref(),
                                &cal.href,
                            ) {
                                crate::journal::Journal::apply_to_tasks(
                                    app.ctx.as_ref(),
                                    &mut tasks,
                                    &cal.href,
                                );
                                app.store.insert(cal.href.clone(), tasks);
                            }
                        } else if let Ok((mut tasks, _)) = Cache::load(app.ctx.as_ref(), &cal.href)
                        {
                            crate::journal::Journal::apply_to_tasks(
                                app.ctx.as_ref(),
                                &mut tasks,
                                &cal.href,
                            );
                            app.store.insert(cal.href.clone(), tasks);
                        }
                    }
                    refresh_filtered_tasks(app);
                } else {
                    app.state = AppState::Onboarding;
                }
            }

            app.loading = false;
            Task::none()
        }
        Message::RefreshedAll(Ok(results)) => {
            for (href, mut tasks) in results {
                Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
                app.store.insert(href.clone(), tasks);
            }

            app.last_sync_failed = false;
            refresh_filtered_tasks(app);
            app.loading = false;

            // FIXED: Do not steal focus after background sync completes
            scroll_to_selected(app, false)
        }
        Message::RefreshedAll(Err(e)) => {
            log::error!("Sync warning (RefreshedAll): {}", e);
            app.error_msg = Some(rust_i18n::t!("sync_warning", msg = e).to_string());
            app.last_sync_failed = true;
            app.loading = false;
            Task::none()
        }
        Message::TasksRefreshed(Ok((href, mut tasks))) => {
            app.error_msg = None;
            app.last_sync_failed = false;
            Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
            app.store.insert(href.clone(), tasks);

            if app.active_cal_href.as_deref() == Some(&href) {
                refresh_filtered_tasks(app);
                app.loading = false;
                // FIXED: Do not steal focus after changing calendars
                return scroll_to_selected(app, false);
            }
            Task::none()
        }
        Message::TasksRefreshed(Err(e)) => {
            log::error!("Fetch failed (TasksRefreshed): {}", e);
            app.error_msg = Some(rust_i18n::t!("error_fetch_failed", error = e).to_string());
            app.last_sync_failed = true;
            app.loading = false;
            Task::none()
        }
        Message::MigrationComplete(Ok(count)) => {
            app.loading = false;
            app.error_msg = Some(if count == 1 {
                rust_i18n::t!("migration_complete_moved.one").to_string()
            } else {
                rust_i18n::t!("migration_complete_moved.other", count = count).to_string()
            });
            refresh_filtered_tasks(app);
            Task::perform(async { Ok::<(), String>(()) }, |_| Message::Refresh)
        }
        Message::MigrationComplete(Err(e)) => {
            log::error!("Migration failed: {}", e);
            app.loading = false;
            app.error_msg = Some(rust_i18n::t!("migration_failed", error = e).to_string());
            Task::none()
        }
        _ => Task::none(),
    }
}
