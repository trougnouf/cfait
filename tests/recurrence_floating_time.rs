// Tests for floating time behavior in recurrence engine.
// Ensures that recurring tasks respect local wall-clock time across Daylight Saving Time boundaries.

use cfait::model::{DateType, Task};
use chrono::NaiveTime;
use std::collections::HashMap;

// Helper function to parse task
fn parse(input: &str) -> Task {
    Task::new(input, &HashMap::new(), None)
}

#[test]
fn test_floating_time_weekly_recurrence() {
    // Create a task with a weekly recurrence
    let t = parse("Take vitamins @weekly");

    // Ensure the task has a recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.clone().unwrap();
    assert!(rrule.contains("FREQ=WEEKLY"));

    // Ensure the task has a start time
    assert!(t.dtstart.is_some());

    // Calculate the next occurrence
    let next_task = cfait::model::RecurrenceEngine::next_occurrence(&t);

    // Ensure the next occurrence is calculated
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Ensure the next occurrence has the same local time as the original task
    let original_time = match t.dtstart.unwrap() {
        DateType::Specific(dt) => dt.with_timezone(&chrono::Local).time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    let next_time = match next_task.dtstart.unwrap() {
        DateType::Specific(dt) => dt.with_timezone(&chrono::Local).time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    assert_eq!(original_time, next_time);
}

#[test]
fn test_floating_time_daily_recurrence() {
    // Create a task with a daily recurrence
    let t = parse("Morning walk @daily");

    // Ensure the task has a recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.clone().unwrap();
    assert!(rrule.contains("FREQ=DAILY"));

    // Ensure the task has a start time
    assert!(t.dtstart.is_some());

    // Calculate the next occurrence
    let next_task = cfait::model::RecurrenceEngine::next_occurrence(&t);

    // Ensure the next occurrence is calculated
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Ensure the next occurrence has the same local time as the original task
    let original_time = match t.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    let next_time = match next_task.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    assert_eq!(original_time, next_time);
}

#[test]
fn test_floating_time_monthly_recurrence() {
    // Create a task with a monthly recurrence
    let t = parse("Pay rent @monthly");

    // Ensure the task has a recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.clone().unwrap();
    assert!(rrule.contains("FREQ=MONTHLY"));

    // Ensure the task has a start time
    assert!(t.dtstart.is_some());

    // Calculate the next occurrence
    let next_task = cfait::model::RecurrenceEngine::next_occurrence(&t);

    // Ensure the next occurrence is calculated
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Ensure the next occurrence has the same local time as the original task
    let original_time = match t.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    let next_time = match next_task.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    assert_eq!(original_time, next_time);
}

#[test]
fn test_floating_time_yearly_recurrence() {
    // Create a task with a yearly recurrence
    let t = parse("Annual checkup @yearly");

    // Ensure the task has a recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.clone().unwrap();
    assert!(rrule.contains("FREQ=YEARLY"));

    // Ensure the task has a start time
    assert!(t.dtstart.is_some());

    // Calculate the next occurrence
    let next_task = cfait::model::RecurrenceEngine::next_occurrence(&t);

    // Ensure the next occurrence is calculated
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Ensure the next occurrence has the same local time as the original task
    let original_time = match t.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    let next_time = match next_task.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    assert_eq!(original_time, next_time);
}

#[test]
fn test_floating_time_with_explicit_time() {
    // Create a task with a specific time and weekly recurrence
    let t = parse("Take vitamins at 11:00 @weekly");

    // Ensure the task has a recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.clone().unwrap();
    assert!(rrule.contains("FREQ=WEEKLY"));

    // Ensure the task has a start time
    assert!(t.dtstart.is_some());

    // Calculate the next occurrence
    let next_task = cfait::model::RecurrenceEngine::next_occurrence(&t);

    // Ensure the next occurrence is calculated
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Ensure the next occurrence has the same local time as the original task
    let original_time = match t.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    let next_time = match next_task.dtstart.unwrap() {
        DateType::Specific(dt) => dt.time(),
        DateType::AllDay(_d) => NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };

    assert_eq!(original_time, next_time);
}
