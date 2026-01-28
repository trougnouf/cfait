// File: ./src/model/mod.rs
pub mod adapter;
pub mod display;
pub mod item;
pub mod matcher;
pub mod parser;
pub mod recurrence;

// Re-export everything from item.rs so `crate::model::Task` works
pub use item::{Alarm, AlarmTrigger, CalendarListEntry, DateType, RawProperty, Task, TaskStatus};

// Re-export specific parser functions needed by other modules
pub use parser::{extract_inline_aliases, validate_alias_integrity};

// Re-export new modules
pub use adapter::IcsAdapter;
pub use display::TaskDisplay;
pub use recurrence::RecurrenceEngine;
