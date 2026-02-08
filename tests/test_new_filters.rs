// Tests for newer filter features (relative dates, etc.).
use cfait::context::TestContext;
use cfait::model::{Task, TaskStatus};
use cfait::store::{FilterOptions, TaskStore};
use chrono::{Datelike, Duration, Local};
use std::collections::HashSet;
use std::sync::Arc;

#[test]
fn test_relative_start_date_parsing() {
    let aliases = std::collections::HashMap::new();
    let now = Local::now().date_naive();

    // Test relative start date with days
    let t1 = Task::new("Task ^2d", &aliases, None);
    assert!(t1.dtstart.is_some());
    assert_eq!(t1.dtstart.unwrap().to_date_naive(), now + Duration::days(2));

    // Test relative start date with weeks
    let t2 = Task::new("Task ^1w", &aliases, None);
    assert!(t2.dtstart.is_some());
    assert_eq!(t2.dtstart.unwrap().to_date_naive(), now + Duration::days(7));

    // Test start date with "tomorrow" keyword
    let t3 = Task::new("Task ^tomorrow", &aliases, None);
    assert!(t3.dtstart.is_some());
    assert_eq!(t3.dtstart.unwrap().to_date_naive(), now + Duration::days(1));
}

#[test]
fn test_start_date_filter_with_relative_dates() {
    let aliases = std::collections::HashMap::new();
    let now = Local::now().date_naive();

    // Create task with start date 3 days from now
    let future_task = Task::new(
        &format!(
            "Future Task ^{}",
            (now + Duration::days(3)).format("%Y-%m-%d")
        ),
        &aliases,
        None,
    );

    // Create task with start date today
    let today_task = Task::new("Today Task ^today", &aliases, None);

    // Filter for tasks starting after 2 days from now
    // future_task (3 days) > 2 days, so it should match
    assert!(future_task.matches_search_term("^>2d"));
    // today_task (0 days) is NOT > 2 days, so it should not match
    assert!(!today_task.matches_search_term("^>2d"));

    // Filter for tasks starting before 5 days from now
    // Both tasks are before 5 days, so both should match
    assert!(future_task.matches_search_term("^<5d"));
    assert!(today_task.matches_search_term("^<5d"));
}

#[test]
fn test_not_set_operator_with_exclamation() {
    let aliases = std::collections::HashMap::new();

    // Task with no start date
    let no_start = Task::new("Task without start", &aliases, None);

    // Task with start date tomorrow
    let has_start = Task::new("Task ^tomorrow", &aliases, None);

    // Without "!" - tasks with no date should be filtered out
    assert!(!no_start.matches_search_term("^>today"));
    assert!(has_start.matches_search_term("^>today"));

    // With "!" - tasks with no date should pass the filter
    assert!(no_start.matches_search_term("^>today!"));
    assert!(has_start.matches_search_term("^>today!"));
}

#[test]
fn test_not_set_operator_with_due_date() {
    let aliases = std::collections::HashMap::new();
    let now = Local::now().date_naive();
    // Dynamic future date to ensure test stability
    let future_year = now.year() + 2;
    let filter = format!("@<{}-01-01", future_year);
    let filter_not_set = format!("{}!", filter);

    // Task with no due date
    let no_due = Task::new("Task without due", &aliases, None);

    // Task with due date tomorrow
    let has_due = Task::new("Task @tomorrow", &aliases, None);

    // Without "!" - tasks with no date should be filtered out
    assert!(!no_due.matches_search_term(&filter));
    assert!(has_due.matches_search_term(&filter));

    // With "!" - tasks with no date should pass the filter
    assert!(no_due.matches_search_term(&filter_not_set));
    assert!(has_due.matches_search_term(&filter_not_set));
}

#[test]
fn test_is_ready_token_consumed() {
    let aliases = std::collections::HashMap::new();

    // Create a task that doesn't contain the text "is:ready"
    let task = Task::new("Work on project", &aliases, None);

    // The is:ready filter should be consumed and not fail text search
    assert!(task.matches_search_term("is:ready"));
}

