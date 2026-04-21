// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/update/tasks.rs
//
// Simplified GUI task handlers: always dispatch controller actions via
// `async_controller_dispatch` (which accepts the `Option<RustyClient>` and
// routes offline vs online automatically). Removed offline helper functions
// and unused imports to silence warnings after the TaskController refactor.

use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{GuiApp, SidebarMode};
use crate::gui::update::common::{
    apply_alias_retroactively, refresh_filtered_tasks, save_config, scroll_to_selected,
    scroll_to_selected_delayed,
};
use crate::model::{Task as TodoTask, extract_inline_aliases};
use chrono::NaiveTime;
use iced::Task;
use iced::widget::text_editor;
use std::collections::HashMap;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(action) => {
            if let text_editor::Action::Edit(text_editor::Edit::Enter) = action {
                return handle_submit(app);
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

            iced::widget::operation::focus(iced::widget::Id::new("main_input"))
        }

        Message::StartCreateWithDescription => {
            if !app.input_value.text().trim().is_empty() {
                app.creating_with_desc = true;
                app.description_value = iced::widget::text_editor::Content::new();
                return iced::widget::operation::focus(iced::widget::Id::new("description_input"));
            }
            Task::none()
        }

        Message::SubmitTask => handle_submit(app),

        Message::EditTaskStart(index) => {
            if let Some(task) = app.get_task_at_index(index) {
                let task_uid = task.uid.clone();
                let task_summary = task.to_smart_string();
                let task_description = task.description.clone();

                app.input_value = text_editor::Content::with_text(&task_summary);
                app.input_value
                    .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));

                app.description_value = text_editor::Content::with_text(&task_description);
                app.editing_uid = Some(task_uid.clone());
                app.selected_uid = Some(task_uid);

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
            scroll_to_selected(app, true)
        }

        Message::ToggleTask(index, _) => {
            if let Some(view_task) = app.get_task_at_index(index) {
                let task_uid = view_task.uid.clone();
                app.selected_uid = Some(task_uid.clone());
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Toggle(task_uid),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::ToggleDoneGroup(key) => {
            if app.expanded_done_groups.contains(&key) {
                app.expanded_done_groups.remove(&key);
            } else {
                app.expanded_done_groups.insert(key.clone());
            }
            refresh_filtered_tasks(app);
            Task::none()
        }

        Message::DeleteTask(index) => {
            if let Some(view_task) = app.get_task_at_index(index) {
                let uid = view_task.uid.clone();
                app.selected_uid = Some(uid.clone());
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Delete(uid),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
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
            if let Some(uid) = &app.selected_uid {
                return handle(app, Message::RemoveParent(uid.clone()));
            }
            Task::none()
        }

        Message::DemoteSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
                && idx > 0
            {
                let parent_candidate_uid = app.get_task_at_index(idx - 1).unwrap().uid.clone();
                if parent_candidate_uid != *uid {
                    app.yanked_uid = Some(parent_candidate_uid);
                    return handle(app, Message::MakeChild(uid.clone()));
                }
            }
            Task::none()
        }

        Message::YankSelected => {
            if let Some(uid) = &app.selected_uid {
                app.yanked_uid = Some(uid.clone());
                let mut tasks = vec![scroll_to_selected(app, false)];
                if let Some(idx) = app.find_task_index_by_uid(uid)
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
            if let Some(parent_uid) = &app.yanked_uid
                && let Some(selected_uid) = &app.selected_uid
                && parent_uid != selected_uid
            {
                return handle(app, Message::MakeChild(selected_uid.clone()));
            }
            Task::none()
        }

        Message::KeyboardCreateChild => {
            if let Some(selected_uid) = &app.selected_uid {
                return handle(app, Message::StartCreateChild(selected_uid.clone()));
            }
            Task::none()
        }

        Message::KeyboardAddDependency => {
            if let Some(_yanked) = &app.yanked_uid
                && let Some(selected) = &app.selected_uid
            {
                return handle(app, Message::AddDependency(selected.clone()));
            }
            Task::none()
        }

        Message::KeyboardAddRelation => {
            if let Some(_yanked) = &app.yanked_uid
                && let Some(selected) = &app.selected_uid
            {
                return handle(app, Message::AddRelatedTo(selected.clone()));
            }
            Task::none()
        }

        Message::KeyboardDuplicateTask => {
            if let Some(selected) = &app.selected_uid {
                return handle(app, Message::DuplicateTask(selected.clone()));
            }
            Task::none()
        }

        Message::DuplicateTask(uid) => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            Task::perform(
                async_controller_dispatch(
                    app.ctx.clone(),
                    app.client.clone(),
                    app.store.clone(),
                    ControllerAction::DuplicateTree(uid),
                ),
                |res| Message::ControllerActionComplete(Box::new(res)),
            )
        }

        Message::KeyboardDeleteTaskTree => {
            if let Some(uid) = &app.selected_uid {
                return handle(app, Message::DeleteTaskTree(uid.clone()));
            }
            Task::none()
        }

        Message::DeleteTaskTree(uid) => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            Task::perform(
                async_controller_dispatch(
                    app.ctx.clone(),
                    app.client.clone(),
                    app.store.clone(),
                    ControllerAction::DeleteTree(uid),
                ),
                |res| Message::ControllerActionComplete(Box::new(res)),
            )
        }

        Message::ToggleActiveSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
                && let Some(t) = app.get_task_at_index(idx)
            {
                if t.status == crate::model::TaskStatus::InProcess {
                    return handle(app, Message::PauseTask(uid.clone()));
                } else {
                    return handle(app, Message::StartTask(uid.clone()));
                }
            }
            Task::none()
        }

        Message::StopSelected => {
            if let Some(uid) = &app.selected_uid {
                return handle(app, Message::StopTask(uid.clone()));
            }
            Task::none()
        }

        Message::CancelSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return handle(
                    app,
                    Message::SetTaskStatus(idx, crate::model::TaskStatus::Cancelled),
                );
            }
            Task::none()
        }

        Message::ChangePrioritySelected(delta) => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.find_task_index_by_uid(uid)
            {
                return handle(app, Message::ChangePriority(idx, delta));
            }
            Task::none()
        }

        Message::ChangePriority(index, delta) => {
            if let Some(view_task) = app.get_task_at_index(index) {
                let task_uid = view_task.uid.clone();
                app.selected_uid = Some(task_uid.clone());
                if let Some(updated) =
                    app.store
                        .change_priority(&task_uid, delta, app.default_priority)
                {
                    refresh_filtered_tasks(app);
                    return Task::perform(
                        async_controller_dispatch(
                            app.ctx.clone(),
                            app.client.clone(), // pass Option<RustyClient> directly
                            app.store.clone(),
                            ControllerAction::Update(updated),
                        ),
                        |res| Message::ControllerActionComplete(Box::new(res)),
                    );
                }
            }
            Task::none()
        }

        Message::SetTaskStatus(index, new_status) => {
            if let Some(view_task) = app.get_task_at_index(index) {
                let task_uid = view_task.uid.clone();
                let mut updated = view_task.clone();
                updated.status = new_status;
                app.selected_uid = Some(task_uid.clone());
                // Delegate to controller for persistence/recurrence handling.
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::StartTask(uid) => {
            let updated_tasks = app.store.set_status_in_process(&uid);
            if !updated_tasks.is_empty() {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                let mut commands = Vec::new();
                for t in updated_tasks {
                    commands.push(Task::perform(
                        async_controller_dispatch(
                            app.ctx.clone(),
                            app.client.clone(),
                            app.store.clone(),
                            ControllerAction::Update(t),
                        ),
                        |res| Message::ControllerActionComplete(Box::new(res)),
                    ));
                }
                return Task::batch(commands);
            }
            Task::none()
        }

        Message::PauseTask(uid) => {
            let updated_tasks = app.store.pause_task(&uid);
            if !updated_tasks.is_empty() {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                let mut commands = Vec::new();
                for t in updated_tasks {
                    commands.push(Task::perform(
                        async_controller_dispatch(
                            app.ctx.clone(),
                            app.client.clone(),
                            app.store.clone(),
                            ControllerAction::Update(t),
                        ),
                        |res| Message::ControllerActionComplete(Box::new(res)),
                    ));
                }
                return Task::batch(commands);
            }
            Task::none()
        }

        Message::StopTask(uid) => {
            let updated_tasks = app.store.stop_task(&uid);
            if !updated_tasks.is_empty() {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                let mut commands = Vec::new();
                for t in updated_tasks {
                    commands.push(Task::perform(
                        async_controller_dispatch(
                            app.ctx.clone(),
                            app.client.clone(),
                            app.store.clone(),
                            ControllerAction::Update(t),
                        ),
                        |res| Message::ControllerActionComplete(Box::new(res)),
                    ));
                }
                return Task::batch(commands);
            }
            Task::none()
        }

        Message::YankTask(uid) => {
            app.yanked_uid = Some(uid.clone());
            app.selected_uid = Some(uid.clone());
            let mut tasks = vec![scroll_to_selected(app, false)];
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

        Message::ClearYank => {
            app.yanked_uid = None;
            app.yank_lock_active = false;
            Task::none()
        }

        Message::EscCaptured => {
            // If editing/creating child -> cancel and focus back; otherwise soft escape.
            if app.editing_uid.is_some() || app.creating_child_of.is_some() {
                app.input_value = text_editor::Content::new();
                app.description_value = text_editor::Content::new();
                app.editing_uid = None;
                app.creating_child_of = None;
                scroll_to_selected_delayed(app, true)
            } else {
                scroll_to_selected_delayed(app, true)
            }
        }

        Message::EscapePressed => {
            let mut needs_refresh = false;
            let mut captured_action = false;

            if app.moving_task_uid.is_some() {
                app.moving_task_uid = None;
                captured_action = true;
            } else if app.editing_uid.is_some() || app.creating_child_of.is_some() {
                app.input_value = text_editor::Content::new();
                app.description_value = text_editor::Content::new();
                app.editing_uid = None;
                app.creating_child_of = None;
                app.child_lock_active = false;
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
            } else if !app.selected_categories.is_empty() {
                app.selected_categories.clear();
                needs_refresh = true;
                captured_action = true;
            } else if !app.selected_locations.is_empty() {
                app.selected_locations.clear();
                needs_refresh = true;
                captured_action = true;
            }

            if needs_refresh {
                refresh_filtered_tasks(app);
            }

            if captured_action || app.editing_uid.is_none() {
                return scroll_to_selected_delayed(app, true);
            }

            Task::none()
        }

        Message::MakeChild(target_uid) => {
            if let Some(parent_uid) = app.yanked_uid.clone()
                && let Some(orig) = app.store.get_task_ref(&target_uid)
            {
                let mut updated = orig.clone();
                updated.parent_uid = Some(parent_uid.clone());
                app.selected_uid = Some(target_uid.clone());
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::RemoveParent(child_uid) => {
            if let Some(updated) = app.store.set_parent(&child_uid, None) {
                app.selected_uid = Some(child_uid);
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::RemoveDependency(task_uid, dep_uid) => {
            if let Some(updated) = app.store.remove_dependency(&task_uid, &dep_uid) {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::RemoveRelatedTo(task_uid, related_uid) => {
            if let Some(updated) = app.store.remove_related_to(&task_uid, &related_uid) {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::AddDependency(target_uid) => {
            let blocker_opt = app.yanked_uid.clone();

            if let Some(blocker_uid) = blocker_opt
                && let Some(updated) = app.store.add_dependency(&target_uid, blocker_uid.clone())
            {
                app.selected_uid = Some(target_uid);
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::AddRelatedTo(target_uid) => {
            let related_opt = app.yanked_uid.clone();

            if let Some(related_uid) = related_opt
                && let Some(updated) = app.store.add_related_to(&target_uid, related_uid.clone())
            {
                app.selected_uid = Some(target_uid);
                if !app.yank_lock_active {
                    app.yanked_uid = None;
                }
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(updated),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::StartMoveTask(uid) => {
            app.moving_task_uid = Some(uid);
            app.active_context_menu = None; // Hide context menu if open
            Task::none()
        }

        Message::CancelMoveTask => {
            app.moving_task_uid = None;
            Task::none()
        }

        Message::MoveTask(task_uid, target_href) => {
            app.selected_uid = Some(task_uid.clone());
            app.moving_task_uid = None; // Hide modal when applying move
            Task::perform(
                async_controller_dispatch(
                    app.ctx.clone(),
                    app.client.clone(),
                    app.store.clone(),
                    ControllerAction::Move(task_uid, target_href),
                ),
                |res| Message::ControllerActionComplete(Box::new(res)),
            )
        }

        Message::MigrateLocalTo(source_href, target_href) => {
            if let Some(local_map) = app.store.calendars.get(&source_href) {
                let tasks_to_move: Vec<_> = local_map.values().cloned().collect();

                if tasks_to_move.is_empty() {
                    return Task::none();
                }
                app.loading = true;

                // async_migrate_wrapper requires a concrete RustyClient, so only call it when
                // we have an active client. Otherwise surface an error to the user.
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_migrate_wrapper(client.clone(), tasks_to_move, target_href),
                        Message::MigrationComplete,
                    );
                } else {
                    app.error_msg = Some("Cannot export while offline/connecting.".to_string());
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
                return handle(app, Message::ToggleTask(idx, true));
            }
            Task::none()
        }

        Message::CancelTaskFromAlarm(t_uid, a_uid) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            if let Some(idx) = app.find_task_index_by_uid(&t_uid) {
                return handle(
                    app,
                    Message::SetTaskStatus(idx, crate::model::TaskStatus::Cancelled),
                );
            }
            Task::none()
        }

        Message::SnoozeAlarm(t_uid, a_uid, mins) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid)
                && task.handle_snooze(&a_uid, mins)
            {
                let t_clone = task.clone();
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(t_clone),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        Message::DismissAlarm(t_uid, a_uid) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid)
                && task.handle_dismiss(&a_uid)
            {
                let t_clone = task.clone();
                refresh_filtered_tasks(app);
                return Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Update(t_clone),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                );
            }
            Task::none()
        }

        // Start showing the inline add-session input for the given task.
        Message::StartAddSession(uid) => {
            app.adding_session_uid = Some(uid.clone());
            app.session_input = iced::widget::text_editor::Content::new();
            app.expanded_tasks.insert(uid.clone());
            // Focus the inline input (best-effort; may be a no-op depending on widget tree)
            iced::widget::operation::focus(iced::widget::Id::from(format!("session_input_{}", uid)))
        }

        // Update session input buffer (from the TextEditor/widget)
        Message::SessionInputChanged(action) => {
            if let iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Enter) =
                action
            {
                return handle(app, Message::SubmitSession);
            }
            app.session_input.perform(action);
            Task::none()
        }

        Message::CancelAddSession => {
            app.adding_session_uid = None;
            app.session_input = iced::widget::text_editor::Content::new();
            Task::none()
        }

        // Submit the session text: parse and apply to the selected task, then persist via controller.
        Message::SubmitSession => {
            if let Some(uid) = &app.adding_session_uid {
                let input_text = app.session_input.text();
                if let Some(session) = crate::model::parser::parse_session_input(&input_text)
                    && let Some((t_mut, _)) = app.store.get_task_mut(uid)
                {
                    t_mut.add_session(session);
                    let cloned = t_mut.clone();
                    app.adding_session_uid = None;
                    app.session_input = iced::widget::text_editor::Content::new();
                    crate::gui::update::common::refresh_filtered_tasks(app);

                    return Task::perform(
                        crate::gui::async_ops::async_controller_dispatch(
                            app.ctx.clone(),
                            app.client.clone(),
                            app.store.clone(),
                            crate::gui::async_ops::ControllerAction::Update(cloned),
                        ),
                        |res| Message::ControllerActionComplete(Box::new(res)),
                    );
                }
            }
            Task::none()
        }

        // Toggle whether to show all sessions for a task in its expanded details
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
            if let Some(selected_uid) = &app.selected_uid {
                return handle(app, Message::StartAddSession(selected_uid.clone()));
            }
            Task::none()
        }

        Message::KeyboardToggleSessions => {
            if let Some(selected_uid) = &app.selected_uid {
                return handle(app, Message::ToggleDetails(selected_uid.clone()));
            }
            Task::none()
        }

        _ => Task::none(),
    }
}

fn handle_submit(app: &mut GuiApp) -> Task<Message> {
    let raw_text = app.input_value.text();
    let text_to_submit = raw_text.trim().to_string();

    if text_to_submit.is_empty() {
        app.input_value = text_editor::Content::new();
        return Task::none();
    }

    let (clean_input, new_aliases): (String, HashMap<String, Vec<String>>) =
        extract_inline_aliases(&text_to_submit);

    let mut retroactive_sync_batch = Vec::new();

    if !new_aliases.is_empty() {
        for (key, tags) in new_aliases {
            app.tag_aliases.insert(key.clone(), tags.clone());
            if let Some(task_cmd) = apply_alias_retroactively(app, &key, &tags) {
                retroactive_sync_batch.push(task_cmd);
            }
        }
        save_config(app);
    }

    // Logic for Tag/Loc isolated jumps ... (kept identical to your existing file)
    if clean_input.starts_with('#')
        && !clean_input.trim().contains(' ')
        && app.editing_uid.is_none()
    {
        let tag = clean_input.trim().trim_start_matches('#').to_string();
        if !tag.is_empty() && !text_to_submit.contains(":=") {
            app.sidebar_mode = SidebarMode::Categories;
            app.selected_categories.clear();
            app.selected_categories.insert(tag);
            app.input_value = text_editor::Content::new();
            refresh_filtered_tasks(app);
            if !retroactive_sync_batch.is_empty() {
                return Task::batch(retroactive_sync_batch);
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
            app.selected_locations.clear();
            app.selected_locations.insert(loc);
            app.input_value = text_editor::Content::new();
            refresh_filtered_tasks(app);
            if !retroactive_sync_batch.is_empty() {
                return Task::batch(retroactive_sync_batch);
            }
            return Task::none();
        }
    }

    let config_time = NaiveTime::parse_from_str(&app.default_reminder_time, "%H:%M").ok();

    // EXTRACT SUBTASKS
    let desc_text = app.description_value.text();
    let (cleaned_desc, extracted_subtasks) =
        crate::model::extractor::extract_markdown_tasks(&desc_text);

    if let Some(edit_uid) = &app.editing_uid {
        // ONLY EDIT EXISTING - Do not extract to avoid duplication!
        if let Some((task, _)) = app.store.get_task_mut(edit_uid) {
            task.apply_smart_input(&clean_input, &app.tag_aliases, config_time);
            task.description = desc_text; // use RAW description to preserve their markdown checklist
            let task_copy = task.clone();

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.selected_uid = Some(task_copy.uid.clone());

            refresh_filtered_tasks(app);

            let save_cmd = Task::perform(
                async_controller_dispatch(
                    app.ctx.clone(),
                    app.client.clone(),
                    app.store.clone(),
                    ControllerAction::Update(task_copy),
                ),
                |res| Message::ControllerActionComplete(Box::new(res)),
            );
            retroactive_sync_batch.push(save_cmd);
            return Task::batch(retroactive_sync_batch);
        }
    } else if !clean_input.is_empty() {
        // CREATE NEW TASK
        let mut new_task = TodoTask::new(&clean_input, &app.tag_aliases, config_time);
        new_task.description = cleaned_desc; // Use the stripped description!

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

            // Create Subtasks resulting from Markdown Extraction
            for ext in extracted_subtasks {
                let mut sub = TodoTask::new(&ext.raw_text, &app.tag_aliases, config_time);
                sub.uid = ext.uid; // Must use Extractor's UID so dependencies map correctly
                sub.description = ext.description;
                if ext.is_completed {
                    sub.status = crate::model::TaskStatus::Completed;
                    sub.set_completion_date(Some(chrono::Utc::now()));
                }

                let actual_parent = ext.parent_uid.unwrap_or(parent_uid.clone());
                sub.parent_uid = Some(actual_parent);
                sub.dependencies = ext.dependencies;
                sub.calendar_href = target_href.clone();

                app.store.add_task(sub.clone());

                retroactive_sync_batch.push(Task::perform(
                    async_controller_dispatch(
                        app.ctx.clone(),
                        app.client.clone(),
                        app.store.clone(),
                        ControllerAction::Create(sub),
                    ),
                    |res| Message::ControllerActionComplete(Box::new(res)),
                ));
            }

            app.selected_uid = Some(parent_uid);
            refresh_filtered_tasks(app);

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.creating_with_desc = false;

            let scroll_cmd = crate::gui::update::common::scroll_to_selected_delayed(app, false);

            let create_cmd = Task::perform(
                async_controller_dispatch(
                    app.ctx.clone(),
                    app.client.clone(),
                    app.store.clone(),
                    ControllerAction::Create(new_task),
                ),
                |res| Message::ControllerActionComplete(Box::new(res)),
            );

            let focus_cmd = iced::widget::operation::focus(iced::widget::Id::new("main_input"));

            retroactive_sync_batch.push(create_cmd);
            retroactive_sync_batch.push(scroll_cmd);
            retroactive_sync_batch.push(focus_cmd);

            return Task::batch(retroactive_sync_batch);
        }
    }

    if !retroactive_sync_batch.is_empty() {
        return Task::batch(retroactive_sync_batch);
    }
    Task::none()
}
