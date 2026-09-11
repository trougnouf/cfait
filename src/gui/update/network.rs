// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/update/network.rs
use crate::cache::Cache;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::update::common::{refresh_filtered_tasks, scroll_to_selected};
use crate::journal::Journal;
use crate::model::{CalendarListEntry, Task as TodoTask};
use crate::storage::{
    LOCAL_CALENDAR_HREF, LOCAL_CALENDAR_NAME, LOCAL_TRASH_HREF, LocalCalendarRegistry,
};
use crate::system::SystemEvent;
use iced::Task;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => {
            if app.loading {
                return Task::none();
            }
            app.loading = true;
            app.error_msg = None;

            if app.client.is_some() {
                let mut cfg = app.core_config.clone();
                cfg.password = app.ob_pass.clone(); // Re-use the securely loaded password
                app.pending_refresh_generation = app.edit_generation;
                Task::perform(connect_and_fetch_wrapper(app.ctx.clone(), cfg), |res| {
                    Message::Loaded(res.map_err(|e| e.to_string()))
                })
            } else {
                let ctx = app.ctx.clone();
                app.pending_refresh_generation = app.edit_generation;
                Task::perform(
                    async move {
                        let mut calendars =
                            crate::cache::Cache::load_calendars(ctx.as_ref()).unwrap_or_default();
                        if let Ok(locals) =
                            crate::storage::LocalCalendarRegistry::load(ctx.as_ref())
                        {
                            for loc in locals {
                                if !calendars.iter().any(|c| c.href == loc.href) {
                                    calendars.push(loc);
                                }
                            }
                        }

                        let mut store_data: Vec<(String, Vec<TodoTask>)> = Vec::new();
                        for cal in &calendars {
                            if cal.href.starts_with("local://") {
                                if let Ok(mut tasks) = crate::storage::LocalStorage::load_for_href(
                                    ctx.as_ref(),
                                    &cal.href,
                                ) {
                                    crate::journal::Journal::apply_to_tasks(
                                        ctx.as_ref(),
                                        &mut tasks,
                                        &cal.href,
                                    );
                                    store_data.push((cal.href.clone(), tasks));
                                }
                            } else if let Ok((mut tasks, _)) =
                                crate::cache::Cache::load(ctx.as_ref(), &cal.href)
                            {
                                crate::journal::Journal::apply_to_tasks(
                                    ctx.as_ref(),
                                    &mut tasks,
                                    &cal.href,
                                );
                                store_data.push((cal.href.clone(), tasks));
                            }
                        }

                        Ok::<_, String>((calendars, store_data))
                    },
                    |res| Message::LocalLoaded(res.map_err(|e| e.to_string())),
                )
            }
        }
        Message::LocalLoaded(Ok((calendars, store_data))) => {
            app.calendars = calendars;
            app.sort_calendars();

            if app.edit_generation == app.pending_refresh_generation {
                // No user edits during the async load: safe to replace the store
                // with fresh disk data (picks up other instances' changes & deletions)
                app.store.clear();
                for (href, tasks) in store_data {
                    app.store.insert(href, tasks);
                }
            } else {
                // Edits happened during the load: skip the store update to avoid
                // wiping in-memory changes. The next refresh will pick up disk changes.
                log::debug!(
                    "Skipping local refresh store update: edit generation changed during load"
                );
            }

            crate::gui::update::common::update_journal_state(app);
            refresh_filtered_tasks(app);
            app.loading = false;
            Task::none()
        }
        Message::LocalLoaded(Err(e)) => {
            log::error!("Local load failed: {}", e);
            app.error_msg = Some(e);
            app.loading = false;
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

            // Update ETags in the GUI's main store so subsequent edits don't trigger 412s.
            let mut rebuild_ui = false;
            for sync_task in &synced_tasks {
                if let Some((existing, _)) = app.store.get_task_mut(&sync_task.uid) {
                    if existing.etag != sync_task.etag || existing.href != sync_task.href {
                        existing.etag = sync_task.etag.clone();
                        existing.href = sync_task.href.clone();
                    }
                } else if sync_task.summary.ends_with("(Conflict Copy)") {
                    app.store.add_task(sync_task.clone());
                    rebuild_ui = true;
                }
            }

            // We only need to trigger a heavy UI rebuild if a completely new task
            // was introduced (like a Conflict Copy), otherwise ETags updating in the
            // background are invisible to the user.
            if rebuild_ui {
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
                    supports_vjournal: Some(true),
                };
                cals.push(local_entry);
            }

            app.calendars = cals.clone();
            app.sort_calendars();

            if app.edit_generation == app.pending_refresh_generation {
                // No user edits during the network fetch: safe to replace the store
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
            } else {
                // Edits happened during the fetch: skip store clear to preserve them.
                // Remote tasks arriving via RefreshedAll will merge via sequence protection.
                log::debug!("Skipping Loaded store clear: edit generation changed during fetch");
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
                valid_active = app
                    .calendars
                    .iter()
                    .find(|c| {
                        !app.hidden_calendars.contains(&c.href)
                            && !app.disabled_calendars.contains(&c.href)
                            && c.href != LOCAL_TRASH_HREF
                            && c.href != "local://recovery"
                    })
                    .map(|c| c.href.clone());
            }

            if valid_active.is_none() {
                valid_active = Some(LOCAL_CALENDAR_HREF.to_string());
            }

            app.active_cal_href = valid_active.clone();

            if let Some(href) = net_active
                && href != LOCAL_CALENDAR_HREF
                && app.error_msg.is_none()
                && app.edit_generation == app.pending_refresh_generation
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
                app.pending_refresh_generation = app.edit_generation;
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
                    let ctx = app.ctx.clone();
                    app.pending_refresh_generation = app.edit_generation;
                    return Task::perform(
                        async move {
                            let mut calendars = crate::cache::Cache::load_calendars(ctx.as_ref())
                                .unwrap_or_default();
                            if let Ok(locals) =
                                crate::storage::LocalCalendarRegistry::load(ctx.as_ref())
                            {
                                for loc in locals {
                                    if !calendars.iter().any(|c| c.href == loc.href) {
                                        calendars.push(loc);
                                    }
                                }
                            }

                            let mut store_data: Vec<(String, Vec<TodoTask>)> = Vec::new();
                            for cal in &calendars {
                                if cal.href.starts_with("local://") {
                                    if let Ok(mut tasks) =
                                        crate::storage::LocalStorage::load_for_href(
                                            ctx.as_ref(),
                                            &cal.href,
                                        )
                                    {
                                        crate::journal::Journal::apply_to_tasks(
                                            ctx.as_ref(),
                                            &mut tasks,
                                            &cal.href,
                                        );
                                        store_data.push((cal.href.clone(), tasks));
                                    }
                                } else if let Ok((mut tasks, _)) =
                                    crate::cache::Cache::load(ctx.as_ref(), &cal.href)
                                {
                                    crate::journal::Journal::apply_to_tasks(
                                        ctx.as_ref(),
                                        &mut tasks,
                                        &cal.href,
                                    );
                                    store_data.push((cal.href.clone(), tasks));
                                }
                            }

                            Ok::<_, String>((calendars, store_data))
                        },
                        |res| Message::LocalLoaded(res.map_err(|e| e.to_string())),
                    );
                } else {
                    app.state = AppState::Onboarding;
                }
            }

            app.loading = false;
            Task::none()
        }
        Message::RefreshedAll(Ok(results)) => {
            if app.edit_generation == app.pending_refresh_generation {
                for (href, mut tasks) in results {
                    Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
                    app.store.insert(href.clone(), tasks);
                }
            }

            app.last_sync_failed = false;
            refresh_filtered_tasks(app);
            app.loading = false;

            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::SyncNow);
            }

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
            if app.edit_generation == app.pending_refresh_generation {
                Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
                app.store.insert(href.clone(), tasks);
            }

            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::SyncNow);
            }

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
