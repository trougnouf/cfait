// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for task sorting logic.
use cfait::config::SortPreset;
use cfait::model::item::{CompareOptions, SortKey, compare_sortkeys};
use cfait::model::{DateType, Task, TaskStatus};
use cfait::store::organize_hierarchy;
use chrono::{Duration, Utc};
use std::collections::{HashMap, HashSet};

fn task(summary: &str) -> Task {
    Task::new(summary, &HashMap::new(), None)
}

#[test]
fn test_sorting_priority_basic() {
    let mut high = task("A");
    high.priority = 1;

    let mut low = task("B");
    low.priority = 9;

    let mut none = task("C");
    none.priority = 0; // 0 is treated as normal (5) priority in sorting logic usually

    // 1 < 9
    assert_eq!(
        high.compare_with_cutoff(
            &low,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ), // Pass defaults
        std::cmp::Ordering::Less
    );

    // 1 < 0 (High vs None/Normal)
    assert_eq!(
        high.compare_with_cutoff(
            &none,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ), // Pass defaults
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_sorting_status_trumps_everything() {
    // An active task (InProcess) with low priority
    let mut active = task("Active Low Prio");
    active.priority = 9;
    active.status = TaskStatus::InProcess;
    active.effective_priority = 9;

    // A waiting task with critical priority
    let mut critical = task("Critical Waiting");
    critical.priority = 1;
    critical.status = TaskStatus::NeedsAction;
    critical.effective_priority = 1;

    // With the urgency logic, tasks that are urgent may beat started tasks.
    // Expect critical urgent task to sort before active started task here.
    assert_eq!(
        critical.compare_with_cutoff(
            &active,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_sorting_completed_sinks() {
    let mut done = task("Done");
    done.status = TaskStatus::Completed;
    done.priority = 1;

    let mut todo = task("Todo");
    todo.status = TaskStatus::NeedsAction;
    todo.priority = 9;

    // Todo should come FIRST (Less), Done should sink (Greater)
    assert_eq!(
        todo.compare_with_cutoff(
            &done,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_sorting_due_dates() {
    let now = Utc::now();

    let mut t1 = task("Due Soon");
    t1.due = Some(DateType::Specific(now + Duration::days(1)));

    let mut t2 = task("Due Later");
    t2.due = Some(DateType::Specific(now + Duration::days(5)));

    let mut t3 = task("No Date");
    t3.due = None;

    // Soon < Later
    assert_eq!(
        t1.compare_with_cutoff(
            &t2,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ),
        std::cmp::Ordering::Less
    );

    // Date < No Date
    assert_eq!(
        t2.compare_with_cutoff(
            &t3,
            &CompareOptions {
                cutoff: None,
                urgent_days: 1,
                urgent_prio: 1,
                default_priority: 5,
                start_grace_period_days: 1,
                sort_standard_by_priority: false,
                sort_preset: SortPreset::default()
            }
        ),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_hierarchy_organization() {
    // Test that children follow parents
    let mut parent = task("Parent");
    parent.uid = "p1".to_string();

    let mut child = task("Child");
    child.uid = "c1".to_string();
    child.parent_uid = Some("p1".to_string());

    let tasks = vec![child.clone(), parent.clone()];

    // This function rebuilds the visual list (flattened tree)
    // Updated signature uses HierarchyOptions.
    let organized = organize_hierarchy(
        tasks,
        cfait::store::HierarchyOptions {
            default_priority: 5,
            sort_standard_by_priority: false,
            expanded_groups: &HashSet::new(),
            max_done_roots: usize::MAX,
            max_done_subtasks: usize::MAX,
            search_active: false,
            sort_preset: SortPreset::default(),
            search_collapsed_tasks: &HashSet::new(),
        },
    );

    assert_eq!(organized.len(), 2);
    if let cfait::store::TaskListItem::Task(task0) = &organized[0] {
        assert_eq!(task0.summary, "Parent");
    } else {
        panic!("Expected Task variant");
    }
    if let cfait::store::TaskListItem::Task(task1) = &organized[1] {
        assert_eq!(task1.summary, "Child");
        assert_eq!(task1.depth, 1);
    } else {
        panic!("Expected Task variant");
    }
}

/// Rank-4 tasks sort by date first by default (sort_standard_by_priority = false).
#[test]
fn test_sort_standard_date_first() {
    let now = Utc::now();
    let high_prio_late = SortKey {
        rank: 4,
        prio: 1,
        due: Some(DateType::Specific(now + Duration::days(10))),
        start: None,
        is_overdue: false,
    };
    let low_prio_soon = SortKey {
        rank: 4,
        prio: 9,
        due: Some(DateType::Specific(now + Duration::days(2))),
        start: None,
        is_overdue: false,
    };
    // date-first: soon (low_prio_soon) should sort BEFORE late (high_prio_late)
    assert_eq!(
        compare_sortkeys(
            &low_prio_soon,
            &high_prio_late,
            5,
            false,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "date-first: soon task should sort before late task regardless of priority"
    );
}

/// Rank-4 tasks sort by priority first when sort_standard_by_priority = true.
#[test]
fn test_sort_standard_priority_first() {
    let now = Utc::now();
    let high_prio_late = SortKey {
        rank: 4,
        prio: 1,
        due: Some(DateType::Specific(now + Duration::days(10))),
        start: None,
        is_overdue: false,
    };
    let low_prio_soon = SortKey {
        rank: 4,
        prio: 9,
        due: Some(DateType::Specific(now + Duration::days(2))),
        start: None,
        is_overdue: false,
    };
    // priority-first: high priority (prio=1) should sort BEFORE low priority (prio=9)
    assert_eq!(
        compare_sortkeys(
            &high_prio_late,
            &low_prio_soon,
            5,
            true,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "priority-first: high-priority task should sort before low-priority task regardless of date"
    );
}

/// When sort_standard_by_priority is true, rank-4 and rank-5 tasks are merged into one group
/// sorted by priority first. A rank-5 task (no date) with high priority should sort before
/// a rank-4 task (has date) with lower priority.
#[test]
fn test_sort_merged_rank4_rank5_priority_wins() {
    let now = Utc::now();
    let rank5_high_prio = SortKey {
        rank: 5,
        prio: 1,
        due: None,
        start: None,
        is_overdue: false,
    };
    let rank4_low_prio = SortKey {
        rank: 4,
        prio: 9,
        due: Some(DateType::Specific(now + chrono::Duration::days(3))),
        start: None,
        is_overdue: false,
    };
    assert_eq!(
        compare_sortkeys(
            &rank5_high_prio,
            &rank4_low_prio,
            5,
            true,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "priority-first: rank-5 high-priority task should sort before rank-4 low-priority task"
    );
}

/// When sort_standard_by_priority is true and two rank-5 tasks share the same priority,
/// the one with a due date sorts before the one without.
/// Note: this rule also holds when sort_standard_by_priority is false (rank-5's own sort is
/// priority-first, then date), so this test validates the date-tiebreaker in the merged group
/// rather than proving something unique to the merged mode.
#[test]
fn test_sort_merged_same_priority_date_wins() {
    let now = Utc::now();
    // Two rank-5 tasks (outside cutoff / no date) with equal priority; one has a due date.
    let rank5_with_date = SortKey {
        rank: 5,
        prio: 3,
        due: Some(DateType::Specific(now + chrono::Duration::days(5))),
        start: None,
        is_overdue: false,
    };
    let rank5_no_date = SortKey {
        rank: 5,
        prio: 3,
        due: None,
        start: None,
        is_overdue: false,
    };
    // With flag=true: both map to effective_rank=4 (merged group), priority equal → date wins.
    assert_eq!(
        compare_sortkeys(
            &rank5_with_date,
            &rank5_no_date,
            5,
            true,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "merged (flag=true): same priority → task with date before task without date"
    );
    // With flag=false: both stay rank-5, sort is priority-first then date → same ordering.
    assert_eq!(
        compare_sortkeys(
            &rank5_with_date,
            &rank5_no_date,
            5,
            false,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "non-merged (flag=false): rank-5 is already priority-first+date, so ordering is the same"
    );
}

/// When sort_standard_by_priority is false, rank-4 and rank-5 remain separate groups —
/// all rank-4 tasks sort before all rank-5 tasks regardless of priority.
#[test]
fn test_sort_rank4_before_rank5_when_flag_off() {
    let rank5_high_prio = SortKey {
        rank: 5,
        prio: 1,
        due: None,
        start: None,
        is_overdue: false,
    };
    let rank4_low_prio = SortKey {
        rank: 4,
        prio: 9,
        due: Some(DateType::Specific(Utc::now() + chrono::Duration::days(3))),
        start: None,
        is_overdue: false,
    };
    assert_eq!(
        compare_sortkeys(
            &rank4_low_prio,
            &rank5_high_prio,
            5,
            false,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "flag off: rank-4 task must always sort before rank-5 task"
    );
}

/// Ranks 2 and 3 are unaffected by sort_standard_by_priority — always date-first.
#[test]
fn test_sort_urgent_ranks_always_date_first() {
    let now = Utc::now();
    let high_prio_late = SortKey {
        rank: 2,
        prio: 1,
        due: Some(DateType::Specific(now + Duration::days(5))),
        start: None,
        is_overdue: false,
    };
    let low_prio_soon = SortKey {
        rank: 2,
        prio: 9,
        due: Some(DateType::Specific(now + Duration::days(1))),
        start: None,
        is_overdue: false,
    };
    // Rank 2 always date-first, even with the flag set to true
    assert_eq!(
        compare_sortkeys(
            &low_prio_soon,
            &high_prio_late,
            5,
            true,
            SortPreset::default()
        ),
        std::cmp::Ordering::Less,
        "rank-2 must remain date-first even when sort_standard_by_priority is true"
    );
}
