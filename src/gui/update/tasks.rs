// File: ./src/gui/update/tasks.rs
// SPDX-License-Identifier: GPL-3.0-or-later
// Simplified GUI task handlers: always dispatch controller actions via
// DispatchIntent which routes to the TaskController.

use crate::gui::message::Message;
use crate::gui::state::{Focus, GuiApp, SidebarMode};
use crate::gui::subscription::ACTIVE_FOCUS;
use crate::gui::update::common;
use crate::model::AppIntent;
use chrono::NaiveTime;
use iced::Task;
use iced::widget::text_editor;

fn dispatch_and_maintain_selection(app: &mut GuiApp, intent: AppIntent, focus_uid: &str) {
    let was_selected = app.selected_uid.as_deref() == Some(focus_uid);
    let old_idx = app.find_task_index_by_uid(focus_uid);

    common::dispatch_intent(app, intent);

    if was_selected
        && let Some(idx) = old_idx
        && app.find_task_index_by_uid(focus_uid).is_none()
    {
        let new_idx = idx.min(app.tasks.len().saturating_sub(1));
        let mut fallback = None;
        for i in new_idx..app.tasks.len() {
            if let Some(t) = app.get_task_at_index(i) {
                fallback = Some(t.uid.clone());
                break;
            }
        }
        if fallback.is_none() {
            for i in (0..new_idx).rev() {
                if let Some(t) = app.get_task_at_index(i) {
                    fallback = Some(t.uid.clone());
                    break;
                }
            }
        }
        if fallback.is_some() {
            app.selected_uid = fallback;
        }
    }
}

