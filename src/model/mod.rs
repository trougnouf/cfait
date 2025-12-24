// File: src/model/mod.rs
pub mod adapter;
pub mod item;
pub mod matcher;
pub mod parser;

pub use item::{CalendarListEntry, Task, TaskStatus};
// Re-export specific parser functions needed by other modules
pub use parser::{extract_inline_aliases, validate_alias_integrity};
