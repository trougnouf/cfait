use cfait::model::item::Task;
use chrono::{Datelike, Timelike};
use std::collections::HashMap;

#[test]
fn test_time_only_alarm_binds_to_task_date() {
    let task = Task::new("Pay rent @2026-04-01 rem:10am", &HashMap::new(), None);

    // Should have one alarm
    assert_eq!(task.alarms.len(), 1);

    // The alarm should be absolute and on 2026-04-01 at 10:00
    match &task.alarms[0].trigger {
        cfait::model::item::AlarmTrigger::Absolute(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            assert_eq!(local.year(), 2026);
            assert_eq!(local.month(), 4);
            assert_eq!(local.day(), 1);
            assert_eq!(local.hour(), 10);
            assert_eq!(local.minute(), 0);
        }
        _ => panic!("Expected absolute alarm"),
    }
}

#[test]
fn test_time_only_alarm_without_task_date_uses_today_tomorrow() {
    let task = Task::new("Some task rem:10am", &HashMap::new(), None);

    // Should have one alarm
    assert_eq!(task.alarms.len(), 1);

    // The alarm should be absolute and on today or tomorrow at 10:00
    match &task.alarms[0].trigger {
        cfait::model::item::AlarmTrigger::Absolute(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            assert_eq!(local.hour(), 10);
            assert_eq!(local.minute(), 0);

            // Should be either today or tomorrow
            let now = chrono::Local::now();
            let alarm_date = local.date_naive();
            let today = now.date_naive();
            let tomorrow = today + chrono::Duration::days(1);

            assert!(alarm_date == today || alarm_date == tomorrow);
        }
        _ => panic!("Expected absolute alarm"),
    }
}

#[test]
fn test_recurrence_advances_absolute_alarms() {
    use cfait::model::recurrence::RecurrenceEngine;

    let task = Task::new(
        "Pay rent @2026-04-01 @monthly rem:10am",
        &HashMap::new(),
        None,
    );

    // Get the original alarm time
    let original_alarm = match &task.alarms[0].trigger {
        cfait::model::item::AlarmTrigger::Absolute(dt) => *dt,
        _ => panic!("Expected absolute alarm"),
    };

    // Advance to next occurrence
    let next_task = RecurrenceEngine::next_occurrence(&task);
    assert!(next_task.is_some());
    let next_task = next_task.unwrap();

    // Should still have one alarm
    assert_eq!(next_task.alarms.len(), 1);

    // The alarm should be advanced by the same amount as the task
    match &next_task.alarms[0].trigger {
        cfait::model::item::AlarmTrigger::Absolute(dt) => {
            let original_local = original_alarm.with_timezone(&chrono::Local);
            let new_local = dt.with_timezone(&chrono::Local);

            // Should be 10:00 on the new date
            assert_eq!(new_local.hour(), 10);
            assert_eq!(new_local.minute(), 0);

            // Should be approximately 1 month later (could be 28-31 days depending on month)
            let days_diff = (new_local.date_naive() - original_local.date_naive()).num_days();
            assert!((28..=31).contains(&days_diff));
        }
        _ => panic!("Expected absolute alarm"),
    }
}

#[test]
fn test_display_formatting_hides_date_when_same_as_task() {
    let task = Task::new("Pay rent @2026-04-01 rem:10am", &HashMap::new(), None);
    let smart_string = task.to_smart_string();

    // Should display as "rem:10:00" not "rem:2026-04-01 10:00"
    assert!(smart_string.contains("rem:10:00"));
    assert!(!smart_string.contains("rem:2026-04-01"));
}
