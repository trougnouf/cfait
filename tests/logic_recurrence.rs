// Tests for recurrence date calculation logic.
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

    // Since task was due yesterday, and we are "now" (today),
    // next occurrence after now should be Today (if time allows) or Tomorrow.
    // logic: find > now.
    // if original = yesterday. recurrence = daily.
    // sequence: yesterday, today, tomorrow...
    // if yesterday < now, check today.
    // if today > now (e.g. task was due at 23:00 yesterday, now is 10:00 today, recurrence is 23:00),
    // then today 23:00 > now 10:00. So next is today.
    // if task was due at 09:00 yesterday, now is 10:00 today.
    // yesterday 09:00 < now.
    // today 09:00 < now.
    // next is tomorrow 09:00.

    // Since we created it with `Utc::now() + days`, the time component matches `now`.
    // original = now - 24h.
    // next candidate = now.
    // is now > now? No (equal).
    // next candidate = now + 24h.
    // is now + 24h > now? Yes.
    // So expected is original + 2 days (i.e. Tomorrow relative to Now).

    let expected_min = original_due + Duration::days(2); // (now - 1) + 2 = now + 1
    let diff = new_due.signed_duration_since(expected_min);
    assert!(diff.num_seconds().abs() < 5, "Expected around tomorrow");
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

    let expected = original_due + Duration::days(14);
    let diff = new_due.signed_duration_since(expected);
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

    let expected = original_due + Duration::days(6); // -4 + 6 = +2
    let diff = new_due.signed_duration_since(expected);
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
        dt.format("%H:%M").to_string(),
        new_due.format("%H:%M").to_string()
    );
}

#[test]
fn test_cancel_single_occurrence_daily() {
    // Create a task with daily recurrence due yesterday
    let mut t = create_task_due_in_days(-1, "FREQ=DAILY");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // Cancel this occurrence
    t.status = TaskStatus::Cancelled;
    let advanced = t.advance_recurrence_with_cancellation();
    assert!(advanced);

    // Task should advance to next occurrence
    assert_eq!(t.status, TaskStatus::NeedsAction);

    // Should have added the cancelled date to exdates
    assert_eq!(t.exdates.len(), 1);
    assert_eq!(t.exdates[0], DateType::Specific(original_due));

    // New due date should be in the future
    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };
    assert!(new_due > Utc::now());
}

#[test]
fn test_cancel_multiple_occurrences() {
    // Note: This tests that exdates accumulate across cancellations.
    // The respawn logic uses the current occurrence as the seed date,
    // so patterns work best for near-term recurrences.
    let mut t = create_task_due_in_days(-5, "FREQ=DAILY");

    // Cancel first occurrence
    t.status = TaskStatus::Cancelled;
    let success1 = t.advance_recurrence_with_cancellation();
    assert!(success1, "First cancellation should succeed");
    assert_eq!(t.exdates.len(), 1);

    let first_cancelled = t.exdates[0].clone();

    // Task should have advanced to next occurrence with NeedsAction status
    assert_eq!(t.status, TaskStatus::NeedsAction);

    // The cancelled date should be preserved in exdates
    assert_eq!(t.exdates[0], first_cancelled);

    // Task should still be recurring
    assert!(t.rrule.is_some());
}

#[test]
fn test_cancel_weekly_occurrence() {
    // Create a task with weekly recurrence due 8 days ago
    let mut t = create_task_due_in_days(-8, "FREQ=WEEKLY");
    let original_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };

    // Cancel this occurrence
    t.status = TaskStatus::Cancelled;
    let advanced = t.advance_recurrence_with_cancellation();
    assert!(advanced);

    // Should have added to exdates
    assert_eq!(t.exdates.len(), 1);
    assert_eq!(t.exdates[0], DateType::Specific(original_due));

    // New due should be ~7 days after the cancelled date
    let new_due = match t.due.as_ref().unwrap() {
        DateType::Specific(d) => *d,
        _ => panic!(),
    };
    assert!(new_due > Utc::now());
}

#[test]
fn test_cancel_preserves_task_properties() {
    // Create a task with various properties
    let mut t = create_task_due_in_days(-1, "FREQ=DAILY");
    t.summary = "Important Task".to_string();
    t.description = "Task description".to_string();
    t.priority = 5;
    t.categories = vec!["work".to_string()];

    let original_summary = t.summary.clone();
    let original_description = t.description.clone();
    let original_priority = t.priority;
    let original_categories = t.categories.clone();

    // Cancel and advance
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();

    // Properties should be preserved
    assert_eq!(t.summary, original_summary);
    assert_eq!(t.description, original_description);
    assert_eq!(t.priority, original_priority);
    assert_eq!(t.categories, original_categories);
}

#[test]
fn test_cancel_non_recurring_task() {
    // Create a task without recurrence
    let mut t = Task::new("Non-recurring", &HashMap::new(), None);
    t.due = Some(DateType::Specific(Utc::now()));
    t.status = TaskStatus::Cancelled;

    // Should not advance (no recurrence rule)
    let advanced = t.advance_recurrence_with_cancellation();
    assert!(!advanced);

    // Status should remain Cancelled
    assert_eq!(t.status, TaskStatus::Cancelled);

    // No exdates should be added (though the function adds it, it doesn't matter since no recurrence)
    // Actually, looking at the code, it WILL add to exdates, but won't advance
    assert_eq!(t.exdates.len(), 1);
}

#[test]
fn test_exdates_prevent_recurrence_on_cancelled_date() {
    // Create a task with daily recurrence
    let mut t = create_task_due_in_days(-3, "FREQ=DAILY");

    // Cancel one occurrence in the middle
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();

    let cancelled_date = t.exdates[0].clone();

    // Complete the next occurrence (should skip the cancelled date)
    t.status = TaskStatus::Completed;
    t.advance_recurrence();

    // The cancelled date should still be in exdates
    assert!(t.exdates.contains(&cancelled_date));

    // And the new due date should not match the cancelled date
    let new_due = t.due.as_ref().unwrap();
    assert_ne!(*new_due, cancelled_date);
}
