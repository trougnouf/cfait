// File: ./src/model/mod.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Central model module re-exports to make types available as `crate::model::*`.

pub mod adapter;
pub mod autocomplete;
pub mod display;
pub mod extractor;
pub mod fractional;
pub mod item;
pub mod matcher;
pub mod merge;
pub mod parser;
pub mod recurrence;
pub mod session;

// Re-export everything from `item.rs` so `crate::model::Task` and related types work.
pub use item::{
    Alarm, AlarmTrigger, CalendarListEntry, DateType, PENDING_REFRESH_ETAG, RawProperty, Task,
    TaskStatus, resolve_default_target, resolve_journal_target,
};

// Re-export specific parser helpers used across the codebase.
pub use parser::{
    expand_braces, extract_inline_aliases, extract_inline_goals, find_useless_alias_values,
    is_pure_alias_remainder, validate_alias_integrity,
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
    } else if href_a == crate::storage::LOCAL_RECOVERY_HREF {
        3
    } else {
        1
    };

    let rank_b = if href_b == crate::storage::LOCAL_TRASH_HREF {
        4
    } else if href_b == crate::storage::LOCAL_RECOVERY_HREF {
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
    let a_is_system = crate::storage::is_system_calendar(href_a);
    let b_is_system = crate::storage::is_system_calendar(href_b);

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
    let norm = lower.trim_end_matches('/');
    // Exact name first, then exact href, then name substring. Href substring
    // is deliberately not used: CalDAV hrefs embed the account path, so a
    // collection named like the account ("trougnouf") would resolve to
    // whichever remote collection happens to be listed first.
    if let Some(c) = calendars.iter().find(|c| c.name.to_lowercase() == lower) {
        return c.href.clone();
    }
    if let Some(c) = calendars
        .iter()
        .find(|c| c.href.to_lowercase().trim_end_matches('/') == norm)
    {
        return c.href.clone();
    }
    if let Some(c) = calendars
        .iter()
        .find(|c| c.name.to_lowercase().contains(&lower))
    {
        return c.href.clone();
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, href: &str) -> CalendarListEntry {
        CalendarListEntry {
            name: name.to_string(),
            href: href.to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        }
    }

    #[test]
    fn resolve_collection_prefers_exact_name_over_shared_href() {
        // Remote collections all share the /trougnouf/ account path, so a
        // target equal to the account must not resolve to whichever remote
        // href happens to be listed first.
        let calendars = vec![
            entry("todo", "/trougnouf/5f710d0c-aaaa-bbbb-cccc-000000000003/"),
            entry(
                "trougnouf",
                "/trougnouf/18ead562-aaaa-bbbb-cccc-000000000001/",
            ),
            entry(
                "Workout",
                "/trougnouf/07f0b86f-aaaa-bbbb-cccc-000000000002/",
            ),
            entry("Herbary", "local://herbary"),
        ];

        assert_eq!(
            resolve_collection("trougnouf", &calendars, "local://default"),
            "/trougnouf/18ead562-aaaa-bbbb-cccc-000000000001/"
        );
        // Case-insensitive exact name
        assert_eq!(
            resolve_collection("WORKOUT", &calendars, "local://default"),
            "/trougnouf/07f0b86f-aaaa-bbbb-cccc-000000000002/"
        );
        // Exact href, with or without the trailing slash
        assert_eq!(
            resolve_collection("local://herbary", &calendars, "local://default"),
            "local://herbary"
        );
        assert_eq!(
            resolve_collection(
                "/trougnouf/5f710d0c-aaaa-bbbb-cccc-000000000003",
                &calendars,
                "local://default"
            ),
            "/trougnouf/5f710d0c-aaaa-bbbb-cccc-000000000003/"
        );
        // Name substring
        assert_eq!(
            resolve_collection("herb", &calendars, "local://default"),
            "local://herbary"
        );
        // Unknown target falls back to the default
        assert_eq!(
            resolve_collection("balcony", &calendars, "local://default"),
            "local://default"
        );
    }
}
