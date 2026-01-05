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
        | Message::ThemeChanged(_)
        | Message::SetAutoReminders(_)
        | Message::SetDefaultReminderTime(_)
        | Message::SetSnoozeShort(_)
        | Message::SetSnoozeLong(_)
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
        | Message::SubmitColorPicker(_) => settings::handle(app, message),

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
        | Message::SnoozeCustomInput(_) // ADD THIS
        | Message::SnoozeCustomSubmit(_, _) // ADD THIS
        => tasks::handle(app, message),

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
        | Message::JumpToTask(_)
        | Message::OpenUrl(_) => view::handle(app, message),

        Message::Refresh
        | Message::Loaded(_)
        | Message::RefreshedAll(_)
        | Message::TasksRefreshed(_)
        | Message::SyncSaved(_)
        | Message::SyncToggleComplete(_)
        | Message::TaskMoved(_)
        | Message::MigrationComplete(_) => network::handle(app, message),
        Message::InitAlarmActor(tx) => {
            app.alarm_tx = Some(tx.clone());
            // Send initial load
            if !app.tasks.is_empty() {
                let all = app.store.calendars.values().flatten().cloned().collect();
                let _ = tx.try_send(SystemEvent::UpdateTasks(all));
            }
            // Note: We do NOT send EnableAlarms here yet.
            Task::none()
        }
        Message::AlarmSignalReceived(msg) => {
            match &*msg {
                AlarmMessage::Fire(task_uid, alarm_uid) => {
                    // FIX: Look up in the full store, not the filtered app.tasks view.
                    // The task might be hidden by filters but should still ring.
                    if let Some((task, _)) = app.store.get_task_mut(task_uid) {
                        // Check if alarm still exists (wasn't dismissed elsewhere)
                        // For implicit alarms (which are not in task.alarms), we check if the synthetic ID implies validity
                        let is_implicit = alarm_uid.starts_with("implicit_");
                        let exists = is_implicit || task.alarms.iter().any(|a| a.uid == *alarm_uid);

                        if exists {
                            // We clone the task state at the moment of firing to show in the modal
                            // We construct a synthetic Alarm object for the modal if it's implicit,
                            // otherwise copy the real one.
                            let alarm_obj = if let Some(a) = task.alarms.iter().find(|a| a.uid == *alarm_uid) {
                                a.clone()
                            } else {
                                // Reconstruct basic info for implicit alarm so the modal doesn't crash
                                crate::model::Alarm {
                                    uid: alarm_uid.clone(),
                                    action: "DISPLAY".to_string(),
                                    trigger: crate::model::AlarmTrigger::Relative(0), // Dummy
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
            // Remove from modal stack
            app.ringing_tasks.retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));

            // Perform Logic
            tasks::handle(app, Message::SnoozeAlarm(t_uid, a_uid, mins))
        }
        Message::DismissAlarm(t_uid, a_uid) => {
            app.ringing_tasks.retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));
            tasks::handle(app, Message::DismissAlarm(t_uid, a_uid))
        }
    };

    update_placeholder(app);
    task
}

fn update_placeholder(app: &mut GuiApp) {
    app.current_placeholder = if app.editing_uid.is_some() {
        "Edit Title...".to_string()
    } else if let Some(parent_uid) = &app.creating_child_of {
        let parent_name = app
            .store
            .get_summary(parent_uid)
            .unwrap_or("Parent".to_string());
        format!("New child of '{}'...", parent_name)
    } else {
        let target_name = app
            .calendars
            .iter()
            .find(|c| Some(&c.href) == app.active_cal_href.as_ref())
            .map(|c| c.name.as_str())
            .unwrap_or("Default");
        format!(
            "Add task to {} (e.g. Buy cat food !1 @tomorrow #groceries ~30m)",
            target_name
        )
    };
}
