// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for goal progress with subtree aggregation and cascade dedup.
//!
//! A task's time counts toward a goal if the task or any ancestor carries the
//! matching tag/location. Sessions from all claimed tasks are union-merged so
//! overlapping cascade sessions (from auto-starting ancestors) collapse —
//! each second of work counts exactly once.
use cfait::config::{Goal, GoalType, Interval, IntervalUnit};
use cfait::context::AppContext;
use cfait::context::TestContext;
use cfait::model::Task;
use cfait::model::item::WorkSession;
use cfait::store::TaskStore;
use std::collections::HashMap;
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn make_store_with_session_counting() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    let config_dir = ctx.get_config_dir().unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "sessions_count_as_completions = true\n",
    )
    .unwrap();
    TaskStore::new(ctx)
}

fn make_goal(goal_type: GoalType, target: u32) -> Goal {
    Goal {
        goal_type,
        target,
        interval: Interval {
            amount: 1,
            unit: IntervalUnit::Weeks,
        },
    }
}

/// A subtask without the tag still counts toward the parent's goal via
/// subtree aggregation. The parent's tag "claims" all untagged descendants.
#[test]
fn untagged_subtask_counts_via_parent_tag() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);
    assert_eq!(
        progress, 60,
        "untagged subtask should count via parent's tag (aggregation)"
    );
}

/// Cascade: parent and child both have overlapping sessions (from
/// auto-start). Union-merge collapses the overlap — total is 60 min, not 120.
#[test]
fn cascade_overlap_collapsed_by_union_merge() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);
    assert_eq!(
        progress, 60,
        "overlapping cascade sessions must collapse to one"
    );
}

/// Staggered pause: child paused at T1, parent paused at T2. Union-merge
/// produces [start, T2] = 90 min. The gap [T1, T2] counts as independent
/// parent work.
#[test]
fn staggered_pause_counts_full_span() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 5400,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);
    // Union of [start, T2] and [start, T1] = [start, T2] = 90 min.
    assert_eq!(
        progress, 90,
        "staggered pause: union spans full parent session"
    );
}

/// Non-overlapping sessions from parent and child both count.
#[test]
fn non_overlapping_sessions_both_count() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 500_000,
        end: 500_000 + 1800,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);
    // Parent: 30 min + child: 60 min = 90 min.
    assert_eq!(
        progress, 90,
        "non-overlapping sessions from parent and child both count"
    );
}

/// Three-level tree: root -> mid -> leaf. Tag on root only. All three have
/// staggered sessions. Union-merge gives the full span [start, T_root] = 150 min.
#[test]
fn three_level_tree_union() {
    let mut store = make_store();

    let mut root = Task::new("Root #work", &HashMap::new(), None);
    root.uid = "root".to_string();
    root.calendar_href = "cal".to_string();
    root.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 9000,
    });
    store.add_task(root);

    let mut mid = Task::new("Mid", &HashMap::new(), None);
    mid.uid = "mid".to_string();
    mid.calendar_href = "cal".to_string();
    mid.parent_uid = Some("root".to_string());
    mid.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 6000,
    });
    store.add_task(mid);

    let mut leaf = Task::new("Leaf", &HashMap::new(), None);
    leaf.uid = "leaf".to_string();
    leaf.calendar_href = "cal".to_string();
    leaf.parent_uid = Some("mid".to_string());
    leaf.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(leaf);

    let goal = make_goal(GoalType::Duration, 300);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);

    // All nested inside each other. Union = [start, T_root] = 150 min.
    assert_eq!(progress, 150, "three-level tree: union gives full span");
}

/// Three-level tree with non-overlapping sessions across levels.
/// Root: [0, 50], mid: [50, 100], leaf: [100, 150]. Total = 150 min.
#[test]
fn three_level_tree_non_overlapping() {
    let mut store = make_store();

    let mut root = Task::new("Root #work", &HashMap::new(), None);
    root.uid = "root".to_string();
    root.calendar_href = "cal".to_string();
    root.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3000,
    });
    store.add_task(root);

    let mut mid = Task::new("Mid", &HashMap::new(), None);
    mid.uid = "mid".to_string();
    mid.calendar_href = "cal".to_string();
    mid.parent_uid = Some("root".to_string());
    mid.sessions.push(WorkSession {
        start: 1_000_000 + 3000,
        end: 1_000_000 + 6000,
    });
    store.add_task(mid);

    let mut leaf = Task::new("Leaf", &HashMap::new(), None);
    leaf.uid = "leaf".to_string();
    leaf.calendar_href = "cal".to_string();
    leaf.parent_uid = Some("mid".to_string());
    leaf.sessions.push(WorkSession {
        start: 1_000_000 + 6000,
        end: 1_000_000 + 9000,
    });
    store.add_task(leaf);

    let goal = make_goal(GoalType::Duration, 300);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);

    // Three non-overlapping sessions: 50 + 50 + 50 = 150 min.
    assert_eq!(
        progress, 150,
        "three-level tree: non-overlapping sessions sum up"
    );
}

