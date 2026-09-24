// SPDX-License-Identifier: GPL-3.0-or-later
/*
File: ./src/tui/action.rs

Defines actions and events for TUI interaction and state updates.

This version removes the intent-style Toggle/Mark variants. The TUI now
performs store mutations locally and emits explicit Create/Update/Delete
actions for the network actor to persist.
*/

use crate::model::{CalendarListEntry, Task};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarMode {
    Calendars,
    Categories,
    Locations,
    Journal,
    Goals,
}

#[derive(Debug)]
pub enum Action {
    SwitchCalendar(String),
    Refresh,
    /// Reload calendars/tasks from the local disk state (no network), used to
    /// pick up changes made by other cfait instances.
    OfflineRefresh,
    Quit,
    StartCreateChild(String),
    MigrateLocal(String, String), // (source_calendar_href, target_calendar_href)
    ToggleCalendarVisibility(String),
    IsolateCalendar(String),
    ToggleTask(String), // UID
    DuplicateTask(String),
    DeleteTaskTree(String),
    PersistBatch(Vec<crate::journal::Action>), // <-- ADD THIS
    ReloadConfig,
}

#[derive(Debug)]
pub enum AppEvent {
    /// A watched data/cache file changed on disk (another cfait instance wrote it).
    ExternalChangeDetected,
    ConfigUpdated(Box<crate::config::Config>),
    CalendarsLoaded(Vec<CalendarListEntry>),
    TasksLoaded(Vec<(String, Vec<Task>)>),
    /// Full replacement of the in-memory store from disk (external change).
    /// Unlike `TasksLoaded`, this also clears calendars that no longer exist.
    FullStateReloaded(Vec<(String, Vec<Task>)>),
    TaskSynced(Box<Task>),
    /// An event that carries a stable message key plus a localized/human string.
    /// Use `key` in tests and logic for stable comparisons; `human` is intended
    /// for UI display (localized).
    Error(String),
    Status {
        key: String,
        human: String,
    },
}
