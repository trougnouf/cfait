// File: ./tests/recurrence_duration_fix.rs
// Reproduces the issue where All-Day tasks lose their duration (gap between start and due)
// upon recurrence calculation.

use cfait::model::{DateType, Task, TaskStatus};
use chrono::NaiveDate;
use std::collections::HashMap;

#[test]
fn test_recurrence_preserves_duration_gap_for_allday_tasks() {
    let aliases = HashMap::new();
    let mut t = Task::new("Yearly Contract Renewal", &aliases, None);

    // Scenario from issue:
    // Start: 2026-01-20
    // Due:   2026-02-20
    // Rec:   Yearly

    let start_date = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();
    t.dtstart = Some(DateType::AllDay(start_date));

    let due_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
    t.due = Some(DateType::AllDay(due_date));

    t.rrule = Some("FREQ=YEARLY".to_string());

    // Mark as completed to trigger recurrence advance
    t.status = TaskStatus::Completed;

    // Perform the advance
    let advanced = t.advance_recurrence();
    assert!(advanced, "Task should have advanced recurrence");

    // Check Next Start Date: Should be 2027-01-20
    let next_start = match t.dtstart.unwrap() {
        DateType::AllDay(d) => d,
        _ => panic!("Expected AllDay start date"),
    };
    assert_eq!(
        next_start,
        NaiveDate::from_ymd_opt(2027, 1, 20).unwrap(),
        "Start date should advance by exactly one year"
    );

    // Check Next Due Date: Should be 2027-02-20
    // BUG BEHAVIOR: Previously this resulted in 2027-01-20 (snapped to start date)
    let next_due = match t.due.unwrap() {
        DateType::AllDay(d) => d,
        _ => panic!("Expected AllDay due date"),
    };

    assert_eq!(
        next_due,
        NaiveDate::from_ymd_opt(2027, 2, 20).unwrap(),
        "Due date should preserve the 1-month gap from start date"
    );
}
