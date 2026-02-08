// Tests for task sorting logic.
use cfait::model::{DateType, Task, TaskStatus};
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
        high.compare_with_cutoff(&low, None, 1, 1, 5, 1), // Pass defaults
        std::cmp::Ordering::Less
    );

    // 1 < 0 (High vs None/Normal)
    assert_eq!(
        high.compare_with_cutoff(&none, None, 1, 1, 5, 1), // Pass defaults
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_sorting_status_trumps_everything() {
    // An active task (InProcess) with low priority
    let mut active = task("Active Low Prio");
    active.priority = 9;
    active.status = TaskStatus::InProcess;

    // A waiting task with critical priority
    let mut critical = task("Critical Waiting");
    critical.priority = 1;
    critical.status = TaskStatus::NeedsAction;

    // With the urgency logic, tasks that are urgent may beat started tasks.
    // Expect critical urgent task to sort before active started task here.
    assert_eq!(
        critical.compare_with_cutoff(&active, None, 1, 1, 5, 1),
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
        todo.compare_with_cutoff(&done, None, 1, 1, 5, 1),
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
        t1.compare_with_cutoff(&t2, None, 1, 1, 5, 1),
        std::cmp::Ordering::Less
    );

    // Date < No Date
    assert_eq!(
        t2.compare_with_cutoff(&t3, None, 1, 1, 5, 1),
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
    // Updated signature includes expanded_done_groups, max_done_roots, max_done_subtasks.
    let organized = Task::organize_hierarchy(tasks, 5, &HashSet::new(), usize::MAX, usize::MAX);

    assert_eq!(organized.len(), 2);
    assert_eq!(organized[0].summary, "Parent");
    assert_eq!(organized[1].summary, "Child");
    assert_eq!(organized[1].depth, 1);
}
