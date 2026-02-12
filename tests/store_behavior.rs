use cfait::context::TestContext;
use cfait::model::{Task, TaskStatus};
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

#[test]
fn test_filter_by_tag() {
    let mut store = make_store();

    let mut t1 = Task::new("Work Task #work", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();

    let mut t2 = Task::new("Home Task #home", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal1".to_string();

    store.add_task(t1);
    store.add_task(t2);

    let mut cats = HashSet::new();
    cats.insert("work".to_string());
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
        expanded_done_groups: &empty_set,
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Work Task");
}

#[test]
fn test_filter_hierarchical_tags() {
    let mut store = make_store();

    let mut t1 = Task::new("Backend #dev:backend", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();
    store.add_task(t1);

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
        expanded_done_groups: &empty_set,
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
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
        expanded_done_groups: &empty_set,
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Cal 1 Task");
}

#[test]
fn test_set_status_cancelled_advances_recurring_task() {
    let mut store = make_store();

    let mut t = Task::new("Daily Task", &HashMap::new(), None);
    t.uid = "recurring-1".to_string();
    t.calendar_href = "cal1".to_string();
    let original_due = chrono::Utc::now() - chrono::Duration::days(1);
    t.due = Some(cfait::model::DateType::Specific(original_due));
    t.rrule = Some("FREQ=DAILY".to_string());
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("recurring-1", TaskStatus::Cancelled);
    assert!(updated.is_some());

    let (history, secondary, _children) = updated.unwrap();
    let recycled_task = secondary.expect("Recurring task should recycle into a secondary task");

    // The recycled task should be the next instance, ready for action.
    assert_eq!(recycled_task.status, TaskStatus::NeedsAction);
    // It should have accumulated the EXDATE from the cancellation.
    assert_eq!(recycled_task.exdates.len(), 1);
    assert_eq!(
        recycled_task.exdates[0],
        cfait::model::DateType::Specific(original_due)
    );

    // The history item should be a snapshot of the cancelled occurrence.
    assert_eq!(history.status, TaskStatus::Cancelled);
    assert!(history.rrule.is_none());
}

#[test]
fn test_set_status_cancelled_non_recurring_task() {
    let mut store = make_store();

    let mut t = Task::new("One-time Task", &HashMap::new(), None);
    t.uid = "one-time-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("one-time-1", TaskStatus::Cancelled);
    assert!(updated.is_some());

    let (task, _secondary, _children) = updated.unwrap();
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert!(task.rrule.is_none());
}

#[test]
fn test_toggle_status_cancelled_back_to_needs_action() {
    let mut store = make_store();

    let mut t = Task::new("Task", &HashMap::new(), None);
    t.uid = "toggle-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("toggle-1", TaskStatus::Cancelled);
    assert!(updated.is_some());
    // FIX: Check status on the primary task from the tuple
    let (primary, _sec, _children) = updated.unwrap();
    assert_eq!(primary.status, TaskStatus::Cancelled);

    let updated = store.set_status("toggle-1", TaskStatus::Cancelled);
    assert!(updated.is_some());
    // FIX: Check status on the primary task from the tuple
    let (primary, _sec, _children) = updated.unwrap();
    assert_eq!(primary.status, TaskStatus::NeedsAction);
}
