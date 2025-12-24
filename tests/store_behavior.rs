// File: tests/store_behavior.rs
use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};

fn make_store() -> TaskStore {
    TaskStore::new()
}

#[test]
fn test_filter_by_tag() {
    let mut store = make_store();

    let mut t1 = Task::new("Work Task #work", &HashMap::new());
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string(); // Required for store index

    let mut t2 = Task::new("Home Task #home", &HashMap::new());
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Work Task");
}

#[test]
fn test_filter_hierarchical_tags() {
    let mut store = make_store();

    // Tag: #dev:backend
    let mut t1 = Task::new("Backend #dev:backend", &HashMap::new());
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Backend");
}

#[test]
fn test_hide_hidden_calendars() {
    let mut store = make_store();

    let mut t1 = Task::new("Cal 1 Task", &HashMap::new());
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();

    let mut t2 = Task::new("Cal 2 Task", &HashMap::new());
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
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].summary, "Cal 1 Task");
}
