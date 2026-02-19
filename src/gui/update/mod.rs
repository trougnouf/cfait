// File: ./src/gui/update/mod.rs
// Central message handler dispatching to specific update modules.
pub mod common;
pub mod network;
pub mod settings;
pub mod tasks;
pub mod view;

use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::system::{AlarmMessage, SystemEvent};
use iced::Task;

pub fn update(app: &mut GuiApp, message: Message) -> Task<Message> {
    let task = match message {
        Message::FontLoaded(_) => Task::none(),
        Message::DeleteComplete(_) => Task::none(),

        // --- Settings Messages ---
        Message::ConfigLoaded(_)
        | Message::ObUrlChanged(_)
        | Message::ObUserChanged(_)
        | Message::ObPassChanged(_)
        | Message::ObDefaultCalChanged(_)
        | Message::ObInsecureToggled(_)
        | Message::ObSubmit
        | Message::OpenSettings
        | Message::CancelSettings
        | Message::ObSubmitOffline
        | Message::AliasKeyInput(_)
        | Message::AliasValueInput(_)
        | Message::AddAlias
        | Message::RemoveAlias(_)
        | Message::ObSortMonthsChanged(_)
        | Message::ObUrgentDaysChanged(_)
        | Message::ObUrgentPrioChanged(_)
        | Message::ObDefaultPriorityChanged(_)
        | Message::ObStartGraceChanged(_)
        | Message::ThemeChanged(_)
        | Message::SetAutoReminders(_)
        | Message::SetDefaultReminderTime(_)
        | Message::SetSnoozeShort(_)
        | Message::SetSnoozeLong(_)
        | Message::SetTrashRetention(_)  // <--- Added this line
        | Message::SetAutoRefreshInterval(_)
        | Message::SetCreateEventsForTasks(_)
        | Message::SetDeleteEventsOnCompletion(_)
        | Message::DeleteAllCalendarEvents
        | Message::BackfillEventsComplete(_)
        | Message::ExportLocalIcs(_)
        | Message::ExportSaved(_)
        | Message::ImportLocalIcs(_)
        | Message::ImportCompleted(_)
        | Message::IcsFileLoaded(_)
        | Message::IcsImportDialogCalendarSelected(_)
        | Message::IcsImportDialogCancel
        | Message::IcsImportDialogConfirm
        | Message::AddLocalCalendar
        | Message::DeleteLocalCalendar(_)
        | Message::LocalCalendarNameChanged(_, _)
        | Message::OpenColorPicker(_, _)
        | Message::CancelColorPicker
        | Message::SubmitColorPicker(_)
        // NEW: Route advanced settings messages
        | Message::ToggleAdvancedSettings(_)
        | Message::SetMaxDoneRoots(_)
        | Message::SetMaxDoneSubtasks(_)
        | Message::SetStrikethroughCompleted(_) => settings::handle(app, message),

        // --- Task Logic Messages ---
        Message::InputChanged(_)
        | Message::DescriptionChanged(_)
        | Message::StartCreateChild(_)
        | Message::SubmitTask
        | Message::ToggleTask(_, _)
        | Message::EditTaskStart(_)
        | Message::CancelEdit
        | Message::DeleteTask(_)
        | Message::ChangePriority(_, _)
        | Message::SetTaskStatus(_, _)
        | Message::YankTask(_)
        | Message::ClearYank
        | Message::EscapePressed
        | Message::MakeChild(_)
        | Message::RemoveParent(_)
        | Message::RemoveDependency(_, _)
        | Message::RemoveRelatedTo(_, _)
        | Message::AddDependency(_)
        | Message::AddRelatedTo(_)
        | Message::MoveTask(_, _)
        | Message::MigrateLocalTo(_, _)
        | Message::StartTask(_)
        | Message::PauseTask(_)
        | Message::StopTask(_)
        | Message::SnoozeCustomInput(_)
        | Message::SnoozeCustomSubmit(_, _)
        // Keyboard Shortcuts routing to Task logic
        | Message::EditSelectedDescription
        | Message::PromoteSelected
        | Message::DemoteSelected
        | Message::YankSelected
        | Message::KeyboardCreateChild
        | Message::KeyboardAddDependency
        | Message::KeyboardAddRelation
        | Message::ToggleActiveSelected
        | Message::StopSelected
        | Message::CancelSelected
        | Message::ChangePrioritySelected(_)
        | Message::CompleteTaskFromAlarm(_, _)
        | Message::CancelTaskFromAlarm(_, _)
        | Message::ToggleDoneGroup(_) => tasks::handle(app, message),

        // --- View & Navigation Messages ---
        Message::TabPressed(_)
        | Message::FocusInput
        | Message::FocusSearch
        | Message::DismissError
        | Message::ToggleAllCalendars(_)
        | Message::ToggleCalendarVisibility(_, _)
        | Message::IsolateCalendar(_)
        | Message::SidebarModeChanged(_)
        | Message::CategoryToggled(_)
        | Message::LocationToggled(_)
        | Message::ClearAllTags
        | Message::ClearAllLocations
        | Message::CategoryMatchModeChanged(_)
        | Message::ToggleHideCompleted(_)
        | Message::ToggleHideFullyCompletedTags(_)
        | Message::SelectCalendar(_)
        | Message::ToggleCalendarDisabled(_, _)
        | Message::SearchChanged(_)
        | Message::ClearSearch
        | Message::SetMinDuration(_)
        | Message::SetMaxDuration(_)
        | Message::ToggleIncludeUnsetDuration(_)
        | Message::ToggleDetails(_)
        | Message::OpenHelp
        | Message::CloseHelp
        | Message::WindowDragged
        | Message::MinimizeWindow
        | Message::CloseWindow
        | Message::ResizeStart(_)
        | Message::WindowResized(_)
        | Message::JumpToTag(_)
        | Message::JumpToLocation(_)
        | Message::SelectNextTask
        | Message::SelectPrevTask
        | Message::SelectNextPage
        | Message::SelectPrevPage
        | Message::DeleteSelected
        | Message::ToggleSelected
        | Message::EditSelected
        | Message::JumpToTask(_)
        | Message::JumpToRandomTask
        | Message::OpenUrl(_)
        | Message::FocusTag(_)
        | Message::TagHovered(_)
        | Message::TaskClick(_, _)
        | Message::FocusLocation(_)
        | Message::ToggleHideCompletedToggle
        | Message::CategoryMatchModeToggle => view::handle(app, message),

        // --- Network Messages ---
        Message::Refresh
        | Message::Loaded(_)
        | Message::RefreshedAll(_)
        | Message::TasksRefreshed(_)
        | Message::SyncSaved(_)
        | Message::SyncToggleComplete(_)
        | Message::TaskMoved(_)
        | Message::MigrationComplete(_) => network::handle(app, message),

        // Delayed focus trigger: when the view has been refreshed and widget IDs are registered,
        // `SnapToSelected` will attempt to focus/scroll to the selected task. If the task
        // is still not present in the filtered view, re-schedule a delayed attempt.
        Message::SnapToSelected { focus } => {
            if let Some(uid) = &app.selected_uid {
                let present_in_list = app.tasks.iter().any(|t| t.uid == *uid);
                let has_cached_id = app.task_ids.contains_key(uid);

                if present_in_list || has_cached_id {
                    common::scroll_to_selected(app, focus)
                } else {
                    // Task not yet visible/registered; try again shortly.
                    common::scroll_to_selected_delayed(app, focus)
                }
            } else {
                Task::none()
            }
        }

        // --- Alarm System ---
        Message::InitAlarmActor(tx) => {
            app.alarm_tx = Some(tx.clone());
            if !app.tasks.is_empty() {
                let all = app.store.calendars.values().flat_map(|m| m.values()).cloned().collect();
                let _ = tx.try_send(SystemEvent::UpdateTasks(all));
            }
            Task::none()
        }
        Message::AlarmSignalReceived(msg) => {
             match &*msg {
                AlarmMessage::Fire(task_uid, alarm_uid) => {
                    if let Some((task, _)) = app.store.get_task_mut(task_uid) {
                        let is_implicit = alarm_uid.starts_with("implicit_");
                        let exists = is_implicit || task.alarms.iter().any(|a| a.uid == *alarm_uid);

                        if exists {
                            let alarm_obj = if let Some(a) = task.alarms.iter().find(|a| a.uid == *alarm_uid) {
                                a.clone()
                            } else {
                                crate::model::Alarm {
                                    uid: alarm_uid.clone(),
                                    action: "DISPLAY".to_string(),
                                    trigger: crate::model::AlarmTrigger::Relative(0),
                                    description: Some(if alarm_uid.contains("due") { "Due now".to_string() } else { "Starting".to_string() }),
                                    acknowledged: None,
                                    related_to_uid: None,
                                    relation_type: None,
                                }
                            };
                            app.ringing_tasks.push((task.clone(), alarm_obj));
                        }
                    }
                }
            }
            Task::none()
        }
        Message::SnoozeAlarm(t_uid, a_uid, mins) => {
            app.ringing_tasks.retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));
            tasks::handle(app, Message::SnoozeAlarm(t_uid, a_uid, mins))
        }
        Message::DismissAlarm(t_uid, a_uid) => {
            app.ringing_tasks.retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));
            tasks::handle(app, Message::DismissAlarm(t_uid, a_uid))
        }
    };

    // Update placeholder text for UI
    if app.editing_uid.is_some() {
        app.current_placeholder = "Edit Title...".to_string();
    } else if let Some(parent_uid) = &app.creating_child_of {
        let parent_name = app
            .store
            .get_summary(parent_uid)
            .unwrap_or("Parent".to_string());
        app.current_placeholder = format!("New child of '{}'...", parent_name);
    } else {
        let target_name = app
            .calendars
            .iter()
            .find(|c| Some(&c.href) == app.active_cal_href.as_ref())
            .map(|c| c.name.as_str())
            .unwrap_or("Default");
        app.current_placeholder = format!(
            "Add task to {} (e.g. Buy cat food !1 @tomorrow #groceries ~30m)",
            target_name
        );
    }

    task
}
