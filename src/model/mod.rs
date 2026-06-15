// File: ./src/model/mod.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Central model module re-exports to make types available as `crate::model::*`.

pub mod adapter;
pub mod display;
pub mod extractor;
pub mod item;
pub mod matcher;
pub mod merge;
pub mod parser;
pub mod recurrence;
pub mod session;

// Re-export everything from `item.rs` so `crate::model::Task` and related types work.
pub use item::{Alarm, AlarmTrigger, CalendarListEntry, DateType, RawProperty, Task, TaskStatus};

// Re-export specific parser helpers used across the codebase.
pub use parser::{expand_braces, extract_inline_aliases, extract_inline_goals, validate_alias_integrity};

// Re-export extractor for markdown task extraction
pub use extractor::{ExtractedTask, extract_markdown_tasks};

// Re-export adapter/display/recurrence helpers for external use.
pub use adapter::IcsAdapter;
pub use display::TaskDisplay;
pub use recurrence::RecurrenceEngine;

// Re-export session model for UI state management
pub use session::{AppIntent, SessionState};
