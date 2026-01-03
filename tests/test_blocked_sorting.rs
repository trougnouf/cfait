// Tests for sorting blocked tasks.
use cfait::model::{DateType, Task, TaskStatus};
use cfait::store::{FilterOptions, TaskStore};
use chrono::Utc;
use std::collections::{HashMap, HashSet};

#[test]
fn test_blocked_tasks_skip_urgent_rank() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Create an urgent task (priority 1)
    let mut urgent_task = Task::new("Urgent Task", &aliases, None);
    urgent_task.priority = 1;
    urgent_task.calendar_href = "cal1".to_string();
    urgent_task.uid = "urgent".to_string();

    // Create a blocked urgent task (also priority 1, but blocked)
    let mut blocked_urgent = Task::new("Blocked Urgent #blocked", &aliases, None);
    blocked_urgent.priority = 1;
    blocked_urgent.calendar_href = "cal1".to_string();
    blocked_urgent.uid = "blocked_urgent".to_string();
    blocked_urgent.categories.push("blocked".to_string());

    // Create a normal task to compare
    let mut normal_task = Task::new("Normal Task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();
    normal_task.uid = "normal".to_string();

    store.add_task(urgent_task.clone());
    store.add_task(blocked_urgent.clone());
    store.add_task(normal_task.clone());

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 1,
    };

    let filtered = store.filter(options);

    // Find positions
    let urgent_pos = filtered.iter().position(|t| t.uid == "urgent");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_urgent");
    let normal_pos = filtered.iter().position(|t| t.uid == "normal");

    // Urgent task should come before normal task (rank 1 < rank 4/5)
    assert!(urgent_pos.unwrap() < normal_pos.unwrap());

    // Blocked urgent task should come AFTER normal task
    // (blocked tasks skip rank 1, fall to rank 4/5)
    assert!(blocked_pos.unwrap() > urgent_pos.unwrap());
}

#[test]
fn test_blocked_tasks_skip_due_soon_rank() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();
    let now = Utc::now();

    // Create a task due soon (within 7 days)
    let mut due_soon = Task::new("Due Soon", &aliases, None);
    due_soon.due = Some(DateType::Specific(now + chrono::Duration::days(3)));
    due_soon.calendar_href = "cal1".to_string();
    due_soon.uid = "due_soon".to_string();

    // Create a blocked task also due soon
    let mut blocked_due_soon = Task::new("Blocked Due Soon #blocked", &aliases, None);
    blocked_due_soon.due = Some(DateType::Specific(now + chrono::Duration::days(2)));
    blocked_due_soon.calendar_href = "cal1".to_string();
    blocked_due_soon.uid = "blocked_due_soon".to_string();
    blocked_due_soon.categories.push("blocked".to_string());

    // Create a normal task due later
    let mut due_later = Task::new("Due Later", &aliases, None);
    due_later.due = Some(DateType::Specific(now + chrono::Duration::days(30)));
    due_later.calendar_href = "cal1".to_string();
    due_later.uid = "due_later".to_string();

    store.add_task(due_soon.clone());
    store.add_task(blocked_due_soon.clone());
    store.add_task(due_later.clone());

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 1,
    };

    let filtered = store.filter(options);

    let due_soon_pos = filtered.iter().position(|t| t.uid == "due_soon");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_due_soon");
    let due_later_pos = filtered.iter().position(|t| t.uid == "due_later");

    // Due soon should come before due later (rank 2 < rank 4)
    assert!(due_soon_pos.unwrap() < due_later_pos.unwrap());

    // Blocked task should NOT be in rank 2, should come after the normal due-soon task
    assert!(blocked_pos.unwrap() > due_soon_pos.unwrap());
}

#[test]
fn test_blocked_tasks_skip_started_rank() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Create a started task
    let mut started = Task::new("Started Task", &aliases, None);
    started.status = TaskStatus::InProcess;
    started.calendar_href = "cal1".to_string();
    started.uid = "started".to_string();

    // Create a blocked started task
    let mut blocked_started = Task::new("Blocked Started #blocked", &aliases, None);
    blocked_started.status = TaskStatus::InProcess;
    blocked_started.calendar_href = "cal1".to_string();
    blocked_started.uid = "blocked_started".to_string();
    blocked_started.categories.push("blocked".to_string());

    // Create a normal task
    let mut normal = Task::new("Normal Task", &aliases, None);
    normal.calendar_href = "cal1".to_string();
    normal.uid = "normal".to_string();

    store.add_task(started.clone());
    store.add_task(blocked_started.clone());
    store.add_task(normal.clone());

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 1,
    };

    let filtered = store.filter(options);

    let started_pos = filtered.iter().position(|t| t.uid == "started");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_started");
    let normal_pos = filtered.iter().position(|t| t.uid == "normal");

    // Started should come before normal (rank 3 < rank 4/5)
    assert!(started_pos.unwrap() < normal_pos.unwrap());

    // Blocked started should NOT be in rank 3, should come after started
    assert!(blocked_pos.unwrap() > started_pos.unwrap());
}