/// Dispatch an intent that re-sorts the focused task, then keep selection on the row below it instead of following that task.
fn dispatch_and_select_next_row(app: &mut GuiApp, intent: AppIntent, uid: String) {
    let was_selected = app.selected_uid.as_ref() == Some(&uid);

    let next_uid = if was_selected {
        app.find_task_index_by_uid(&uid)
            .and_then(|idx| {
                app.get_task_at_index(idx + 1).or_else(|| {
                    if idx > 0 {
                        app.get_task_at_index(idx - 1)
                    } else {
                        None
                    }
                })
            })
            .map(|task| task.uid.clone())
    } else {
        None
    };

    dispatch_and_maintain_selection(app, intent, &uid);

    if was_selected
        && let Some(next_uid) = next_uid
        && app.find_task_index_by_uid(&next_uid).is_some()
    {
        app.selected_uid = Some(next_uid);
    }
}

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(action) => {
            app.active_focus = Focus::AddTaskInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::AddTaskInput;
            }
            if let text_editor::Action::Edit(text_editor::Edit::Enter) = action {
                return handle_submit(app);
            }
            if let text_editor::Action::Edit(text_editor::Edit::Insert('\t')) = action {
                return Task::none();
            }
            app.input_value.perform(action);
            Task::none()
        }

        Message::DescriptionChanged(action) => {
            app.description_value.perform(action);
            Task::none()
        }

        Message::StartCreateChild(parent_uid) => {
            app.creating_child_of = Some(parent_uid.clone());
            app.selected_uid = Some(parent_uid.clone());
            app.creating_with_desc = false;

            let mut initial_input = String::new();
            if let Some(parent) = app.store.get_task_ref(&parent_uid) {
                for cat in &parent.categories {
                    initial_input
                        .push_str(&format!("#{} ", crate::model::parser::quote_value(cat)));
                }
                if let Some(loc) = &parent.location {
                    initial_input
                        .push_str(&format!("@@{} ", crate::model::parser::quote_value(loc)));
                }
            }

            app.input_value = text_editor::Content::with_text(&initial_input);
            app.input_value
                .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));

            app.active_focus = Focus::AddTaskInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::AddTaskInput;
            }

            iced::widget::operation::focus(iced::widget::Id::new("main_input"))
        }

        Message::StartCreateWithDescription => {
            app.creating_with_desc = true;

            app.active_focus = Focus::AddTaskInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::AddTaskInput;
            }

            if app.input_value.text().trim().is_empty() {
                iced::widget::operation::focus(iced::widget::Id::new("main_input"))
            } else {
                iced::widget::operation::focus(iced::widget::Id::new("description_input"))
            }
        }

        Message::SubmitTask => handle_submit(app),

        Message::EditTaskStart(index) => {
            let data = app
                .get_task_at_index(index)
                .map(|t| (t.uid.clone(), t.to_smart_string(), t.description.clone()));
            if let Some((task_uid, task_summary, task_description)) = data {
                app.input_value = text_editor::Content::with_text(&task_summary);
                app.input_value
                    .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));

                app.description_value = text_editor::Content::with_text(&task_description);
                app.editing_uid = Some(task_uid.clone());
                app.selected_uid = Some(task_uid);

                app.active_focus = Focus::AddTaskInput;
                if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                    *focus = Focus::AddTaskInput;
                }

                return iced::widget::operation::focus(iced::widget::Id::new("main_input"));
            }
            Task::none()
        }

        Message::CancelEdit => {
            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.creating_child_of = None;
            app.child_lock_active = false;
            app.creating_with_desc = false;
            common::scroll_to_selected(app, true)
        }

        Message::ToggleTaskShift(uid) => {
            dispatch_and_select_next_row(app, AppIntent::ToggleTaskShift { uid: uid.clone() }, uid);
            Task::none()
        }

        Message::ToggleTaskShiftSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                dispatch_and_select_next_row(
                    app,
                    AppIntent::ToggleTaskShift { uid: uid.clone() },
                    uid,
                );
            }
            Task::none()
        }

        Message::ToggleTask(index, _) => {
            let data = app.get_task_at_index(index).map(|t| {
                (
                    t.uid.clone(),
                    t.etag == "pending_refresh",
                    t.status.is_done(),
                )
            });
            if let Some((uid, is_pending, is_done)) = data {
                if is_pending {
                    return Task::none();
                }
                if is_done {
                    dispatch_and_maintain_selection(
                        app,
                        AppIntent::ToggleTask { uid: uid.clone() },
                        &uid,
                    );
                } else {
                    dispatch_and_select_next_row(
                        app,
                        AppIntent::ToggleTask { uid: uid.clone() },
                        uid,
                    );
                }
            }
            Task::none()
        }

        Message::ToggleDoneGroup(key) => {
            common::dispatch_intent(app, crate::model::AppIntent::ToggleDoneGroup { key });
            common::save_config(app);
            Task::none()
        }
        Message::ToggleTreeCollapse(uid) => {
            common::dispatch_intent(app, AppIntent::ToggleTreeCollapse { uid });
            Task::none()
        }
        Message::ToggleHelpSection(title) => {
            if app.help_expanded_sections.contains(&title) {
                app.help_expanded_sections.remove(&title);
            } else {
                app.help_expanded_sections.insert(title);
            }
            Task::none()
        }

        Message::DeleteTask(index) => {
            if let Some(uid) = app.get_task_at_index(index).map(|t| t.uid.clone()) {
                app.selected_uid = Some(uid.clone());
                dispatch_and_maintain_selection(
                    app,
                    AppIntent::DeleteTask { uid: uid.clone() },
                    &uid,
                );
            }
            Task::none()
        }

        Message::EditSelectedDescription => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return Task::batch(vec![
                    handle(app, Message::EditTaskStart(idx)),
                    iced::widget::operation::focus(iced::widget::Id::new("description_input")),
                ]);
            }
            Task::none()
        }

        Message::PromoteSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                common::dispatch_intent(app, AppIntent::RemoveParent { uid });
            }
            Task::none()
        }

        Message::DemoteSelected => {
            if let Some(uid) = app.selected_uid.clone()
                && let Some(idx) = app.find_task_index_by_uid(&uid)
                && idx > 0
            {
                let parent_candidate_uid = app.get_task_at_index(idx - 1).unwrap().uid.clone();
                if parent_candidate_uid != uid {
                    app.yanked_uid = Some(parent_candidate_uid);
                    return handle(app, Message::MakeChild(uid));
                }
            }
            Task::none()
        }

        Message::YankSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                app.yanked_uid = Some(uid.clone());
                let mut tasks = vec![common::scroll_to_selected(app, false)];
                if let Some(idx) = app.find_task_index_by_uid(&uid)
                    && let Some(t) = app.get_task_at_index(idx)
                {
                    let text = if t.description.is_empty() {
                        t.to_smart_string()
                    } else {
                        format!("{}\n\n{}", t.to_smart_string(), t.description)
                    };
                    tasks.push(iced::clipboard::write(text));
                }
                return Task::batch(tasks);
            }
            Task::none()
        }

        Message::KeyboardLinkChild => {
            if let Some(parent_uid) = app.yanked_uid.clone()
                && let Some(selected_uid) = app.selected_uid.clone()
                && parent_uid != selected_uid
            {
                dispatch_and_maintain_selection(
                    app,
                    AppIntent::MakeChild {
                        uid: selected_uid.clone(),
                        parent_uid,
                    },
                    &selected_uid,
                );
            }
            Task::none()
        }

        Message::KeyboardCreateChild => {
            if let Some(selected_uid) = app.selected_uid.clone() {
                return handle(app, Message::StartCreateChild(selected_uid));
            }
            Task::none()
        }

        Message::KeyboardAddDependency => {
            if let Some(blocker_uid) = app.yanked_uid.clone()
                && let Some(uid) = app.selected_uid.clone()
            {
                common::dispatch_intent(app, AppIntent::AddDependency { uid, blocker_uid });
            }
            Task::none()
        }

        Message::KeyboardAddRelation => {
            if let Some(related_uid) = app.yanked_uid.clone()
                && let Some(uid) = app.selected_uid.clone()
            {
                common::dispatch_intent(app, AppIntent::AddRelatedTo { uid, related_uid });
            }
            Task::none()
        }

        Message::KeyboardOpenContextMenu => {
            if let Some(uid) = app.selected_uid.clone() {
                return crate::gui::update::view::handle(app, Message::OpenContextMenu(uid, true));
            }
            Task::none()
        }

        Message::KeyboardToggleDetails => {
            if let Some(uid) = app.selected_uid.clone() {
                return crate::gui::update::view::handle(app, Message::ToggleDetails(uid));
            }
            Task::none()
        }

        Message::KeyboardDuplicateTask => {
            if let Some(selected) = app.selected_uid.clone() {
                return handle(app, Message::DuplicateTask(selected));
            }
            Task::none()
        }

        Message::KeyboardToggleTreeCollapse => {
            if app.active_focus == crate::gui::state::Focus::Sidebar {
                match app.sidebar_mode {
                    SidebarMode::Calendars => {}
                    SidebarMode::Categories => {
                        let cats = &app.cached_categories;
                        if let Some(cat) = cats.get(app.sidebar_selection_idx)
                            && cat.has_children
                        {
                            return crate::gui::update::view::handle(
                                app,
                                Message::ToggleTagCollapse(cat.full_key.clone()),
                            );
                        }
                    }
                    SidebarMode::Locations => {
                        let locs = &app.cached_locations;
                        if let Some(loc) = locs.get(app.sidebar_selection_idx)
                            && loc.has_children
                        {
                            return crate::gui::update::view::handle(
                                app,
                                Message::ToggleLocationCollapse(loc.full_key.clone()),
                            );
                        }
                    }
                    SidebarMode::Goals => {}
                }
                return Task::none();
            }

            if let Some(uid) = app.selected_uid.clone() {
                return handle(app, Message::ToggleTreeCollapse(uid));
            }
            Task::none()
        }

        Message::DuplicateTask(uid) => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            common::dispatch_intent(app, AppIntent::DuplicateTaskTree { uid });
            Task::none()
        }

        Message::KeyboardOpenLocations => {
            if let Some(uid) = app.selected_uid.clone() {
                let count = app.store.count_tree_locations(&uid);
                if count > 1 {
                    return crate::gui::update::view::handle(app, Message::OpenLocations(uid));
                } else if count == 1 {
                    return crate::gui::update::view::handle(app, Message::OpenCoordinates(uid));
                }
            }
            Task::none()
        }

        Message::KeyboardOpenUrl => {
            if let Some(uid) = app.selected_uid.clone()
                && let Some(task) = app.store.get_task_ref(&uid)
                && let Some(url) = &task.url
            {
                return crate::gui::update::view::handle(app, Message::OpenUrl(url.clone()));
            }
            Task::none()
        }

        Message::KeyboardDeleteTaskTree => {
            if let Some(uid) = app.selected_uid.clone() {
                return handle(app, Message::DeleteTaskTree(uid));
            }
            Task::none()
        }

        Message::DeleteTaskTree(uid) => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            dispatch_and_maintain_selection(
                app,
                AppIntent::DeleteTaskTree { uid: uid.clone() },
                &uid,
            );
            Task::none()
        }

        Message::ToggleActiveSelected => {
            if let Some(uid) = app.selected_uid.clone()
                && let Some(idx) = app.find_task_index_by_uid(&uid)
                && let Some(t) = app.get_task_at_index(idx)
            {
                if t.status == crate::model::TaskStatus::InProcess {
                    common::dispatch_intent(app, AppIntent::PauseTask { uid });
                } else {
                    common::dispatch_intent(app, AppIntent::StartTask { uid });
                }
            }
            Task::none()
        }

        Message::StopSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                common::dispatch_intent(app, AppIntent::StopTask { uid });
            }
            Task::none()
        }

        Message::CancelSelected => {
            if let Some(uid) = app.selected_uid.clone() {
                dispatch_and_select_next_row(app, AppIntent::CancelTask { uid: uid.clone() }, uid);
            }
            Task::none()
        }

        Message::ChangePrioritySelected(delta) => {
            if let Some(uid) = app.selected_uid.clone() {
                common::dispatch_intent(app, AppIntent::ChangePriority { uid, delta });
            }
            Task::none()
        }

        Message::ChangePriority(index, delta) => {
            if let Some(uid) = app.get_task_at_index(index).map(|t| t.uid.clone()) {
                app.selected_uid = Some(uid.clone());
                common::dispatch_intent(app, AppIntent::ChangePriority { uid, delta });
            }
            Task::none()
        }

        Message::SetTaskStatus(index, new_status) => {
            if let Some(uid) = app.get_task_at_index(index).map(|t| t.uid.clone()) {
                app.selected_uid = Some(uid.clone());
                if new_status == crate::model::TaskStatus::Cancelled {
                    dispatch_and_select_next_row(
                        app,
                        AppIntent::CancelTask { uid: uid.clone() },
                        uid,
                    );
                } else if new_status.is_done() {
                    dispatch_and_select_next_row(
                        app,
                        AppIntent::ToggleTask { uid: uid.clone() },
                        uid,
                    );
                }
            }
            Task::none()
        }

        Message::MoveTask(uid, target_href) => {
            app.selected_uid = Some(uid.clone());
            app.moving_task_uid = None;
            dispatch_and_maintain_selection(
                app,
                AppIntent::MoveTask {
                    uid: uid.clone(),
                    target_href,
                },
                &uid,
            );
            Task::none()
        }

        Message::StartTask(uid) => {
            common::dispatch_intent(app, AppIntent::StartTask { uid });
            Task::none()
        }

        Message::PauseTask(uid) => {
            common::dispatch_intent(app, AppIntent::PauseTask { uid });
            Task::none()
        }

        Message::StopTask(uid) => {
            common::dispatch_intent(app, AppIntent::StopTask { uid });
            Task::none()
        }

        Message::YankTask(uid) => {
            app.yanked_uid = Some(uid.clone());
            app.selected_uid = Some(uid.clone());
            let mut tasks = vec![common::scroll_to_selected(app, false)];
            if let Some(idx) = app.find_task_index_by_uid(&uid)
                && let Some(t) = app.get_task_at_index(idx)
            {
                let text = if t.description.is_empty() {
                    t.to_smart_string()
                } else {
                    format!("{}\n\n{}", t.to_smart_string(), t.description)
                };
                tasks.push(iced::clipboard::write(text));
            }
            Task::batch(tasks)
        }

        Message::CopyToClipboard(text) => Task::batch(vec![iced::clipboard::write(text)]),

        Message::TogglePin(uid) => {
            common::dispatch_intent(app, AppIntent::TogglePin { uid });
            Task::none()
        }

        Message::ExtractSubtasks(uid) => {
            if let Some((parent, _)) = app.store.get_task_mut(&uid) {
                let desc_text = parent.description.clone();
                let (clean_desc, extracted) =
                    crate::model::extractor::extract_markdown_tasks(&desc_text);

                if !extracted.is_empty() {
                    parent.description = clean_desc;
                    parent.sequence += 1;
                    let parent_copy = parent.clone();
                    let target_href = parent_copy.calendar_href.clone();

                    let config = &app.core_config;
                    let def_time =
                        chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
                            .ok();

                    let mut actions = vec![crate::journal::Action::Update(parent_copy)];

                    for ext in extracted {
                        let mut sub =
                            crate::model::Task::new(&ext.raw_text, &app.tag_aliases, def_time);
                        sub.uid = ext.uid;
                        if !ext.description.is_empty() {
                            if sub.description.is_empty() {
                                sub.description = ext.description;
                            } else {
                                sub.description
                                    .push_str(&format!("\n\n{}", ext.description));
                            }
                        }
                        if ext.is_completed {
                            sub.status = crate::model::TaskStatus::Completed;
                            sub.set_completion_date(Some(chrono::Utc::now()));
                        }
                        sub.parent_uid = Some(ext.parent_uid.unwrap_or(uid.clone()));
                        sub.dependencies = ext.dependencies;
                        sub.calendar_href = target_href.clone();

                        app.store.add_task(sub.clone());
                        actions.push(crate::journal::Action::Create(sub));
                    }

                    common::refresh_filtered_tasks(app);

                    if let Some(tx) = &app.bg_tx {
                        let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
                    }
                }
            }
            Task::none()
        }

        Message::ClearYank => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            Task::none()
        }

        Message::EscCaptured => {
            app.active_focus = Focus::MainList;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::MainList;
            }
            if app.editing_uid.is_some()
                || app.creating_child_of.is_some()
                || app.creating_with_desc
            {
                app.input_value = text_editor::Content::new();
                app.description_value = text_editor::Content::new();
                app.editing_uid = None;
                app.creating_child_of = None;
                app.creating_with_desc = false;
            }
            common::scroll_to_selected_delayed(app, false)
        }

        Message::EscapePressed => {
            app.active_focus = Focus::MainList;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::MainList;
            }
            let mut needs_refresh = false;
            let mut captured_action = false;

            if app.moving_task_uid.is_some() {
                app.moving_task_uid = None;
                captured_action = true;
            } else if app.ics_import_dialog_open {
                app.ics_import_dialog_open = false;
                app.ics_import_file_path = None;
                app.ics_import_content = None;
                app.ics_import_selected_calendar = None;
                app.ics_import_task_count = None;
                captured_action = true;
            } else if app.editing_uid.is_some()
                || app.creating_child_of.is_some()
                || app.creating_with_desc
            {
                app.input_value = text_editor::Content::new();
                app.description_value = text_editor::Content::new();
                app.editing_uid = None;
                app.creating_child_of = None;
                app.child_lock_active = false;
                app.creating_with_desc = false;
                captured_action = true;
            } else if app.yanked_uid.is_some() {
                app.yanked_uid = None;
                app.yank_lock_active = false;
                captured_action = true;
            } else if !app.input_value.text().is_empty() {
                app.input_value = text_editor::Content::new();
                captured_action = true;
            } else if !app.search_value.text().is_empty() {
                app.search_value = text_editor::Content::new();
                needs_refresh = true;
                captured_action = true;
            } else if !app.session.selected_categories.is_empty() {
                app.session.selected_categories.clear();
                needs_refresh = true;
                captured_action = true;
            } else if !app.session.selected_locations.is_empty() {
                app.session.selected_locations.clear();
                needs_refresh = true;
                captured_action = true;
            }

            if needs_refresh {
                common::refresh_filtered_tasks(app);
            }

            if captured_action || app.editing_uid.is_none() {
                return common::scroll_to_selected_delayed(app, true);
            }

            Task::none()
        }

        Message::MakeChild(target_uid) => {
            if let Some(parent_uid) = app.yanked_uid.clone()
                && let Some(_orig) = app.store.get_task_ref(&target_uid)
            {
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                dispatch_and_maintain_selection(
                    app,
                    AppIntent::MakeChild {
                        uid: target_uid.clone(),
                        parent_uid,
                    },
                    &target_uid,
                );
            }
            Task::none()
        }

        Message::RemoveParent(child_uid) => {
            dispatch_and_maintain_selection(
                app,
                AppIntent::RemoveParent {
                    uid: child_uid.clone(),
                },
                &child_uid,
            );
            Task::none()
        }

        Message::RemoveDependency(uid, blocker_uid) => {
            dispatch_and_maintain_selection(
                app,
                AppIntent::RemoveDependency {
                    uid: uid.clone(),
                    blocker_uid,
                },
                &uid,
            );
            Task::none()
        }

        Message::RemoveRelatedTo(uid, related_uid) => {
            dispatch_and_maintain_selection(
                app,
                AppIntent::RemoveRelatedTo {
                    uid: uid.clone(),
                    related_uid,
                },
                &uid,
            );
            Task::none()
        }

        Message::AddDependency(target_uid) => {
            if let Some(blocker_uid) = app.yanked_uid.clone() {
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                dispatch_and_maintain_selection(
                    app,
                    AppIntent::AddDependency {
                        uid: target_uid.clone(),
                        blocker_uid,
                    },
                    &target_uid,
                );
            }
            Task::none()
        }

        Message::AddRelatedTo(target_uid) => {
            if let Some(related_uid) = app.yanked_uid.clone() {
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                dispatch_and_maintain_selection(
                    app,
                    AppIntent::AddRelatedTo {
                        uid: target_uid.clone(),
                        related_uid,
                    },
                    &target_uid,
                );
            }
            Task::none()
        }

        Message::StartMoveTask(uid) => {
            app.moving_task_uid = Some(uid);
            app.move_target_idx = 0;
            app.active_context_menu = None; // Hide context menu if open
            Task::none()
        }

        Message::CancelMoveTask => {
            app.moving_task_uid = None;
            Task::none()
        }

        Message::MigrateLocalTo(source_href, target_href) => {
            if let Some(local_map) = app.store.calendars.get(&source_href) {
                let tasks_to_move: Vec<_> = local_map.values().cloned().collect();

                if tasks_to_move.is_empty() {
                    return Task::none();
                }
                app.loading = true;

                if let Some(client) = &app.client {
                    return Task::perform(
                        crate::gui::async_ops::async_migrate_wrapper(
                            client.clone(),
                            tasks_to_move,
                            target_href,
                        ),
                        |res| Message::MigrationComplete(res.map_err(|e| e.to_string())),
                    );
                } else {
                    app.error_msg = Some(rust_i18n::t!("error_cannot_export_offline").to_string());
                    app.loading = false;
                }
            }
            Task::none()
        }

        Message::SnoozeCustomInput(val) => {
            app.snooze_custom_input = val;
            Task::none()
        }

        Message::SnoozeCustomSubmit(t_uid, a_uid) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            let mins = if let Ok(n) = app.snooze_custom_input.parse::<u32>() {
                n
            } else {
                crate::model::parser::parse_duration(&app.snooze_custom_input).unwrap_or(10)
            };
            app.snooze_custom_input.clear();

            handle(app, Message::SnoozeAlarm(t_uid, a_uid, mins))
        }

        Message::CompleteTaskFromAlarm(t_uid, a_uid) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            if let Some(idx) = app.find_task_index_by_uid(&t_uid)
                && let Some(task) = app.get_task_at_index(idx)
                && !task.status.is_done()
            {
                if task.etag == "pending_refresh" {
                    return Task::none();
                }
                dispatch_and_select_next_row(
                    app,
                    AppIntent::ToggleTask { uid: t_uid.clone() },
                    t_uid.clone(),
                );
            }
            Task::none()
        }

        Message::CancelTaskFromAlarm(t_uid, a_uid) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            if app.find_task_index_by_uid(&t_uid).is_some() {
                dispatch_and_select_next_row(
                    app,
                    AppIntent::CancelTask { uid: t_uid.clone() },
                    t_uid.clone(),
                );
            }
            Task::none()
        }

        Message::SnoozeAlarm(t_uid, a_uid, mins) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid)
                && task.handle_snooze(&a_uid, mins)
            {
                task.sequence += 1;
                let cloned = task.clone();
                common::refresh_filtered_tasks(app);
                if let Some(tx) = &app.bg_tx {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                        crate::journal::Action::Update(cloned),
                    ]));
                }
            }
            Task::none()
        }

        Message::DismissAlarm(t_uid, a_uid) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid)
                && task.handle_dismiss(&a_uid)
            {
                task.sequence += 1;
                let cloned = task.clone();
                common::refresh_filtered_tasks(app);
                if let Some(tx) = &app.bg_tx {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                        crate::journal::Action::Update(cloned),
                    ]));
                }
            }
            Task::none()
        }

        Message::StartAddSession(uid) => {
            app.adding_session_uid = Some(uid.clone());
            app.session_input = iced::widget::text_editor::Content::new();
            app.expanded_tasks.insert(uid.clone());

            app.active_focus = Focus::AddTaskInput;
            if let Ok(mut focus) = ACTIVE_FOCUS.write() {
                *focus = Focus::AddTaskInput;
            }

            iced::widget::operation::focus(iced::widget::Id::from(format!("session_input_{}", uid)))
        }

        Message::SessionInputChanged(action) => {
            if let iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Enter) =
                action
            {
                return handle(app, Message::SubmitSession);
            }
            if let iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert('\t'),
            ) = action
            {
                return Task::none();
            }
            app.session_input.perform(action);
            Task::none()
        }

        Message::CancelAddSession => {
            app.adding_session_uid = None;
            app.session_input = iced::widget::text_editor::Content::new();
            Task::none()
        }

        Message::SubmitSession => {
            if let Some(uid) = app.adding_session_uid.clone() {
                let input_text = app.session_input.text();

                if let Some(session) = crate::model::parser::parse_session_input(&input_text)
                    && let Some((t_mut, _)) = app.store.get_task_mut(&uid)
                {
                    t_mut.add_session(session);
                    t_mut.sequence += 1;
                    let cloned = t_mut.clone();

                    app.adding_session_uid = None;
                    app.session_input = iced::widget::text_editor::Content::new();
                    common::refresh_filtered_tasks(app);

                    if let Some(tx) = &app.bg_tx {
                        let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                            crate::journal::Action::Update(cloned),
                        ]));
                    }
                }
            }
            Task::none()
        }

        Message::DeleteSession(uid, idx) => {
            if let Some((t_mut, _)) = app.store.get_task_mut(&uid) {
                t_mut.remove_session(idx);
                t_mut.sequence += 1;
                let cloned = t_mut.clone();
                common::refresh_filtered_tasks(app);

                if let Some(tx) = &app.bg_tx {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(vec![
                        crate::journal::Action::Update(cloned),
                    ]));
                }
            }
            Task::none()
        }

        Message::ToggleShowAllSessions(uid) => {
            app.expanded_tasks.insert(uid.clone());
            if app.show_all_sessions.contains(&uid) {
                app.show_all_sessions.remove(&uid);
            } else {
                app.show_all_sessions.insert(uid);
            }
            Task::none()
        }

        Message::KeyboardAddSession => {
            if let Some(selected_uid) = app.selected_uid.clone() {
                return handle(app, Message::StartAddSession(selected_uid));
            }
            Task::none()
        }

        Message::KeyboardToggleSessions => {
            if let Some(selected_uid) = app.selected_uid.clone() {
                return handle(app, Message::ToggleDetails(selected_uid));
            }
            Task::none()
        }

        _ => Task::none(),
    }
}

