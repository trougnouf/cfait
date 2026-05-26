// SPDX-License-Identifier: GPL-3.0-or-later
// Handles view/navigation-related messages in the GUI.
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp, ResizeDirection, SidebarMode};
use crate::gui::update::common::{
    refresh_filtered_tasks, save_config, scroll_to_selected, scroll_to_selected_delayed,
};
use crate::gui::update::tasks;
use crate::store::select_weighted_random_index;
use fastrand;
use iced::widget::operation;
use iced::{Task, window};

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::TaskClick(index, uid) => {
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
            Task::none()
        }
        Message::ZoomOut => {
            // Decrease scale by 10%, clamp at 50%
            app.ui_scale = (app.ui_scale - 0.1).max(0.5);
            Task::none()
        }
        Message::ZoomReset => {
            app.ui_scale = 1.0;
            Task::none()
        }

        Message::SelectNextPage => {
            if app.tasks.is_empty() {
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
            app.active_context_menu = Some((uid, is_full, app.cursor_position));
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

        Message::TabPressed(shift_held) => {
            // Ignore Tab navigation if we are actively editing a description,
            // allowing the Tab character to be inserted in the description editor instead.
            if app.editing_uid.is_some() || app.creating_with_desc {
                return Task::none();
            }

            if shift_held {
                operation::focus_previous()
            } else {
                operation::focus_next()
            }
        }
        Message::FocusInput => operation::focus("main_input"),
        Message::FocusSearch => operation::focus("header_search_input"),
        Message::EnterPressed => {
            if let Some(uid) = &app.moving_task_uid {
                if let Some(idx) = app.find_task_index_by_uid(uid)
                    && let Some(task) = app.get_task_at_index(idx)
                {
                    let targets = app.get_move_targets(&task.calendar_href);
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
                if app.ics_import_selected_calendar.is_some() && app.ics_import_task_count.unwrap_or(0) > 0 {
                    return crate::gui::update::settings::handle(app, Message::IcsImportDialogConfirm);
                }
                return Task::none();
            }

            Task::none()
        }
        Message::SelectNextTask => {
            if let Some(uid) = &app.moving_task_uid {
                if let Some(idx) = app.find_task_index_by_uid(uid)
                    && let Some(task) = app.get_task_at_index(idx)
                {
                    let targets = app.get_move_targets(&task.calendar_href);
                    let targets_len = targets.len();
                    if !targets.is_empty() {
                        app.move_target_idx = (app.move_target_idx + 1).min(targets_len - 1);
                        
                        let viewport_h = 250.0;
                        let item_h = 39.0;
                        let content_h = targets_len as f32 * item_h;
                        let item_center = (app.move_target_idx as f32 + 0.5) * item_h;
                        let max_scroll_px = (content_h - viewport_h).max(0.0);
                        let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                        let y = if max_scroll_px > 0.0 {
                            (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        return iced::widget::operation::snap_to(
                            iced::widget::Id::new("move_modal_scrollable"),
                            iced::widget::scrollable::RelativeOffset { x: 0.0, y }
                        );
                    }
                }
                return Task::none();
            }

            if app.ics_import_dialog_open {
                let targets: Vec<_> = app.calendars.iter().filter(|c| !app.disabled_calendars.contains(&c.href)).collect();
                if !targets.is_empty() {
                    let current_idx = targets.iter().position(|c| Some(&c.href) == app.ics_import_selected_calendar.as_ref()).unwrap_or(0);
                    let next_idx = (current_idx + 1).min(targets.len() - 1);
                    app.ics_import_selected_calendar = Some(targets[next_idx].href.clone());

                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets.len() as f32 * item_h;
                    let item_center = (next_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("ics_import_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y }
                    );
                }
                return Task::none();
            }

            if app.tasks.is_empty() {
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
        Message::SelectPrevTask => {
            if app.moving_task_uid.is_some() {
                app.move_target_idx = app.move_target_idx.saturating_sub(1);
                
                if let Some(uid) = &app.moving_task_uid
                    && let Some(idx) = app.find_task_index_by_uid(uid)
                    && let Some(task) = app.get_task_at_index(idx)
                {
                    let targets = app.get_move_targets(&task.calendar_href);
                    let targets_len = targets.len();
                    
                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets_len as f32 * item_h;
                    let item_center = (app.move_target_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("move_modal_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y }
                    );
                }
                
                return Task::none();
            }

            if app.ics_import_dialog_open {
                let targets: Vec<_> = app.calendars.iter().filter(|c| !app.disabled_calendars.contains(&c.href)).collect();
                if !targets.is_empty() {
                    let current_idx = targets.iter().position(|c| Some(&c.href) == app.ics_import_selected_calendar.as_ref()).unwrap_or(0);
                    let prev_idx = current_idx.saturating_sub(1);
                    app.ics_import_selected_calendar = Some(targets[prev_idx].href.clone());

                    let viewport_h = 250.0;
                    let item_h = 39.0;
                    let content_h = targets.len() as f32 * item_h;
                    let item_center = (prev_idx as f32 + 0.5) * item_h;
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll_px);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    return iced::widget::operation::snap_to(
                        iced::widget::Id::new("ics_import_scrollable"),
                        iced::widget::scrollable::RelativeOffset { x: 0.0, y }
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
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return crate::gui::update::tasks::handle(app, Message::DeleteTask(idx));
            }
            Task::none()
        }
        Message::ToggleSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                let task = app.get_task_at_index(idx).unwrap();
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
            app.sidebar_mode = mode;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::CategoryToggled(cat) => {
            if let Some(pos) = app
                .session
                .selected_categories
                .iter()
                .position(|x| x == &cat)
            {
                app.session.selected_categories.remove(pos);
            } else {
                app.session.selected_categories.push(cat);
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::LocationToggled(loc) => {
            if let Some(pos) = app
                .session
                .selected_locations
                .iter()
                .position(|x| x == &loc)
            {
                app.session.selected_locations.remove(pos);
            } else {
                app.session.selected_locations.push(loc);
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
        Message::ToggleSortStandardByPriority(val) => {
            app.sort_standard_by_priority = val;
            save_config(app);
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleSortStandardByPriorityToggle => {
            let new_val = !app.sort_standard_by_priority;
            handle(app, Message::ToggleSortStandardByPriority(new_val))
        }
        Message::SelectCalendar(href) => {
            if app.sidebar_mode == SidebarMode::Categories {
                app.sidebar_mode = SidebarMode::Calendars;
            }
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
            if let iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert('\t'),
            ) = &action
            {
                return Task::none();
            }
            app.search_value.perform(action);
            app.session.search_term = app.search_value.text();

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
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ToggleSidebar => {
            app.sidebar_is_hidden = !app.sidebar_is_hidden;
            save_config(app);
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
        Message::WindowDragged => window::latest().then(|id| {
            if let Some(id) = id {
                window::drag(id)
            } else {
                Task::none()
            }
        }),
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
        Message::WindowResized(size) => {
            let was_narrow = app.current_window_size.width < 750.0;
            let is_narrow = size.width < 750.0;
            app.current_window_size = size;

            if !was_narrow && is_narrow {
                app.sidebar_is_hidden = true;
                save_config(app);
            } else if was_narrow && !is_narrow {
                app.sidebar_is_hidden = false;
                save_config(app);
            }
            Task::none()
        }
        Message::CursorMoved(position) => {
            app.cursor_position = position;
            Task::none()
        }
        // Focus Handlers (No scrolling)
        Message::FocusTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.session.selected_categories.clear();
            app.session.selected_categories.push(tag.clone());
            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
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
            refresh_filtered_tasks(app);
            // DO NOT scroll sidebar here
            Task::none()
        }

        // KEEP: JumpToTag still scrolls (used for tags in task list)
        Message::JumpToTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.session.selected_categories.clear();
            app.session.selected_categories.push(tag.clone());
            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            refresh_filtered_tasks(app);

            // Auto-scroll logic is kept for JumpTo...
            let all_cats = &app.cached_categories;
            if let Some(index) = all_cats.iter().position(|(t, _)| t == &tag) {
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
            app.session.selected_locations.push(loc.clone());
            app.search_value = iced::widget::text_editor::Content::new();
            app.session.search_term.clear();
            refresh_filtered_tasks(app);

            let all_locs = &app.cached_locations;
            if let Some(index) = all_locs.iter().position(|(l, _)| l == &loc) {
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
                if !app.search_value.text().is_empty() {
                    app.search_value = iced::widget::text_editor::Content::new();
                    app.session.search_term.clear();
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

                if needs_refresh {
                    refresh_filtered_tasks(app);
                }

                // 4. Select and Expand
                app.selected_uid = Some(uid.clone());
                app.expanded_tasks.insert(uid.clone()); // Auto-expand details

                // 5. USE DELAYED SCROLL
                // We use delayed here because if we just un-hid the calendar or cleared filters,
                // the row widget does not exist in the current frame.
                return scroll_to_selected_delayed(app, true);
            }
            Task::none()
        }
        Message::TagHovered(uid) => {
            app.hovered_tag_uid = uid;
            Task::none()
        }
        Message::OpenUrl(target) => {
            // Note: target can be "https://..." or "geo:lat,long"
            let target_url = target.clone();
            #[cfg(not(target_os = "android"))]
            std::thread::spawn(move || {
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open")
                    .arg(target_url)
                    .spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer")
                    .arg(target_url)
                    .spawn();
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(target_url).spawn();
            });
            Task::none()
        }
        Message::OpenCoordinates(uid) => {
            if let Some(task) = app.store.get_task_ref(&uid)
                && let Some(geo) = &task.geo
            {
                let geo_target = format!("geo:{}", geo);
                let target_url = geo_target.clone();
                #[cfg(not(target_os = "android"))]
                std::thread::spawn(move || {
                    #[cfg(target_os = "linux")]
                    let _ = std::process::Command::new("xdg-open")
                        .arg(target_url)
                        .spawn();
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("explorer")
                        .arg(target_url)
                        .spawn();
                    #[cfg(target_os = "macos")]
                    let _ = std::process::Command::new("open").arg(target_url).spawn();
                });
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
                        let target = path.to_string_lossy().to_string();
                        #[cfg(not(target_os = "android"))]
                        std::thread::spawn(move || {
                            #[cfg(target_os = "linux")]
                            let _ = std::process::Command::new("xdg-open").arg(target).spawn();
                            #[cfg(target_os = "windows")]
                            let _ = std::process::Command::new("explorer").arg(target).spawn();
                            #[cfg(target_os = "macos")]
                            let _ = std::process::Command::new("open").arg(target).spawn();
                        });
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
