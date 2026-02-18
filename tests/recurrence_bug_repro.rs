// Updated recurrence tests to use the `recycle` method which mirrors how the
// application/store handles recurring task completion/cancellation.
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
    let t = create_weekly_task("recycle_uid_test", 0);
    let original_uid = t.uid.clone();

    // Simulate completing the task which the store handles by recycling
    let (history, secondary) = t.recycle(TaskStatus::Completed);
    assert_eq!(history.status, TaskStatus::Completed);

    let t = secondary.unwrap();
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

    let old_due = (Utc::now()).date_naive();
    let new_due = t.due.as_ref().unwrap().to_date_naive();
    let diff = new_due - old_due;
    assert_eq!(diff.num_days(), 7, "Date should advance 1 week");
}

#[test]
fn test_scenario_1_cancel_then_done_sequence() {
    let t = create_weekly_task("task1", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // 1. Mark as done -> Recycle
    let (history1, secondary1) = t.recycle(TaskStatus::Completed);
    assert_eq!(history1.status, TaskStatus::Completed);
    let t = secondary1.unwrap();
    let due1 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due1, initial_due + Duration::days(7));
    assert_eq!(t.status, TaskStatus::NeedsAction);

    // 2. Mark as done -> Recycle again
    let (history2, secondary2) = t.recycle(TaskStatus::Completed);
    assert_eq!(history2.status, TaskStatus::Completed);
    let t = secondary2.unwrap();
    let due2 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due2, initial_due + Duration::days(14));

    // 3. Mark as canceled -> Recycle
    let (history3, secondary3) = t.recycle(TaskStatus::Cancelled);
    assert_eq!(history3.status, TaskStatus::Cancelled);
    let t = secondary3.unwrap();
    let due3 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due3, initial_due + Duration::days(21));
    assert!(
        !t.exdates.is_empty(),
        "Next instance should contain the EXDATE"
    );
    assert_eq!(t.exdates.len(), 1);

    // 4. Mark as canceled again
    let (history4, secondary4) = t.recycle(TaskStatus::Cancelled);
    assert_eq!(history4.status, TaskStatus::Cancelled);
    let t = secondary4.unwrap();
    let due4 = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due4, initial_due + Duration::days(28));
    assert_eq!(t.exdates.len(), 2, "Should have accumulated another EXDATE");
}

#[test]
fn test_scenario_2_multiple_cancels() {
    let t = create_weekly_task("task2", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // 1. Cancel -> +7 days
    let (_, secondary1) = t.recycle(TaskStatus::Cancelled);
    let t = secondary1.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(7)
    );

    // 2. Cancel -> +14 days
    let (_, secondary2) = t.recycle(TaskStatus::Cancelled);
    let t = secondary2.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(14)
    );

    // 3. Cancel -> +21 days
    let (_, secondary3) = t.recycle(TaskStatus::Cancelled);
    let t = secondary3.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(21)
    );

    assert_eq!(t.exdates.len(), 3);
}

#[test]
fn test_scenario_3_cancel_loop() {
    let t = create_weekly_task("task3", -5);
    let initial_due = t.due.as_ref().unwrap().to_date_naive();

    // Done -> +7
    let (_, secondary1) = t.recycle(TaskStatus::Completed);
    let t = secondary1.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(7)
    );

    // Cancel -> +14
    let (_, secondary2) = t.recycle(TaskStatus::Cancelled);
    let t = secondary2.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(14)
    );

    // Done -> +21
    let (_, secondary3) = t.recycle(TaskStatus::Completed);
    let t = secondary3.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(21)
    );

    // Cancel -> +28
    let (_, secondary4) = t.recycle(TaskStatus::Cancelled);
    let t = secondary4.unwrap();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        initial_due + Duration::days(28)
    );
}