fn handle_submit(app: &mut GuiApp) -> Task<Message> {
    use crate::gui::update::common::{
        apply_alias_retroactively, refresh_filtered_tasks, save_config,
    };
    use crate::model::{Task as TodoTask, extract_inline_aliases};

    let raw_text = app.input_value.text();
    let text_to_submit = raw_text.trim().to_string();

    if text_to_submit.is_empty() {
        app.input_value = text_editor::Content::new();
        return Task::none();
    }

    let (clean_input_1, new_goals) = crate::model::parser::extract_inline_goals(&text_to_submit);
    let (clean_input, new_aliases) = extract_inline_aliases(&clean_input_1);

    let mut retroactive_sync_batch = Vec::new();

    let mut config_changed = false;
    if !new_goals.is_empty() {
        for (key, goal) in new_goals {
            app.core_config.goals.insert(key, goal);
        }
        config_changed = true;
    }

    if !new_aliases.is_empty() {
        for (key, tags) in new_aliases {
            app.tag_aliases.insert(key.clone(), tags.clone());
            retroactive_sync_batch.extend(apply_alias_retroactively(app, &key, &tags));
        }
        config_changed = true;
    }

    if config_changed {
        save_config(app);
    }

    let trimmed = clean_input.trim();
    if trimmed.is_empty()
        || (!trimmed.contains(' ')
            && (trimmed.contains(":=") || trimmed.to_lowercase().starts_with("loc:")))
            && app.editing_uid.is_none()
    {
        app.input_value = text_editor::Content::new();
        refresh_filtered_tasks(app);
        if !retroactive_sync_batch.is_empty() {
            let actions: Vec<_> = retroactive_sync_batch
                .into_iter()
                .map(crate::journal::Action::Update)
                .collect();
            if let Some(tx) = &app.bg_tx {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
            }
        }
        return Task::none();
    }

    if clean_input.starts_with('#')
        && !clean_input.trim().contains(' ')
        && app.editing_uid.is_none()
    {
        let tag = clean_input.trim().trim_start_matches('#').to_string();
        if !tag.is_empty() && !text_to_submit.contains(":=") {
            app.sidebar_mode = SidebarMode::Categories;
            app.session.selected_categories.clear();
            app.session.selected_categories.push(tag);
            app.input_value = text_editor::Content::new();
            refresh_filtered_tasks(app);

            if !retroactive_sync_batch.is_empty() {
                let mut actions = Vec::new();
                for t in retroactive_sync_batch {
                    actions.push(crate::journal::Action::Update(t));
                }
                if !actions.is_empty()
                    && let Some(tx) = &app.bg_tx
                {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
                }
            }
            return Task::none();
        }
    }
    let is_loc_jump = clean_input.starts_with("@@") || clean_input.starts_with("loc:");
    if is_loc_jump && !clean_input.trim().contains(' ') && app.editing_uid.is_none() {
        let loc = crate::model::parser::strip_quotes(
            clean_input
                .trim_start_matches("@@")
                .trim_start_matches("loc:"),
        );
        if !loc.is_empty() {
            app.sidebar_mode = SidebarMode::Locations;
            app.session.selected_locations.clear();
            app.session.selected_locations.push(loc);
            app.input_value = text_editor::Content::new();
            refresh_filtered_tasks(app);

            if !retroactive_sync_batch.is_empty() {
                let mut actions = Vec::new();
                for t in retroactive_sync_batch {
                    actions.push(crate::journal::Action::Update(t));
                }
                if !actions.is_empty()
                    && let Some(tx) = &app.bg_tx
                {
                    let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
                }
            }
            return Task::none();
        }
    }

    let config_time = NaiveTime::parse_from_str(&app.default_reminder_time, "%H:%M").ok();

    let desc_text = app.description_value.text();
    let (cleaned_desc, extracted_subtasks) =
        crate::model::extractor::extract_markdown_tasks(&desc_text);

    if let Some(edit_uid) = &app.editing_uid {
        if let Some((task, _)) = app.store.get_task_mut(edit_uid) {
            task.description = desc_text;
            task.apply_smart_input(&clean_input, &app.tag_aliases, config_time);
            task.sequence += 1;
            let task_copy = task.clone();

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.selected_uid = Some(task_copy.uid.clone());

            refresh_filtered_tasks(app);

            let mut actions = Vec::new();
            actions.push(crate::journal::Action::Update(task_copy));
            for t in retroactive_sync_batch {
                actions.push(crate::journal::Action::Update(t));
            }

            if !actions.is_empty()
                && let Some(tx) = &app.bg_tx
            {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
            }

            return Task::none();
        }
    } else if !clean_input.is_empty() {
        let mut new_task = TodoTask::new(&clean_input, &app.tag_aliases, config_time);

        if !cleaned_desc.is_empty() {
            if new_task.description.is_empty() {
                new_task.description = cleaned_desc;
            } else {
                new_task
                    .description
                    .push_str(&format!("\n\n{}", cleaned_desc));
            }
        }

        if let Some(parent) = &app.creating_child_of {
            new_task.parent_uid = Some(parent.clone());
            if !app.child_lock_active {
                app.creating_child_of = None;
            }
        }

        let target_href = app
            .active_cal_href
            .clone()
            .or_else(|| app.calendars.first().map(|c| c.href.clone()))
            .unwrap_or_default();

        if !target_href.is_empty() {
            new_task.calendar_href = target_href.clone();
            let parent_uid = new_task.uid.clone();

            app.store.add_task(new_task.clone());
            app.task_ids
                .entry(new_task.uid.clone())
                .or_insert_with(iced::widget::Id::unique);

            let mut tasks_to_create = vec![new_task];

            for ext in extracted_subtasks {
                let mut sub = TodoTask::new(&ext.raw_text, &app.tag_aliases, config_time);
                sub.uid = ext.uid;
                if !ext.description.is_empty() {
                    if sub.description.is_empty() {
                        sub.description = ext.description;
                    } else {
                        sub.description
                            .push_str(&format!("\n\n{}", ext.description));
                    }
                }
                if ext.is_completed {
                    sub.status = crate::model::TaskStatus::Completed;
                    sub.set_completion_date(Some(chrono::Utc::now()));
                }

                let actual_parent = ext.parent_uid.unwrap_or(parent_uid.clone());
                sub.parent_uid = Some(actual_parent);
                sub.dependencies = ext.dependencies;
                sub.calendar_href = target_href.clone();

                app.store.add_task(sub.clone());
                tasks_to_create.push(sub);
            }

            app.selected_uid = Some(parent_uid.clone());
            refresh_filtered_tasks(app);

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.creating_with_desc = false;

            let scroll_cmd = common::scroll_to_selected_delayed(app, false);
            let focus_cmd = iced::widget::operation::focus(iced::widget::Id::new("main_input"));

            let mut actions = Vec::new();
            for t in tasks_to_create {
                actions.push(crate::journal::Action::Create(t));
            }

            if !retroactive_sync_batch.is_empty() {
                for t in retroactive_sync_batch {
                    actions.push(crate::journal::Action::Update(t));
                }
            }

            if !actions.is_empty()
                && let Some(tx) = &app.bg_tx
            {
                let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
            }

            return Task::batch(vec![scroll_cmd, focus_cmd]);
        }
    }

    if !retroactive_sync_batch.is_empty() {
        let mut actions = Vec::new();
        for t in retroactive_sync_batch {
            actions.push(crate::journal::Action::Update(t));
        }
        if !actions.is_empty()
            && let Some(tx) = &app.bg_tx
        {
            let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
        }
    }

    Task::none()
}
