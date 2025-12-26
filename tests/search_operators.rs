// File: tests/search_operators.rs
use cfait::model::{DateType, Task, TaskStatus}; // Added DateType import
use chrono::{Duration, Local, Utc}; // Added Utc import
use std::collections::HashMap;

fn make_task() -> Task {
    Task::new("Test Task", &HashMap::new())
}

#[test]
fn test_status_filters() {
    let mut active = make_task();
    active.status = TaskStatus::NeedsAction;

    let mut done = make_task();
    done.status = TaskStatus::Completed;

    let mut ongoing = make_task();
    ongoing.status = TaskStatus::InProcess;

    // is:done
    assert!(!active.matches_search_term("is:done"));
    assert!(done.matches_search_term("is:done"));

    // is:active (Should match NeedsAction and InProcess, but NOT Completed)
    assert!(active.matches_search_term("is:active"));
    assert!(ongoing.matches_search_term("is:active"));
    assert!(!done.matches_search_term("is:active"));

    // is:ongoing
    assert!(ongoing.matches_search_term("is:ongoing"));
    assert!(!active.matches_search_term("is:ongoing"));
}

#[test]
fn test_priority_operators() {
    let mut high = make_task();
    high.priority = 1;

    let mut med = make_task();
    med.priority = 5;

    let mut low = make_task();
    low.priority = 9;

    // !<3 (High priority only: 1, 2)
    assert!(high.matches_search_term("!<3"));
    assert!(!med.matches_search_term("!<3"));

    // !>=5 (Medium or Lower)
    assert!(!high.matches_search_term("!>=5"));
    assert!(med.matches_search_term("!>=5"));
    assert!(low.matches_search_term("!>=5"));

    // !1 (Exact match)
    assert!(high.matches_search_term("!1"));
    assert!(!med.matches_search_term("!1"));
}

#[test]
fn test_duration_operators() {
    let mut quick = make_task();
    quick.estimated_duration = Some(15); // 15m

    let mut long = make_task();
    long.estimated_duration = Some(120); // 2h

    // ~<30m
    assert!(quick.matches_search_term("~<30m"));
    assert!(!long.matches_search_term("~<30m"));

    // ~>1h
    assert!(!quick.matches_search_term("~>1h"));
    assert!(long.matches_search_term("~>1h"));
}

#[test]
fn test_date_operators() {
    let now = Local::now();

    let mut overdue = make_task();
    // Explicit conversion
    overdue.due = Some(DateType::Specific(
        (now - Duration::days(5)).with_timezone(&Utc),
    ));

    let mut future = make_task();
    // Explicit conversion
    future.due = Some(DateType::Specific(
        (now + Duration::days(5)).with_timezone(&Utc),
    ));

    // @<today (Overdue)
    // Note: The matcher logic compares against today's date
    assert!(overdue.matches_search_term("@<today"));
    assert!(!future.matches_search_term("@<today"));

    // @>tomorrow
    assert!(future.matches_search_term("@>tomorrow"));
    assert!(!overdue.matches_search_term("@>tomorrow"));
}

#[test]
fn test_combined_filters() {
    let mut t = make_task();
    t.priority = 1;
    t.estimated_duration = Some(30);
    t.categories.push("work".to_string());

    // Should match: High priority AND short duration AND #work
    assert!(t.matches_search_term("!<3 ~<1h #work"));

    // Should fail: wrong tag
    assert!(!t.matches_search_term("!<3 #personal"));

    // Should fail: duration mismatch
    assert!(!t.matches_search_term("~>2h"));
}
