// Handles settings-related messages and updates in the GUI.
use crate::cache::Cache;
use crate::config::Config;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::update::common::{apply_alias_retroactively, refresh_filtered_tasks, save_config};
use crate::model::parser::{format_duration_compact, parse_duration, validate_alias_integrity}; // Updated import
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use iced::Task;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::ConfigLoaded(Ok(config)) => {
            // Load Local Calendars from registry
            let locals = LocalCalendarRegistry::load().unwrap_or_default();
            app.local_cals_editing = locals.clone();

            // ... [Same loading logic as before] ...
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
            app.ob_urgent_days_input = app.urgent_days.to_string();
            app.ob_urgent_prio_input = app.urgent_prio.to_string();

            // --- LOAD NEW FIELDS ---
            app.auto_reminders = config.auto_reminders;
            app.default_reminder_time = config.default_reminder_time.clone();
            app.snooze_short_mins = config.snooze_short_mins;
            app.create_events_for_tasks = config.create_events_for_tasks;
            app.delete_events_on_completion = config.delete_events_on_completion;
            app.snooze_long_mins = config.snooze_long_mins;

            // Initialize inputs with formatted strings
            app.ob_snooze_short_input = format_duration_compact(config.snooze_short_mins);
            app.ob_snooze_long_input = format_duration_compact(config.snooze_long_mins);

            let mut cached_cals = Cache::load_calendars().unwrap_or_default();

            // Merge locals into main calendar list
            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
            app.calendars = cached_cals;

            app.store.clear();

            // Load data for all calendars
            for cal in &app.calendars {
                if cal.href.starts_with("local://") {
                    if let Ok(tasks) = LocalStorage::load_for_href(&cal.href) {
                        app.store.insert(cal.href.clone(), tasks);
                    }
                } else if let Ok((tasks, _)) = Cache::load(&cal.href) {
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
            Task::perform(connect_and_fetch_wrapper(config), Message::Loaded)
        }
        Message::ConfigLoaded(Err(_)) => {
            app.state = AppState::Onboarding;
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
            // Sync `calendars` list with modified `local_cals_editing`
            app.calendars.retain(|c| !c.href.starts_with("local://"));
            app.calendars.extend(app.local_cals_editing.clone());

            if app.ob_sort_months_input.trim().is_empty() {
                app.sort_cutoff_months = None;
            } else if let Ok(n) = app.ob_sort_months_input.trim().parse::<u32>() {
                app.sort_cutoff_months = Some(n);
            }

            let mut config_to_save = Config::load().unwrap_or_else(|_| Config {
                url: String::new(),
                username: String::new(),
                password: String::new(),
                default_calendar: None,
                allow_insecure_certs: false,
                hidden_calendars: Vec::new(),
                disabled_calendars: Vec::new(),
                hide_completed: app.hide_completed,
                hide_fully_completed_tags: app.hide_fully_completed_tags,
                tag_aliases: app.tag_aliases.clone(),
                sort_cutoff_months: Some(2),
                theme: app.current_theme,
                urgent_days_horizon: app.urgent_days,
                urgent_priority_threshold: app.urgent_prio,
                // NEW FIELDS
                auto_reminders: app.auto_reminders,
                default_reminder_time: app.default_reminder_time.clone(),
                snooze_short_mins: app.snooze_short_mins,
                snooze_long_mins: app.snooze_long_mins,
                create_events_for_tasks: app.create_events_for_tasks,
                delete_events_on_completion: app.delete_events_on_completion,
            });

            config_to_save.url = app.ob_url.clone();
            config_to_save.username = app.ob_user.clone();
            config_to_save.password = app.ob_pass.clone();
            config_to_save.default_calendar = app.ob_default_cal.clone();
            config_to_save.allow_insecure_certs = app.ob_insecure;
            config_to_save.hidden_calendars = app.hidden_calendars.iter().cloned().collect();
            config_to_save.disabled_calendars = app.disabled_calendars.iter().cloned().collect();
            config_to_save.hide_completed = app.hide_completed;
            config_to_save.hide_fully_completed_tags = app.hide_fully_completed_tags;
            config_to_save.tag_aliases = app.tag_aliases.clone();
            config_to_save.sort_cutoff_months = app.sort_cutoff_months;
            config_to_save.theme = app.current_theme;
            config_to_save.auto_reminders = app.auto_reminders;
            config_to_save.default_reminder_time = app.default_reminder_time.clone();
            config_to_save.snooze_short_mins = app.snooze_short_mins;
            config_to_save.snooze_long_mins = app.snooze_long_mins;
            config_to_save.create_events_for_tasks = app.create_events_for_tasks;
            config_to_save.delete_events_on_completion = app.delete_events_on_completion;

            let _ = config_to_save.save();

            app.state = AppState::Loading;
            app.error_msg = Some("Connecting...".to_string());

            Task::perform(connect_and_fetch_wrapper(config_to_save), Message::Loaded)
        }
        Message::OpenSettings => {
            if let Ok(cfg) = Config::load() {
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
            }
            app.state = AppState::Settings;
            Task::none()
        }
        Message::CancelSettings => {
            // Sync `calendars` list with modified `local_cals_editing`
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

            let config_to_save = Config {
                url: String::new(),
                username: String::new(),
                password: String::new(),
                default_calendar: None,
                allow_insecure_certs: false,
                hidden_calendars: Vec::new(),
                disabled_calendars: Vec::new(),
                hide_completed: app.hide_completed,
                hide_fully_completed_tags: app.hide_fully_completed_tags,
                tag_aliases: app.tag_aliases.clone(),
                sort_cutoff_months: app.sort_cutoff_months,
                theme: app.current_theme,
                urgent_days_horizon: app.urgent_days,
                urgent_priority_threshold: app.urgent_prio,

                // NEW FIELDS
                auto_reminders: app.auto_reminders,
                default_reminder_time: app.default_reminder_time.clone(),
                snooze_short_mins: app.snooze_short_mins,
                snooze_long_mins: app.snooze_long_mins,
                create_events_for_tasks: app.create_events_for_tasks,
                delete_events_on_completion: app.delete_events_on_completion,
            };

            let _ = config_to_save.save();

            app.state = AppState::Loading;
            Task::perform(connect_and_fetch_wrapper(config_to_save), Message::Loaded)
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
                    // FIX: Removed .trim_start_matches('#') to preserve #tags, @@locs, etc.
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !tags.is_empty() {
                    // Normalize key input for consistency with parser
                    let raw_key = app.alias_input_key.trim();
                    let key = if raw_key.starts_with("@@") {
                        raw_key.to_string()
                    } else if raw_key.to_lowercase().starts_with("loc:") {
                        format!("@@{}", raw_key[4..].trim())
                    } else {
                        raw_key.trim_start_matches('#').to_string()
                    };

                    // --- VALIDATION ADDED HERE ---
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
            app.ob_snooze_short_input = val.clone(); // Allow user to type "1h"
            if let Some(n) = parse_duration(&val) {
                app.snooze_short_mins = n;
                save_config(app);
            } else if val.is_empty() {
                // Optional: handle empty
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
        Message::SetCreateEventsForTasks(val) => {
            let was_disabled = !app.create_events_for_tasks;
            app.create_events_for_tasks = val;
            save_config(app);

            // BACKFILL: Only trigger retroactive event creation when toggling ON
            if val
                && was_disabled
                && let Some(client) = &app.client
            {
                // Collect all tasks from all calendars
                let all_tasks: Vec<_> = app.store.calendars.values().flatten().cloned().collect();

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
        Message::DeleteAllCalendarEvents => {
            if let Some(client) = &app.client {
                app.deleting_events = true;

                // Collect all tasks from all calendars
                let all_tasks: Vec<_> = app.store.calendars.values().flatten().cloned().collect();

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
                // Determine action based on whether events were created or deleted
                // If setting is ON, we created. Otherwise we deleted.
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
            // 1. Load tasks from specified calendar
            let tasks_result = LocalStorage::load_for_href(&calendar_href);
            let cal_name = calendar_href.clone();

            Task::perform(
                async move {
                    let tasks = tasks_result.map_err(|e| e.to_string())?;
                    let ics_content = LocalStorage::to_ics_string(&tasks);

                    // Extract calendar name/id for filename
                    let cal_id = cal_name.strip_prefix("local://").unwrap_or("backup");
                    let filename = format!("cfait_{}.ics", cal_id);

                    // 2. Open File Dialog (Async)
                    let file_handle = rfd::AsyncFileDialog::new()
                        .add_filter("Calendar", &["ics"])
                        .set_file_name(&filename)
                        .save_file()
                        .await;

                    if let Some(handle) = file_handle {
                        let path = handle.path().to_path_buf();
                        // 3. Write file
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
                        // User cancelled
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
            // Don't show error if user just cancelled
            if e != "Export cancelled" {
                app.error_msg = Some(format!("Export failed: {}", e));
            }
            Task::none()
        }
        Message::ImportLocalIcs(calendar_href) => {
            // Open file picker to select ICS file
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
                                // Use canonical import function
                                match LocalStorage::import_from_ics(&calendar_href, &ics_content) {
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
            // Refresh tasks after import
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
            // Don't show error if user just cancelled
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

            // Auto-save registry
            let _ = LocalCalendarRegistry::save(&app.local_cals_editing);

            // Also add to main calendar list
            if !app.calendars.iter().any(|c| c.href == new_cal.href) {
                app.calendars.push(new_cal.clone());
            }

            // Initialize empty task list for this calendar
            app.store.insert(new_cal.href.clone(), vec![]);

            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::DeleteLocalCalendar(href) => {
            // Don't delete default
            if href == LOCAL_CALENDAR_HREF {
                return Task::none();
            }

            if let Some(idx) = app.local_cals_editing.iter().position(|c| c.href == href) {
                app.local_cals_editing.remove(idx);
                let _ = LocalCalendarRegistry::save(&app.local_cals_editing);

                // Also remove from main calendar list
                app.calendars.retain(|c| c.href != href);
                app.store.remove(&href);

                // Delete data file
                if let Some(path) = LocalStorage::get_path_for_href(&href) {
                    let _ = std::fs::remove_file(path);
                }

                refresh_filtered_tasks(app);
            }
            Task::none()
        }
        Message::LocalCalendarNameChanged(href, name) => {
            if let Some(cal) = app.local_cals_editing.iter_mut().find(|c| c.href == href) {
                cal.name = name.clone();
                let _ = LocalCalendarRegistry::save(&app.local_cals_editing);

                // Also update in main calendar list
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
                // Convert color to hex
                let r = (color.r * 255.0) as u8;
                let g = (color.g * 255.0) as u8;
                let b = (color.b * 255.0) as u8;
                let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
                cal.color = Some(hex.clone());
                let _ = LocalCalendarRegistry::save(&app.local_cals_editing);

                // Also update in main calendar list
                if let Some(main_cal) = app.calendars.iter_mut().find(|c| c.href == *href) {
                    main_cal.color = Some(hex);
                }
            }
            app.color_picker_active_href = None;
            Task::none()
        }

        _ => Task::none(),
    }
}
