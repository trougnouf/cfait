// File: tests/alarm_tests.rs
use cfait::model::{AlarmTrigger, DateType, Task};
use chrono::{Duration, Local, Timelike};
use std::collections::HashMap;

fn mock_aliases() -> HashMap<String, Vec<String>> {
    HashMap::new()
}

#[test]
fn test_time_parsing_specific() {
    let t = Task::new("Meeting @14:00", &mock_aliases(), None);
    match t.due {
        Some(DateType::Specific(dt)) => {
            let local = dt.with_timezone(&Local);
            // Now .hour() and .minute() are available via Timelike trait
            assert_eq!(local.hour(), 14);
            assert_eq!(local.minute(), 00);
        }
        _ => panic!("Expected Specific time"),
    }
}

#[test]
fn test_time_parsing_merge() {
    // "@tomorrow 2pm" -> Should be merged into one Specific DateType
    let t = Task::new("Meeting @tomorrow 2pm", &mock_aliases(), None);

    match t.due {
        Some(DateType::Specific(dt)) => {
            let now = Local::now();
            let tomorrow = now.date_naive() + Duration::days(1);
            assert_eq!(dt.with_timezone(&Local).date_naive(), tomorrow);
            assert_eq!(dt.with_timezone(&Local).hour(), 14);
        }
        _ => panic!("Expected merged Specific time"),
    }
}

#[test]
fn test_reminder_relative_anchor() {
    // Anchor to Due
    let t = Task::new("Deadline @15:00 rem:30m", &mock_aliases(), None);
    assert_eq!(t.alarms.len(), 1);

    let alarm = &t.alarms[0];
    match alarm.trigger {
        AlarmTrigger::Relative(mins) => assert_eq!(mins, -30),
        _ => panic!("Expected relative trigger"),
    }

    // Verify next_trigger_timestamp calculation
    let trigger_ts = t.next_trigger_timestamp().expect("Should have trigger");

    let due_dt = match t.due.unwrap() {
        DateType::Specific(d) => d,
        _ => panic!(),
    };
    let expected = due_dt - Duration::minutes(30);
    assert_eq!(trigger_ts, expected.timestamp());
}

#[test]
fn test_reminder_absolute() {
    let t = Task::new("Meds rem:8am", &mock_aliases(), None);
    assert_eq!(t.alarms.len(), 1);

    match t.alarms[0].trigger {
        AlarmTrigger::Absolute(dt) => {
            let local = dt.with_timezone(&Local);
            assert_eq!(local.hour(), 8);
            assert_eq!(local.date_naive(), Local::now().date_naive());
        }
        _ => panic!("Expected absolute trigger"),
    }
}

#[test]
fn test_reminder_no_anchor_ignored() {
    // Relative reminder with NO specific time -> Should effectively be ignored or not calc'd
    let t = Task::new("Vague Task @tomorrow rem:10m", &mock_aliases(), None);

    // It is parsed into the list...
    assert_eq!(t.alarms.len(), 1);

    // ...but next_trigger_timestamp should ignore it because anchor is missing/AllDay
    let ts = t.next_trigger_timestamp();
    assert!(
        ts.is_none(),
        "Should not trigger relative alarm on AllDay task"
    );
}

#[test]
fn test_snooze_logic_rfc9074() {
    let mut t = Task::new("Wake up rem:8am", &mock_aliases(), None);
    let original_uid = t.alarms[0].uid.clone();

    // 1. Snooze
    let success = t.snooze_alarm(&original_uid, 10);
    assert!(success);

    // Should now have 2 alarms
    assert_eq!(t.alarms.len(), 2);

    // Verify Parent
    let parent = t.alarms.iter().find(|a| a.uid == original_uid).unwrap();
    assert!(parent.acknowledged.is_some());

    // Verify Child
    let child = t.alarms.iter().find(|a| a.uid != original_uid).unwrap();
    assert!(child.acknowledged.is_none());
    assert_eq!(child.related_to_uid, Some(original_uid.clone()));
    assert_eq!(child.relation_type, Some("SNOOZE".to_string()));

    match child.trigger {
        AlarmTrigger::Absolute(_) => {} // Correct
        _ => panic!("Snooze should set absolute time"),
    }
}

#[test]
fn test_snooze_chain_cleanup() {
    let mut t = Task::new("Wake up rem:8am", &mock_aliases(), None);
    // Original alarm (rem:8am)
    let original_uid = t.alarms[0].uid.clone();

    // Snooze 1 (10 mins)
    t.snooze_alarm(&original_uid, 10);

    // Find Snooze 1
    let snooze1_uid = t
        .alarms
        .iter()
        .find(|a| a.uid != original_uid)
        .expect("Snooze 1 not found")
        .uid
        .clone();

    assert_eq!(t.alarms.len(), 2);

    // Snooze 2 (Snoozing the snooze 1 by 5 mins)
    // This should delete snooze 1 and create snooze 2
    let success = t.snooze_alarm(&snooze1_uid, 5);
    assert!(success, "Snooze 2 failed");

    // Should still have 2 alarms (Original + Snooze2). Snooze1 should be deleted.
    assert_eq!(t.alarms.len(), 2);

    let original = t
        .alarms
        .iter()
        .find(|a| a.uid == original_uid)
        .expect("Original missing");
    assert!(original.acknowledged.is_some()); // Original stays ack'd

    // Find the new snooze (it is neither original nor snooze1)
    let snooze2 = t
        .alarms
        .iter()
        .find(|a| a.uid != original_uid && a.uid != snooze1_uid)
        .expect("Snooze 2 missing");

    // CRITICAL ASSERTION: Snooze 2 must point to ORIGINAL, not SNOOZE 1 (which is gone)
    assert_eq!(snooze2.related_to_uid, Some(original_uid));
}

#[test]
fn test_ics_roundtrip_alarms() {
    // Create task with alarm
    let t_in = Task::new("Ping @14:00 rem:15m", &mock_aliases(), None);
    let ics = t_in.to_ics();

    // Validate ICS string contains VALARM
    assert!(ics.contains("BEGIN:VALARM"));
    assert!(ics.contains("TRIGGER:-PT15M"));

    // Parse back
    let t_out = Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).unwrap();

    assert_eq!(t_out.alarms.len(), 1);
    match t_out.alarms[0].trigger {
        AlarmTrigger::Relative(mins) => assert_eq!(mins, -15),
        _ => panic!("Failed roundtrip"),
    }
}
