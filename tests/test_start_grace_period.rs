use cfait::context::TestContext;
use cfait::model::{Alarm, DateType, Task};
use cfait::store::{FilterOptions, TaskStore};
use chrono::{Duration, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[test]
fn test_start_grace_period_keeps_tasks_in_active_section() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let now = Utc::now();

    // Task starting in 2 days (within 2-day grace period)
    let mut task_within_grace = Task::new("Task starting soon", &aliases, None);
    task_within_grace.dtstart = Some(DateType::Specific(now + Duration::days(2)));
    task_within_grace.priority = 5;
    task_within_grace.calendar_href = "cal1".to_string();

    // Task starting in 5 days (outside 2-day grace period)
    let mut task_beyond_grace = Task::new("Task starting later", &aliases, None);
    task_beyond_grace.dtstart = Some(DateType::Specific(now + Duration::days(5)));
    task_beyond_grace.priority = 5;
    task_beyond_grace.calendar_href = "cal1".to_string();

    // Normal task with no start date
    let mut normal_task = Task::new("Normal task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();

    store.add_task(task_within_grace.clone());
    store.add_task(task_beyond_grace.clone());
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
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 2, // 2-day grace period
    };

    let filtered = store.filter(options);

    // Find positions of tasks
    let within_pos = filtered
        .iter()
        .position(|t| t.summary.contains("starting soon"));
    let beyond_pos = filtered
        .iter()
        .position(|t| t.summary.contains("starting later"));
    let normal_pos = filtered.iter().position(|t| t.summary.contains("Normal"));

    assert!(within_pos.is_some(), "Task within grace should be present");
    assert!(beyond_pos.is_some(), "Task beyond grace should be present");
    assert!(normal_pos.is_some(), "Normal task should be present");

    // Task within grace period should come before task beyond grace
    // (both have same priority, but one is in future section)
    assert!(
        within_pos.unwrap() < beyond_pos.unwrap(),
        "Task within grace period should not be pushed to future section"
    );
}

#[test]
fn test_grace_period_zero_pushes_all_future_starts() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let now = Utc::now();

    // Task starting in 1 hour
    let mut task_soon = Task::new("Task starting in 1 hour", &aliases, None);
    task_soon.dtstart = Some(DateType::Specific(now + Duration::hours(1)));
    task_soon.priority = 5;
    task_soon.calendar_href = "cal1".to_string();

    // Normal task with no start date
    let mut normal_task = Task::new("Normal task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();

    store.add_task(task_soon.clone());
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
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 0, // No grace period
    };

    let filtered = store.filter(options);

    let soon_pos = filtered.iter().position(|t| t.summary.contains("1 hour"));
    let normal_pos = filtered.iter().position(|t| t.summary.contains("Normal"));

    assert!(soon_pos.is_some());
    assert!(normal_pos.is_some());

    // With 0 grace period, future task should be pushed to end
    assert!(
        normal_pos.unwrap() < soon_pos.unwrap(),
        "With 0 grace period, any future start should be in future section"
    );
}

#[test]
fn test_acknowledged_alarm_keeps_task_in_active_section() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let now = Utc::now();

    // Task starting in 5 days (beyond 1-day grace) but with acknowledged alarm
    let mut task_with_ack_alarm = Task::new("Task with dismissed reminder", &aliases, None);
    task_with_ack_alarm.dtstart = Some(DateType::Specific(now + Duration::days(5)));
    task_with_ack_alarm.priority = 5;
    task_with_ack_alarm.calendar_href = "cal1".to_string();

    // Add an acknowledged alarm (dismissed recently)
    let mut alarm = Alarm::new_absolute(now - Duration::hours(2));
    alarm.acknowledged = Some(now - Duration::hours(1));
    task_with_ack_alarm.alarms.push(alarm);

    // Task starting in 5 days without any alarms
    let mut task_no_alarm = Task::new("Task without reminder", &aliases, None);
    task_no_alarm.dtstart = Some(DateType::Specific(now + Duration::days(5)));
    task_no_alarm.priority = 5;
    task_no_alarm.calendar_href = "cal1".to_string();

    // Normal task with no start date
    let mut normal_task = Task::new("Normal task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();

    store.add_task(task_with_ack_alarm.clone());
    store.add_task(task_no_alarm.clone());
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
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1, // 1-day grace period
    };

    let filtered = store.filter(options);

    let with_alarm_pos = filtered
        .iter()
        .position(|t| t.summary.contains("dismissed reminder"));
    let without_alarm_pos = filtered
        .iter()
        .position(|t| t.summary.contains("without reminder"));
    let normal_pos = filtered.iter().position(|t| t.summary.contains("Normal"));

    assert!(with_alarm_pos.is_some());
    assert!(without_alarm_pos.is_some());
    assert!(normal_pos.is_some());

    // Task with acknowledged alarm should stay in active section (before future section)
    assert!(
        with_alarm_pos.unwrap() < without_alarm_pos.unwrap(),
        "Task with acknowledged alarm should not be pushed to future section"
    );
}

