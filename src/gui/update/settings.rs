// File: src/gui/update/settings.rs
use crate::cache::Cache;

use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::update::common::{apply_alias_retroactively, refresh_filtered_tasks, save_config};
use crate::model::parser::{format_duration_compact, parse_duration, validate_alias_integrity};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use iced::Task;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::ConfigLoaded(Ok(config)) => {
            let locals = LocalCalendarRegistry::load(app.ctx.as_ref()).unwrap_or_default();
            app.local_cals_editing = locals.clone();

            app.hidden_calendars = config.hidden_calendars.clone().into_iter().collect();
            app.disabled_calendars = config.disabled_calendars.clone().into_iter().collect();
            app.sort_cutoff_months = config.sort_cutoff_months;
            app.ob_sort_months_input = match config.sort_cutoff_months {
                Some(m) => m.to_string(),
                None => "".to_string(),
            };
            app.ob_insecure = config.allow_insecure_certs;
            app.tag_aliases = config.tag_aliases.clone();
            app.hide_completed = config.hide_completed;
            app.hide_fully_completed_tags = config.hide_fully_completed_tags;
            app.current_theme = config.theme;

            app.ob_url = config.url.clone();
            app.ob_user = config.username.clone();
            app.ob_pass = config.password.clone();
            app.ob_default_cal = config.default_calendar.clone();
            app.urgent_days = config.urgent_days_horizon;
            app.urgent_prio = config.urgent_priority_threshold;
            app.default_priority = config.default_priority;
            app.start_grace_period_days = config.start_grace_period_days;
            app.ob_urgent_days_input = app.urgent_days.to_string();
            app.ob_urgent_prio_input = app.urgent_prio.to_string();
            app.ob_default_priority_input = app.default_priority.to_string();
            app.ob_start_grace_input = app.start_grace_period_days.to_string();

            app.auto_reminders = config.auto_reminders;
            app.default_reminder_time = config.default_reminder_time.clone();
            app.snooze_short_mins = config.snooze_short_mins;
            app.create_events_for_tasks = config.create_events_for_tasks;
            app.delete_events_on_completion = config.delete_events_on_completion;
            app.snooze_long_mins = config.snooze_long_mins;
            app.auto_refresh_interval_mins = config.auto_refresh_interval_mins;
            app.strikethrough_completed = config.strikethrough_completed;
            app.trash_retention_days = config.trash_retention_days;

            app.ob_snooze_short_input = format_duration_compact(config.snooze_short_mins);
            app.ob_snooze_long_input = format_duration_compact(config.snooze_long_mins);
            app.ob_auto_refresh_input = format_duration_compact(config.auto_refresh_interval_mins);
            app.ob_trash_retention_input = config.trash_retention_days.to_string();

            app.ob_max_done_roots_input = config.max_done_roots.to_string();
            app.ob_max_done_subtasks_input = config.max_done_subtasks.to_string();

            let mut cached_cals = Cache::load_calendars(app.ctx.as_ref()).unwrap_or_default();

            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
            app.calendars = cached_cals;

            app.store.clear();

            for cal in &app.calendars {
                if cal.href.starts_with("local://") {
                    if let Ok(tasks) = LocalStorage::load_for_href(app.ctx.as_ref(), &cal.href) {
                        app.store.insert(cal.href.clone(), tasks);
                    }
                } else if let Ok((tasks, _)) = Cache::load(app.ctx.as_ref(), &cal.href) {
                    app.store.insert(cal.href.clone(), tasks);
                }
            }

            let mut target_href = None;
            if let Some(def) = &app.ob_default_cal
                && let Some(cal) = app
                    .calendars
                    .iter()
                    .find(|c| c.name == *def || c.href == *def)
            {
                if app.hidden_calendars.contains(&cal.href) {
                    app.hidden_calendars.remove(&cal.href);
                }
                target_href = Some(cal.href.clone());
            }

            if target_href.is_none() {
                target_href = Some(LOCAL_CALENDAR_HREF.to_string());
            }
            app.active_cal_href = target_href;

            refresh_filtered_tasks(app);
            app.state = AppState::Active;
            app.loading = true;
            Task::perform(
                connect_and_fetch_wrapper(app.ctx.clone(), config),
                Message::Loaded,
            )
        }
        Message::ConfigLoaded(Err(e)) => {
            app.state = AppState::Onboarding;
            if !e.contains("Config file not found") {
                app.error_msg = Some(format!("Configuration Error:\n{}", e));
                app.config_was_corrupted = true;
            }
            Task::none()
        }
        Message::ObUrlChanged(v) => {
            app.ob_url = v;
            Task::none()
        }
        Message::ObUserChanged(v) => {
            app.ob_user = v;
            Task::none()
        }
        Message::ObPassChanged(v) => {
            app.ob_pass = v;
            Task::none()
        }
        Message::ObDefaultCalChanged(v) => {
            app.ob_default_cal = Some(v);
            save_config(app);
            Task::none()
        }
        Message::ObInsecureToggled(val) => {
            app.ob_insecure = val;
            Task::none()
        }
        Message::ThemeChanged(theme) => {
            app.current_theme = theme;
            save_config(app);
            Task::none()
        }
        Message::ObSubmit => {
            app.calendars.retain(|c| !c.href.starts_with("local://"));
            app.calendars.extend(app.local_cals_editing.clone());

            if app.ob_sort_months_input.trim().is_empty() {
                app.sort_cutoff_months = None;
            } else if let Ok(n) = app.ob_sort_months_input.trim().parse::<u32>() {
                app.sort_cutoff_months = Some(n);
            }

            let config_to_save = save_config(app);

            let _ = config_to_save.save(app.ctx.as_ref());

            app.state = AppState::Loading;
            app.error_msg = Some("Connecting...".to_string());

            Task::perform(
                connect_and_fetch_wrapper(app.ctx.clone(), config_to_save),
                Message::Loaded,
            )
        }
        Message::OpenSettings => {
            if let Ok(cfg) = crate::config::Config::load(app.ctx.as_ref()) {
                app.ob_url = cfg.url;
                app.ob_user = cfg.username;
                app.ob_pass = cfg.password;
                app.ob_default_cal = cfg.default_calendar;
                app.hide_completed = cfg.hide_completed;
                app.hide_fully_completed_tags = cfg.hide_fully_completed_tags;
                app.ob_insecure = cfg.allow_insecure_certs;
                app.hidden_calendars = cfg.hidden_calendars.into_iter().collect();
                app.tag_aliases = cfg.tag_aliases;
                app.sort_cutoff_months = cfg.sort_cutoff_months;
                app.current_theme = cfg.theme;
                app.ob_sort_months_input = match cfg.sort_cutoff_months {
                    Some(m) => m.to_string(),
                    None => "".to_string(),
                };
                app.trash_retention_days = cfg.trash_retention_days;
                app.ob_trash_retention_input = cfg.trash_retention_days.to_string();
            }
            app.state = AppState::Settings;
            Task::none()
        }
        Message::CancelSettings => {
            app.calendars.retain(|c| !c.href.starts_with("local://"));
            app.calendars.extend(app.local_cals_editing.clone());

            save_config(app);
            refresh_filtered_tasks(app);
            app.state = AppState::Active;
            Task::none()
        }
        Message::ObSubmitOffline => {
            app.ob_url.clear();
            app.ob_user.clear();
            app.ob_pass.clear();

            let config_to_save = save_config(app);

            let _ = config_to_save.save(app.ctx.as_ref());

            app.state = AppState::Loading;
            Task::perform(
                connect_and_fetch_wrapper(app.ctx.clone(), config_to_save),
                Message::Loaded,
            )
        }
        Message::AliasKeyInput(v) => {
            app.alias_input_key = v;
            Task::none()
        }
        Message::AliasValueInput(v) => {
            app.alias_input_values = v;
            Task::none()
        }
        Message::AddAlias => {
            if !app.alias_input_key.is_empty() && !app.alias_input_values.is_empty() {
                let tags: Vec<String> = app
                    .alias_input_values
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !tags.is_empty() {
                    let raw_key = app.alias_input_key.trim();
                    let key = if raw_key.starts_with("@@") {
                        raw_key.to_string()
                    } else if raw_key.to_lowercase().starts_with("loc:") {
                        format!("@@{}", raw_key[4..].trim())
                    } else {
                        raw_key.trim_start_matches('#').to_string()
                    };

                    match validate_alias_integrity(&key, &tags, &app.tag_aliases) {
                        Ok(_) => {
                            app.tag_aliases.insert(key.clone(), tags.clone());
                            app.alias_input_key.clear();
                            app.alias_input_values.clear();
                            app.error_msg = None;
                            save_config(app);

                            if let Some(task) = apply_alias_retroactively(app, &key, &tags) {
                                return task;
                            }
                        }
                        Err(e) => {
                            app.error_msg = Some(format!("Cannot add alias: {}", e));
                        }
                    }
                }
            }
            Task::none()
        }
        Message::RemoveAlias(key) => {
            app.tag_aliases.remove(&key);
            save_config(app);
            Task::none()
        }
        Message::ObSortMonthsChanged(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_sort_months_input = val.clone();

                if val.trim().is_empty() {
                    app.sort_cutoff_months = None;
                } else if let Ok(n) = val.trim().parse::<u32>() {
                    app.sort_cutoff_months = Some(n);
                }
                save_config(app);
                refresh_filtered_tasks(app);
            }
            Task::none()
        }
        Message::ObUrgentDaysChanged(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_urgent_days_input = val.clone();
                if let Ok(n) = val.trim().parse::<u32>() {
                    app.urgent_days = n;
                    save_config(app);
                    refresh_filtered_tasks(app);
                }
            }
            Task::none()
        }
        Message::ObUrgentPrioChanged(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_urgent_prio_input = val.clone();
                if let Ok(n) = val.trim().parse::<u8>() {
                    app.urgent_prio = n;
                    save_config(app);
                    refresh_filtered_tasks(app);
                }
            }
            Task::none()
        }
        Message::ObDefaultPriorityChanged(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_default_priority_input = val.clone();
                if let Ok(n) = val.trim().parse::<u8>()
                    && n > 0
                {
                    app.default_priority = n;
                    save_config(app);
                    refresh_filtered_tasks(app);
                }
            }
            Task::none()
        }
        Message::ObStartGraceChanged(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_start_grace_input = val.clone();
                if let Ok(n) = val.trim().parse::<u32>() {
                    app.start_grace_period_days = n;
                    save_config(app);
                    refresh_filtered_tasks(app);
                }
            }
            Task::none()
        }
        Message::SetAutoReminders(val) => {
            app.auto_reminders = val;
            save_config(app);
            Task::none()
        }
        Message::SetDefaultReminderTime(val) => {
            app.default_reminder_time = val;
            save_config(app);
            Task::none()
        }
        Message::SetSnoozeShort(val) => {
            app.ob_snooze_short_input = val.clone();
            if let Some(n) = parse_duration(&val) {
                app.snooze_short_mins = n;
                save_config(app);
            }
            Task::none()
        }
        Message::SetSnoozeLong(val) => {
            app.ob_snooze_long_input = val.clone();
            if let Some(n) = parse_duration(&val) {
                app.snooze_long_mins = n;
                save_config(app);
            }
            Task::none()
        }
        Message::SetTrashRetention(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_trash_retention_input = val.clone();
                if let Ok(n) = val.trim().parse::<u32>() {
                    app.trash_retention_days = n;
                    save_config(app);
                }
            }
            Task::none()
        }
        Message::SetAutoRefreshInterval(val) => {
            app.ob_auto_refresh_input = val.clone();
            if let Some(n) = parse_duration(&val) {
                app.auto_refresh_interval_mins = n;
                save_config(app);
            }
            Task::none()
        }
        Message::ToggleAdvancedSettings(val) => {
            app.show_advanced_settings = val;
            Task::none()
        }
        Message::SetMaxDoneRoots(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_max_done_roots_input = val;
                save_config(app);
                refresh_filtered_tasks(app);
            }
            Task::none()
        }
        Message::SetMaxDoneSubtasks(val) => {
            if val.is_empty() || val.chars().all(|c| c.is_numeric()) {
                app.ob_max_done_subtasks_input = val;
                save_config(app);
                refresh_filtered_tasks(app);
            }
            Task::none()
        }

        Message::SetCreateEventsForTasks(val) => {
            let was_disabled = !app.create_events_for_tasks;
            app.create_events_for_tasks = val;
            save_config(app);

            if val
                && was_disabled
                && let Some(client) = &app.client
            {
                let all_tasks: Vec<_> = app
                    .store
                    .calendars
                    .values()
                    .flat_map(|m| m.values())
                    .cloned()
                    .collect();

                return Task::perform(
                    async_backfill_events_wrapper(client.clone(), all_tasks, val),
                    Message::BackfillEventsComplete,
                );
            }
            Task::none()
        }
        Message::SetDeleteEventsOnCompletion(val) => {
            app.delete_events_on_completion = val;
            save_config(app);
            Task::none()
        }

        Message::SetStrikethroughCompleted(val) => {
            app.strikethrough_completed = val;
            save_config(app);
            Task::none()
        }
        Message::DeleteAllCalendarEvents => {
            if let Some(client) = &app.client {
                app.deleting_events = true;

                let all_tasks: Vec<_> = app
                    .store
                    .calendars
                    .values()
                    .flat_map(|m| m.values())
                    .cloned()
                    .collect();

                return Task::perform(
                    async_backfill_events_wrapper(client.clone(), all_tasks, false),
                    Message::BackfillEventsComplete,
                );
            }
            Task::none()
        }
        Message::BackfillEventsComplete(Ok(count)) => {
            app.deleting_events = false;

            if count > 0 {
                let action = if app.create_events_for_tasks {
                    "Created"
                } else {
                    "Deleted"
                };
                let plural = if count == 1 { "event" } else { "events" };
                app.error_msg = Some(format!("✓ {} {} calendar {}", action, count, plural));
            } else {
                app.error_msg = Some("No events were created or deleted".to_string());
            }
            Task::none()
        }
        Message::BackfillEventsComplete(Err(e)) => {
            app.deleting_events = false;
            app.error_msg = Some(format!("Backfill error: {}", e));
            Task::none()
        }
        Message::ExportLocalIcs(calendar_href) => {
            let tasks_result = LocalStorage::load_for_href(app.ctx.as_ref(), &calendar_href);
            let cal_name = calendar_href.clone();

            Task::perform(
                async move {
                    let tasks = tasks_result.map_err(|e| e.to_string())?;
                    let ics_content = LocalStorage::to_ics_string(&tasks);

                    let cal_id = cal_name.strip_prefix("local://").unwrap_or("backup");
                    let filename = format!("cfait_{}.ics", cal_id);

                    let file_handle = rfd::AsyncFileDialog::new()
                        .add_filter("Calendar", &["ics"])
                        .set_file_name(&filename)
                        .save_file()
                        .await;

                    if let Some(handle) = file_handle {
                        let path = handle.path().to_path_buf();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            use tokio::io::AsyncWriteExt;
                            let mut file = tokio::fs::File::create(&path)
                                .await
                                .map_err(|e| e.to_string())?;
                            file.write_all(ics_content.as_bytes())
                                .await
                                .map_err(|e| e.to_string())?;
                        }
                        Ok(path)
                    } else {
                        Err("Export cancelled".to_string())
                    }
                },
                Message::ExportSaved,
            )
        }
        Message::ExportSaved(Ok(path)) => {
            app.error_msg = Some(format!(
                "Exported to: {:?}",
                path.file_name().unwrap_or_default()
            ));
            Task::none()
        }
        Message::ExportSaved(Err(e)) => {
            if e != "Export cancelled" {
                app.error_msg = Some(format!("Export failed: {}", e));
            }
            Task::none()
        }
        Message::ImportLocalIcs(calendar_href) => {
            let ctx = app.ctx.clone();
            Task::perform(
                async move {
                    if let Some(file) = rfd::AsyncFileDialog::new()
                        .add_filter("iCalendar", &["ics"])
                        .pick_file()
                        .await
                    {
                        let content = file.read().await;
                        match String::from_utf8(content) {
                            Ok(ics_content) => {
                                match LocalStorage::import_from_ics(
                                    ctx.as_ref(),
                                    &calendar_href,
                                    &ics_content,
                                ) {
                                    Ok(count) => {
                                        Ok(format!("Successfully imported {} task(s)", count))
                                    }
                                    Err(e) => Err(format!("Import failed: {}", e)),
                                }
                            }
                            Err(e) => Err(format!("Failed to read file as text: {}", e)),
                        }
                    } else {
                        Err("Import cancelled".to_string())
                    }
                },
                Message::ImportCompleted,
            )
        }
        Message::ImportCompleted(Ok(msg)) => {
            app.error_msg = Some(msg);
            if let Some(client) = &app.client {
                app.loading = true;
                return Task::perform(
                    async_fetch_all_wrapper(client.clone(), app.calendars.clone()),
                    Message::RefreshedAll,
                );
            }
            Task::none()
        }
        Message::ImportCompleted(Err(e)) => {
            if e != "Import cancelled" {
                app.error_msg = Some(format!("Import failed: {}", e));
            }
            Task::none()
        }
        Message::AddLocalCalendar => {
            use uuid::Uuid;
            let id = Uuid::new_v4().to_string();
            let new_cal = crate::model::CalendarListEntry {
                name: "New Calendar".to_string(),
                href: format!("local://{}", id),
                color: None,
            };
            app.local_cals_editing.push(new_cal.clone());

            let _ = LocalCalendarRegistry::save(app.ctx.as_ref(), &app.local_cals_editing);

            if !app.calendars.iter().any(|c| c.href == new_cal.href) {
                app.calendars.push(new_cal.clone());
            }

            app.store.insert(new_cal.href.clone(), vec![]);

            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::DeleteLocalCalendar(href) => {
            if href == LOCAL_CALENDAR_HREF {
                return Task::none();
            }

            if let Some(idx) = app.local_cals_editing.iter().position(|c| c.href == href) {
                app.local_cals_editing.remove(idx);
                let _ = LocalCalendarRegistry::save(app.ctx.as_ref(), &app.local_cals_editing);

                app.calendars.retain(|c| c.href != href);
                app.store.remove(&href);

                if let Some(path) = LocalStorage::get_path_for_href(app.ctx.as_ref(), &href) {
                    let _ = std::fs::remove_file(path);
                }

                refresh_filtered_tasks(app);
            }
            Task::none()
        }
        Message::LocalCalendarNameChanged(href, name) => {
            if let Some(cal) = app.local_cals_editing.iter_mut().find(|c| c.href == href) {
                cal.name = name.clone();
                let _ = LocalCalendarRegistry::save(app.ctx.as_ref(), &app.local_cals_editing);

                if let Some(main_cal) = app.calendars.iter_mut().find(|c| c.href == href) {
                    main_cal.name = name;
                }
            }
            Task::none()
        }
        Message::OpenColorPicker(href, current) => {
            app.color_picker_active_href = Some(href);
            app.temp_color = current;
            Task::none()
        }
        Message::CancelColorPicker => {
            app.color_picker_active_href = None;
            Task::none()
        }
        Message::SubmitColorPicker(color) => {
            if let Some(href) = &app.color_picker_active_href.clone()
                && let Some(cal) = app.local_cals_editing.iter_mut().find(|c| c.href == *href)
            {
                let r = (color.r * 255.0) as u8;
                let g = (color.g * 255.0) as u8;
                let b = (color.b * 255.0) as u8;
                let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
                cal.color = Some(hex.clone());
                let _ = LocalCalendarRegistry::save(app.ctx.as_ref(), &app.local_cals_editing);

                if let Some(main_cal) = app.calendars.iter_mut().find(|c| c.href == *href) {
                    main_cal.color = Some(hex);
                }
            }
            app.color_picker_active_href = None;
            Task::none()
        }

        Message::IcsFileLoaded(Ok((file_path, content))) => {
            let task_count = content.split("BEGIN:VTODO").count().saturating_sub(1);

            app.ics_import_dialog_open = true;
            app.ics_import_file_path = Some(file_path);
            app.ics_import_content = Some(content);
            app.ics_import_task_count = Some(task_count);
            app.ics_import_selected_calendar = None;

            Task::none()
        }
        Message::IcsFileLoaded(Err(e)) => {
            app.error_msg = Some(format!("Failed to load ICS file: {}", e));
            Task::none()
        }
        Message::IcsImportDialogCalendarSelected(href) => {
            app.ics_import_selected_calendar = Some(href);
            Task::none()
        }
        Message::IcsImportDialogCancel => {
            app.ics_import_dialog_open = false;
            app.ics_import_file_path = None;
            app.ics_import_content = None;
            app.ics_import_selected_calendar = None;
            app.ics_import_task_count = None;
            Task::none()
        }
        Message::IcsImportDialogConfirm => {
            if let Some(calendar_href) = &app.ics_import_selected_calendar.clone()
                && let Some(ics_content) = &app.ics_import_content.clone()
            {
                app.ics_import_dialog_open = false;
                let file_path = app.ics_import_file_path.take();
                app.ics_import_content = None;
                app.ics_import_selected_calendar = None;
                app.ics_import_task_count = None;

                let href = calendar_href.clone();
                let content = ics_content.clone();
                let ctx = app.ctx.clone();
                return Task::perform(
                    async move {
                        match LocalStorage::import_from_ics(ctx.as_ref(), &href, &content) {
                            Ok(count) => {
                                let file_name = file_path
                                    .as_ref()
                                    .and_then(|p| std::path::Path::new(p).file_name())
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("file");
                                Ok(format!(
                                    "Successfully imported {} task(s) from {}",
                                    count, file_name
                                ))
                            }
                            Err(e) => Err(format!("Import failed: {}", e)),
                        }
                    },
                    Message::ImportCompleted,
                );
            }
            Task::none()
        }

        _ => Task::none(),
    }
}
