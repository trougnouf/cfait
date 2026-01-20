// File: ./tests/recurrence_bug_repro.rs
use cfait::model::{DateType, Task, TaskStatus};
use chrono::NaiveDate;
use std::collections::HashMap;

fn create_weekly_task(uid: &str, start_ymd: (i32, u32, u32)) -> Task {
    let aliases = HashMap::new();
    let mut t = Task::new("Rec Test Task", &aliases, None);
    t.uid = uid.to_string();

    // Set specific future dates (e.g., 2026) to avoid "now" interference
    let dt = NaiveDate::from_ymd_opt(start_ymd.0, start_ymd.1, start_ymd.2).unwrap();
    t.dtstart = Some(DateType::AllDay(dt));
    t.due = Some(DateType::AllDay(dt));

    // Explicitly set weekly recurrence
    t.rrule = Some("FREQ=WEEKLY".to_string());
    t.status = TaskStatus::NeedsAction;
    t
}

#[test]
fn test_scenario_1_cancel_then_done_sequence() {
    // create "rec test task @weekly" starting 2026-01-20
    let mut t = create_weekly_task("task1", (2026, 1, 20));

    // 1. Mark as done -> Should move to Jan 27
    t.status = TaskStatus::Completed;
    assert!(t.advance_recurrence(), "Should advance from Jan 20");
    let due = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due, NaiveDate::from_ymd_opt(2026, 1, 27).unwrap());
    assert_eq!(t.status, TaskStatus::NeedsAction);

    // 2. Mark as done -> Should move to Feb 03
    t.status = TaskStatus::Completed;
    assert!(t.advance_recurrence(), "Should advance from Jan 27");
    let due = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due, NaiveDate::from_ymd_opt(2026, 2, 3).unwrap());

    // 3. Mark as canceled -> Should move to Feb 10, adding Feb 03 to exceptions
    t.status = TaskStatus::Cancelled;
    assert!(
        t.advance_recurrence_with_cancellation(),
        "Should cancel Feb 03"
    );
    let due = t.due.as_ref().unwrap().to_date_naive();
    assert_eq!(due, NaiveDate::from_ymd_opt(2026, 2, 10).unwrap());
    assert!(!t.exdates.is_empty(), "Should have exdates");

    // Verify Feb 03 is in exdates
    let has_exdate = t
        .exdates
        .iter()
        .any(|d| d.to_date_naive() == NaiveDate::from_ymd_opt(2026, 2, 3).unwrap());
    assert!(has_exdate, "Feb 03 should be excluded");

    // 4. Mark as canceled -> Should move to Feb 17 (This is where it was getting stuck on Feb 10)
    t.status = TaskStatus::Cancelled;
    assert!(
        t.advance_recurrence_with_cancellation(),
        "Should cancel Feb 10"
    );

    let due = t.due.as_ref().unwrap().to_date_naive();

    // BUG REPRO: If this fails, it means it stayed at Feb 10
    assert_eq!(
        due,
        NaiveDate::from_ymd_opt(2026, 2, 17).unwrap(),
        "Should have advanced to Feb 17"
    );
}

#[test]
fn test_scenario_2_multiple_cancels() {
    // create "rec test task2 @weekly" starting 2026-01-20
    let mut t = create_weekly_task("task2", (2026, 1, 20));

    // 1. Cancel Jan 20 -> Moves to Jan 27
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 1, 27).unwrap()
    );

    // 2. Cancel Jan 27 -> Moves to Feb 03
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();

    // BUG REPRO: Should allow consecutive cancellations
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 2, 3).unwrap()
    );

    // 3. Cancel Feb 03 -> Moves to Feb 10
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 2, 10).unwrap()
    );

    // Verify accumulated exdates
    assert_eq!(t.exdates.len(), 3);
}

#[test]
fn test_scenario_3_cancel_loop() {
    // create "rec test task3 @weekly" starting 2026-01-20
    let mut t = create_weekly_task("task3", (2026, 1, 20));

    // Done -> Jan 27
    t.status = TaskStatus::Completed;
    t.advance_recurrence();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 1, 27).unwrap()
    );

    // Cancel -> Feb 03
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 2, 3).unwrap()
    );

    // Done -> Feb 10
    t.status = TaskStatus::Completed;
    t.advance_recurrence();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 2, 10).unwrap()
    );

    // Cancel -> Feb 17 (Bug was sticking at Feb 10)
    t.status = TaskStatus::Cancelled;
    t.advance_recurrence_with_cancellation();
    assert_eq!(
        t.due.as_ref().unwrap().to_date_naive(),
        NaiveDate::from_ymd_opt(2026, 2, 17).unwrap()
    );
}
