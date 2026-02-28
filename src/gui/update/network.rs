// File: ./src/gui/update/network.rs
use crate::cache::Cache;
use crate::config::Config;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::update::common::{refresh_filtered_tasks, save_config, scroll_to_selected};
use crate::journal::Journal;
use crate::model::CalendarListEntry;
use crate::storage::{LOCAL_CALENDAR_HREF, LOCAL_CALENDAR_NAME, LocalCalendarRegistry};
use crate::system::SystemEvent;
use iced::Task;
use iced::widget::text_editor;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::Refresh => {
            app.loading = true;
            app.error_msg = None;

            if app.client.is_some()
                && let Ok(cfg) = Config::load(app.ctx.as_ref())
            {
                return Task::perform(
                    connect_and_fetch_wrapper(app.ctx.clone(), cfg),
                    Message::Loaded,
                );
            }
            Task::none()
        }
        Message::Loaded(Ok((client, mut cals, mut tasks, mut active, warning))) => {
            app.client = Some(client.clone());

            if let Some(w) = warning {
                app.error_msg = Some(w);
            } else {
                app.error_msg = None;
            }

            app.unsynced_changes = !Journal::load(app.ctx.as_ref()).is_empty();

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

            app.store.clear();

            for cal in &app.calendars {
                if cal.href.starts_with("local://")
                    && let Ok(mut local_t) = crate::gui::async_ops::get_runtime()
                        .block_on(async { client.get_tasks(&cal.href).await })
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

            let mut valid_active = None;
            if let Some(current) = &app.active_cal_href
                && app.calendars.iter().any(|c| c.href == *current)
                && !app.hidden_calendars.contains(current)
            {
                valid_active = Some(current.clone());
            }

            if valid_active.is_none()
                && let Some(net_active) = active
                && !app.hidden_calendars.contains(&net_active)
            {
                valid_active = Some(net_active);
            }

            if valid_active.is_none() {
                valid_active = Some(LOCAL_CALENDAR_HREF.to_string());
            }

            active = valid_active;
            app.active_cal_href = active.clone();

            if let Some(href) = &active
                && href != LOCAL_CALENDAR_HREF
                && app.error_msg.is_none()
            {
                Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, href);
                app.store.insert(href.clone(), tasks);
            }

            if let Ok(cfg) = Config::load(app.ctx.as_ref()) {
                app.hide_completed = cfg.hide_completed;
                app.hide_fully_completed_tags = cfg.hide_fully_completed_tags;
                app.tag_aliases = cfg.tag_aliases;
                app.disabled_calendars = cfg.disabled_calendars.into_iter().collect();
            }

            if !app.ob_url.is_empty() {
                save_config(app);
            }

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
                    Task::perform(async_fetch_all_wrapper(client, cals), Message::RefreshedAll),
                    scroll_cmd,
                ])
            } else {
                scroll_cmd
            }
        }
        Message::Loaded(Err(e)) => {
            app.error_msg = Some(format!("Connection Failed: {}", e));

            if let Some(tx) = &app.alarm_tx {
                let _ = tx.try_send(SystemEvent::EnableAlarms);
            }

            app.state = AppState::Onboarding;
            app.loading = false;
            Task::none()
        }
        Message::RefreshedAll(Ok(results)) => {
            for (href, mut tasks) in results {
                Journal::apply_to_tasks(app.ctx.as_ref(), &mut tasks, &href);
                app.store.insert(href.clone(), tasks);
            }

            refresh_filtered_tasks(app);
            app.loading = false;

            // FIXED: Do not steal focus after background sync completes
            scroll_to_selected(app, false)
        }
        Message::RefreshedAll(Err(e)) => {
            app.error_msg = Some(format!("Sync warning: {}", e));
            app.loading = false;
            Task::none()
        }
        Message::TasksRefreshed(Ok((href, mut tasks))) => {
            app.error_msg = None;
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
            app.error_msg = Some(format!("Fetch: {}", e));
            app.loading = false;
            Task::none()
        }
        Message::SyncSaved(Ok(mut updated)) => {
            if updated.etag.is_empty() {
                updated.etag = "pending_refresh".to_string();
            }

            app.store.update_or_add_task(updated);

            app.unsynced_changes = !Journal::load(app.ctx.as_ref()).is_empty();
            if app.unsynced_changes {
                app.error_msg = Some("Offline: Changes queued.".to_string());
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SyncSaved(Err(e)) => {
            app.error_msg = Some(format!("Sync Error: {}", e));
            Task::none()
        }
        Message::SyncToggleComplete(boxed_res) => match *boxed_res {
            Ok((mut updated, created_opt)) => {
                if updated.etag.is_empty() {
                    updated.etag = "pending_refresh".to_string();
                }
                app.store.update_or_add_task(updated);

                if let Some(mut created) = created_opt {
                    if created.etag.is_empty() {
                        created.etag = "pending_refresh".to_string();
                    }
                    app.store.update_or_add_task(created);
                }
                refresh_filtered_tasks(app);
                Task::none()
            }
            Err(e) => {
                app.error_msg = Some(format!("Toggle Error: {}", e));
                Task::none()
            }
        },
        Message::TaskMoved(Ok(mut new_task)) => {
            if new_task.etag.is_empty() {
                new_task.etag = "pending_refresh".to_string();
            }

            if let Some(map) = app.store.calendars.get_mut(&new_task.calendar_href) {
                map.insert(new_task.uid.clone(), new_task.clone());

                let list: Vec<_> = map.values().cloned().collect();
                let (_, token) = Cache::load(app.ctx.as_ref(), &new_task.calendar_href)
                    .unwrap_or((vec![], None));
                let _ = Cache::save(app.ctx.as_ref(), &new_task.calendar_href, &list, token);
            }
            refresh_filtered_tasks(app);

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.creating_child_of = None;

            app.selected_uid = Some(new_task.uid.clone());

            Task::none()
        }
        Message::TaskMoved(Err(e)) => {
            app.error_msg = Some(format!("Move failed: {}", e));
            Task::none()
        }
        Message::MigrationComplete(Ok(count)) => {
            app.loading = false;
            app.error_msg = Some(format!("Exported {} tasks successfully.", count));
            if let Some(client) = &app.client {
                app.loading = true;
                return Task::perform(
                    async_fetch_all_wrapper(client.clone(), app.calendars.clone()),
                    Message::RefreshedAll,
                );
            }
            Task::none()
        }
        Message::MigrationComplete(Err(e)) => {
            app.loading = false;
            app.error_msg = Some(format!("Export failed: {}", e));
            Task::none()
        }
        _ => Task::none(),
    }
}
