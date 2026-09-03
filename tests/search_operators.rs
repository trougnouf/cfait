// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for search query operators.
use cfait::context::TestContext;
use cfait::model::{DateType, Task, TaskStatus}; // Added DateType import
use cfait::store::TaskStore;
use chrono::{Duration, Local, Utc}; // Added Utc import
use std::collections::HashMap;
use std::sync::Arc;

fn make_task() -> Task {
    Task::new("Test Task", &HashMap::new(), None)
}

fn create_store() -> TaskStore {
    TaskStore::new(Arc::new(TestContext::new()))
}

#[test]
fn test_status_filters() {
    let mut active = make_task();
    active.status = TaskStatus::NeedsAction;

    let mut done = make_task();
    done.status = TaskStatus::Completed;

    let mut started = make_task();
    started.status = TaskStatus::InProcess;

    // is:done
    assert!(!active.matches_search_term("is:done", &create_store()));
    assert!(done.matches_search_term("is:done", &create_store()));

    // is:active (Should match NeedsAction and InProcess, but NOT Completed)
    assert!(active.matches_search_term("is:active", &create_store()));
    assert!(started.matches_search_term("is:active", &create_store()));
    assert!(!done.matches_search_term("is:active", &create_store()));

    // is:started
    assert!(started.matches_search_term("is:started", &create_store()));
    assert!(!active.matches_search_term("is:started", &create_store()));

    // is:ongoing (legacy alias for is:started)
    assert!(started.matches_search_term("is:ongoing", &create_store()));
    assert!(!active.matches_search_term("is:ongoing", &create_store()));

    // is:ready and is:blocked are consumed by the matcher but actual logic is in TaskStore
    // Here we just verify the tokens don't cause text match failures
    assert!(active.matches_search_term("is:ready", &create_store()));
    assert!(active.matches_search_term("is:blocked", &create_store()));
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
    assert!(high.matches_search_term("!<3", &create_store()));
    assert!(!med.matches_search_term("!<3", &create_store()));

    // !>=5 (Medium or Lower)
    assert!(!high.matches_search_term("!>=5", &create_store()));
    assert!(med.matches_search_term("!>=5", &create_store()));
    assert!(low.matches_search_term("!>=5", &create_store()));

    // !1 (Exact match)
    assert!(high.matches_search_term("!1", &create_store()));
    assert!(!med.matches_search_term("!1", &create_store()));
}

#[test]
fn test_duration_operators() {
    let mut quick = make_task();
    quick.estimated_duration = Some(15); // 15m

    let mut long = make_task();
    long.estimated_duration = Some(120); // 2h

    // ~<30m
    assert!(quick.matches_search_term("~<30m", &create_store()));
    assert!(!long.matches_search_term("~<30m", &create_store()));

    // ~>1h
    assert!(!quick.matches_search_term("~>1h", &create_store()));
    assert!(long.matches_search_term("~>1h", &create_store()));
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
    assert!(overdue.matches_search_term("@<today", &create_store()));
    assert!(!future.matches_search_term("@<today", &create_store()));

    // @>tomorrow
    assert!(future.matches_search_term("@>tomorrow", &create_store()));
    assert!(!overdue.matches_search_term("@>tomorrow", &create_store()));
}

#[test]
fn test_combined_filters() {
    let mut t = make_task();
    t.priority = 1;
    t.estimated_duration = Some(30);
    t.categories.push("work".to_string());

    // Should match: High priority AND short duration AND #work
    assert!(t.matches_search_term("!<3 ~<1h #work", &create_store()));

    // Should fail: wrong tag
    assert!(!t.matches_search_term("!<3 #personal", &create_store()));

    // Should fail: duration mismatch
    assert!(!t.matches_search_term("~>2h", &create_store()));
}