#[test]
fn test_is_ready_filters_future_start_dates() {
    // Create a TestContext and TaskStore with it
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx.clone());

    let aliases = std::collections::HashMap::new();
    // Use UTC to avoid timezone issues where "yesterday" in local time is still "today" in UTC
    let now = chrono::Utc::now().date_naive();

    // Task with future start date
    let mut future = Task::new(
        &format!(
            "Future Task ^{}",
            (now + Duration::days(5)).format("%Y-%m-%d")
        ),
        &aliases,
        None,
    );
    future.calendar_href = "cal1".to_string();

    // Task with past start date (2 days ago to avoid timezone edge cases)
    let mut past = Task::new(
        &format!(
            "Past Task ^{}",
            (now - Duration::days(2)).format("%Y-%m-%d")
        ),
        &aliases,
        None,
    );
    past.calendar_href = "cal1".to_string();

    // Task with no start date
    let mut no_start = Task::new("No Start Task", &aliases, None);
    no_start.calendar_href = "cal1".to_string();

    store.add_task(future);
    store.add_task(past);
    store.add_task(no_start);

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

    // Future task should be filtered out
    assert!(!filtered.iter().any(|t| t.summary.contains("Future")));

    // Past task and no start task should be included
    assert!(filtered.iter().any(|t| t.summary.contains("Past")));
    assert!(filtered.iter().any(|t| t.summary.contains("No Start")));
}

#[test]
fn test_is_ready_filters_blocked_tasks() {
    // Create a TestContext and TaskStore with it
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx.clone());

    let aliases = std::collections::HashMap::new();

    // Create a dependency task that is not done
    let mut dep_task = Task::new("Dependency Task", &aliases, None);
    dep_task.status = TaskStatus::NeedsAction;
    dep_task.calendar_href = "cal1".to_string();

    // Create a task that depends on the first
    let mut blocked_task = Task::new("Blocked Task", &aliases, None);
    blocked_task.dependencies.push(dep_task.uid.clone());
    blocked_task.calendar_href = "cal1".to_string();

    // Create an unblocked task
    let mut unblocked_task = Task::new("Unblocked Task", &aliases, None);
    unblocked_task.calendar_href = "cal1".to_string();

    store.add_task(dep_task);
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

    // Blocked task should be filtered out
    assert!(!filtered.iter().any(|t| t.summary.contains("Blocked")));

    // Unblocked and dependency tasks should be included
    assert!(filtered.iter().any(|t| t.summary.contains("Unblocked")));
    assert!(filtered.iter().any(|t| t.summary.contains("Dependency")));
}

#[test]
fn test_is_ready_combines_with_other_filters() {
    // Create a TestContext and TaskStore with it
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx.clone());

    let aliases = std::collections::HashMap::new();
    let now = Local::now().date_naive();

    // Ready task with #work tag
    let mut work_task = Task::new("Work Task #work", &aliases, None);
    work_task.calendar_href = "cal1".to_string();

    // Ready task with #personal tag
    let mut personal_task = Task::new("Personal Task #personal", &aliases, None);
    personal_task.calendar_href = "cal1".to_string();

    // Future task with #work tag (not ready)
    let mut future_work = Task::new(
        &format!(
            "Future Work #work ^{}",
            (now + Duration::days(5)).format("%Y-%m-%d")
        ),
        &aliases,
        None,
    );
    future_work.calendar_href = "cal1".to_string();

    store.add_task(work_task);
    store.add_task(personal_task);
    store.add_task(future_work);

    let options = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "is:ready #work",
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

    // Only the ready work task should be included
    assert!(filtered.len() == 1);
    assert!(filtered[0].summary.contains("Work Task"));
    assert!(!filtered[0].summary.contains("Future"));
    assert!(!filtered[0].summary.contains("Personal"));
}
