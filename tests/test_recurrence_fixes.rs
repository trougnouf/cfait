// Tests for recurrence and respawn fixes.
use cfait::model::{
    DateType, Task, TaskStatus,
    parser::{SyntaxType, tokenize_smart_input},
};
use chrono::{Duration, Local, NaiveTime, Timelike, Utc};
use std::collections::HashMap;

#[test]
fn test_recurrence_with_time_parsing() {
    // Test that "@every sunday 20:00" correctly parses the time.
    let aliases = HashMap::new();
    let task = Task::new("Water plants @every sunday 20:00", &aliases, None);

    assert_eq!(task.summary, "Water plants");
    assert!(task.rrule.is_some());
    assert!(task.due.is_some());

    // Check that the due date is set to the next Sunday at 20:00.
    let due_date = match task.due.unwrap() {
        DateType::Specific(dt) => dt,
        _ => panic!("Expected a specific date"),
    };

    // Ensure the time is 20:00 in the local timezone.
    let local_time = due_date.with_timezone(&Local).time();
    assert_eq!(local_time.hour(), 20);
    assert_eq!(local_time.minute(), 0);
}

#[test]
fn test_recurrence_with_time_syntax_highlighting() {
    // Test that the syntax highlighter includes the time in the recurrence highlighting.
    let input = "Water plants @every sunday 20:00";
    let tokens = tokenize_smart_input(input);

    // Find the recurrence token and ensure it includes the time.
    let recurrence_token = tokens.iter().find(|t| t.kind == SyntaxType::Recurrence);
    assert!(recurrence_token.is_some());

    let token = recurrence_token.unwrap();
    assert!(input[token.start..token.end].contains("20:00"));
}

#[test]
fn test_recurrence_with_weekdays_and_time() {
    // Test that "@every mon,wed,fri 8pm" correctly parses the time.
    let aliases = HashMap::new();
    let task = Task::new("Meeting @every mon,wed,fri 8pm", &aliases, None);

    assert_eq!(task.summary, "Meeting");
    assert!(task.rrule.is_some());
    assert!(task.due.is_some());

    // Check that the due date is set to the next occurrence at 20:00.
    let due_date = match task.due.unwrap() {
        DateType::Specific(dt) => dt,
        _ => panic!("Expected a specific date"),
    };

    // Ensure the time is 20:00 in the local timezone.
    let local_time = due_date.with_timezone(&Local).time();
    assert_eq!(local_time.hour(), 20);
    assert_eq!(local_time.minute(), 0);
}

#[test]
fn test_respawn_with_all_day_task() {
    // Test that respawn correctly handles all-day tasks.
    let mut task = Task::new("Daily task @daily", &HashMap::new(), None);
    task.status = TaskStatus::Completed;

    // Set the task's due date to yesterday (all-day).
    let yesterday = Utc::now().date_naive() - Duration::days(1);
    task.due = Some(DateType::AllDay(yesterday));

    // Respawn the task.
    let respawned_task = task.respawn();

    assert!(respawned_task.is_some());
    let respawned_task = respawned_task.unwrap();

    // Ensure the respawned task is due today (all-day).
    let due_date = match respawned_task.due.unwrap() {
        DateType::AllDay(d) => d,
        _ => panic!("Expected an all-day date"),
    };

    assert_eq!(due_date, Utc::now().date_naive());
}

#[test]
fn test_respawn_with_specific_time() {
    // Test that respawn correctly handles tasks with a specific time.
    let mut task = Task::new("Evening task @every sunday 20:00", &HashMap::new(), None);
    task.status = TaskStatus::Completed;

    // Set the task's due date to last Sunday at 20:00.
    let last_sunday = Utc::now().date_naive() - Duration::weeks(1);
    let due_time = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
    let due_datetime = last_sunday.and_time(due_time)
        .and_utc();
    task.due = Some(DateType::Specific(due_datetime));

    // Respawn the task.
    let respawned_task = task.respawn();

    assert!(respawned_task.is_some());
    let respawned_task = respawned_task.unwrap();

    // Ensure the respawned task is due next Sunday at 20:00.
    let due_date = match respawned_task.due.unwrap() {
        DateType::Specific(dt) => dt,
        _ => panic!("Expected a specific date"),
    };

    // Ensure the time is 20:00 in UTC.
    assert_eq!(due_date.time().hour(), 20);
    assert_eq!(due_date.time().minute(), 0);
}

#[test]
fn test_respawn_with_search_floor() {
    // Test that respawn correctly uses the search_floor to avoid respawning on the same day.
    let mut task = Task::new("Evening task @every sunday 20:00", &HashMap::new(), None);
    task.status = TaskStatus::Completed;

    // Set the task's due date to today at 20:00 (later today).
    let today = Utc::now().date_naive();
    let due_time = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
    let due_datetime = today.and_time(due_time)
        .and_utc();
    task.due = Some(DateType::Specific(due_datetime));

    // Respawn the task.
    let respawned_task = task.respawn();

    assert!(respawned_task.is_some());
    let respawned_task = respawned_task.unwrap();

    // Ensure the respawned task is due next Sunday at 20:00, not today.
    let due_date = match respawned_task.due.unwrap() {
        DateType::Specific(dt) => dt,
        _ => panic!("Expected a specific date"),
    };

    assert!(due_date.date_naive() > today);
    // Ensure the time is 20:00 in UTC.
    assert_eq!(due_date.time().hour(), 20);
    assert_eq!(due_date.time().minute(), 0);
}