#[test]
fn test_dependency_blocked_tasks_also_skip_ranks() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Create a dependency task that is not done
    let mut dep_task = Task::new("Dependency Task", &aliases, None);
    dep_task.status = TaskStatus::NeedsAction;
    dep_task.calendar_href = "cal1".to_string();
    dep_task.uid = "dep".to_string();

    // Create an urgent task blocked by dependency
    let mut blocked_by_dep = Task::new("Blocked by Dependency", &aliases, None);
    blocked_by_dep.priority = 1; // Would be urgent if not blocked
    blocked_by_dep.dependencies.push(dep_task.uid.clone());
    blocked_by_dep.calendar_href = "cal1".to_string();
    blocked_by_dep.uid = "blocked_by_dep".to_string();

    // Create a normal urgent task
    let mut urgent = Task::new("Normal Urgent", &aliases, None);
    urgent.priority = 1;
    urgent.calendar_href = "cal1".to_string();
    urgent.uid = "urgent".to_string();

    store.add_task(dep_task.clone());
    store.add_task(blocked_by_dep.clone());
    store.add_task(urgent.clone());

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 1,
    };

    let filtered = store.filter(options);

    let urgent_pos = filtered.iter().position(|t| t.uid == "urgent");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_by_dep");

    // Normal urgent should come before blocked urgent
    assert!(urgent_pos.unwrap() < blocked_pos.unwrap());

    // Verify the blocked task is recognized as blocked
    assert!(store.is_blocked(&blocked_by_dep));
}

#[test]
fn test_blocked_tag_is_recognized() {
    let store = TaskStore::new();
    let aliases = HashMap::new();

    // Create a task with #blocked tag
    let mut task = Task::new("Task #blocked", &aliases, None);
    task.categories.push("blocked".to_string());

    // Should be recognized as blocked
    assert!(store.is_blocked(&task));

    // Remove the tag
    task.categories.clear();

    // Should no longer be blocked
    assert!(!store.is_blocked(&task));
}

#[test]
fn test_is_ready_filters_manually_blocked_tasks() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Create a manually blocked task
    let mut blocked_task = Task::new("Blocked Task #blocked", &aliases, None);
    blocked_task.calendar_href = "cal1".to_string();
    blocked_task.categories.push("blocked".to_string());

    // Create an unblocked task
    let mut unblocked_task = Task::new("Unblocked Task", &aliases, None);
    unblocked_task.calendar_href = "cal1".to_string();

    store.add_task(blocked_task);
    store.add_task(unblocked_task);

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "is:ready",
        hide_completed_global: true,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 5,
    };

    let filtered = store.filter(options);

    // Manually blocked task should be filtered out by is:ready
    assert!(!filtered.iter().any(|t| t.summary.contains("Blocked Task")));

    // Unblocked task should be included
    assert!(
        filtered
            .iter()
            .any(|t| t.summary.contains("Unblocked Task"))
    );
}

#[test]
fn test_is_blocked_filter_shows_only_blocked() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Create a dependency task that is not done
    let mut dep_task = Task::new("Dependency Task", &aliases, None);
    dep_task.status = TaskStatus::NeedsAction;
    dep_task.calendar_href = "cal1".to_string();
    dep_task.uid = "dep".to_string();

    // Create a task blocked by dependency
    let mut blocked_by_dep = Task::new("Blocked by Dependency", &aliases, None);
    blocked_by_dep.dependencies.push(dep_task.uid.clone());
    blocked_by_dep.calendar_href = "cal1".to_string();

    // Create a manually blocked task
    let mut manually_blocked = Task::new("Manually Blocked #blocked", &aliases, None);
    manually_blocked.calendar_href = "cal1".to_string();
    manually_blocked.categories.push("blocked".to_string());

    // Create an unblocked task
    let mut unblocked = Task::new("Unblocked Task", &aliases, None);
    unblocked.calendar_href = "cal1".to_string();

    store.add_task(dep_task);
    store.add_task(blocked_by_dep.clone());
    store.add_task(manually_blocked.clone());
    store.add_task(unblocked);

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "is:blocked",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 5,
    };

    let filtered = store.filter(options);

    // Should show both types of blocked tasks
    assert!(
        filtered
            .iter()
            .any(|t| t.summary.contains("Blocked by Dependency"))
    );
    assert!(
        filtered
            .iter()
            .any(|t| t.summary.contains("Manually Blocked"))
    );

    // Should NOT show unblocked tasks
    assert!(
        !filtered
            .iter()
            .any(|t| t.summary.contains("Unblocked Task"))
    );

    // Should NOT show the dependency task itself (it's not blocked)
    assert!(
        !filtered
            .iter()
            .any(|t| t.summary.contains("Dependency Task"))
    );
}
