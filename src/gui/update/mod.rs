// File: src/gui/update/mod.rs
pub mod common;
pub mod network;
pub mod settings;
pub mod tasks;
pub mod view;

use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::system::AlarmMessage;
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
        | Message::ThemeChanged(_) => settings::handle(app, message),

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
        | Message::AddDependency(_)
        | Message::MoveTask(_, _)
        | Message::MigrateLocalTo(_)
        | Message::StartTask(_)
        | Message::PauseTask(_)
        | Message::StopTask(_) => tasks::handle(app, message),

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
        | Message::ClearAllLocations // <--- ADDED
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
        | Message::JumpToLocation(_) // <--- ADDED
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
                let _ = tx.try_send(app.tasks.clone());
            }
            Task::none()
        }
        Message::AlarmSignalReceived(msg) => {
            match &*msg {
                AlarmMessage::Fire(task_uid, alarm_uid) => {
                    // Find task and alarm data to show in modal
                    if let Some(task) = app.tasks.iter().find(|t| t.uid == *task_uid)
                        && let Some(alarm) = task.alarms.iter().find(|a| a.uid == *alarm_uid) {
                            app.ringing_tasks.push((task.clone(), alarm.clone()));
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