#[test]
fn test_any_acknowledged_alarm_keeps_task_active() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let now = Utc::now();

    // Task starting in 5 days with acknowledged alarm (user engaged with it)
    let mut task_with_alarm = Task::new("Task with dismissed alarm", &aliases, None);
    task_with_alarm.dtstart = Some(DateType::Specific(now + Duration::days(5)));
    task_with_alarm.priority = 5;
    task_with_alarm.calendar_href = "cal1".to_string();

    // Add an acknowledged alarm - doesn't matter when it was acknowledged
    // The user engaged with this task instance, so it should stay visible
    let mut alarm = Alarm::new_absolute(now - Duration::days(10));
    alarm.acknowledged = Some(now - Duration::days(10));
    task_with_alarm.alarms.push(alarm);

    // Task starting in 5 days WITHOUT acknowledged alarm (should go to Future)
    let mut task_without_alarm = Task::new("Task without alarm", &aliases, None);
    task_without_alarm.dtstart = Some(DateType::Specific(now + Duration::days(5)));
    task_without_alarm.priority = 5;
    task_without_alarm.calendar_href = "cal1".to_string();

    // Normal task with no start date
    let mut normal_task = Task::new("Normal task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();

    store.add_task(task_with_alarm.clone());
    store.add_task(task_without_alarm.clone());
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
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1, // 1-day grace period
    };

    let filtered = store.filter(options);

    let with_alarm_pos = filtered
        .iter()
        .position(|t| t.summary.contains("dismissed alarm"));
    let without_alarm_pos = filtered
        .iter()
        .position(|t| t.summary.contains("without alarm"));
    let normal_pos = filtered.iter().position(|t| t.summary.contains("Normal"));

    assert!(with_alarm_pos.is_some());
    assert!(without_alarm_pos.is_some());
    assert!(normal_pos.is_some());

    // Task with acknowledged alarm should come before task without alarm
    // (with alarm stays in rank 5, without alarm goes to rank 6)
    assert!(
        with_alarm_pos.unwrap() < without_alarm_pos.unwrap(),
        "Task with acknowledged alarm should stay in active section, not pushed to Future"
    );
}

#[test]
fn test_recurring_task_with_fresh_dates_goes_to_future() {
    let aliases = HashMap::new();
    let now = Utc::now();

    // Simulate a recurring task that has advanced to a future occurrence
    // Old alarms from previous recurrence should have been cleared
    let mut recurring_task = Task::new("Recurring task", &aliases, None);
    recurring_task.dtstart = Some(DateType::Specific(now + Duration::days(7)));
    recurring_task.rrule = Some("FREQ=DAILY".to_string());
    recurring_task.priority = 5;
    recurring_task.calendar_href = "cal1".to_string();
    // No alarms (they were cleared when task advanced)

    let mut normal_task = Task::new("Normal task", &aliases, None);
    normal_task.priority = 5;
    normal_task.calendar_href = "cal1".to_string();

    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    store.add_task(recurring_task.clone());
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
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
    };

    let filtered = store.filter(options);

    let recurring_pos = filtered
        .iter()
        .position(|t| t.summary.contains("Recurring"));
    let normal_pos = filtered.iter().position(|t| t.summary.contains("Normal"));

    assert!(recurring_pos.is_some());
    assert!(normal_pos.is_some());

    // Recurring task with future start (7 days out, beyond 1-day grace) should be in future section
    assert!(
        normal_pos.unwrap() < recurring_pos.unwrap(),
        "Recurring task with future start date should go to future section"
    );
}
