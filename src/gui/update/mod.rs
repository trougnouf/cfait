// File: ./src/gui/update/mod.rs
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
    // Automatically close the context menu on any interaction, unless it's the
    // message explicitly keeping it open or safe background ticks.
    if app.active_context_menu.is_some() {
        match &message {
            Message::OpenContextMenu(_, _)
            | Message::CloseContextMenu
            | Message::Tick
            | Message::WindowResized(_)
            | Message::CursorMoved(_) => {}
            _ => {
                app.active_context_menu = None;
            }
        }
    }

    let task = match message {
        Message::FontLoaded(_) => Task::none(),
        Message::Tick => Task::none(), // Just forces a view redraw

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
        | Message::SetTrashRetention(_)
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
        | Message::ToggleAdvancedSettings(_)
        | Message::SetMaxDoneRoots(_)
        | Message::SetMaxDoneSubtasks(_)
        | Message::SetShowPriorityNumbers(_)
        | Message::SetLanguage(_)
        | Message::SetStrikethroughCompleted(_)
        | Message::TogglePinnedAction(_, _) => settings::handle(app, message),

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
        | Message::EscCaptured
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
        | Message::EditSelectedDescription
        | Message::PromoteSelected
        | Message::DemoteSelected
        | Message::YankSelected
        | Message::KeyboardLinkChild
        | Message::KeyboardCreateChild
        | Message::KeyboardAddDependency
        | Message::KeyboardAddRelation
        | Message::KeyboardDuplicateTask
        | Message::KeyboardDeleteTaskTree
        | Message::DuplicateTask(_)
        | Message::DeleteTaskTree(_)
        | Message::ToggleActiveSelected
        | Message::StopSelected
        | Message::CancelSelected
        | Message::ChangePrioritySelected(_)
        | Message::CompleteTaskFromAlarm(_, _)
        | Message::CancelTaskFromAlarm(_, _)
        | Message::ToggleDoneGroup(_)
        | Message::StartAddSession(_)
        | Message::SessionInputChanged(_)
        | Message::SubmitSession
        | Message::CancelAddSession
        | Message::DeleteSession(_, _)
        | Message::ToggleShowAllSessions(_)
        | Message::KeyboardAddSession
        | Message::KeyboardToggleSessions
        | Message::StartMoveTask(_)
        | Message::CancelMoveTask => tasks::handle(app, message),

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
        | Message::ClearAllFilters
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
        | Message::OpenHelp(_)
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
        | Message::CategoryMatchModeToggle
        | Message::ToggleChildLock
        | Message::ToggleYankLock
        | Message::ZoomIn
        | Message::ZoomOut
        | Message::ZoomReset
        | Message::MoveSelected => view::handle(app, message),

        Message::Refresh
        | Message::Loaded(_)
        | Message::RefreshedAll(_)
        | Message::TasksRefreshed(_)
        | Message::ControllerActionComplete(_)
        | Message::MigrationComplete(_) => network::handle(app, message),

        Message::OpenContextMenu(..)
        | Message::CloseContextMenu
        | Message::CursorMoved(_) => view::handle(app, message),

        Message::SnapToSelected { focus } => {
            if let Some(uid) = &app.selected_uid {
                let present_in_list = app.tasks.iter().any(|t| t.uid == *uid);
                let has_cached_id = app.task_ids.contains_key(uid);

                if present_in_list || has_cached_id {
                    common::scroll_to_selected(app, focus)
                } else {
                    common::scroll_to_selected_delayed(app, focus)
                }
            } else {
                Task::none()
            }
        }

        Message::InitAlarmActor(tx) => {
            app.alarm_tx = Some(tx.clone());
            if !app.tasks.is_empty() {
                let all = app
                    .store
                    .calendars
                    .values()
                    .flat_map(|m| m.values())
                    .cloned()
                    .collect();
                let _ = tx.try_send(SystemEvent::UpdateTasks(all));
            }
            Task::none()
        }
        Message::AlarmSignalReceived(msg) => {
            let mut triggered = false;
            match &*msg {
                AlarmMessage::Fire(task_uid, alarm_uid) => {
                    if let Some((task, _)) = app.store.get_task_mut(task_uid) {
                        let is_implicit = alarm_uid.starts_with("implicit_");
                        let exists = is_implicit || task.alarms.iter().any(|a| a.uid == *alarm_uid);

                        if exists {
                            let alarm_obj =
                                if let Some(a) = task.alarms.iter().find(|a| a.uid == *alarm_uid) {
                                    (*a).clone() // Safely deref &&Alarm
                                } else {
                                    crate::model::Alarm {
                                        uid: alarm_uid.clone(),
                                        action: "DISPLAY".to_string(),
                                        trigger: crate::model::AlarmTrigger::Relative(0),
                                        description: Some(if alarm_uid.contains("due") {
                                            "Due now".to_string()
                                        } else {
                                            "Starting".to_string()
                                        }),
                                        acknowledged: None,
                                        related_to_uid: None,
                                        relation_type: None,
                                    }
                                };
                            app.ringing_tasks.push((task.clone(), alarm_obj));
                            triggered = true;
                        }
                    }
                }
                AlarmMessage::FocusTask(task_uid) => {
                    return Task::done(Message::JumpToTask(task_uid.clone()));
                }
            }
            if triggered {
                Task::done(Message::Refresh)
            } else {
                Task::none()
            }
        }
        Message::SnoozeAlarm(t_uid, a_uid, mins) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));
            tasks::handle(app, Message::SnoozeAlarm(t_uid, a_uid, mins))
        }
        Message::DismissAlarm(t_uid, a_uid) => {
            app.ringing_tasks
                .retain(|(t, a)| !(t.uid == t_uid && a.uid == a_uid));
            tasks::handle(app, Message::DismissAlarm(t_uid, a_uid))
        }
    };

    // Prune ringing tasks that are no longer valid (done, canceled, or alarm acknowledged/snoozed/removed)
    app.ringing_tasks.retain(|(t, alarm)| {
        if let Some(store_task) = app.store.get_task_ref(&t.uid) {
            if store_task.status.is_done()
                || store_task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                || store_task.calendar_href == "local://recovery"
            {
                return false;
            }

            if alarm.uid.starts_with("implicit_") {
                let parts: Vec<&str> = alarm.uid.split('|').collect();
                if parts.len() >= 2 {
                    let type_key_with_colon = parts[0];
                    let expected_ts = parts[1];

                    let config = crate::config::Config::load(app.ctx.as_ref()).unwrap_or_default();
                    let default_time =
                        chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
                            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());

                    let mut current_ts = None;
                    if type_key_with_colon == "implicit_due:" {
                        if let Some(due) = &store_task.due {
                            current_ts =
                                Some(due.to_utc_with_default_time(default_time).to_rfc3339());
                        }
                    } else if type_key_with_colon == "implicit_start:"
                        && let Some(start) = &store_task.dtstart
                    {
                        current_ts =
                            Some(start.to_utc_with_default_time(default_time).to_rfc3339());
                    }
                    if current_ts.as_deref() != Some(expected_ts) {
                        return false;
                    }
                } else {
                    return false;
                }
            } else if let Some(store_alarm) = store_task.alarms.iter().find(|a| a.uid == alarm.uid)
            {
                if store_alarm.acknowledged.is_some() {
                    return false;
                }
            } else {
                return false; // explicit alarm was removed
            }
            true
        } else {
            false
        }
    });

    if app.editing_uid.is_some() {
        app.current_placeholder = rust_i18n::t!("edit_task_title").to_string();
    } else if let Some(parent_uid) = &app.creating_child_of {
        let parent_name = app
            .store
            .get_summary(parent_uid)
            .unwrap_or("Parent".to_string());
        app.current_placeholder = rust_i18n::t!("new_child_of", name = parent_name).to_string();
    } else {
        let target_name = app
            .calendars
            .iter()
            .find(|c| Some(&c.href) == app.active_cal_href.as_ref())
            .map(|c| c.name.as_str())
            .unwrap_or("Default");
        app.current_placeholder = format!(
            "{} ({} {} !1 @tomorrow #groceries ~30m)",
            rust_i18n::t!("add_task_to_target", target = target_name),
            rust_i18n::t!("eg"),
            rust_i18n::t!("example_buy_cat_food")
        );
    }

    // Persist search/notes placeholder translations
    app.search_placeholder = rust_i18n::t!("search_placeholder").to_string();
    app.notes_placeholder = rust_i18n::t!("notes_placeholder").to_string();

    task
}
