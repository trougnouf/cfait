// File: tests/logic_recurrence.rs
use cfait::model::{Task, TaskStatus};
use chrono::{Duration, TimeZone, Utc};
use std::collections::HashMap;

fn create_task_due(date_str: &str, recurrence: &str) -> Task {
    let mut t = Task::new("Task", &HashMap::new());
    // Create a date at 12:00 UTC to avoid timezone edge cases in tests
    let dt = Utc
        .datetime_from_str(
            format!("{} 12:00:00", date_str).as_str(),
            "%Y-%m-%d %H:%M:%S",
        )
        .unwrap();
    t.due = Some(dt);
    t.rrule = Some(recurrence.to_string());
    t.status = TaskStatus::Completed; // Simulate completion
    t
}

#[test]
fn test_daily_recurrence() {
    let mut t = create_task_due("2023-01-01", "FREQ=DAILY");

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(t.status, TaskStatus::NeedsAction); // Should reset status
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-01-02");
}

#[test]
fn test_weekly_recurrence() {
    let mut t = create_task_due("2023-01-01", "FREQ=WEEKLY"); // Sunday

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-01-08");
}

#[test]
fn test_monthly_recurrence() {
    let mut t = create_task_due("2023-02-01", "FREQ=MONTHLY");

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-03-01");
}

#[test]
fn test_custom_interval() {
    // "Every 3 days"
    let mut t = create_task_due("2023-01-01", "FREQ=DAILY;INTERVAL=3");

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-01-04");
}

#[test]
fn test_complex_weekday_recurrence() {
    // "Every week on Monday"
    // Start on a Sunday (Jan 1 2023)
    let mut t = create_task_due("2023-01-01", "FREQ=WEEKLY;BYDAY=MO");

    let advanced = t.advance_recurrence();
    assert!(advanced);
    // Should jump to the immediate next Monday (Jan 2)
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-01-02");

    // Advance again -> Next Monday (Jan 9)
    let advanced_again = t.advance_recurrence();
    assert!(advanced_again);
    assert_eq!(t.due.unwrap().format("%Y-%m-%d").to_string(), "2023-01-09");
}

#[test]
fn test_recurrence_preserves_time() {
    // Ensure that if a task is due at 14:00, the next one is also due at 14:00
    let mut t = Task::new("Time Test", &HashMap::new());
    let dt = Utc
        .datetime_from_str("2023-01-01 14:30:00", "%Y-%m-%d %H:%M:%S")
        .unwrap();
    t.due = Some(dt);
    t.rrule = Some("FREQ=DAILY".to_string());

    t.advance_recurrence();

    let new_due = t.due.unwrap();
    assert_eq!(new_due.format("%H:%M").to_string(), "14:30");
}
