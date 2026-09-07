// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for goal progress with cascade interval dedup.
//!
//! Tags are explicitly copied onto subtasks at creation time (via
//! `inherit_properties` or the GUI pre-populating the input). The user can
//! remove them. Goal matching is therefore explicit-only — no parent-chain
//! walk. These tests verify that when both parent and child carry the same
//! tag, overlapping cascade sessions don't double-count, but independent
//! parent time (e.g. after pausing a subtask) still counts.
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

/// A subtask that does NOT carry the parent's tag explicitly must not count
/// toward the goal. Removing the inherited tag is a deliberate opt-out.
#[test]
fn subtask_without_explicit_tag_does_not_count() {
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
    assert_eq!(progress, 0, "subtask without explicit tag must not count");
}

/// A subtask that explicitly carries the parent's tag counts toward the goal.
#[test]
fn subtask_with_explicit_tag_counts() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    store.add_task(parent);

    let mut child = Task::new("Subtask #work", &HashMap::new(), None);
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
    assert_eq!(progress, 60, "subtask with explicit tag should count");
}

/// When a subtask's session overlaps with the parent's session (cascade from
/// `set_status_in_process` starting ancestors), the parent's overlapping
/// session is interval-subtracted. Only the subtask's time counts for the
/// overlap; the parent's independent remainder still counts.
#[test]
fn cascaded_parent_session_does_not_double_count() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask #work", &HashMap::new(), None);
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
    // Both carry the tag. Child: 60 min (full, no descendants).
    // Parent: 60 min - 60 min (child overlap) = 0 min. Total = 60.
    assert_eq!(
        progress, 60,
        "overlapping parent session subtracted, no double-count"
    );
}

/// A parent session that does NOT overlap any descendant session still counts.
#[test]
fn non_overlapping_parent_session_still_counts() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 500_000,
        end: 500_000 + 1800,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask #work", &HashMap::new(), None);
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
    // Parent: 30 min (non-overlapping) + child: 60 min = 90 min.
    assert_eq!(
        progress, 90,
        "non-overlapping parent session should still count"
    );
}

/// Count goals: a subtask session counts, but an overlapping parent session
/// is fully subtracted (0 remaining) and skipped.
#[test]
fn count_goal_dedup() {
    let mut store = make_store_with_session_counting();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 3600,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask #work", &HashMap::new(), None);
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
    // Child has 2 sessions (both count). Parent's 1 session overlaps child's
    // first fully (0 remaining) and is skipped. Total = 2.
    assert_eq!(
        progress, 2,
        "overlapping parent session skipped, child sessions count"
    );
}

/// Staggered pause: leaf paused at T1, root paused later at T2. Both carry
/// the tag. The leaf's session counts in full; the root's session is
/// interval-subtracted by the leaf's overlap, so the [T1, T2] gap counts as
/// independent root work.
#[test]
fn staggered_pause_counts_root_gap() {
    let mut store = make_store();

    let mut parent = Task::new("Parent #work", &HashMap::new(), None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal".to_string();
    parent.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 5400,
    });
    store.add_task(parent);

    let mut child = Task::new("Subtask #work", &HashMap::new(), None);
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
    // Child: 60 min (full). Parent: 90 min - 60 min overlap = 30 min. Total = 90.
    assert_eq!(
        progress, 90,
        "staggered pause: leaf counts fully, root gap counts as independent work"
    );
}

/// Three-level tree: root -> mid -> leaf. All three carry the tag explicitly.
/// Each level has a staggered session. Interval subtraction counts every
/// second exactly once.
#[test]
fn three_level_tree_staggered_sessions() {
    let mut store = make_store();

    let mut root = Task::new("Root #work", &HashMap::new(), None);
    root.uid = "root".to_string();
    root.calendar_href = "cal".to_string();
    root.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 9000,
    });
    store.add_task(root);

    let mut mid = Task::new("Mid #work", &HashMap::new(), None);
    mid.uid = "mid".to_string();
    mid.calendar_href = "cal".to_string();
    mid.parent_uid = Some("root".to_string());
    mid.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 6000,
    });
    store.add_task(mid);

    let mut leaf = Task::new("Leaf #work", &HashMap::new(), None);
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

    // Leaf: 60 min (no descendants).
    // Mid: 100 min - 60 min (leaf overlap) = 40 min.
    // Root: 150 min - 100 min (merged leaf+mid overlap) = 50 min.
    // Total = 150 min.
    assert_eq!(
        progress, 150,
        "three-level tree: each second counted exactly once"
    );
}

/// Three-level tree where only mid and leaf carry the tag. Root does NOT
/// carry it, so root is excluded despite being an ancestor.
#[test]
fn three_level_tree_tag_on_mid_only() {
    let mut store = make_store();

    let mut root = Task::new("Root", &HashMap::new(), None);
    root.uid = "root".to_string();
    root.calendar_href = "cal".to_string();
    root.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 9000,
    });
    store.add_task(root);

    let mut mid = Task::new("Mid #work", &HashMap::new(), None);
    mid.uid = "mid".to_string();
    mid.calendar_href = "cal".to_string();
    mid.parent_uid = Some("root".to_string());
    mid.sessions.push(WorkSession {
        start: 1_000_000,
        end: 1_000_000 + 6000,
    });
    store.add_task(mid);

    let mut leaf = Task::new("Leaf #work", &HashMap::new(), None);
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

    // Root excluded (no tag). Leaf: 60 min. Mid: 100 - 60 = 40 min. Total = 100.
    assert_eq!(
        progress, 100,
        "tag on mid only: root excluded, mid+leaf counted once"
    );
}
