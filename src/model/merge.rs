// File: ./src/model/merge.rs
use crate::model::Task;

/// Performs a 3-way merge between a base state, a local modification, and a server state.
/// Returns Some(merged_task) if successful, or None if a hard conflict exists.
pub fn three_way_merge(base: &Task, local: &Task, server: &Task) -> Option<Task> {
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
    merge_field!(rrule);
    merge_field!(percent_complete);
    merge_field!(location);
    merge_field!(url);
    merge_field!(geo);
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
        for prop in &local.unmapped_properties {
            if !merged.unmapped_properties.iter().any(|p| p.key == prop.key) {
                merged.unmapped_properties.push(prop.clone());
            }
        }
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
