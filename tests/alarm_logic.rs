// Tests for alarm snoozing and dismissal logic.
use cfait::model::{Alarm, AlarmTrigger, DateType, Task};
use chrono::{Duration, Utc};
use std::collections::HashMap;

#[test]
fn test_snooze_creates_new_alarm() {
    let mut t = Task::new("Test Task rem:14:42", &HashMap::new(), None);

    // Simulate parsing creating an absolute alarm (like the GUI does)
    // We manually inject it to ensure environment consistency
    let now = Utc::now();
    let trigger_time = now - Duration::minutes(1); // Fired 1 min ago
    t.alarms.clear();
    t.alarms
        .push(cfait::model::Alarm::new_absolute(trigger_time));

    let original_uid = t.alarms[0].uid.clone();

    // Snooze for 1 minute
    let success = t.snooze_alarm(&original_uid, 1);

    assert!(success, "Snooze should succeed");
    assert_eq!(
        t.alarms.len(),
        2,
        "Should have original (ack) and snooze (active)"
    );

    let snooze_alarm = t.alarms.iter().find(|a| a.uid != original_uid).unwrap();

    // Verify relation
    assert_eq!(snooze_alarm.related_to_uid, Some(original_uid));
    assert!(snooze_alarm.is_snooze());

    // Verify trigger time (approx now + 1m)
    match snooze_alarm.trigger {
        AlarmTrigger::Absolute(dt) => {
            let diff = dt - now;
            // Should be roughly 60 seconds (allow slight execution delay)
            assert!(diff.num_seconds() >= 59 && diff.num_seconds() <= 61);
        }
        _ => panic!("Snooze should be absolute"),
    }
}

#[test]
fn test_custom_snooze_string_parsing() {
    // Mimic the GUI input parsing logic found in src/gui/update/tasks.rs
    let input_1m = "1m";
    let parsed_1m = cfait::model::parser::parse_duration(input_1m);
    assert_eq!(parsed_1m, Some(1));

    let input_10 = "10"; // Plain number
    // GUI uses: val.parse::<u32>().ok()... else parse_duration
    let val_10 = input_10.parse::<u32>().ok();
    assert_eq!(val_10, Some(10));

    let input_2h = "2h";
    let parsed_2h = cfait::model::parser::parse_duration(input_2h);
    assert_eq!(parsed_2h, Some(120));
}

#[test]
fn test_implicit_alarm_dismissal_creates_entry() {
    let mut t = Task::new("Auto Reminder", &HashMap::new(), None);
    let due = Utc::now();
    t.due = Some(DateType::Specific(due));

    // Task has 0 explicit alarms
    assert!(t.alarms.is_empty());

    // System actor creates synthetic alarm, user dismisses it
    t.dismiss_implicit_alarm(due, "Due Now".to_string());

    // Should now have 1 real alarm that is acknowledged
    assert_eq!(t.alarms.len(), 1);
    assert!(t.alarms[0].acknowledged.is_some());

    // Ensure `has_alarm_at` returns true so system doesn't refire
    assert!(t.has_alarm_at(due));
}
