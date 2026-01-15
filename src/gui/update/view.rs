// Handles view/navigation-related messages in the GUI.
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp, ResizeDirection, SidebarMode};
use crate::gui::update::common::{refresh_filtered_tasks, save_config, scroll_to_selected};
use iced::widget::operation;
use iced::{Task, window};

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::SelectNextPage => {
            if app.tasks.is_empty() {
                return Task::none();
            }
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.tasks.iter().position(|t| t.uid == *uid))
                .unwrap_or(0);

            let next_idx = (current_idx + 10).min(app.tasks.len() - 1);

            if let Some(task) = app.tasks.get(next_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app);
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
                .and_then(|uid| app.tasks.iter().position(|t| t.uid == *uid))
                .unwrap_or(0);

            let prev_idx = current_idx.saturating_sub(10);

            if let Some(task) = app.tasks.get(prev_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app);
            }
            Task::none()
        }

        // Stateless toggles: read current state, flip it, call existing logic
        Message::ToggleHideCompletedToggle => {
            let new_val = !app.hide_completed;
            handle(app, Message::ToggleHideCompleted(new_val))
        }
        Message::CategoryMatchModeToggle => {
            let new_val = !app.match_all_categories;
            handle(app, Message::CategoryMatchModeChanged(new_val))
        }

        Message::TabPressed(shift_held) => {
            if shift_held {
                operation::focus_previous()
            } else {
                operation::focus_next()
            }
        }
        Message::FocusInput => operation::focus("main_input"),
        Message::FocusSearch => operation::focus("header_search_input"),
        Message::SelectNextTask => {
            if app.tasks.is_empty() {
                return Task::none();
            }

            // Find current index
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.tasks.iter().position(|t| t.uid == *uid))
                .unwrap_or(0);

            // Calculate next index (wrapping or clamping)
            let next_idx = if current_idx + 1 >= app.tasks.len() {
                0
            } else {
                current_idx + 1
            };
            if let Some(task) = app.tasks.get(next_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app);
            }
            Task::none()
        }
        Message::SelectPrevTask => {
            if app.tasks.is_empty() {
                return Task::none();
            }
            let current_idx = app
                .selected_uid
                .as_ref()
                .and_then(|uid| app.tasks.iter().position(|t| t.uid == *uid))
                .unwrap_or(0);
            let prev_idx = if current_idx == 0 {
                app.tasks.len() - 1
            } else {
                current_idx - 1
            };
            if let Some(task) = app.tasks.get(prev_idx) {
                app.selected_uid = Some(task.uid.clone());
                return scroll_to_selected(app);
            }
            Task::none()
        }
        Message::DeleteSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
            {
                return crate::gui::update::tasks::handle(app, Message::DeleteTask(idx));
            }
            Task::none()
        }
        Message::ToggleSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
            {
                let task = &app.tasks[idx];
                return crate::gui::update::tasks::handle(
                    app,
                    Message::ToggleTask(idx, !task.status.is_done()),
                );
            }
            Task::none()
        }
        Message::EditSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
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
                return Task::perform(
                    async_fetch_wrapper(client.clone(), href),
                    Message::TasksRefreshed,
                );
            }
            Task::none()
        }
        Message::SidebarModeChanged(mode) => {
            app.sidebar_mode = mode;
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::CategoryToggled(cat) => {
            if app.selected_categories.contains(&cat) {
                app.selected_categories.remove(&cat);
            } else {
                app.selected_categories.insert(cat);
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::LocationToggled(loc) => {
            if app.selected_locations.contains(&loc) {
                app.selected_locations.remove(&loc);
            } else {
                app.selected_locations.insert(loc);
            }
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ClearAllTags => {
            app.selected_categories.clear();
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::ClearAllLocations => {
            app.selected_locations.clear();
            refresh_filtered_tasks(app);
            Task::none()
        }
        Message::CategoryMatchModeChanged(val) => {
            app.match_all_categories = val;
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
                return Task::perform(
                    async_fetch_wrapper(client.clone(), href),
                    Message::TasksRefreshed,
                );
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
        Message::SearchChanged(val) => {
            app.search_value = val;
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
        Message::OpenHelp => {
            app.state = AppState::Help;
            Task::none()
        }
        Message::CloseHelp => {
            app.state = AppState::Active;
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
            app.current_window_size = size;
            Task::none()
        }
        // Focus Handlers (No scrolling)
        Message::FocusTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.selected_categories.clear();
            app.selected_categories.insert(tag.clone());
            app.search_value.clear();
            refresh_filtered_tasks(app);
            // DO NOT scroll sidebar here, as user just clicked the arrow
            Task::none()
        }
        Message::FocusLocation(loc) => {
            app.sidebar_mode = SidebarMode::Locations;
            app.selected_locations.clear();
            app.selected_locations.insert(loc.clone());
            app.search_value.clear();
            refresh_filtered_tasks(app);
            // DO NOT scroll sidebar here
            Task::none()
        }

        // KEEP: JumpToTag still scrolls (used for tags in task list)
        Message::JumpToTag(tag) => {
            app.sidebar_mode = SidebarMode::Categories;
            app.selected_categories.clear();
            app.selected_categories.insert(tag.clone());
            app.search_value.clear();
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
            app.selected_locations.clear();
            app.selected_locations.insert(loc.clone());
            app.search_value.clear();
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
                if !app.search_value.is_empty() {
                    app.search_value.clear();
                    needs_refresh = true;
                }
                if !app.selected_categories.is_empty() {
                    app.selected_categories.clear();
                    needs_refresh = true;
                }
                if !app.selected_locations.is_empty() {
                    app.selected_locations.clear();
                    needs_refresh = true;
                }

                if needs_refresh {
                    refresh_filtered_tasks(app);
                }

                // 4. Select and Expand
                app.selected_uid = Some(uid.clone());
                app.expanded_tasks.insert(uid.clone()); // Auto-expand details

                // 5. Scroll
                return scroll_to_selected(app);
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
        _ => Task::none(),
    }
}