/// A task outside the tagged subtree does NOT count.
#[test]
fn unrelated_task_does_not_count() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    // Unrelated task with no tag and no tagged ancestor.
    let mut other = Task::new("Other", &HashMap::new(), None);
    other.uid = "other".to_string();
    other.calendar_href = "cal".to_string();
    other.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(other);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 2_000_000);
    assert_eq!(progress, 60, "unrelated task without tag must not count");
}

/// Count goals: per-task session counting with cascade dedup. A parent's
/// session fully covered by a descendant's overlapping session is skipped.
/// Non-overlapping child sessions still count.
#[test]
fn count_goal_cascade_and_completion() {
    let mut store = make_store_with_session_counting();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    // Second non-overlapping child session.
    child.sessions.push(WorkSession {
        start: 2_000_000,
        end: 2_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Count, 5);
    let progress = store.calculate_goal_progress_for_bounds("#work", &goal, 0, 3_000_000);
    // Parent: 1 session, fully covered by child's overlap → skipped (0).
    // Child: 2 sessions, no descendants → counts 2.
    assert_eq!(
        progress, 2,
        "count: parent cascade skipped, child sessions count"
    );
}

/// Task-specific goal: a goal on a task claims its descendants' time too.
#[test]
fn task_specific_goal_aggregates_descendants() {
    let mut store = make_store();

    let mut parent = Task::new("Parent", &HashMap::new(), None);
    parent.uid = "target".to_string();
    parent.calendar_href = "cal".to_string();
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("target".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    let goal = make_goal(GoalType::Duration, 120);
    let progress = store.calculate_goal_progress_for_bounds("task:target", &goal, 0, 2_000_000);
    assert_eq!(
        progress, 60,
        "task-specific goal should aggregate descendant time"
    );
}

/// get_aggregated_time_seconds: a parent's aggregated time includes its own
/// sessions plus all descendants', with overlapping cascade sessions merged.
#[test]
fn aggregated_time_cascade_merge() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 5400,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    // Parent: [0, 90min]. Child: [0, 60min]. Union = [0, 90min] = 5400s.
    let agg = store.get_aggregated_time_seconds("parent");
    assert_eq!(agg, 5400, "aggregated time should be union-merged");
}

/// get_aggregated_time_seconds: non-overlapping sessions from parent and
/// child both count.
#[test]
fn aggregated_time_non_overlapping() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 500_000,
        end: 500_000 + 1800,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask", &HashMap::new(), None);
    child.uid = "child".to_string();
    child.calendar_href = "cal".to_string();
    child.parent_uid = Some("parent".to_string());
    child.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(child);

    // Parent: 1800s + child: 3600s = 5400s (non-overlapping).
    let agg = store.get_aggregated_time_seconds("parent");
    assert_eq!(agg, 5400, "non-overlapping sessions should sum up");
}

/// get_aggregated_time_seconds: a leaf task returns its own time only.
#[test]
fn aggregated_time_leaf_task() {
    let mut store = make_store();

    let mut task = Task::new("Leaf #work", &HashMap::new(), None);
    task.uid = "leaf".to_string();
    task.calendar_href = "cal".to_string();
    task.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(task);

    let agg = store.get_aggregated_time_seconds("leaf");
    assert_eq!(agg, 3600, "leaf task should return its own time");
}

/// get_aggregated_time_seconds: three-level tree with nested sessions.
#[test]
fn aggregated_time_three_level_tree() {
    let mut store = make_store();

    let mut root = Task::new("Root", &HashMap::new(), None);
    root.uid = "root".to_string();
    root.calendar_href = "cal".to_string();
    root.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 9000,
    });
    store.add_task(root);

    let mut mid = Task::new("Mid", &HashMap::new(), None);
    mid.uid = "mid".to_string();
    mid.calendar_href = "cal".to_string();
    mid.parent_uid = Some("root".to_string());
    mid.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 6000,
    });
    store.add_task(mid);

    let mut leaf = Task::new("Leaf", &HashMap::new(), None);
    leaf.uid = "leaf".to_string();
    leaf.calendar_href = "cal".to_string();
    leaf.parent_uid = Some("mid".to_string());
    leaf.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(leaf);

    // All nested: union = [0, 9000s] = root's full session.
    let agg = store.get_aggregated_time_seconds("root");
    assert_eq!(agg, 9000, "three-level tree: union gives root's full span");
}
