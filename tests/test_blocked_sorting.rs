use cfait::context::TestContext;
use cfait::model::{DateType, Task, TaskStatus};
use cfait::store::{FilterOptions, TaskStore};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[test]
fn test_blocked_tasks_skip_urgent_rank() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut urgent_task = Task::new("Urgent Task", &aliases, None);
    urgent_task.priority = 1;
    urgent_task.calendar_href = "cal1".to_string();
    urgent_task.uid = "urgent".to_string();

    let mut blocked_urgent = Task::new("Blocked Urgent #blocked", &aliases, None);
    blocked_urgent.priority = 1;
    blocked_urgent.calendar_href = "cal1".to_string();
    blocked_urgent.uid = "blocked_urgent".to_string();
    blocked_urgent.categories.push("blocked".to_string());

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

    let urgent_pos = filtered.iter().position(|t| t.uid == "urgent");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_urgent");
    let normal_pos = filtered.iter().position(|t| t.uid == "normal");

    assert!(urgent_pos.unwrap() < normal_pos.unwrap());
    assert!(blocked_pos.unwrap() > urgent_pos.unwrap());
}

#[test]
fn test_blocked_tasks_skip_due_soon_rank() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let now = Utc::now();

    let mut due_soon = Task::new("Due Soon", &aliases, None);
    due_soon.due = Some(DateType::Specific(now + chrono::Duration::days(3)));
    due_soon.calendar_href = "cal1".to_string();
    due_soon.uid = "due_soon".to_string();

    let mut blocked_due_soon = Task::new("Blocked Due Soon #blocked", &aliases, None);
    blocked_due_soon.due = Some(DateType::Specific(now + chrono::Duration::days(2)));
    blocked_due_soon.calendar_href = "cal1".to_string();
    blocked_due_soon.uid = "blocked_due_soon".to_string();
    blocked_due_soon.categories.push("blocked".to_string());

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

    let due_soon_pos = filtered.iter().position(|t| t.uid == "due_soon");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_due_soon");
    let due_later_pos = filtered.iter().position(|t| t.uid == "due_later");

    assert!(due_soon_pos.unwrap() < due_later_pos.unwrap());
    assert!(blocked_pos.unwrap() > due_soon_pos.unwrap());
}

#[test]
fn test_blocked_tasks_skip_started_rank() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut started = Task::new("Started Task", &aliases, None);
    started.status = TaskStatus::InProcess;
    started.calendar_href = "cal1".to_string();
    started.uid = "started".to_string();

    let mut blocked_started = Task::new("Blocked Started #blocked", &aliases, None);
    blocked_started.status = TaskStatus::InProcess;
    blocked_started.calendar_href = "cal1".to_string();
    blocked_started.uid = "blocked_started".to_string();
    blocked_started.categories.push("blocked".to_string());

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

    let started_pos = filtered.iter().position(|t| t.uid == "started");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_started");
    let normal_pos = filtered.iter().position(|t| t.uid == "normal");

    assert!(started_pos.unwrap() < normal_pos.unwrap());
    assert!(blocked_pos.unwrap() > started_pos.unwrap());
}

#[test]
fn test_dependency_blocked_tasks_also_skip_ranks() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut dep_task = Task::new("Dependency Task", &aliases, None);
    dep_task.status = TaskStatus::NeedsAction;
    dep_task.calendar_href = "cal1".to_string();
    dep_task.uid = "dep".to_string();

    let mut blocked_by_dep = Task::new("Blocked by Dependency", &aliases, None);
    blocked_by_dep.priority = 1;
    blocked_by_dep.dependencies.push(dep_task.uid.clone());
    blocked_by_dep.calendar_href = "cal1".to_string();
    blocked_by_dep.uid = "blocked_by_dep".to_string();

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

    let urgent_pos = filtered.iter().position(|t| t.uid == "urgent");
    let blocked_pos = filtered.iter().position(|t| t.uid == "blocked_by_dep");

    assert!(urgent_pos.unwrap() < blocked_pos.unwrap());
    assert!(store.is_blocked(&blocked_by_dep));
}

#[test]
fn test_blocked_tag_is_recognized() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut task = Task::new("Task #blocked", &aliases, None);
    task.categories.push("blocked".to_string());
    // Must add to store for is_blocked check (needs index for dependency checks)
    task.calendar_href = "cal1".to_string();
    store.add_task(task.clone());

    assert!(store.is_blocked(&task));

    let mut unblocked = task.clone();
    unblocked.categories.clear();
    // Re-insert/update
    store.add_task(unblocked.clone());

    assert!(!store.is_blocked(&unblocked));
}

#[test]
fn test_is_ready_filters_manually_blocked_tasks() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut blocked_task = Task::new("Blocked Task #blocked", &aliases, None);
    blocked_task.calendar_href = "cal1".to_string();
    blocked_task.categories.push("blocked".to_string());

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

    assert!(!filtered.iter().any(|t| t.summary.contains("Blocked Task")));
    assert!(
        filtered
            .iter()
            .any(|t| t.summary.contains("Unblocked Task"))
    );
}

#[test]
fn test_is_blocked_filter_shows_only_blocked() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut dep_task = Task::new("Dependency Task", &aliases, None);
    dep_task.status = TaskStatus::NeedsAction;
    dep_task.calendar_href = "cal1".to_string();
    dep_task.uid = "dep".to_string();

    let mut blocked_by_dep = Task::new("Blocked by Dependency", &aliases, None);
    blocked_by_dep.dependencies.push(dep_task.uid.clone());
    blocked_by_dep.calendar_href = "cal1".to_string();

    let mut manually_blocked = Task::new("Manually Blocked #blocked", &aliases, None);
    manually_blocked.calendar_href = "cal1".to_string();
    manually_blocked.categories.push("blocked".to_string());

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
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    };

    let filtered = store.filter(options);

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
    assert!(
        !filtered
            .iter()
            .any(|t| t.summary.contains("Unblocked Task"))
    );
    assert!(
        !filtered
            .iter()
            .any(|t| t.summary.contains("Dependency Task"))
    );
}
