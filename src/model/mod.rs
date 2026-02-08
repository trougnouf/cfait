// File: ./src/model/mod.rs
// Central model module re-exports to make types available as `crate::model::*`.

pub mod adapter;
pub mod display;
pub mod item;
pub mod matcher;
pub mod merge;
pub mod parser;
pub mod recurrence;

// Re-export everything from `item.rs` so `crate::model::Task` and related types work.
pub use item::{
    Alarm, AlarmTrigger, CalendarListEntry, DateType, RawProperty, Task, TaskStatus, VirtualState,
};

// Re-export specific parser helpers used across the codebase.
pub use parser::{extract_inline_aliases, validate_alias_integrity};

// Re-export adapter/display/recurrence helpers for external use.
pub use adapter::IcsAdapter;
pub use display::TaskDisplay;
pub use recurrence::RecurrenceEngine;
