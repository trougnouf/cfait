// File: ./src/model/mod.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Central model module re-exports to make types available as `crate::model::*`.

pub mod adapter;
pub mod autocomplete;
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
pub use parser::{
    expand_braces, extract_inline_aliases, extract_inline_goals, validate_alias_integrity,
};

// Re-export extractor for markdown task extraction
pub use extractor::{ExtractedTask, extract_markdown_tasks, has_extractable_subtasks};

// Re-export adapter/display/recurrence helpers for external use.
pub use adapter::IcsAdapter;
pub use display::TaskDisplay;
pub use recurrence::RecurrenceEngine;

// Re-export session model for UI state management
pub use session::{AppIntent, SessionState};

pub fn compare_calendars(
    href_a: &str,
    name_a: &str,
    href_b: &str,
    name_b: &str,
    order: &[String],
) -> std::cmp::Ordering {
    let rank_a = if href_a == crate::storage::LOCAL_TRASH_HREF {
        4
    } else if href_a == "local://recovery" {
        3
    } else {
        1
    };

    let rank_b = if href_b == crate::storage::LOCAL_TRASH_HREF {
        4
    } else if href_b == "local://recovery" {
        3
    } else {
        1
    };

    if rank_a != rank_b {
        return rank_a.cmp(&rank_b);
    }

    let idx_a = order.iter().position(|h| h == href_a).unwrap_or(usize::MAX);
    let idx_b = order.iter().position(|h| h == href_b).unwrap_or(usize::MAX);

    if idx_a != idx_b {
        return idx_a.cmp(&idx_b);
    }

    name_a.cmp(name_b)
}

/// Compare calendars with optional size information.
/// When sizes are provided and differ, larger collections come first,
/// but Trash and Recovery are always sorted below standard collections.
pub fn compare_calendars_with_size(
    href_a: &str,
    name_a: &str,
    count_a: usize,
    href_b: &str,
    name_b: &str,
    count_b: usize,
    order: &[String],
) -> std::cmp::Ordering {
    // First, check if one is a system collection (Trash/Recovery) and the other is not
    let a_is_system = href_a == crate::storage::LOCAL_TRASH_HREF || href_a == "local://recovery";
    let b_is_system = href_b == crate::storage::LOCAL_TRASH_HREF || href_b == "local://recovery";

    // If one is system and the other is standard, standard always comes first
    if a_is_system && !b_is_system {
        return std::cmp::Ordering::Greater;
    }
    if !a_is_system && b_is_system {
        return std::cmp::Ordering::Less;
    }

    // Both are standard or both are system
    // For standard collections, compare by size (descending)
    // For system collections, we still want Recovery before Trash regardless of size
    if !a_is_system && !b_is_system {
        // Both are standard, compare by size
        if count_a != count_b {
            return count_b.cmp(&count_a);
        }
    }

    // Fall back to regular comparison which handles:
    // - Recovery vs Trash ordering (Recovery has rank 3, Trash has rank 4)
    // - Custom order
    // - Alphabetical
    compare_calendars(href_a, name_a, href_b, name_b, order)
}

pub fn resolve_collection(target: &str, calendars: &[CalendarListEntry], default: &str) -> String {
    let lower = target.to_lowercase();
    if let Some(c) = calendars
        .iter()
        .find(|c| c.name.to_lowercase().contains(&lower) || c.href.to_lowercase().contains(&lower))
    {
        c.href.clone()
    } else {
        default.to_string()
    }
}
