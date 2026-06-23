// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/merge.rs
use crate::model::Task;
use std::collections::HashSet;

/// A generic set-based 3-way merge for lists.
/// Formula: (Local U Server) \ (Deleted by Local) \ (Deleted by Server)
/// We use `PartialEq` and `Vec::contains` because N is very small (tags, deps) and some types lack `Hash`.
fn merge_lists<T: Clone + PartialEq>(base: &[T], local: &[T], server: &[T]) -> Vec<T> {
    let mut result = Vec::new();
    let mut union = Vec::new();

    for item in local.iter().chain(server.iter()) {
        if !union.contains(item) {
            union.push(item.clone());
        }
    }

    let deleted_by_local = |item: &T| base.contains(item) && !local.contains(item);
    let deleted_by_server = |item: &T| base.contains(item) && !server.contains(item);

    for item in union {
        if !deleted_by_local(&item) && !deleted_by_server(&item) {
            result.push(item);
        }
    }
    result
}

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

    // Standard properties
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
    merge_field!(collapsed);
    merge_field!(pinned);
    merge_field!(goal);
    merge_field!(last_started_at);
    merge_field!(parent_uid);

    // List properties (Set-based 3-way merge to handle deletions correctly)
    merged.categories = merge_lists(&base.categories, &local.categories, &server.categories);
    merged.categories.sort();

    merged.dependencies = merge_lists(
        &base.dependencies,
        &local.dependencies,
        &server.dependencies,
    );
    merged.related_to = merge_lists(&base.related_to, &local.related_to, &server.related_to);
    merged.exdates = merge_lists(&base.exdates, &local.exdates, &server.exdates);

    merged.sessions = merge_lists(&base.sessions, &local.sessions, &server.sessions);
    merged.sessions.sort_by_key(|s| s.start);

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

    // Smart merge for Alarms (UID-based merge to handle `acknowledged` timing discrepancies safely)
    let mut merged_alarms = Vec::new();
    let mut all_alarm_uids = HashSet::new();
    for a in &base.alarms {
        all_alarm_uids.insert(&a.uid);
    }
    for a in &local.alarms {
        all_alarm_uids.insert(&a.uid);
    }
    for a in &server.alarms {
        all_alarm_uids.insert(&a.uid);
    }

    for uid in all_alarm_uids {
        let b = base.alarms.iter().find(|a| &a.uid == uid);
        let l = local.alarms.iter().find(|a| &a.uid == uid);
        let s = server.alarms.iter().find(|a| &a.uid == uid);

        let resolved = match (l, b, s) {
            (Some(lv), Some(bv), Some(sv)) => {
                if lv == bv {
                    Some(sv.clone())
                } else if sv == bv {
                    Some(lv.clone())
                } else if lv == sv {
                    Some(sv.clone())
                } else {
                    // Both modified.
                    let mut merged_alarm = sv.clone();
                    // Safely merge `acknowledged` by taking the earliest timestamp
                    if lv.acknowledged != bv.acknowledged && sv.acknowledged != bv.acknowledged {
                        merged_alarm.acknowledged = match (lv.acknowledged, sv.acknowledged) {
                            (Some(lt), Some(st)) => Some(lt.min(st)),
                            (Some(lt), None) => Some(lt),
                            (None, Some(st)) => Some(st),
                            (None, None) => None,
                        };
                    } else if lv.acknowledged != bv.acknowledged {
                        merged_alarm.acknowledged = lv.acknowledged;
                    }

                    // Check if any other core fields differ
                    let mut test_lv = lv.clone();
                    let mut test_sv = sv.clone();
                    test_lv.acknowledged = None;
                    test_sv.acknowledged = None;
                    if test_lv != test_sv {
                        return None; // Hard conflict on trigger/description/etc.
                    }
                    Some(merged_alarm)
                }
            }
            (Some(lv), None, None) => Some(lv.clone()),
            (None, None, Some(sv)) => Some(sv.clone()),
            (None, Some(_), None) => None, // Deleted on both
            (None, Some(bv), Some(sv)) => {
                if sv == bv {
                    None
                } else {
                    return None; /* Deleted locally, modified server */
                }
            }
            (Some(lv), Some(bv), None) => {
                if lv == bv {
                    None
                } else {
                    return None; /* Modified locally, deleted server */
                }
            }
            (Some(lv), None, Some(sv)) => {
                // Highly unlikely: same UID generated concurrently. Try to merge safely.
                if lv == sv {
                    Some(sv.clone())
                } else {
                    let mut merged_alarm = sv.clone();
                    merged_alarm.acknowledged = match (lv.acknowledged, sv.acknowledged) {
                        (Some(lt), Some(st)) => Some(lt.min(st)),
                        (Some(lt), None) => Some(lt),
                        (None, Some(st)) => Some(st),
                        (None, None) => None,
                    };
                    let mut test_lv = lv.clone();
                    let mut test_sv = sv.clone();
                    test_lv.acknowledged = None;
                    test_sv.acknowledged = None;
                    if test_lv != test_sv {
                        return None;
                    }
                    Some(merged_alarm)
                }
            }
            _ => None,
        };

        if let Some(a) = resolved {
            merged_alarms.push(a);
        }
    }
    merged.alarms = merged_alarms;

    // Unmapped properties
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
