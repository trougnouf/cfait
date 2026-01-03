// Module declarations for the data model.
pub mod adapter;
pub mod item;
pub mod matcher;
pub mod parser;

// Re-export everything from item.rs so `crate::model::Task` works
pub use item::{Alarm, AlarmTrigger, CalendarListEntry, DateType, RawProperty, Task, TaskStatus};

// Re-export specific parser functions needed by other modules
pub use parser::{extract_inline_aliases, validate_alias_integrity};
