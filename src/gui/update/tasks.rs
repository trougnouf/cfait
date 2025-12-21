// File: src/gui/update/tasks.rs
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::{GuiApp, SidebarMode};
use crate::gui::update::common::{apply_alias_retroactively, refresh_filtered_tasks, save_config};
use crate::journal::{Action, Journal};
use crate::model::{Task as TodoTask, extract_inline_aliases};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalStorage};
use iced::Task;
use iced::widget::operation;
use iced::widget::scrollable::RelativeOffset;
use iced::widget::text_editor;

pub fn handle(app: &mut GuiApp, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(action) => {
            let previous_text = app.input_value.text();
            app.input_value.perform(action);
            let current_text = app.input_value.text();

            // Check if 'Enter' was pressed (ends with newline).
            if current_text.len() == previous_text.len() + 1 && current_text.ends_with('\n') {
                return handle_submit(app);
            }

            Task::none()
        }
        Message::DescriptionChanged(action) => {
            app.description_value.perform(action);
            Task::none()
        }
        Message::StartCreateChild(parent_uid) => {
            app.creating_child_of = Some(parent_uid.clone());
            app.selected_uid = Some(parent_uid.clone());

            // Auto-fill tags from parent
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
        Message::ChangePriority(index, delta) => {
            if let Some(view_task) = app.tasks.get(index) {
                app.selected_uid = Some(view_task.uid.clone());
                if let Some(updated) = app.store.change_priority(&view_task.uid, delta) {
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
        // --- START / PAUSE / STOP HANDLERS ---
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
        // -----------------------------------------
        // --- YANK / LINKING Handlers ---
        Message::YankTask(uid) => {
            app.yanked_uid = Some(uid);
            Task::none()
        }
        Message::ClearYank => {
            app.yanked_uid = None;
            Task::none()
        }
        Message::MakeChild(target_uid) => {
            let parent_opt = app.yanked_uid.clone();

            if let Some(parent_uid) = parent_opt
                && let Some(updated) = app.store.set_parent(&target_uid, Some(parent_uid.clone()))
            {
                app.selected_uid = Some(target_uid);
                app.yanked_uid = None; // Clear yank state
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
        Message::AddDependency(target_uid) => {
            let blocker_opt = app.yanked_uid.clone();

            if let Some(blocker_uid) = blocker_opt
                && let Some(updated) = app.store.add_dependency(&target_uid, blocker_uid.clone())
            {
                app.selected_uid = Some(target_uid);
                app.yanked_uid = None; // Clear yank state
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
            // **FIX**: Capture the original task state *before* moving it.
            let original_task = app
                .store
                .calendars
                .values()
                .flatten()
                .find(|t| t.uid == task_uid)
                .cloned();

            if let Some(updated) = app.store.move_task(&task_uid, target_href.clone()) {
                app.selected_uid = Some(task_uid);
                refresh_filtered_tasks(app);
                if let Some(client) = &app.client {
                    return Task::perform(
                        async_move_wrapper(client.clone(), updated, target_href),
                        Message::TaskMoved,
                    );
                } else if let Some(old_task) = original_task {
                    // Correctly journal the MOVE action using the original task snapshot.
                    app.unsynced_changes = true;
                    let _ = Journal::push(Action::Move(old_task, target_href));
                }
            }
            Task::none()
        }
        Message::MigrateLocalTo(target_href) => {
            // Migration is strictly "Local -> Server".
            // It cannot run without a client.
            if let Some(local_tasks) = app.store.calendars.get(LOCAL_CALENDAR_HREF) {
                let tasks_to_move = local_tasks.clone();
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
                    // Cannot migrate offline
                    app.error_msg = Some("Cannot export while offline/connecting.".to_string());
                    app.loading = false;
                }
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

// --- HELPER HANDLERS FOR OFFLINE ACTIONS ---

fn handle_offline_update(app: &mut GuiApp, task: TodoTask) {
    app.unsynced_changes = true;
    if task.calendar_href == LOCAL_CALENDAR_HREF {
        if let Some(list) = app.store.calendars.get(&task.calendar_href) {
            let _ = LocalStorage::save(list);
        }
    } else {
        let _ = Journal::push(Action::Update(task));
    }
}

fn handle_offline_delete(app: &mut GuiApp, task: TodoTask) {
    app.unsynced_changes = true;
    if task.calendar_href == LOCAL_CALENDAR_HREF {
        if let Some(list) = app.store.calendars.get(&task.calendar_href) {
            let _ = LocalStorage::save(list);
        }
    } else {
        let _ = Journal::push(Action::Delete(task));
    }
}

// --- SUBMIT HANDLER ---

fn handle_submit(app: &mut GuiApp) -> Task<Message> {
    let raw_text = app.input_value.text();
    let text_to_submit = raw_text.trim().to_string();

    if text_to_submit.is_empty() {
        app.input_value = text_editor::Content::new();
        return Task::none();
    }

    let (clean_input, new_aliases) = extract_inline_aliases(&text_to_submit);

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
        let was_alias_definition = text_to_submit.contains('=');

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

    if let Some(edit_uid) = &app.editing_uid {
        if let Some((task, _)) = app.store.get_task_mut(edit_uid) {
            task.apply_smart_input(&clean_input, &app.tag_aliases);
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
        let mut new_task = TodoTask::new(&clean_input, &app.tag_aliases);
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

            app.selected_uid = Some(new_task.uid.clone());
            refresh_filtered_tasks(app);
            app.input_value = text_editor::Content::new();

            let len = app.tasks.len().max(1) as f32;
            let idx = app
                .tasks
                .iter()
                .position(|t| t.uid == new_task.uid)
                .unwrap_or(0) as f32;
            let scroll_cmd = operation::snap_to(
                app.scrollable_id.clone(),
                RelativeOffset {
                    x: 0.0,
                    y: idx / len,
                },
            );

            if let Some(client) = &app.client {
                let create_cmd = Task::perform(
                    async_create_wrapper(client.clone(), new_task),
                    Message::SyncSaved,
                );

                retroactive_sync_batch.push(create_cmd);
                retroactive_sync_batch.push(scroll_cmd);

                return Task::batch(retroactive_sync_batch);
            } else {
                // Client not ready (Loading/Offline): Persist to Journal immediately
                if new_task.calendar_href == LOCAL_CALENDAR_HREF {
                    if let Ok(mut local) = LocalStorage::load() {
                        local.push(new_task.clone());
                        let _ = LocalStorage::save(&local);
                    }
                } else {
                    let _ = Journal::push(Action::Create(new_task.clone()));
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
