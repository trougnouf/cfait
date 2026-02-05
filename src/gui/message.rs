// Defines all messages used for the Elm architecture in the GUI.

use crate::client::RustyClient;
use crate::config::{AppTheme, Config};
use crate::gui::state::{ResizeDirection, SidebarMode};
use crate::model::{CalendarListEntry, Task as TodoTask};
use crate::system::{AlarmMessage, SystemEvent};
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
    // --- Settings & Onboarding ---
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
    ObSubmitOffline,

    // --- Input & Editing ---
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
    StartTask(String),
    PauseTask(String),
    StopTask(String),

    // --- Keyboard Shortcuts (Stateless / Context-Aware) ---
    SelectNextTask,
    SelectPrevTask,
    SelectNextPage,
    SelectPrevPage,
    DeleteSelected,
    ToggleSelected,
    EditSelected,
    EditSelectedDescription,
    PromoteSelected,
    DemoteSelected,
    YankSelected,
    ClearYank,
    EscapePressed,
    KeyboardCreateChild,
    KeyboardAddDependency,
    KeyboardAddRelation,
    ToggleActiveSelected,       // 's' logic
    StopSelected,               // 'S' logic
    CancelSelected,             // 'x' logic
    ChangePrioritySelected(i8), // '+' / '-' logic
    ToggleHideCompletedToggle,  // 'H' (Stateless switch)
    CategoryMatchModeToggle,    // 'm' (Stateless switch)
    FocusInput,
    FocusSearch,
    Refresh,

    // --- View & Filter ---
    SetMinDuration(Option<u32>),
    SetMaxDuration(Option<u32>),
    ToggleIncludeUnsetDuration(bool),
    ToggleDetails(String),
    SidebarModeChanged(SidebarMode),
    SelectCalendar(String),
    IsolateCalendar(String),
    CategoryToggled(String),
    LocationToggled(String),
    ClearAllTags,
    ClearAllLocations,
    CategoryMatchModeChanged(bool),
    ToggleHideCompleted(bool),
    ToggleHideFullyCompletedTags(bool),
    TabPressed(bool),
    OpenHelp,
    CloseHelp,

    // --- Navigation & Actions ---
    YankTask(String),
    StartCreateChild(String),
    AddDependency(String),
    AddRelatedTo(String),
    MakeChild(String),
    RemoveParent(String),
    RemoveDependency(String, String),
    RemoveRelatedTo(String, String),
    AliasKeyInput(String),
    AliasValueInput(String),
    AddAlias,
    RemoveAlias(String),
    MoveTask(String, String),
    MigrateLocalTo(String, String),
    JumpToTag(String),
    JumpToLocation(String),
    JumpToTask(String),
    JumpToRandomTask, // Jump to a weighted-random task
    SnapToSelected { focus: bool },
    TagHovered(Option<String>),
    FocusTag(String),
    FocusLocation(String),
    OpenUrl(String),
    TaskClick(usize, String), // Added

    // --- System & Network Events ---
    ConfigLoaded(Result<Config, String>),
    ObSortMonthsChanged(String),
    ThemeChanged(AppTheme),
    Loaded(LoadedResult),
    SyncSaved(Result<TodoTask, String>),
    SyncToggleComplete(Box<Result<(TodoTask, Option<TodoTask>), String>>),
    TasksRefreshed(Result<(String, Vec<TodoTask>), String>),
    DeleteComplete(Result<(), String>),
    RefreshedAll(Result<Vec<(String, Vec<TodoTask>)>, String>),
    TaskMoved(Result<TodoTask, String>),
    MigrationComplete(Result<usize, String>),
    FontLoaded(Result<(), String>),
    DismissError,
    ToggleAllCalendars(bool),

    // --- Window Management ---
    WindowDragged,
    MinimizeWindow,
    CloseWindow,
    WindowResized(iced::Size),
    ResizeStart(ResizeDirection),

    // --- Settings Input Fields ---
    ObUrgentDaysChanged(String),
    ObUrgentPrioChanged(String),
    ObDefaultPriorityChanged(String),
    ObStartGraceChanged(String),

    // --- Alarms & Reminders ---
    InitAlarmActor(mpsc::Sender<SystemEvent>),
    AlarmSignalReceived(Arc<AlarmMessage>),
    SnoozeAlarm(String, String, u32),
    DismissAlarm(String, String),
    SnoozeCustomInput(String),
    SnoozeCustomSubmit(String, String),
    SetAutoReminders(bool),
    SetDefaultReminderTime(String),
    SetSnoozeShort(String),
    SetSnoozeLong(String),
    SetCreateEventsForTasks(bool),
    SetDeleteEventsOnCompletion(bool),
    DeleteAllCalendarEvents,
    BackfillEventsComplete(Result<usize, String>),

    // --- Local Calendar & ICS ---
    ExportLocalIcs(String),
    ExportSaved(Result<std::path::PathBuf, String>),
    ImportLocalIcs(String),
    ImportCompleted(Result<String, String>),
    IcsFileLoaded(Result<(String, String), String>),
    IcsImportDialogCalendarSelected(String),
    IcsImportDialogCancel,
    IcsImportDialogConfirm,
    AddLocalCalendar,
    DeleteLocalCalendar(String),
    LocalCalendarNameChanged(String, String),
    OpenColorPicker(String, iced::Color),
    CancelColorPicker,
    SubmitColorPicker(iced::Color),
}
