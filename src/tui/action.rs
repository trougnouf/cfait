// Defines actions and events for TUI interaction and state updates.
use crate::model::{CalendarListEntry, Task};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarMode {
    Calendars,
    Categories,
    Locations, // NEW
}

#[derive(Debug)]
pub enum Action {
    SwitchCalendar(String),
    CreateTask(Task),
    UpdateTask(Task),
    ToggleTask(Task),
    MarkInProcess(Task),
    MarkCancelled(Task),
    DeleteTask(Task),
    Refresh,
    Quit,
    MoveTask(Task, String),
    StartCreateChild(String),
    MigrateLocal(String, String), // (source_calendar_href, target_calendar_href)
    ToggleCalendarVisibility(String),
    IsolateCalendar(String),
}

#[derive(Debug)]
pub enum AppEvent {
    CalendarsLoaded(Vec<CalendarListEntry>),
    TasksLoaded(Vec<(String, Vec<Task>)>),
    Error(String),
    Status(String),
}
