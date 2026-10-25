// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for recurrence date calculation logic.
use cfait::model::{DateType, Task, TaskStatus};
use chrono::{Duration, Utc};
use std::collections::HashMap;

// Helper to create a task due at Now + Offset days
fn create_task_due_in_days(days: i64, recurrence: &str) -> Task {
    let mut t = Task::new("Task", &HashMap::new(), None);
    // Use Utc directly as internal storage uses Utc
    let dt = Utc::now() + Duration::days(days);
    t.due = Some(DateType::Specific(dt));
    t.rrule = Some(recurrence.to_string());
    t.status = TaskStatus::Completed;
    t
}

#[test]
fn test_daily_recurrence() {
    // Create task due yesterday (-1 day)
    let mut t = create_task_due_in_days(-1, "FREQ=DAILY");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(t.status, TaskStatus::NeedsAction);

    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // The engine works in local wall-clock time (floating time): the next
    // occurrence is the first daily slot strictly after now. With a seed of
    // "yesterday at the current wall-clock time", that is usually tomorrow —
    // but within 24h after a DST fall-back the "today" slot is still ahead
    // of now (the seed shifted +1h in wall-clock terms), so it is today.
    // Accept whichever slot is first after now, compared in local naive time.
    let original_local = original_due.with_timezone(&chrono::Local).naive_local();
    let new_local = new_due.with_timezone(&chrono::Local).naive_local();
    let today_slot = original_local + Duration::days(1);
    let expected_local = if today_slot > chrono::Local::now().naive_local() {
        today_slot
    } else {
        original_local + Duration::days(2)
    };

    let diff = new_local - expected_local;
    assert!(
        diff.num_seconds().abs() < 5,
        "Expected the next daily slot after now"
    );
}

#[test]
fn test_weekly_recurrence() {
    // Due 8 days ago
    let mut t = create_task_due_in_days(-8, "FREQ=WEEKLY");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    let advanced = t.advance_recurrence();
    assert!(advanced);

    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // Sequence: now-8d, now-1d, now+6d...
    // now-1d < now.
    // now+6d > now.
    // Expected: now + 6 days.
    // original (-8) + 14 = +6.

    let original_local = original_due.with_timezone(&chrono::Local).naive_local();
    let new_local = new_due.with_timezone(&chrono::Local).naive_local();
    let expected_local = original_local + Duration::days(14);

    let diff = new_local - expected_local;
    assert!(diff.num_seconds().abs() < 5);
}

#[test]
fn test_monthly_recurrence() {
    // Due 35 days ago (~1 month ago)
    let mut t = create_task_due_in_days(-35, "FREQ=MONTHLY");

    // We can't easily predict exact day due to variable month lengths without complex logic matching the rrule crate.
    // But we know it should be in the future relative to now.

    let advanced = t.advance_recurrence();
    assert!(advanced);

    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    assert!(new_due > Utc::now());
}

#[test]
fn test_custom_interval() {
    // Every 3 days. Due 4 days ago.
    // Sequence: -4, -1, +2.
    // -1 < now.
    // +2 > now.
    // Next should be +2 days from now.
    let mut t = create_task_due_in_days(-4, "FREQ=DAILY;INTERVAL=3");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    let advanced = t.advance_recurrence();
    assert!(advanced);

    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    let original_local = original_due.with_timezone(&chrono::Local).naive_local();
    let new_local = new_due.with_timezone(&chrono::Local).naive_local();
    let expected_local = original_local + Duration::days(6);

    let diff = new_local - expected_local;
    assert!(diff.num_seconds().abs() < 5);
}

#[test]
fn test_complex_weekday_recurrence() {
    // This implies we need to match specific weekdays.
    // It's safer to just check it advances to future.
    let mut t = create_task_due_in_days(-10, "FREQ=WEEKLY;BYDAY=MO");
    t.advance_recurrence();
    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };
    assert!(new_due > Utc::now());
}

#[test]
fn test_recurrence_preserves_time() {
    let mut t = Task::new("Time Test", &HashMap::new(), None);
    let dt = Utc::now();
    t.due = Some(DateType::Specific(dt));
    t.rrule = Some("FREQ=DAILY".to_string());

    t.advance_recurrence();

    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // Check hour/minute match (allowing slight drift if rrule recalculates, but usually it preserves)
    // Actually rrule calculated from DTSTART preserves time.
    assert_eq!(
        dt.with_timezone(&chrono::Local).format("%H:%M").to_string(),
        new_due
            .with_timezone(&chrono::Local)
            .format("%H:%M")
            .to_string()
    );
}

#[test]
fn test_advance_increments_sequence_instead_of_resetting() {
    // Regression: advance used to reset SEQUENCE to 1, making a recycled task
    // look older than its previous versions and losing stale-read protection
    // (store.rs), cross-calendar dedup, and sync conflict resolution.
    let mut t = create_task_due_in_days(-1, "FREQ=DAILY");
    let original_uid = t.uid.clone();
    t.sequence = 5;

    let advanced = t.advance_recurrence();
    assert!(advanced);
    assert_eq!(
        t.sequence, 6,
        "sequence must increment from the original value"
    );
    assert_eq!(t.uid, original_uid, "recycling preserves the UID");
}

#[test]
fn test_cancel_single_occurrence_daily() {
    // Create a task with daily recurrence due yesterday
    let t = create_task_due_in_days(-1, "FREQ=DAILY");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // Recycle the task to a Cancelled state
    let (history, recycled) = t.recycle(TaskStatus::Cancelled, false);
    let advanced_task = recycled.expect("Should advance");

    assert_eq!(history.status, TaskStatus::Cancelled);
    assert_eq!(
        history.due.as_ref().unwrap().to_comparison_time(),
        original_due
    );

    assert_eq!(advanced_task.status, TaskStatus::NeedsAction);
    assert!(advanced_task.due.as_ref().unwrap().to_comparison_time() > Utc::now());
}
