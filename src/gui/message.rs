// ### File: `src/gui/message.rs`

use crate::client::RustyClient;
use crate::config::{AppTheme, Config};
use crate::gui::state::{ResizeDirection, SidebarMode};
use crate::model::{CalendarListEntry, Task as TodoTask};
use crate::system::AlarmMessage;
use iced::widget::text_editor;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type LoadedResult = Result<
    (
        RustyClient,
        Vec<CalendarListEntry>,
        Vec<TodoTask>,
        Option<String>,
        Option<String>,
    ),
    String,
>;

#[derive(Debug, Clone)]
pub enum Message {
    ObUrlChanged(String),
    ObUserChanged(String),
    ObPassChanged(String),
    ObInsecureToggled(bool),
    ToggleCalendarVisibility(String, bool),
    ToggleCalendarDisabled(String, bool),
    ObDefaultCalChanged(String),
    ObSubmit,
    OpenSettings,
    CancelSettings,
    OpenHelp,
    CloseHelp,
    InputChanged(text_editor::Action),

    DescriptionChanged(text_editor::Action),

    SearchChanged(String),
    SubmitTask,
    ToggleTask(usize, bool),
    DeleteTask(usize),
    EditTaskStart(usize),
    CancelEdit,
    ChangePriority(usize, i8),
    SetTaskStatus(usize, crate::model::TaskStatus),
    // --- NEW MESSAGES ---
    StartTask(String),
    PauseTask(String),
    StopTask(String),
    // --------------------
    SetMinDuration(Option<u32>),
    SetMaxDuration(Option<u32>),
    ToggleIncludeUnsetDuration(bool),
    ToggleDetails(String),
    ConfigLoaded(Result<Config, String>),
    ObSortMonthsChanged(String),
    ThemeChanged(AppTheme),

    Loaded(LoadedResult),
    Refresh,

    SyncSaved(Result<TodoTask, String>),
    SyncToggleComplete(Box<Result<(TodoTask, Option<TodoTask>), String>>),

    TasksRefreshed(Result<(String, Vec<TodoTask>), String>),
    DeleteComplete(Result<(), String>),

    SidebarModeChanged(SidebarMode),
    SelectCalendar(String),
    IsolateCalendar(String),
    CategoryToggled(String),
    LocationToggled(String),
    ClearAllTags,
    ClearAllLocations, // <--- NEW
    CategoryMatchModeChanged(bool),
    RefreshedAll(Result<Vec<(String, Vec<TodoTask>)>, String>),

    ToggleHideCompleted(bool),
    ToggleHideFullyCompletedTags(bool),

    YankTask(String),
    ClearYank,
    StartCreateChild(String),
    AddDependency(String),
    MakeChild(String),
    RemoveParent(String),
    RemoveDependency(String, String),

    AliasKeyInput(String),
    AliasValueInput(String),
    AddAlias,
    RemoveAlias(String),
    MoveTask(String, String),

    JumpToTag(String),
    JumpToLocation(String), // <--- NEW

    TaskMoved(Result<TodoTask, String>),
    ObSubmitOffline,
    MigrateLocalTo(String),

    MigrationComplete(Result<usize, String>),
    FontLoaded(Result<(), String>),
    DismissError,
    ToggleAllCalendars(bool),

    TabPressed(bool),

    // Shortcuts
    FocusInput,
    FocusSearch,

    // Window Controls
    WindowDragged,
    MinimizeWindow,
    CloseWindow,
    WindowResized(iced::Size),

    // Resize
    ResizeStart(ResizeDirection),

    // Open URL
    OpenUrl(String),
    ObUrgentDaysChanged(String),
    ObUrgentPrioChanged(String),
    InitAlarmActor(mpsc::Sender<Vec<TodoTask>>),
    AlarmSignalReceived(Arc<AlarmMessage>), // Arc to make it Clone-able easily
    SnoozeAlarm(String, String, u32),       // TaskUID, AlarmUID, Minutes
    DismissAlarm(String, String),           // TaskUID, AlarmUID
    SnoozeCustomInput(String),
    SnoozeCustomSubmit(String, String),

    // Reminder & Snooze Settings
    SetAutoReminders(bool),
    SetDefaultReminderTime(String),
    SetSnoozeShort(String),
    SetSnoozeLong(String),
}
