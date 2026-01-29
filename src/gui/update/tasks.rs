// File: ./src/gui/update/tasks.rs
// Handles task manipulation messages in the GUI.
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{GuiApp, SidebarMode};
use crate::gui::update::common::{
    apply_alias_retroactively, refresh_filtered_tasks, save_config, scroll_to_selected_delayed,
};
use crate::journal::{Action, Journal};
use crate::model::{Task as TodoTask, extract_inline_aliases};
use crate::storage::LocalStorage;
use chrono::{DateTime, NaiveTime, Utc};
use iced::Task;
use iced::widget::text_editor;
use std::collections::HashMap;

// Helper to parse the packed implicit UID
fn parse_implicit_id(alarm_uid: &str) -> Option<(DateTime<Utc>, String)> {
    if alarm_uid.starts_with("implicit_") {
        let parts: Vec<&str> = alarm_uid.split('|').collect();
        if parts.len() >= 3 {
            // parts[0] = "implicit_due:"
            // parts[1] = ISO Date
            // parts[2] = task_uid (redundant but safe)
            if let Ok(dt) = DateTime::parse_from_rfc3339(parts[1]) {
                return Some((dt.with_timezone(&Utc), parts[0].to_string()));
            }
        }
    }
    None
}

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
            if let Some((parent, _)) = app.store.get_task_mut(&parent_uid) {
                for cat in &parent.categories {
                    initial_input.push_str(&format!("#{} ", cat));
                }
            }

            app.input_value = text_editor::Content::with_text(&initial_input);
            Task::none()
        }
        Message::SubmitTask => handle_submit(app),

        Message::EditTaskStart(index) => {
            if let Some(task) = app.tasks.get(index) {
                app.input_value = text_editor::Content::with_text(&task.to_smart_string());
                app.description_value = text_editor::Content::with_text(&task.description);
                app.editing_uid = Some(task.uid.clone());
                app.selected_uid = Some(task.uid.clone());
            }
            Task::none()
        }
        Message::CancelEdit => {
            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.creating_child_of = None;
            Task::none()
        }

        Message::ToggleTask(index, _) => {
            if let Some(view_task) = app.tasks.get(index) {
                let uid = view_task.uid.clone();
                app.selected_uid = Some(uid.clone());
                if let Some(updated) = app.store.toggle_task(&uid) {
                    refresh_filtered_tasks(app);
                    if let Some(client) = &app.client {
                        return Task::perform(
                            async_toggle_wrapper(client.clone(), updated),
                            |res| Message::SyncToggleComplete(Box::new(res)),
                        );
                    } else {
                        handle_offline_update(app, updated);
                    }
                }
            }
            Task::none()
        }
        Message::DeleteTask(index) => {
            if let Some(view_task) = app.tasks.get(index)
                && let Some((deleted_task, _)) = app.store.delete_task(&view_task.uid)
            {
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_delete_wrapper(client.clone(), deleted_task),
                        Message::DeleteComplete,
                    );
                } else {
                    handle_offline_delete(app, deleted_task);
                }
            }
            Task::none()
        }

        // --- SHORTCUT LOGIC ---
        Message::EditSelectedDescription => {
            if let Some(uid) = &app.selected_uid
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
            {
                // Enter edit mode AND focus the description box
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
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
                && idx > 0
            {
                let parent_candidate_uid = app.tasks[idx - 1].uid.clone();
                if parent_candidate_uid != *uid {
                    // Temporarily use yanked_uid to pass the target parent context
                    app.yanked_uid = Some(parent_candidate_uid);
                    return handle(app, Message::MakeChild(uid.clone()));
                }
            }
            Task::none()
        }
        Message::YankSelected => {
            if let Some(uid) = &app.selected_uid {
                app.yanked_uid = Some(uid.clone());
            }
            Task::none()
        }
        Message::KeyboardCreateChild => {
            if let Some(_yanked) = &app.yanked_uid {
                if let Some(selected) = &app.selected_uid {
                    return handle(app, Message::MakeChild(selected.clone()));
                }
            } else if let Some(selected) = &app.selected_uid {
                return handle(app, Message::StartCreateChild(selected.clone()));
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
        Message::ToggleActiveSelected => {
            if let Some(uid) = &app.selected_uid
                && let Some(t) = app.tasks.iter().find(|t| t.uid == *uid)
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
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
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
                && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
            {
                return handle(app, Message::ChangePriority(idx, delta));
            }
            Task::none()
        }

        // --- STANDARD TASK ACTIONS ---
        Message::ChangePriority(index, delta) => {
            if let Some(view_task) = app.tasks.get(index) {
                app.selected_uid = Some(view_task.uid.clone());
                // Pass app.default_priority from the GUI state
                if let Some(updated) =
                    app.store
                        .change_priority(&view_task.uid, delta, app.default_priority)
                {
                    refresh_filtered_tasks(app);
                    if let Some(client) = &app.client {
                        return Task::perform(
                            async_update_wrapper(client.clone(), updated),
                            Message::SyncSaved,
                        );
                    } else {
                        handle_offline_update(app, updated);
                    }
                }
            }
            Task::none()
        }
        Message::SetTaskStatus(index, new_status) => {
            if let Some(view_task) = app.tasks.get(index) {
                app.selected_uid = Some(view_task.uid.clone());
                if let Some(updated) = app.store.set_status(&view_task.uid, new_status) {
                    refresh_filtered_tasks(app);
                    if let Some(client) = &app.client {
                        return Task::perform(
                            async_update_wrapper(client.clone(), updated),
                            Message::SyncSaved,
                        );
                    } else {
                        handle_offline_update(app, updated);
                    }
                }
            }
            Task::none()
        }
        Message::StartTask(uid) => {
            if let Some(updated) = app.store.set_status_in_process(&uid) {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::PauseTask(uid) => {
            if let Some(updated) = app.store.pause_task(&uid) {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::StopTask(uid) => {
            if let Some(updated) = app.store.stop_task(&uid) {
                app.selected_uid = Some(uid.clone());
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::YankTask(uid) => {
            app.yanked_uid = Some(uid);
            Task::none()
        }
        Message::ClearYank => {
            // Explicit button click: ALWAYS just clear the yank, ignore hierarchy
            app.yanked_uid = None;
            Task::none()
        }
        Message::EscapePressed => {
            // Context-aware "Back" logic
            let mut needs_refresh = false;
            let mut captured_action = false;

            // Priority 1: Cancel active editing/creation
            if app.editing_uid.is_some() || app.creating_child_of.is_some() {
                app.input_value = text_editor::Content::new();
                app.description_value = text_editor::Content::new();
                app.editing_uid = None;
                app.creating_child_of = None;
                captured_action = true;
            }
            // Priority 2: Clear Yank
            else if app.yanked_uid.is_some() {
                app.yanked_uid = None;
                captured_action = true;
            }
            // Priority 3: Clear Search
            else if !app.search_value.is_empty() {
                app.search_value.clear();
                needs_refresh = true;
                captured_action = true;
            }
            // Priority 4: Clear Filters
            else if !app.selected_categories.is_empty() {
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

            // ALWAYS shift focus back to the task list on Escape if we were
            // in an input field or just finished clearing a state.
            // This enables j/k navigation immediately.
            if captured_action || app.editing_uid.is_none() {
                return scroll_to_selected_delayed(app);
            }

            Task::none()
        }
        Message::MakeChild(target_uid) => {
            let parent_opt = app.yanked_uid.clone();

            if let Some(parent_uid) = parent_opt
                && let Some(updated) = app.store.set_parent(&target_uid, Some(parent_uid.clone()))
            {
                app.selected_uid = Some(target_uid);
                app.yanked_uid = None;
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::RemoveParent(child_uid) => {
            if let Some(updated) = app.store.set_parent(&child_uid, None) {
                app.selected_uid = Some(child_uid);
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::RemoveDependency(task_uid, dep_uid) => {
            if let Some(updated) = app.store.remove_dependency(&task_uid, &dep_uid) {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::RemoveRelatedTo(task_uid, related_uid) => {
            if let Some(updated) = app.store.remove_related_to(&task_uid, &related_uid) {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::AddDependency(target_uid) => {
            let blocker_opt = app.yanked_uid.clone();

            if let Some(blocker_uid) = blocker_opt
                && let Some(updated) = app.store.add_dependency(&target_uid, blocker_uid.clone())
            {
                app.selected_uid = Some(target_uid);
                app.yanked_uid = None;
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::AddRelatedTo(target_uid) => {
            let related_opt = app.yanked_uid.clone();

            if let Some(related_uid) = related_opt
                && let Some(updated) = app.store.add_related_to(&target_uid, related_uid.clone())
            {
                app.selected_uid = Some(target_uid);
                app.yanked_uid = None;
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_update_wrapper(client.clone(), updated),
                        Message::SyncSaved,
                    );
                } else {
                    handle_offline_update(app, updated);
                }
            }
            Task::none()
        }
        Message::MoveTask(task_uid, target_href) => {
            // Use atomic store API that returns both the original (pre-mutation)
            // and the updated (post-mutation) task so callers do not have to
            // capture/cloning state separately and risk races.
            if let Some((original, _updated)) = app.store.move_task(&task_uid, target_href.clone())
            {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    // Pass the original (pre-mutation) task to the network layer
                    // so the backend/journal can identify the source calendar.
                    return Task::perform(
                        async_move_wrapper(client.clone(), original.clone(), target_href),
                        Message::TaskMoved,
                    );
                } else {
                    app.unsynced_changes = true;
                    let _ = Journal::push(app.ctx.as_ref(), Action::Move(original, target_href));
                }
            }
            Task::none()
        }
        Message::MigrateLocalTo(source_href, target_href) => {
            if let Some(local_map) = app.store.calendars.get(&source_href) {
                // CHANGED: Collect values from map
                let tasks_to_move: Vec<_> = local_map.values().cloned().collect();

                if tasks_to_move.is_empty() {
                    return Task::none();
                }
                app.loading = true;
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
            // Remove from modal
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            // Parse duration
            // reuse logic from parser or simple parsing
            let mins = if let Ok(n) = app.snooze_custom_input.parse::<u32>() {
                n
            } else {
                crate::model::parser::parse_duration(&app.snooze_custom_input).unwrap_or(10)
            };
            app.snooze_custom_input.clear();

            // Redirect to standard snooze handler
            handle(app, Message::SnoozeAlarm(t_uid, a_uid, mins))
        }
        Message::SnoozeAlarm(t_uid, a_uid, mins) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid) {
                let mut changed = false;

                // Case A: Implicit Alarm
                if let Some((dt, prefix)) = parse_implicit_id(&a_uid) {
                    let desc = if prefix.contains("due") {
                        "Due now"
                    } else {
                        "Starting"
                    }
                    .to_string();
                    task.snooze_implicit_alarm(dt, desc, mins);
                    changed = true;
                }
                // Case B: Explicit Alarm
                else if task.snooze_alarm(&a_uid, mins) {
                    changed = true;
                }

                if changed {
                    let t_clone = task.clone();
                    refresh_filtered_tasks(app);
                    if let Some(client) = &app.client {
                        return Task::perform(
                            async_update_wrapper(client.clone(), t_clone),
                            Message::SyncSaved,
                        );
                    } else {
                        handle_offline_update(app, t_clone);
                    }
                }
            }
            Task::none()
        }
        Message::DismissAlarm(t_uid, a_uid) => {
            if let Some((task, _)) = app.store.get_task_mut(&t_uid) {
                let mut changed = false;

                // Case A: Implicit Alarm
                if let Some((dt, prefix)) = parse_implicit_id(&a_uid) {
                    let desc = if prefix.contains("due") {
                        "Due now"
                    } else {
                        "Starting"
                    }
                    .to_string();
                    task.dismiss_implicit_alarm(dt, desc);
                    changed = true;
                }
                // Case B: Explicit Alarm
                else if task.dismiss_alarm(&a_uid) {
                    changed = true;
                }

                if changed {
                    let t_clone = task.clone();
                    refresh_filtered_tasks(app);
                    if let Some(client) = &app.client {
                        return Task::perform(
                            async_update_wrapper(client.clone(), t_clone),
                            Message::SyncSaved,
                        );
                    } else {
                        handle_offline_update(app, t_clone);
                    }
                }
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

fn handle_offline_update(app: &mut GuiApp, task: TodoTask) {
    app.unsynced_changes = true;
    if task.calendar_href.starts_with("local://") {
        if let Some(map) = app.store.calendars.get(&task.calendar_href) {
            // CHANGED: Collect values
            let list: Vec<_> = map.values().cloned().collect();
            let _ = LocalStorage::save_for_href(app.ctx.as_ref(), &task.calendar_href, &list);
        }
    } else {
        let _ = Journal::push(app.ctx.as_ref(), Action::Update(task));
    }
}

fn handle_offline_delete(app: &mut GuiApp, task: TodoTask) {
    app.unsynced_changes = true;
    if task.calendar_href.starts_with("local://") {
        if let Some(map) = app.store.calendars.get(&task.calendar_href) {
            // CHANGED: Collect values
            let list: Vec<_> = map.values().cloned().collect();
            let _ = LocalStorage::save_for_href(app.ctx.as_ref(), &task.calendar_href, &list);
        }
    } else {
        let _ = Journal::push(app.ctx.as_ref(), Action::Delete(task));
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

    if clean_input.starts_with('#')
        && !clean_input.trim().contains(' ')
        && app.editing_uid.is_none()
    {
        let was_alias_definition = text_to_submit.contains(":=");

        if !was_alias_definition {
            let tag = clean_input.trim().trim_start_matches('#').to_string();
            if !tag.is_empty() {
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
        } else {
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
        let loc = if clean_input.starts_with("@@") {
            clean_input.trim_start_matches("@@")
        } else {
            clean_input.trim_start_matches("loc:")
        };

        let clean_loc = crate::model::parser::strip_quotes(loc);

        if !clean_loc.is_empty() {
            app.sidebar_mode = SidebarMode::Locations;
            app.selected_locations.clear();
            app.selected_locations.insert(clean_loc);
            app.input_value = text_editor::Content::new();
            refresh_filtered_tasks(app);

            if !retroactive_sync_batch.is_empty() {
                return Task::batch(retroactive_sync_batch);
            }
            return Task::none();
        }
    }

    // Parse the config time stored in AppState
    let config_time = NaiveTime::parse_from_str(&app.default_reminder_time, "%H:%M").ok();

    if let Some(edit_uid) = &app.editing_uid {
        if let Some((task, _)) = app.store.get_task_mut(edit_uid) {
            // PASS CONFIG TIME HERE
            task.apply_smart_input(&clean_input, &app.tag_aliases, config_time);
            task.description = app.description_value.text();
            let task_copy = task.clone();

            app.input_value = text_editor::Content::new();
            app.description_value = text_editor::Content::new();
            app.editing_uid = None;
            app.selected_uid = Some(task_copy.uid.clone());

            refresh_filtered_tasks(app);

            if let Some(client) = &app.client {
                let save_cmd = Task::perform(
                    async_update_wrapper(client.clone(), task_copy),
                    Message::SyncSaved,
                );
                retroactive_sync_batch.push(save_cmd);
                return Task::batch(retroactive_sync_batch);
            } else {
                handle_offline_update(app, task_copy);
                return Task::none();
            }
        }
    } else if !clean_input.is_empty() {
        // PASS CONFIG TIME HERE
        let mut new_task = TodoTask::new(&clean_input, &app.tag_aliases, config_time);
        if let Some(parent) = &app.creating_child_of {
            new_task.parent_uid = Some(parent.clone());
            app.creating_child_of = None;
        }

        let target_href = app
            .active_cal_href
            .clone()
            .or_else(|| app.calendars.first().map(|c| c.href.clone()))
            .unwrap_or_default();

        if !target_href.is_empty() {
            new_task.calendar_href = target_href.clone();

            app.store.add_task(new_task.clone());

            // Ensure an Id is allocated and cached for this new task immediately so that
            // later focus operations can target the correct widget once it's rendered.
            app.task_ids
                .entry(new_task.uid.clone())
                .or_insert_with(iced::widget::Id::unique);

            app.selected_uid = Some(new_task.uid.clone());
            refresh_filtered_tasks(app);
            app.input_value = text_editor::Content::new();

            // Use a delayed scroll helper: waiting a short interval allows the view to
            // rebuild and register the new row widget before attempting to focus it.
            let scroll_cmd = scroll_to_selected_delayed(app);

            if let Some(client) = &app.client {
                let create_cmd = Task::perform(
                    async_create_wrapper(client.clone(), new_task),
                    Message::SyncSaved,
                );

                retroactive_sync_batch.push(create_cmd);
                retroactive_sync_batch.push(scroll_cmd);

                return Task::batch(retroactive_sync_batch);
            } else {
                if new_task.calendar_href.starts_with("local://") {
                    if let Ok(mut local) =
                        LocalStorage::load_for_href(app.ctx.as_ref(), &new_task.calendar_href)
                    {
                        local.push(new_task.clone());
                        let _ = LocalStorage::save_for_href(
                            app.ctx.as_ref(),
                            &new_task.calendar_href,
                            &local,
                        );
                    }
                } else {
                    let _ = Journal::push(app.ctx.as_ref(), Action::Create(new_task.clone()));
                }

                app.unsynced_changes = true;
                retroactive_sync_batch.push(scroll_cmd);
                return Task::batch(retroactive_sync_batch);
            }
        }
    }

    if !retroactive_sync_batch.is_empty() {
        return Task::batch(retroactive_sync_batch);
    }
    Task::none()
}
