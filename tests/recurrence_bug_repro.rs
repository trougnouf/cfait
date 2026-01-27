// File: ./src/tests/recurrence_bug_repro.rs
use cfait::model::{DateType, Task, TaskStatus};
use chrono::{Duration, Utc};
use std::collections::HashMap;

fn create_weekly_task(uid: &str, offset_days: i64) -> Task {
    let aliases = HashMap::new();
    let mut t = Task::new("Rec Test Task", &aliases, None);
    t.uid = uid.to_string();

    // Set dates relative to now to avoid time-bomb test failures
    let dt = (Utc::now() + Duration::days(offset_days)).date_naive();
    t.dtstart = Some(DateType::AllDay(dt));
    t.due = Some(DateType::AllDay(dt));

    t.rrule = Some("FREQ=WEEKLY".to_string());
    t.status = TaskStatus::NeedsAction;
    t
}

#[test]
fn test_recurrence_recycling_preserves_uid() {
    // Task due today
    let mut t = create_weekly_task("recycle_uid_test", 0);
    let original_uid = t.uid.clone();

    // Mark as done (Advance Recurrence)
    let advanced = t.advance_recurrence();

    assert!(advanced, "Should have advanced");
    assert_eq!(
        t.uid, original_uid,
        "UID must be preserved for Tasks.org compatibility"
    );
    assert_eq!(
        t.status,
        TaskStatus::NeedsAction,
        "Status should reset to NeedsAction"
    );
    assert_ne!(t.dtstart, None, "Dates should be updated");

    // Check it moved 1 week forward
    let old_due = (Utc::now()).date_naive();
    let new_due = t.due.as_ref().unwrap().to_date_naive();
    let diff = new_due - old_due;
    assert_eq!(diff.num_days(), 7, "Date should advance 1 week");
}

#[test]
fn test_scenario_1_cancel_then_done_sequence() {
    // create task starting 5 days ago
    let mut t = create_weekly_task("task1", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // 1. Mark as done -> Should move +7 days from initial
    t.status = TaskStatus::Completed;
    assert!(t.advance_recurrence(), "Should advance");
    let due1 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due1, initial_due + Duration::days(7));
    assert_eq!(t.status, TaskStatus::NeedsAction);

    // 2. Mark as done -> Should move another +7 days
    t.status = TaskStatus::Completed;
    assert!(t.advance_recurrence(), "Should advance");
    let due2 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due2, initial_due + Duration::days(14));

    // 3. Mark as canceled -> Should move another +7 days, adding exception
    t.status = TaskStatus::Cancelled;
    assert!(t.advance_recurrence_with_cancellation(), "Should cancel");
    let due3 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due3, initial_due + Duration::days(21));
    assert!(!t.exdates.is_empty(), "Should have exdates");

    // 4. Mark as canceled -> Should move another +7 days
    t.status = TaskStatus::Cancelled;
    assert!(
        t.advance_recurrence_with_cancellation(),
        "Should cancel again"
    );

    let due4 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(
        due4,
        initial_due + Duration::days(28),
        "Should have advanced again"
    );
}

#[test]
fn test_scenario_2_multiple_cancels() {
    // task starting 5 days ago
    let mut t = create_weekly_task("task2", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // 1. Cancel -> +7 days
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(7)
    );

    // 2. Cancel -> +14 days
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(14)
    );

    // 3. Cancel -> +21 days
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(21)
    );

    assert_eq!(t.exdates.len(), 3);
}

#[test]
fn test_scenario_3_cancel_loop() {
    let mut t = create_weekly_task("task3", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // Done -> +7
    t.status = TaskStatus::Completed;
    t.advance_recurrence();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(7)
    );

    // Cancel -> +14
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(14)
    );

    // Done -> +21
    t.status = TaskStatus::Completed;
    t.advance_recurrence();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(21)
    );

    // Cancel -> +28
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(28)
    );
}
