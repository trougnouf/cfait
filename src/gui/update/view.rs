// SPDX-License-Identifier: GPL-3.0-or-later
// Handles view/navigation-related messages in the GUI.
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, Focus, GuiApp, ResizeDirection, SidebarMode};
use crate::gui::subscription::ACTIVE_FOCUS;
use crate::gui::update::common::{
    refresh_filtered_tasks, save_config, scroll_to_selected, scroll_to_selected_delayed,
};
use crate::gui::update::tasks;
use crate::store::select_weighted_random_index;
use fastrand;
use iced::widget::operation;
use iced::{Task, window};

fn flush_journal_save(app: &mut GuiApp) {
    app.journal_debounce_version = app.journal_debounce_version.wrapping_add(1);

    let new_text = app.journal_editor_content.text();

    let uid_to_sync = if let Some(uid) = &app.journal_editing_uid {
        Some(uid.clone())
    } else {
        let date = app.journal_date;
        let href = app
            .journal_editing_href
            .clone()
            .or(app.active_cal_href.clone())
            .unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string());

        let existing_opt = app
            .store
            .get_journal_entry(&href, date)
            .map(|t| t.uid.clone());

        if let Some(uid) = existing_opt {
            Some(uid)
        } else if !new_text.trim().is_empty() {
            let mut new_journal = crate::model::Task::new("", &app.tag_aliases, None);
            new_journal.is_journal = true;
            new_journal.calendar_href = href.clone();
            new_journal.dtstart = Some(crate::model::DateType::AllDay(date));
            new_journal.summary = date.format("%Y-%m-%d").to_string();
            app.store.add_task(new_journal.clone());
            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                    crate::journal::Action::Create(new_journal.clone()),
                ]));
            }
            Some(new_journal.uid)
        } else {
            None
        }
    };

    if let Some(uid) = uid_to_sync {
        let config = crate::config::Config::load(app.ctx.as_ref()).unwrap_or_default();
        let def_time =
            chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
        let sync_options = crate::store::SyncTreeOptions {
            aliases: &app.tag_aliases,
            default_reminder_time: def_time,
            trash_retention_days: config.trash_retention_days,
            calendars: &app.calendars,
        };

        if let Ok((actions, _warnings)) = app.store.sync_tree_from_markdown(
            &uid,
            &new_text,
            &sync_options,
            true, // is_journal is always true here
        ) && !actions.is_empty()
            && let Some(tx) = &app.bg_tx
        {
            let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
        }
    }
}

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::TaskClick(index, uid) => {
            app.active_focus = Focus::MainList;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::MainList;
            }
            let now = std::time::Instant::now();
            let mut is_double = false;

            if let Some((last_time, last_uid)) = &app.last_click
                && last_uid == &uid
                && now.duration_since(*last_time).as_millis() < 400
            {
                is_double = true;
            }

            app.last_click = Some((now, uid.clone()));

            if is_double {
                app.last_click = None;
                tasks::handle(app, Message::EditTaskStart(index))
            } else {
                handle(app, Message::ToggleDetails(uid))
            }
        }
        // --- UI Zoom (global scale factor) ---
        Message::ZoomIn => {
            // Increase scale by 10%, clamp at 300%
            app.ui_scale = (app.ui_scale + 0.1).min(3.0);
            save_config(app);
            Task::none()
        }
        Message::ZoomOut => {
            // Decrease scale by 10%, clamp at 50%
            app.ui_scale = (app.ui_scale - 0.1).max(0.5);
            save_config(app);
            Task::none()
        }
        Message::ZoomReset => {
            app.ui_scale = 1.0;
            save_config(app);
            Task::none()
        }

        Message::SelectNextPage => {
            if app.sidebar_mode == SidebarMode::Journal || app.tasks.is_empty() {
                return Task::none();
            }
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.find_task_index_by_uid(uid))
                .unwrap_or(0);

            let next_idx = (current_idx + 10).min(app.tasks.len() - 1);

            if let Some(task) = app.get_task_at_index(next_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app, true);
            }
            Task::none()
        }
        Message::FocusSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                crate::gui::update::common::dispatch_intent(
                    app,
                    crate::model::AppIntent::FocusTaskTree { uid: Some(uid) },
                );
            }
            Task::none()
        }
        Message::ClearFocus => {
            crate::gui::update::common::dispatch_intent(
                app,
                crate::model::AppIntent::FocusTaskTree { uid: None },
            );
            Task::none()
        }
        Message::ArrowRight => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                match app.sidebar_mode {
                    SidebarMode::Calendars => {
                        let cals = app.get_filtered_calendars();
                        if let Some(cal) = cals.get(app.sidebar_selection_idx) {
                            return handle(app, Message::IsolateCalendar(cal.href.clone()));
                        }
                    }
                    SidebarMode::Categories => {
                        let cats = &app.cached_categories;
                        if let Some(cat) = cats.get(app.sidebar_selection_idx) {
                            return handle(app, Message::FocusTag(cat.full_key.clone()));
                        }
                    }
                    SidebarMode::Locations => {
                        let locs = &app.cached_locations;
                        if let Some(loc) = locs.get(app.sidebar_selection_idx) {
                            return handle(app, Message::FocusLocation(loc.full_key.clone()));
                        }
                    }
                    SidebarMode::Journal => {}
                    SidebarMode::Goals => {
                        let mut keys: Vec<_> = app.core_config.goals.keys().cloned().collect();
                        keys.sort();
                        if let Some(key) = keys.get(app.sidebar_selection_idx) {
                            if key.starts_with('#') {
                                return handle(
                                    app,
                                    Message::FocusTag(key.trim_start_matches('#').to_string()),
                                );
                            } else if key.starts_with("@@") {
                                return handle(
                                    app,
                                    Message::FocusLocation(
                                        key.trim_start_matches("@@").to_string(),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            Task::none()
        }
        Message::ArrowLeft => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                return handle(app, Message::CycleFocus(true));
            }
            Task::none()
        }
        Message::OpenWikiLink(title, context_uid) => {
            let clean_title = title.trim_start_matches("[[").trim_end_matches("]]").trim();

            let is_relative = clean_title.starts_with('+');
            let path_str = if is_relative {
                clean_title[1..].trim()
            } else {
                clean_title
            };

            let path_segments = crate::model::parser::split_path_respecting_quotes(path_str);
            if path_segments.is_empty() {
                return Task::none();
            }

            match app
                .store
                .resolve_dependency_ref(clean_title, context_uid.as_deref())
            {
                Ok(uid) => handle(app, Message::JumpToTask(uid)),
                Err(msg) if msg.starts_with("Ambiguous") => {
                    app.error_msg = Some(msg);
                    Task::none()
                }
                Err(_) => {
                    // Not found. Proceed with standard wiki behavior: create it.
                    if app.sidebar_mode == SidebarMode::Journal {
                        flush_journal_save(app);
                    }

                    let config = crate::config::Config::load(app.ctx.as_ref()).unwrap_or_default();
                    let def_time =
                        chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
                            .ok();

                    let context_is_journal = if let Some(uid) = &context_uid {
                        app.store
                            .get_task_ref(uid)
                            .map(|t| t.is_journal)
                            .unwrap_or(false)
                    } else {
                        app.sidebar_mode == SidebarMode::Journal
                    };

                    let mut current_parent_uid = if is_relative {
                        context_uid.clone()
                    } else {
                        None
                    };

                    let mut final_uid = String::new();
                    let mut actions = Vec::new();

                    for (i, segment) in path_segments.iter().enumerate() {
                        let seg_clean = crate::model::parser::strip_quotes(segment);

                        let mut found_uid = None;
                        for (href, map) in &app.store.calendars {
                            if href == crate::storage::LOCAL_TRASH_HREF
                                || href == "local://recovery"
                            {
                                continue;
                            }
                            for (uid, t) in map {
                                if t.parent_uid == current_parent_uid
                                    && t.summary.eq_ignore_ascii_case(&seg_clean)
                                {
                                    found_uid = Some(uid.clone());
                                    break;
                                }
                            }
                            if found_uid.is_some() {
                                break;
                            }
                        }

                        if let Some(uid) = found_uid {
                            current_parent_uid = Some(uid.clone());
                            if i == path_segments.len() - 1 {
                                final_uid = uid;
                            }
                        } else {
                            let mut new_task =
                                crate::model::Task::new(&seg_clean, &app.tag_aliases, def_time);

                            if context_is_journal {
                                new_task.is_journal = true;
                                new_task.is_note = true;
                            }

                            new_task.parent_uid = current_parent_uid.clone();

                            let target_href = if let Some(p_uid) = &current_parent_uid {
                                app.store
                                    .get_task_ref(p_uid)
                                    .map(|t| t.calendar_href.clone())
                            } else {
                                None
                            }
                            .or_else(|| app.active_cal_href.clone())
                            .unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string());
                            new_task.calendar_href = target_href.clone();

                            let uid = new_task.uid.clone();
                            app.store.add_task(new_task.clone());
                            actions.push(crate::journal::Action::Create(new_task));

                            current_parent_uid = Some(uid.clone());
                            if i == path_segments.len() - 1 {
                                final_uid = uid;
                            }
                        }
                    }

                    if let Some(tx) = &app.bg_tx {
                        let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
                    }

                    if app.sidebar_mode == SidebarMode::Journal {
                        app.journal_editing_uid = Some(final_uid);
                        app.journal_title_input =
                            crate::model::parser::strip_quotes(path_segments.last().unwrap());
                        app.journal_editor_content =
                            iced::widget::text_editor::Content::with_text("");
                        app.editor_maximized = true;

                        refresh_filtered_tasks(app);
                        Task::none()
                    } else {
                        refresh_filtered_tasks(app);
                        handle(app, Message::JumpToTask(final_uid))
                    }
                }
            }
        }
        Message::SidebarInteractSpace => {
            match app.sidebar_mode {
                SidebarMode::Calendars => {
                    let cals = app.get_filtered_calendars();
                    if let Some(cal) = cals.get(app.sidebar_selection_idx) {
                        let is_visible = !app.hidden_calendars.contains(&cal.href);
                        return handle(
                            app,
                            Message::ToggleCalendarVisibility(cal.href.clone(), !is_visible),
                        );
                    }
                }
                SidebarMode::Categories => {
                    let cats = &app.cached_categories;
                    if let Some(cat) = cats.get(app.sidebar_selection_idx) {
                        return handle(app, Message::CategoryToggled(cat.full_key.clone()));
                    }
                }
                SidebarMode::Locations => {
                    let locs = &app.cached_locations;
                    if let Some(loc) = locs.get(app.sidebar_selection_idx) {
                        return handle(app, Message::LocationToggled(loc.full_key.clone()));
                    }
                }
                SidebarMode::Journal => {}
                SidebarMode::Goals => {
                    let mut keys: Vec<_> = app.core_config.goals.keys().cloned().collect();
                    keys.sort();
                    if let Some(key) = keys.get(app.sidebar_selection_idx) {
                        if key.starts_with('#') {
                            return handle(
                                app,
                                Message::JumpToTag(key.trim_start_matches('#').to_string()),
                            );
                        } else if key.starts_with("@@") {
                            return handle(
                                app,
                                Message::JumpToLocation(key.trim_start_matches("@@").to_string()),
                            );
                        }
                    }
                }
            }
            Task::none()
        }
        Message::SidebarInteractEnter => {
            match app.sidebar_mode {
                SidebarMode::Calendars => {
                    let cals = app.get_filtered_calendars();
                    if let Some(cal) = cals.get(app.sidebar_selection_idx) {
                        return handle(app, Message::SelectCalendar(cal.href.clone()));
                    }
                }
                SidebarMode::Categories => {
                    let cats = &app.cached_categories;
                    if let Some(cat) = cats.get(app.sidebar_selection_idx) {
                        return handle(app, Message::CategoryToggled(cat.full_key.clone()));
                    }
                }
                SidebarMode::Locations => {
                    let locs = &app.cached_locations;
                    if let Some(loc) = locs.get(app.sidebar_selection_idx) {
                        return handle(app, Message::LocationToggled(loc.full_key.clone()));
                    }
                }
                SidebarMode::Journal => {}
                SidebarMode::Goals => {
                    let mut keys: Vec<_> = app.core_config.goals.keys().cloned().collect();
                    keys.sort();
                    if let Some(key) = keys.get(app.sidebar_selection_idx) {
                        if key.starts_with('#') {
                            return handle(
                                app,
                                Message::JumpToTag(key.trim_start_matches('#').to_string()),
                            );
                        } else if key.starts_with("@@") {
                            return handle(
                                app,
                                Message::JumpToLocation(key.trim_start_matches("@@").to_string()),
                            );
                        }
                    }
                }
            }
            Task::none()
        }
        Message::SelectPrevPage => {
            if app.tasks.is_empty() {
                return Task::none();
            }
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.find_task_index_by_uid(uid))
                .unwrap_or(0);

            let prev_idx = current_idx.saturating_sub(10);

            if let Some(task) = app.get_task_at_index(prev_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app, true);
            }
            Task::none()
        }

        // Stateless toggles: read current state, flip it, call existing logic
        Message::ToggleChildLock => {
            app.child_lock_active = !app.child_lock_active;
            Task::none()
        }
        Message::ToggleYankLock => {
            app.yank_lock_active = !app.yank_lock_active;
            Task::none()
        }
        Message::ToggleHideCompletedToggle => {
            let new_val = !app.hide_completed;
            handle(app, Message::ToggleHideCompleted(new_val))
        }
        Message::OpenContextMenu(uid, is_full) => {
            let mut pt = iced::Point::new(
                app.current_window_size.width / 2.0,
                app.current_window_size.height / 2.0,
            ); // Fallback

            if let Some(id) = app.task_ids.get(&uid)
                && let Some(bounds) = crate::gui::view::focusable::get_focus_bounds(id)
            {
                if let Ok(pos) = crate::gui::subscription::LAST_MOUSE_POS.read()
                    && bounds.contains(*pos)
                {
                    // If the mouse is directly over the task, open exactly at the cursor
                    pt = *pos;
                } else if !is_full {
                    // Ellipsis button clicked via keyboard navigation
                    pt = iced::Point::new(
                        bounds.x + bounds.width - 20.0,
                        bounds.y + (bounds.height / 2.0),
                    );
                } else {
                    // Right-click triggered via keyboard navigation
                    pt = iced::Point::new(
                        bounds.x + (bounds.width / 2.0),
                        bounds.y + (bounds.height / 2.0),
                    );
                }
            } else if let Ok(pos) = crate::gui::subscription::LAST_MOUSE_POS.read()
                && (pos.x > 0.0 || pos.y > 0.0)
            {
                pt = *pos;
            }

            app.active_context_menu = Some((uid, is_full, pt));
            Task::none()
        }
        Message::CloseContextMenu => {
            app.active_context_menu = None;
            Task::none()
        }
        Message::CategoryMatchModeToggle => {
            app.session.match_all_categories = !app.session.match_all_categories;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::MoveSelected => {
            if let Some(uid) = &app.selected_uid {
                return crate::gui::update::tasks::handle(app, Message::StartMoveTask(uid.clone()));
            }
            Task::none()
        }

        Message::TabPressed(forward) => {
            let is_desc_focused = app.last_edited_field == 1 || app.editing_tree_uid.is_some();
            let (target_text, cursor_pos) = if is_desc_focused {
                let content = &app.description_value;
                let text = content.text();
                let line_idx = content.cursor().position.line;
                let col_idx = content.cursor().position.column;
                let mut byte_offset = 0;
                for (current_line, line_str) in text.split('\n').enumerate() {
                    if current_line == line_idx {
                        let col_bytes: usize =
                            line_str.chars().take(col_idx).map(|c| c.len_utf8()).sum();
                        byte_offset += col_bytes;
                        break;
                    }
                    byte_offset += line_str.len() + 1;
                }
                (text.to_string(), byte_offset)
            } else {
                let content = &app.input_value;
                let text = content.text();
                let line_idx = content.cursor().position.line;
                let col_idx = content.cursor().position.column;
                let mut byte_offset = 0;
                for (current_line, line_str) in text.split('\n').enumerate() {
                    if current_line == line_idx {
                        let col_bytes: usize =
                            line_str.chars().take(col_idx).map(|c| c.len_utf8()).sum();
                        byte_offset += col_bytes;
                        break;
                    }
                    byte_offset += line_str.len() + 1;
                }
                (text.to_string(), byte_offset)
            };

            if let Some((range, suggs)) = crate::model::autocomplete::suggest(
                &target_text,
                cursor_pos,
                &app.store,
                &app.tag_aliases,
                &app.calendars,
            ) && let Some(s) = suggs.into_iter().next()
            {
                return handle(app, Message::ApplySuggestion(range, s.replacement));
            }

            if is_desc_focused {
                let old_text = app.description_value.text();
                app.description_value
                    .perform(iced::widget::text_editor::Action::Edit(
                        iced::widget::text_editor::Edit::Insert('\t'),
                    ));
                let new_text = app.description_value.text();
                if old_text != new_text {
                    app.desc_undo_stack.push(old_text);
                    app.desc_redo_stack.clear();
                    if app.desc_undo_stack.len() > 50 {
                        app.desc_undo_stack.remove(0);
                    }
                    app.last_edited_field = 1;
                }
                Task::none()
            } else {
                handle(app, Message::CycleFocus(forward))
            }
        }
        Message::CycleFocus(forward) => {
            if app.editing_uid.is_some() || app.creating_with_desc || app.editing_tree_uid.is_some()
            {
                return Task::none();
            }

            let order = [
                crate::gui::state::Focus::MainList,
                crate::gui::state::Focus::Sidebar,
                crate::gui::state::Focus::SearchInput,
                crate::gui::state::Focus::AddTaskInput,
            ];
            let current_idx = order
                .iter()
                .position(|&f| f == app.active_focus)
                .unwrap_or(0);
            let next_idx = if forward {
                (current_idx + 1) % order.len()
            } else {
                (current_idx + order.len() - 1) % order.len()
            };
            app.active_focus = order[next_idx];
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = order[next_idx];
            }

            match app.active_focus {
                crate::gui::state::Focus::MainList => {
                    let unfocus =
                        iced::widget::operation::focus(iced::widget::Id::new("dummy_unfocus"));
                    let scroll = crate::gui::update::common::scroll_to_selected(app, true);
                    Task::batch(vec![unfocus, scroll])
                }
                crate::gui::state::Focus::Sidebar => {
                    let unfocus =
                        iced::widget::operation::focus(iced::widget::Id::new("dummy_unfocus"));
                    let max = match app.sidebar_mode {
                        SidebarMode::Calendars => app.get_filtered_calendars().len(),
                        SidebarMode::Categories => app.cached_categories.len(),
                        SidebarMode::Locations => app.cached_locations.len(),
                        SidebarMode::Journal => 31, // Mini-calendar has ~31 days
                        SidebarMode::Goals => app.core_config.goals.len(),
                    };
                    if max > 0 {
                        let y_offset = app.sidebar_selection_idx as f32
                            / (max.saturating_sub(1)).max(1) as f32;
                        let snap = iced::widget::operation::snap_to(
                            app.sidebar_scrollable_id.clone(),
                            iced::widget::scrollable::RelativeOffset {
                                x: 0.0,
                                y: y_offset,
                            },
                        );
                        return Task::batch(vec![unfocus, snap]);
                    }
                    unfocus
                }
                crate::gui::state::Focus::SearchInput => {
                    iced::widget::operation::focus("header_search_input")
                }
                crate::gui::state::Focus::AddTaskInput => {
                    iced::widget::operation::focus("main_input")
                }
            }
        }
        Message::FocusInput => {
            app.active_focus = Focus::AddTaskInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::AddTaskInput;
            }
            operation::focus("main_input")
        }
        Message::FocusSearch => {
            app.active_focus = Focus::SearchInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::SearchInput;
            }
            operation::focus("header_search_input")
        }
        Message::EnterPressed => {
            if let Some(uid) = &app.moving_task_uid {
                if let Some(task) = app.store.get_task_ref(uid) {
                    let targets =
                        app.get_move_targets(&task.calendar_href, app.moving_task_is_tree);
                    if let Some(target) = targets.get(app.move_target_idx) {
                        return crate::gui::update::tasks::handle(
                            app,
                            Message::MoveTask(task.uid.clone(), target.href.clone()),
                        );
                    }
                }
                return Task::none();
            }

            if app.ics_import_dialog_open {
                if app.ics_import_selected_calendar.is_some()
                    && app.ics_import_task_count.unwrap_or(0) > 0
                {
                    return crate::gui::update::settings::handle(
                        app,
                        Message::IcsImportDialogConfirm,
                    );
                }
                return Task::none();
            }

            if app.active_focus == crate::gui::state::Focus::Sidebar {
                return handle(app, Message::SidebarInteractEnter);
            }

            if let Some(uid) = app.selected_uid.clone() {
                return crate::gui::update::view::handle(app, Message::OpenContextMenu(uid, true));
            }

            Task::none()
        }
        Message::SelectNext => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                let max = match app.sidebar_mode {
                    SidebarMode::Calendars => app.get_filtered_calendars().len(),
                    SidebarMode::Categories => app.cached_categories.len(),
                    SidebarMode::Locations => app.cached_locations.len(),
                    SidebarMode::Journal => 31, // Mini-calendar has ~31 days
                    SidebarMode::Goals => app.core_config.goals.len(),
                };
                if max > 0 {
                    app.sidebar_selection_idx = (app.sidebar_selection_idx + 1) % max;
                    let y_offset =
                        app.sidebar_selection_idx as f32 / (max.saturating_sub(1)).max(1) as f32;
                    return iced::widget::operation::snap_to(
                        app.sidebar_scrollable_id.clone(),
                        iced::widget::scrollable::RelativeOffset {
                            x: 0.0,
                            y: y_offset,
                        },
                    );
                }
                return Task::none();
            }

            if let Some(uid) = &app.moving_task_uid {
                if let Some(task) = app.store.get_task_ref(uid) {
                    let targets =
                        app.get_move_targets(&task.calendar_href, app.moving_task_is_tree);
                    let targets_len = targets.len();
                    if !targets.is_empty() {
                        app.move_target_idx = (app.move_target_idx + 1).min(targets_len - 1);

                        let viewport_h = 250.0;
                        let item_h = 39.0;
                        let content_h = targets_len as f32 * item_h;
                        let item_center = (app.move_target_idx as f32 + 0.5) * item_h;
                        let max_scroll_px = (content_h - viewport_h).max(0.0);
                        let desired_offset_px =
                            (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                        let y = if max_scroll_px > 0.0 {
                            (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        return iced::widget::operation::snap_to(
                            iced::widget::Id::new("move_modal_scrollable"),
                            iced::widget::scrollable::RelativeOffset { x: 0.0, y },
                        );
                    }
                }
                return Task::none();
            }

            if app.ics_import_dialog_open {
                let targets: Vec<_> = app
                    .calendars
                    .iter()
                    .filter(|c| !app.disabled_calendars.contains(&c.href))
                    .collect();
                if !targets.is_empty() {
                    let current_idx = targets
                        .iter()
                        .position(|c| Some(&c.href) == app.ics_import_selected_calendar.as_ref())
                        .unwrap_or(0);
                    let next_idx = (current_idx + 1).min(targets.len() - 1);
                    app.ics_import_selected_calendar = Some(targets[next_idx].href.clone());

                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets.len() as f32 * item_h;
                    let item_center = (next_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px =
                        (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("ics_import_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y },
                    );
                }
                return Task::none();
            }

            if app.sidebar_mode == SidebarMode::Journal || app.tasks.is_empty() {
                return Task::none();
            }

            // Find current index
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.find_task_index_by_uid(uid))
                .unwrap_or(0);

            // Calculate next index (wrapping or clamping)
            let next_idx = if current_idx + 1 >= app.tasks.len() {
                0
            } else {
                current_idx + 1
            };
            if let Some(task) = app.get_task_at_index(next_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app, true);
            }
            Task::none()
        }
        Message::SelectPrev => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                let max = match app.sidebar_mode {
                    SidebarMode::Calendars => app.get_filtered_calendars().len(),
                    SidebarMode::Categories => app.cached_categories.len(),
                    SidebarMode::Locations => app.cached_locations.len(),
                    SidebarMode::Journal => 31, // Mini-calendar has ~31 days
                    SidebarMode::Goals => app.core_config.goals.len(),
                };
                if max > 0 {
                    if app.sidebar_selection_idx == 0 {
                        app.sidebar_selection_idx = max.saturating_sub(1);
                    } else {
                        app.sidebar_selection_idx -= 1;
                    }
                    let y_offset =
                        app.sidebar_selection_idx as f32 / (max.saturating_sub(1)).max(1) as f32;
                    return iced::widget::operation::snap_to(
                        app.sidebar_scrollable_id.clone(),
                        iced::widget::scrollable::RelativeOffset {
                            x: 0.0,
                            y: y_offset,
                        },
                    );
                }
                return Task::none();
            }

            if app.moving_task_uid.is_some() {
                app.move_target_idx = app.move_target_idx.saturating_sub(1);

                if let Some(uid) = &app.moving_task_uid
                    && let Some(task) = app.store.get_task_ref(uid)
                {
                    let targets =
                        app.get_move_targets(&task.calendar_href, app.moving_task_is_tree);
                    let targets_len = targets.len();

                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets_len as f32 * item_h;
                    let item_center = (app.move_target_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px =
                        (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("move_modal_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y },
                    );
                }

                return Task::none();
            }

            if app.ics_import_dialog_open {
                let targets: Vec<_> = app
                    .calendars
                    .iter()
                    .filter(|c| !app.disabled_calendars.contains(&c.href))
                    .collect();
                if !targets.is_empty() {
                    let current_idx = targets
                        .iter()
                        .position(|c| Some(&c.href) == app.ics_import_selected_calendar.as_ref())
                        .unwrap_or(0);
                    let prev_idx = current_idx.saturating_sub(1);
                    app.ics_import_selected_calendar = Some(targets[prev_idx].href.clone());

                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets.len() as f32 * item_h;
                    let item_center = (prev_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px =
                        (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("ics_import_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y },
                    );
                }
                return Task::none();
            }

            if app.tasks.is_empty() {
                return Task::none();
            }
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.find_task_index_by_uid(uid))
                .unwrap_or(0);
            let prev_idx = if current_idx == 0 {
                app.tasks.len() - 1
            } else {
                current_idx - 1
            };
            if let Some(task) = app.get_task_at_index(prev_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app, true);
            }
            Task::none()
        }
        Message::DeleteSelected => {
            if app.sidebar_mode == SidebarMode::Journal {
                if let Some(uid) = app.journal_editing_uid.clone() {
                    return crate::gui::update::tasks::handle(app, Message::DeleteTaskTree(uid));
                }
                return Task::none();
            }
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return crate::gui::update::tasks::handle(app, Message::DeleteTask(idx));
            }
            Task::none()
        }
        Message::ToggleSelected => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                return handle(app, Message::SidebarInteractSpace);
            }
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                let task = app.get_task_at_index(idx).unwrap();
                if task.is_note {
                    return Task::none();
                }
                return crate::gui::update::tasks::handle(
                    app,
                    Message::ToggleTask(idx, !task.status.is_done()),
                );
            }
            Task::none()
        }
        Message::EditSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return crate::gui::update::tasks::handle(app, Message::EditTaskStart(idx));
            }
            Task::none()
        }
        Message::DismissError => {
            app.error_msg = None;
            Task::none()
        }
        Message::DismissInfo(version) => {
            if app.info_msg_version == version {
                app.info_msg = None;
                // Prevent the list from jumping to the top when the banner layout is removed
                return crate::gui::update::common::scroll_to_selected_delayed(app, false);
            }
            Task::none()
        }
        Message::ToggleAllCalendars(show_all) => {
            if show_all {
                app.hidden_calendars.clear();
                // Explicitly re-hide trash unless it is active
                if app.active_cal_href.as_deref() != Some("local://trash") {
                    app.hidden_calendars.insert("local://trash".to_string());
                }
            } else {
                for cal in &app.calendars {
                    if app.active_cal_href.as_ref() != Some(&cal.href) {
                        app.hidden_calendars.insert(cal.href.clone());
                    }
                }
            }
            save_config(app);
            refresh_filtered_tasks(app);
            Task::perform(async { Ok::<(), String>(()) }, |_| Message::Refresh)
        }
        Message::IsolateCalendar(href) => {
            if app.sidebar_mode == SidebarMode::Categories {
                app.sidebar_mode = SidebarMode::Calendars;
            }
            app.session.focused_task_uid = None;
            app.active_cal_href = Some(href.clone());
            app.hidden_calendars.clear();
            for cal in &app.calendars {
                if cal.href != href {
                    app.hidden_calendars.insert(cal.href.clone());
                }
            }
            if app.disabled_calendars.contains(&href) {
                app.disabled_calendars.remove(&href);
            }
            save_config(app);
            refresh_filtered_tasks(app);

            if let Some(client) = &app.client {
                if !app.store.calendars.contains_key(&href) {
                    app.loading = true;
                }
                return Task::perform(async_fetch_wrapper(client.clone(), href), |res| {
                    Message::TasksRefreshed(res.map_err(|e| e.to_string()))
                });
            }
            Task::none()
        }
        Message::SidebarModeChanged(mode) => {
            if mode == SidebarMode::Calendars && !app.show_calendars_tab {
                return Task::none();
            }
            if mode == SidebarMode::Categories && !app.show_tags_tab {
                return Task::none();
            }
            if mode == SidebarMode::Locations && !app.show_locations_tab {
                return Task::none();
            }
            if mode == SidebarMode::Goals && !app.show_goals_tab {
                return Task::none();
            }
            if mode == SidebarMode::Journal && !app.show_journal_tab {
                return Task::none();
            }
            if app.sidebar_mode == SidebarMode::Journal && mode != SidebarMode::Journal {
                flush_journal_save(app);
            }
            app.sidebar_mode = mode;
            app.sidebar_selection_idx = 0;
            app.active_focus = Focus::Sidebar;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::Sidebar;
            }
            if mode == SidebarMode::Journal {
                let d = app.journal_date;
                if app.journal_date_input.is_empty() {
                    app.journal_date_input = d.format("%Y-%m-%d").to_string();
                }
                let href = app
                    .journal_editing_href
                    .clone()
                    .or(app.active_cal_href.clone())
                    .unwrap_or_default();
                app.journal_editing_uid = None;
                if let Some(entry) = app.store.get_journal_entry(&href, d) {
                    let md = crate::model::extractor::serialize_task_tree(
                        &app.store,
                        &entry.uid,
                        &app.calendars,
                        true,
                    );
                    app.journal_editor_content = iced::widget::text_editor::Content::with_text(&md);
                    app.journal_editing_uid = Some(entry.uid.clone());
                } else {
                    app.journal_editor_content = iced::widget::text_editor::Content::new();
                }
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SelectJournalDate(date) => {
            flush_journal_save(app);
            app.journal_editing_uid = None;
            app.journal_title_input.clear();
            app.journal_date = date;
            app.journal_date_input = date.format("%Y-%m-%d").to_string();
            let href = app
                .journal_editing_href
                .clone()
                .or(app.active_cal_href.clone())
                .unwrap_or_default();

            if let Some(entry) = app.store.get_journal_entry(&href, date) {
                let md = crate::model::extractor::serialize_task_tree(
                    &app.store,
                    &entry.uid,
                    &app.calendars,
                    true,
                );
                app.journal_editor_content = iced::widget::text_editor::Content::with_text(&md);
            } else {
                app.journal_editor_content = iced::widget::text_editor::Content::new();
            }
            Task::none()
        }
        Message::SelectJournalCollection(href) => {
            flush_journal_save(app);
            app.journal_editing_href = Some(href.clone());
            app.active_cal_href = Some(href.clone());
            app.journal_editing_uid = None;
            app.journal_title_input.clear();
            let date = app.journal_date;
            if let Some(entry) = app.store.get_journal_entry(&href, date) {
                let md = crate::model::extractor::serialize_task_tree(
                    &app.store,
                    &entry.uid,
                    &app.calendars,
                    true,
                );
                app.journal_editor_content = iced::widget::text_editor::Content::with_text(&md);
            } else {
                app.journal_editor_content = iced::widget::text_editor::Content::new();
            }
            Task::none()
        }
        Message::OpenJournalPage(uid) => {
            flush_journal_save(app);
            app.journal_editing_uid = Some(uid.clone());
            if let Some(task) = app.store.get_task_ref(&uid) {
                app.active_cal_href = Some(task.calendar_href.clone());
                app.journal_title_input = task.summary.clone();
                let md = crate::model::extractor::serialize_task_tree(
                    &app.store,
                    &uid,
                    &app.calendars,
                    true,
                );
                app.journal_editor_content = iced::widget::text_editor::Content::with_text(&md);
                app.editor_maximized = true;
            }
            Task::none()
        }
        Message::CreateJournalPage => {
            flush_journal_save(app);
            let target_href = app
                .journal_editing_href
                .clone()
                .or(app.active_cal_href.clone())
                .unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string());

            let mut new_page = crate::model::Task::new("", &app.tag_aliases, None);
            new_page.summary =
                rust_i18n::t!("untitled_page", default = "Untitled page").to_string();
            app.journal_title_input = new_page.summary.clone();
            new_page.is_journal = true;
            new_page.calendar_href = target_href.clone();
            let uid = new_page.uid.clone();

            app.store.add_task(new_page.clone());
            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                    crate::journal::Action::Create(new_page),
                ]));
            }

            app.active_cal_href = Some(target_href);
            app.journal_editing_uid = Some(uid);
            app.journal_editor_content = iced::widget::text_editor::Content::with_text("");
            app.editor_maximized = true;

            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::CreateJournalSubPage(parent_uid) => {
            flush_journal_save(app);
            let target_href = app
                .journal_editing_href
                .clone()
                .or(app.active_cal_href.clone())
                .unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string());

            let mut new_page = crate::model::Task::new("", &app.tag_aliases, None);
            new_page.summary =
                rust_i18n::t!("untitled_page", default = "Untitled page").to_string();
            app.journal_title_input = new_page.summary.clone();
            new_page.is_journal = true;
            new_page.calendar_href = target_href.clone();
            new_page.parent_uid = Some(parent_uid);
            let uid = new_page.uid.clone();

            app.store.add_task(new_page.clone());
            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                    crate::journal::Action::Create(new_page),
                ]));
            }

            app.active_cal_href = Some(target_href);
            app.journal_editing_uid = Some(uid);
            app.journal_editor_content = iced::widget::text_editor::Content::with_text("");
            app.editor_maximized = true;

            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::JournalDateInputChanged(s) => {
            app.journal_date_input = s;
            Task::none()
        }
        Message::JournalTitleInputChanged(s) => {
            app.journal_title_input = s.clone();
            if let Some(uid) = &app.journal_editing_uid
                && let Some((t, _)) = app.store.get_task_mut(uid)
                && t.summary != s
            {
                t.summary = s.clone();
                t.sequence += 1;
                let clone = t.clone();
                if let Some(tx) = &app.bg_tx {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                        crate::journal::Action::Update(clone),
                    ]));
                }
            }
            Task::none()
        }
        Message::JournalDateInputSubmit => {
            if let Ok(parsed) =
                chrono::NaiveDate::parse_from_str(app.journal_date_input.trim(), "%Y-%m-%d")
            {
                flush_journal_save(app);
                app.journal_editing_uid = None;
                app.journal_date = parsed;
                let href = app
                    .journal_editing_href
                    .clone()
                    .or(app.active_cal_href.clone())
                    .unwrap_or_default();
                if let Some(entry) = app.store.get_journal_entry(&href, parsed) {
                    let md = crate::model::extractor::serialize_task_tree(
                        &app.store,
                        &entry.uid,
                        &app.calendars,
                        true,
                    );
                    app.journal_editor_content = iced::widget::text_editor::Content::with_text(&md);
                    app.journal_editing_uid = Some(entry.uid.clone());
                } else {
                    app.journal_editor_content = iced::widget::text_editor::Content::new();
                }
            }
            Task::none()
        }
        Message::JournalContentChanged(action) => {
            app.journal_editor_content.perform(action);
            app.journal_debounce_version = app.journal_debounce_version.wrapping_add(1);
            let version = app.journal_debounce_version;
            Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    version
                },
                Message::SaveJournal,
            )
        }
        Message::SaveJournal(version) => {
            if version == app.journal_debounce_version {
                flush_journal_save(app);
            }
            Task::none()
        }
        Message::CategoryToggled(cat) => {
            app.active_focus = Focus::Sidebar;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::Sidebar;
            }
            if let Some(pos) = app
                .session
                .selected_categories
                .iter()
                .position(|x| x == &cat)
            {
                app.session.selected_categories.remove(pos);
            } else {
                app.session.selected_categories.push(cat.clone());
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::LocationToggled(loc) => {
            app.active_focus = Focus::Sidebar;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::Sidebar;
            }
            if let Some(pos) = app
                .session
                .selected_locations
                .iter()
                .position(|x| x == &loc)
            {
                app.session.selected_locations.remove(pos);
            } else {
                app.session.selected_locations.push(loc.clone());
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ClearAllTags => {
            app.session.selected_categories.clear();
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ClearAllLocations => {
            app.session.selected_locations.clear();
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ClearAllFilters => {
            app.session.selected_categories.clear();
            app.session.selected_locations.clear();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            app.session.focused_task_uid = None;
            if !app.search_value.text().is_empty() {
                app.search_value = iced::widget::text_editor::Content::new();
            }
            refresh_filtered_tasks(app);
            app.sidebar_mode = SidebarMode::Calendars;
            Task::none()
        }
        Message::CategoryMatchModeChanged(val) => {
            app.session.match_all_categories = val;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleHideCompleted(val) => {
            app.hide_completed = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleHideFullyCompletedTags(val) => {
            app.hide_fully_completed_tags = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleHideAliasesInSidebar(val) => {
            app.hide_aliases_in_sidebar = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleTagCollapse(tag) => {
            crate::gui::update::common::dispatch_intent(
                app,
                crate::model::AppIntent::ToggleTagCollapse { tag },
            );
            save_config(app);
            Task::none()
        }
        Message::ToggleLocationCollapse(location) => {
            crate::gui::update::common::dispatch_intent(
                app,
                crate::model::AppIntent::ToggleLocationCollapse { location },
            );
            save_config(app);
            Task::none()
        }
        Message::ToggleSortStandardByPriority(val) => {
            app.sort_standard_by_priority = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SetPausedSortBehavior(val) => {
            app.paused_sort_behavior = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleSortTiebreakRecent(val) => {
            app.sort_tiebreak_recent = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SetSortPreset(val) => {
            app.sort_preset = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleSortStandardByPriorityToggle => {
            let new_val = !app.sort_standard_by_priority;
            handle(app, Message::ToggleSortStandardByPriority(new_val))
        }
        Message::SelectCalendar(href) => {
            app.active_focus = Focus::Sidebar;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::Sidebar;
            }
            if app.sidebar_mode == SidebarMode::Categories {
                app.sidebar_mode = SidebarMode::Calendars;
            }
            app.session.focused_task_uid = None;
            app.active_cal_href = Some(href.clone());
            if app.hidden_calendars.contains(&href) {
                app.hidden_calendars.remove(&href);
                save_config(app);
            }
            refresh_filtered_tasks(app);
            if let Some(client) = &app.client {
                if !app.store.calendars.contains_key(&href) {
                    app.loading = true;
                }
                return Task::perform(async_fetch_wrapper(client.clone(), href), |res| {
                    Message::TasksRefreshed(res.map_err(|e| e.to_string()))
                });
            }
            Task::none()
        }
        Message::ToggleCalendarDisabled(href, is_disabled) => {
            if is_disabled {
                app.disabled_calendars.insert(href.clone());
                if app.active_cal_href.as_ref() == Some(&href) {
                    app.active_cal_href = None;
                }
            } else {
                app.disabled_calendars.remove(&href);
            }
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleCalendarVisibility(href, is_visible) => {
            if !is_visible && app.active_cal_href.as_ref() == Some(&href) {
                return Task::none();
            }
            if is_visible {
                app.hidden_calendars.remove(&href);
            } else {
                app.hidden_calendars.insert(href);
            }
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SearchChanged(action) => {
            app.active_focus = Focus::SearchInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::SearchInput;
            }
            if let iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert('\t'),
            ) = &action
            {
                return Task::done(Message::CycleFocus(true));
            }
            app.search_value.perform(action);
            app.session.search_term = app.search_value.text();
            if app.session.search_term.is_empty() {
                app.session.search_collapsed_tasks.clear();
            }

            app.search_debounce_version = app.search_debounce_version.wrapping_add(1);
            let version = app.search_debounce_version;

            Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    version
                },
                Message::ApplySearch,
            )
        }
        Message::ApplySearch(version) => {
            // Only refresh if the user hasn't typed anything else
            if version == app.search_debounce_version {
                refresh_filtered_tasks(app);
            }
            Task::none()
        }
        Message::ClearSearch => {
            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SetSearchTerm(term) => {
            app.search_value = iced::widget::text_editor::Content::with_text(&term);
            app.session.search_term = term;
            app.session.search_collapsed_tasks.clear();
            app.search_value
                .perform(iced::widget::text_editor::Action::Move(
                    iced::widget::text_editor::Motion::DocumentEnd,
                ));
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleSidebar => {
            app.sidebar_is_hidden = !app.sidebar_is_hidden;
            save_config(app);
            Task::none()
        }
        Message::ToggleEditorMaximize => {
            app.editor_maximized = !app.editor_maximized;
            Task::none()
        }
        Message::ToggleQuickFilter => {
            let current = app.search_value.text();
            let new_text = if current.contains(&app.quick_filter_term) {
                current
                    .replace(&app.quick_filter_term, "")
                    .trim()
                    .to_string()
            } else {
                if current.is_empty() {
                    app.quick_filter_term.clone()
                } else {
                    format!("{} {}", app.quick_filter_term, current)
                }
            };
            app.search_value = iced::widget::text_editor::Content::with_text(&new_text);
            app.session.search_term = new_text.clone();
            app.search_value
                .perform(iced::widget::text_editor::Action::Move(
                    iced::widget::text_editor::Motion::DocumentEnd,
                ));
            app.search_debounce_version = app.search_debounce_version.wrapping_add(1);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SetMinDuration(val) => {
            app.filter_min_duration = val;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::SetMaxDuration(val) => {
            app.filter_max_duration = val;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleIncludeUnsetDuration(val) => {
            app.filter_include_unset_duration = val;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleDetails(uid) => {
            if app.expanded_tasks.contains(&uid) {
                app.expanded_tasks.remove(&uid);
            } else {
                app.expanded_tasks.insert(uid.clone());
            }
            app.selected_uid = Some(uid);
            Task::none()
        }
        Message::OpenHelp(tab) => {
            let icon_choice = match app.state {
                AppState::Help(_, choice) => choice,
                _ => fastrand::u8(0..crate::help::SUPPORT_ICONS.len() as u8),
            };
            app.state = AppState::Help(tab, icon_choice);
            Task::none()
        }
        Message::CloseHelp => {
            app.state = AppState::Active;
            Task::none()
        }
        Message::SwitchHelpTab(forward) => {
            if let AppState::Help(current_tab, icon_choice) = app.state {
                let next_tab = if forward {
                    match current_tab {
                        crate::help::HelpTab::Syntax => crate::help::HelpTab::Shortcuts,
                        crate::help::HelpTab::Shortcuts => crate::help::HelpTab::About,
                        crate::help::HelpTab::About => crate::help::HelpTab::Syntax,
                    }
                } else {
                    match current_tab {
                        crate::help::HelpTab::Syntax => crate::help::HelpTab::About,
                        crate::help::HelpTab::Shortcuts => crate::help::HelpTab::Syntax,
                        crate::help::HelpTab::About => crate::help::HelpTab::Shortcuts,
                    }
                };
                app.state = AppState::Help(next_tab, icon_choice);
            }
            Task::none()
        }
        Message::WindowDragged => {
            let now = std::time::Instant::now();
            let mut is_double = false;

            if let Some(last_time) = app.last_title_click
                && now.duration_since(last_time).as_millis() < 400
            {
                is_double = true;
            }
            app.last_title_click = Some(now);

            if is_double {
                app.last_title_click = None;
                window::latest().then(|id| {
                    if let Some(id) = id {
                        window::toggle_maximize(id)
                    } else {
                        Task::none()
                    }
                })
            } else {
                window::latest().then(|id| {
                    if let Some(id) = id {
                        window::drag(id)
                    } else {
                        Task::none()
                    }
                })
            }
        }
        Message::MinimizeWindow => window::latest().then(|id| {
            if let Some(id) = id {
                window::minimize(id, true)
            } else {
                Task::none()
            }
        }),
        Message::CloseWindow => window::latest().then(|id| {
            if let Some(id) = id {
                window::close(id)
            } else {
                Task::none()
            }
        }),
        Message::ResizeStart(direction) => {
            let dir = match direction {
                ResizeDirection::North => window::Direction::North,
                ResizeDirection::South => window::Direction::South,
                ResizeDirection::East => window::Direction::East,
                ResizeDirection::West => window::Direction::West,
                ResizeDirection::NorthEast => window::Direction::NorthEast,
                ResizeDirection::NorthWest => window::Direction::NorthWest,
                ResizeDirection::SouthEast => window::Direction::SouthEast,
                ResizeDirection::SouthWest => window::Direction::SouthWest,
            };
            window::latest().then(move |id| {
                if let Some(id) = id {
                    window::drag_resize(id, dir)
                } else {
                    Task::none()
                }
            })
        }
        Message::WindowFocused(focused) => {
            app.is_window_focused = focused;
            Task::none()
        }
        Message::WindowResized(size) => {
            let was_narrow = app.current_window_size.width < 750.0;
            let is_narrow = size.width < 750.0;
            app.current_window_size = size;
            app.core_config.window_width = size.width;
            app.core_config.window_height = size.height;

            if !was_narrow && is_narrow {
                app.sidebar_is_hidden = true;
                save_config(app);
                Task::none()
            } else if was_narrow && !is_narrow {
                app.sidebar_is_hidden = false;
                save_config(app);
                Task::none()
            } else {
                app.resize_debounce_version = app.resize_debounce_version.wrapping_add(1);
                let version = app.resize_debounce_version;
                Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        version
                    },
                    Message::ApplyWindowResize,
                )
            }
        }
        Message::ApplyWindowResize(version) => {
            if version == app.resize_debounce_version {
                save_config(app);
            }
            Task::none()
        }
        // Focus Handlers (No scrolling)
        Message::FocusTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.session.selected_categories.clear();
            app.session.selected_categories.push(tag.clone());

            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            refresh_filtered_tasks(app);
            // DO NOT scroll sidebar here, as user just clicked the arrow
            Task::none()
        }
        Message::FocusLocation(loc) => {
            app.sidebar_mode = SidebarMode::Locations;
            app.session.selected_locations.clear();
            app.session.selected_locations.push(loc.clone());

            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            refresh_filtered_tasks(app);
            // DO NOT scroll sidebar here
            Task::none()
        }
        // KEEP: JumpToTag still scrolls (used for tags in task list)
        Message::JumpToTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.session.selected_categories.clear();

            let tags =
                crate::model::parser::resolve_selection_aliases(&tag, false, &app.tag_aliases);
            for t in &tags {
                app.session.selected_categories.push(t.clone());
            }

            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            refresh_filtered_tasks(app);

            // Auto-scroll logic is kept for JumpTo...
            let all_cats = &app.cached_categories;
            let scroll_target = tags.first().unwrap_or(&tag);
            if let Some(index) = all_cats
                .iter()
                .position(|item| item.full_key == *scroll_target)
            {
                let total = all_cats.len();
                if total > 1 {
                    let y_offset = index as f32 / (total - 1) as f32;
                    return iced::widget::operation::snap_to(
                        app.sidebar_scrollable_id.clone(),
                        iced::widget::scrollable::RelativeOffset {
                            x: 0.0,
                            y: y_offset,
                        },
                    );
                }
            }
            Task::none()
        }
        // KEEP: JumpToLocation still scrolls
        Message::JumpToLocation(loc) => {
            app.sidebar_mode = SidebarMode::Locations;
            app.session.selected_locations.clear();

            let locs =
                crate::model::parser::resolve_selection_aliases(&loc, true, &app.tag_aliases);
            for l in &locs {
                app.session.selected_locations.push(l.clone());
            }

            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            app.session.search_collapsed_tasks.clear();
            refresh_filtered_tasks(app);

            let all_locs = &app.cached_locations;
            let scroll_target = locs.first().unwrap_or(&loc);
            if let Some(index) = all_locs
                .iter()
                .position(|item| item.full_key == *scroll_target)
            {
                let total = all_locs.len();
                if total > 1 {
                    let y_offset = index as f32 / (total - 1) as f32;
                    return iced::widget::operation::snap_to(
                        app.sidebar_scrollable_id.clone(),
                        iced::widget::scrollable::RelativeOffset {
                            x: 0.0,
                            y: y_offset,
                        },
                    );
                }
            }
            Task::none()
        }
        Message::JumpToTask(uid) => {
            // Check if it's a journal page first
            if let Some(task) = app.store.get_task_ref(&uid).cloned()
                && task.is_journal
            {
                if app.sidebar_mode != SidebarMode::Journal {
                    app.sidebar_mode = SidebarMode::Journal;
                }
                if app.active_cal_href.as_ref() != Some(&task.calendar_href) {
                    app.active_cal_href = Some(task.calendar_href.clone());
                    if app.hidden_calendars.contains(&task.calendar_href) {
                        app.hidden_calendars.remove(&task.calendar_href);
                        save_config(app);
                    }
                }
                refresh_filtered_tasks(app);
                return handle(app, Message::OpenJournalPage(uid));
            }

            // 1. Find which calendar this task belongs to
            if let Some(href) = app.store.index.get(&uid).cloned() {
                // 2. If it's in a hidden or different active calendar, switch to it
                let mut needs_refresh = false;

                if app.active_cal_href.as_ref() != Some(&href) {
                    app.active_cal_href = Some(href.clone());
                    // Ensure it's not hidden
                    if app.hidden_calendars.contains(&href) {
                        app.hidden_calendars.remove(&href);
                        save_config(app);
                    }
                    needs_refresh = true;
                }

                // 3. Clear filters that might hide the task
                if app.sidebar_mode == SidebarMode::Journal
                    || app.sidebar_mode == SidebarMode::Goals
                {
                    app.sidebar_mode = SidebarMode::Calendars;
                    needs_refresh = true;
                }
                if !app.search_value.text().is_empty() {
                    app.search_value = iced::widget::text_editor::Content::new();
                    app.session.search_term.clear();
                    app.session.search_collapsed_tasks.clear();
                    needs_refresh = true;
                }
                if !app.session.selected_categories.is_empty() {
                    app.session.selected_categories.clear();
                    needs_refresh = true;
                }
                if !app.session.selected_locations.is_empty() {
                    app.session.selected_locations.clear();
                    needs_refresh = true;
                }

                if let Some(task) = app.store.get_task_ref(&uid).cloned() {
                    if task.status.is_done() && app.hide_completed {
                        app.hide_completed = false;
                        app.core_config.hide_completed = false;
                        save_config(app);
                        needs_refresh = true;
                    }

                    if task.status.is_done() {
                        let group_key = task.parent_uid.clone().unwrap_or_default();
                        if !app.session.expanded_done_groups.contains(&group_key) {
                            app.session.expanded_done_groups.push(group_key);
                            needs_refresh = true;
                        }
                    }

                    // Uncollapse ancestors
                    let mut curr = task.parent_uid.clone();
                    let mut to_uncollapse = Vec::new();
                    let mut visited = std::collections::HashSet::new();
                    while let Some(p_uid) = curr {
                        if !visited.insert(p_uid.clone()) {
                            break;
                        }
                        if let Some(p_task) = app.store.get_task_ref(&p_uid) {
                            if p_task.collapsed {
                                to_uncollapse.push(p_uid.clone());
                            }
                            curr = p_task.parent_uid.clone();
                        } else {
                            break;
                        }
                    }
                    for p_uid in to_uncollapse {
                        crate::gui::update::common::dispatch_intent(
                            app,
                            crate::model::AppIntent::SetTreeCollapse {
                                uid: p_uid,
                                collapsed: false,
                            },
                        );
                        needs_refresh = true;
                    }
                }

                if needs_refresh {
                    refresh_filtered_tasks(app);
                }

                // 4. Select and Expand
                app.selected_uid = Some(uid.clone());
                app.expanded_tasks.insert(uid.clone()); // Auto-expand details

                // 5. USE DELAYED SCROLL
                // We use delayed here because if we just un-hid the calendar or cleared filters,
                // the row widget does not exist in the current frame.
                scroll_to_selected_delayed(app, true)
            } else {
                // FALLBACK: Treat degraded/unresolved references as a search query!
                app.search_value = iced::widget::text_editor::Content::with_text(&uid);
                app.session.search_term = uid.clone();
                app.session.search_collapsed_tasks.clear();
                app.session.selected_categories.clear();
                app.session.selected_locations.clear();

                app.search_value
                    .perform(iced::widget::text_editor::Action::Move(
                        iced::widget::text_editor::Motion::DocumentEnd,
                    ));

                refresh_filtered_tasks(app);

                app.active_focus = Focus::SearchInput;
                if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                    *focus = Focus::SearchInput;
                }

                iced::widget::operation::focus(iced::widget::Id::new("header_search_input"))
            }
        }
        Message::TagHovered(uid) => {
            app.hovered_tag_uid = uid;
            Task::none()
        }
        Message::OpenUrl(target) => {
            crate::system::open_url(&target);
            Task::none()
        }
        Message::OpenCoordinates(uid) => {
            if let Some(task) = app.store.get_task_ref(&uid)
                && let Some(geo) = &task.geo
            {
                crate::system::open_url(&format!("geo:{}", geo));
            }
            Task::none()
        }
        Message::OpenLocations(uid) => {
            let waypoints = app.store.get_tree_waypoints(&uid);
            if !waypoints.is_empty() {
                // Generate GPX content
                let mut gpx_string = String::from(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Cfait\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
                );
                for (name, geo) in waypoints {
                    let parts: Vec<&str> = geo.split(',').collect();
                    if parts.len() >= 2 {
                        let escaped_name = name
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;");
                        gpx_string.push_str(&format!(
                            "  <wpt lat=\"{}\" lon=\"{}\"><name>{}</name></wpt>\n",
                            parts[0].trim(),
                            parts[1].trim(),
                            escaped_name
                        ));
                    }
                }
                gpx_string.push_str("</gpx>");

                if let Ok(cache_dir) = app.ctx.get_cache_dir() {
                    let path = cache_dir.join(format!("locations_{}.gpx", uuid::Uuid::new_v4()));
                    if std::fs::write(&path, gpx_string).is_ok() {
                        crate::system::open_url(&path.to_string_lossy());
                    }
                }
            }
            Task::none()
        }
        Message::JumpToRandomTask => {
            // 1. Randomize icon for next time
            let mut rng = fastrand::Rng::new();
            let icons = crate::gui::icon::RANDOM_ICONS;
            app.random_icon = icons[rng.usize(..icons.len())];

            // Extract real tasks for the random weighted selector
            let real_tasks: Vec<crate::model::Task> = app
                .tasks
                .iter()
                .filter_map(|item| {
                    if let crate::store::TaskListItem::Task(t) = item {
                        Some((**t).clone())
                    } else {
                        None
                    }
                })
                .collect();

            // 2. Select Weighted Random Task
            if let Some(idx) = select_weighted_random_index(&real_tasks, app.default_priority)
                && let Some(task) = real_tasks.get(idx)
            {
                app.selected_uid = Some(task.uid.clone());
                // 3. Scroll to it
                return scroll_to_selected(app, true);
            }
            Task::none()
        }
        _ => Task::none(),
    }
}
