// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/merge.rs
use crate::model::Task;
use std::collections::HashSet;

/// Performs a 3-way merge between a base state, a local modification, and a server state.
/// Returns Some(merged_task) if successful, or None if a hard conflict exists.
pub fn three_way_merge(base: &Task, local: &Task, server: &Task) -> Option<Task> {
    // Exhaustive destructuring to ensure compile-time errors if new fields are added to Task.
    // If you add a new field to Task, you MUST also add it here AND to the merge logic below.
    let Task {
        uid: _,
        summary: _,
        description: _,
        status: _,
        estimated_duration: _,
        estimated_duration_max: _,
        due: _,
        dtstart: _,
        alarms: _,
        exdates: _,
        priority: _,
        percent_complete: _,
        parent_uid: _,
        dependencies: _,
        related_to: _,
        etag: _,
        href: _,
        calendar_href: _,
        categories: _,
        depth: _,
        rrule: _,
        location: _,
        url: _,
        geo: _,
        collapsed: _,
        pinned: _,
        time_spent_seconds: _,
        last_started_at: _,
        sessions: _,
        unmapped_properties: _,
        sequence: _,
        raw_alarms: _,
        raw_components: _,
        create_event: _,
        goal: _,
        is_blocked: _,
        is_implicitly_blocked: _,
        is_implicitly_future: _,
        has_subtasks: _,
        has_visible_subtasks: _,
        sort_rank: _,
        effective_priority: _,
        effective_due: _,
        effective_dtstart: _,
        visible_categories: _,
        visible_location: _,
        has_blocking_tasks: _,
        has_related_tasks: _,
        is_future_start: _,
        is_overdue: _,
    } = local;

    let mut merged = server.clone();

    macro_rules! merge_field {
        ($field:ident) => {
            if local.$field != base.$field {
                if server.$field == base.$field {
                    merged.$field = local.$field.clone();
                } else if local.$field != server.$field {
                    return None; // Conflict cannot be resolved automatically
                }
            }
        };
    }

    merge_field!(summary);
    merge_field!(description);
    merge_field!(status);
    merge_field!(priority);
    merge_field!(due);
    merge_field!(dtstart);
    merge_field!(estimated_duration);
    merge_field!(estimated_duration_max);
    merge_field!(rrule);
    merge_field!(percent_complete);
    merge_field!(location);
    merge_field!(url);
    merge_field!(geo);
    merge_field!(create_event);
    merge_field!(alarms);
    merge_field!(exdates);
    merge_field!(collapsed);
    merge_field!(pinned);
    merge_field!(goal);
    // Smart merge for time tracking (accumulate offline time from both clients)
    if local.time_spent_seconds != base.time_spent_seconds
        || server.time_spent_seconds != base.time_spent_seconds
    {
        let local_diff = local
            .time_spent_seconds
            .saturating_sub(base.time_spent_seconds);
        let server_diff = server
            .time_spent_seconds
            .saturating_sub(base.time_spent_seconds);
        merged.time_spent_seconds = base.time_spent_seconds + local_diff + server_diff;
    }

    // Smart merge for sessions (union both lists)
    if local.sessions != base.sessions || server.sessions != base.sessions {
        let mut all_sessions = server.sessions.clone();
        for local_session in &local.sessions {
            if !all_sessions.contains(local_session) {
                all_sessions.push(local_session.clone());
            }
        }
        all_sessions.sort_by_key(|s| s.start);
        merged.sessions = all_sessions;
    }

    merge_field!(last_started_at);

    if local.categories != base.categories {
        let mut new_cats = server.categories.clone();
        for cat in &local.categories {
            if !new_cats.contains(cat) {
                new_cats.push(cat.clone());
            }
        }
        new_cats.sort();
        new_cats.dedup();
        merged.categories = new_cats;
    }

    if local.unmapped_properties != base.unmapped_properties {
        let mut merged_props = Vec::new();
        let mut all_keys = HashSet::new();
        for p in &local.unmapped_properties {
            all_keys.insert(&p.key);
        }
        for p in &base.unmapped_properties {
            all_keys.insert(&p.key);
        }
        for p in &server.unmapped_properties {
            all_keys.insert(&p.key);
        }

        for key in all_keys {
            let l = local.unmapped_properties.iter().find(|p| &p.key == key);
            let b = base.unmapped_properties.iter().find(|p| &p.key == key);
            let s = server.unmapped_properties.iter().find(|p| &p.key == key);

            let chosen = match (l, b, s) {
                (Some(l_val), Some(b_val), Some(s_val)) => {
                    if l_val != b_val {
                        if s_val == b_val {
                            Some(l_val)
                        } else if l_val != s_val {
                            return None; // Conflict cannot be resolved automatically
                        } else {
                            Some(l_val)
                        }
                    } else {
                        Some(s_val)
                    }
                }
                (Some(l_val), Some(_), None) => Some(l_val),
                (Some(l_val), None, Some(_)) => Some(l_val),
                (Some(l_val), None, None) => Some(l_val),
                (None, Some(_), Some(s_val)) => Some(s_val),
                (None, Some(_), None) => None,
                (None, None, Some(s_val)) => Some(s_val),
                (None, None, None) => None,
            };

            if let Some(prop) = chosen {
                merged_props.push((*prop).clone());
            }
        }
        merged_props.sort_unstable_by(|a, b| a.key.cmp(&b.key).then(a.value.cmp(&b.value)));
        merged.unmapped_properties = merged_props;
    }

    merge_field!(parent_uid);
    if local.dependencies != base.dependencies {
        let mut new_deps = server.dependencies.clone();
        for dep in &local.dependencies {
            if !new_deps.contains(dep) {
                new_deps.push(dep.clone());
            }
        }
        merged.dependencies = new_deps;
    }

    Some(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_three_way_merge_preserves_new_fields() {
        let mut base = Task::new("Base Task", &HashMap::new(), None);
        base.location = Some("Old Loc".to_string());
        base.url = None;

        // Local client changed Location
        let mut local = base.clone();
        local.location = Some("New Loc".to_string());

        // Server client changed Summary
        let mut server = base.clone();
        server.summary = "Server Title Change".to_string();

        let merged = three_way_merge(&base, &local, &server).expect("Should merge successfully");

        assert_eq!(
            merged.summary, "Server Title Change",
            "Failed to keep server's summary change"
        );
        assert_eq!(
            merged.location,
            Some("New Loc".to_string()),
            "Failed to keep local's location change"
        );
    }
}
