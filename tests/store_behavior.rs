// Tests for task storage filtering logic.
use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};

fn make_store() -> TaskStore {
    TaskStore::new()
}

#[test]
fn test_filter_by_tag() {
    let mut store = make_store();

    let mut t1 = Task::new("Work Task #work", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string(); // Required for store index

    let mut t2 = Task::new("Home Task #home", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal1".to_string();

    store.add_task(t1);
    store.add_task(t2);

    let mut cats = HashSet::new();
    cats.insert("work".to_string());

    let empty_set = HashSet::new(); // for hidden/locations

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &empty_set,
        selected_categories: &cats,
        selected_locations: &empty_set,
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Work Task");
}

#[test]
fn test_filter_hierarchical_tags() {
    let mut store = make_store();

    // Tag: #dev:backend
    let mut t1 = Task::new("Backend #dev:backend", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();
    store.add_task(t1);

    // Filter: #dev (Should match #dev:backend)
    let mut cats = HashSet::new();
    cats.insert("dev".to_string());

    let empty_set = HashSet::new();

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &empty_set,
        selected_categories: &cats,
        selected_locations: &empty_set,
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Backend");
}

#[test]
fn test_hide_hidden_calendars() {
    let mut store = make_store();

    let mut t1 = Task::new("Cal 1 Task", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();

    let mut t2 = Task::new("Cal 2 Task", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal2".to_string();

    store.add_task(t1);
    store.add_task(t2);

    let mut hidden = HashSet::new();
    hidden.insert("cal2".to_string());

    let empty_set = HashSet::new();

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden,
        selected_categories: &empty_set,
        selected_locations: &empty_set,
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Cal 1 Task");
}

#[test]
fn test_set_status_cancelled_advances_recurring_task() {
    use cfait::model::{DateType, TaskStatus};
    use chrono::{Duration, Utc};

    let mut store = make_store();

    // Create a recurring task due yesterday
    let mut t = Task::new("Daily Task", &HashMap::new(), None);
    t.uid = "recurring-1".to_string();
    t.calendar_href = "cal1".to_string();
    let original_due = Utc::now() - Duration::days(1);
    t.due = Some(DateType::Specific(original_due));
    t.rrule = Some("FREQ=DAILY".to_string());
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    // Cancel the task - should advance to next recurrence
    let updated = store.set_status("recurring-1", TaskStatus::Cancelled);
    assert!(updated.is_some());

    let task = updated.unwrap();

    // Should have advanced to next occurrence
    assert_eq!(task.status, TaskStatus::NeedsAction);

    // Should have added original date to exdates
    assert_eq!(task.exdates.len(), 1);
    assert_eq!(task.exdates[0], DateType::Specific(original_due));

    // New due date should be in the future
    match task.due {
        Some(DateType::Specific(d)) => assert!(d > Utc::now()),
        _ => panic!("Expected specific date"),
    }
}

#[test]
fn test_set_status_cancelled_non_recurring_task() {
    use cfait::model::TaskStatus;

    let mut store = make_store();

    // Create a non-recurring task
    let mut t = Task::new("One-time Task", &HashMap::new(), None);
    t.uid = "one-time-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    // Cancel the task - should just set status to Cancelled
    let updated = store.set_status("one-time-1", TaskStatus::Cancelled);
    assert!(updated.is_some());

    let task = updated.unwrap();

    // Should be cancelled and not advanced
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert!(task.rrule.is_none());
}

#[test]
fn test_toggle_status_cancelled_back_to_needs_action() {
    use cfait::model::TaskStatus;

    let mut store = make_store();

    let mut t = Task::new("Task", &HashMap::new(), None);
    t.uid = "toggle-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    // Set to cancelled
    let updated = store.set_status("toggle-1", TaskStatus::Cancelled);
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().status, TaskStatus::Cancelled);

    // Toggle back (calling set_status again with same status)
    let updated = store.set_status("toggle-1", TaskStatus::Cancelled);
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().status, TaskStatus::NeedsAction);
}
