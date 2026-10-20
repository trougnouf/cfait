// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for alarm snoozing and dismissal logic.
use cfait::model::{AlarmTrigger, DateType, Task};
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

/// The shared snooze-bar parser used by all three clients (TUI, GUI, mobile).
fn snooze_target(val: &str, now: chrono::DateTime<chrono::Local>) -> Option<u32> {
    let lex = cfait::model::parser::LEXICON.read().unwrap();
    let default_time = chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap();
    cfait::model::parser::parse_snooze_target_with_lex(val, &lex, now, default_time)
}

/// A fixed local "now" (2026-10-20 09:00), far from any DST transition.
fn fixed_now() -> chrono::DateTime<chrono::Local> {
    chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
        chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
    )
    .and_local_timezone(chrono::Local)
    .earliest()
    .unwrap()
}

#[test]
fn test_snooze_target_durations() {
    let now = fixed_now();
    assert_eq!(snooze_target("1m", now), Some(1));
    assert_eq!(snooze_target("15", now), Some(15)); // plain number = minutes
    assert_eq!(snooze_target("2h", now), Some(120));
    assert_eq!(snooze_target("16h", now), Some(16 * 60));
    assert_eq!(snooze_target("16h20m", now), Some(16 * 60 + 20));
    assert_eq!(snooze_target("1h 30m", now), Some(90));
    assert_eq!(snooze_target("1d", now), Some(1440));
    assert_eq!(snooze_target("", now), None);
    assert_eq!(snooze_target("2h20", now), None); // bare trailing minutes
    assert_eq!(snooze_target("banana", now), None);
}

#[test]
fn test_snooze_target_clock_time() {
    let now = fixed_now(); // 09:00 local

    // Still ahead today -> minutes until it strikes.
    assert_eq!(snooze_target("16:20", now), Some(7 * 60 + 20));
    assert_eq!(snooze_target("4:20pm", now), Some(7 * 60 + 20));
    assert_eq!(snooze_target(" 4:20 PM ", now), Some(7 * 60 + 20));

    // Already past today -> tomorrow.
    assert_eq!(snooze_target("08:00", now), Some(23 * 60));
    assert_eq!(snooze_target("12:15am", now), Some(15 * 60 + 15));

    // 12-hour edges.
    assert_eq!(snooze_target("12:30pm", now), Some(3 * 60 + 30));
    assert_eq!(snooze_target("12:00am", now), Some(15 * 60));

    // Invalid clock times.
    assert_eq!(snooze_target("24:00", now), None);
    assert_eq!(snooze_target("13:00pm", now), None);
    assert_eq!(snooze_target("16:60", now), None);
    assert_eq!(snooze_target("16:", now), None);
}

#[test]
fn test_snooze_target_dates() {
    let now = fixed_now(); // 09:00 local, default reminder time 08:00
    assert_eq!(snooze_target("tomorrow", now), Some(23 * 60));
    assert_eq!(snooze_target("yesterday", now), None); // already past
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
