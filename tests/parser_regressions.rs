// Regression tests for input parsing bugs.
use cfait::model::{AlarmTrigger, Task};
use chrono::{Local, Timelike};
use std::collections::HashMap;

#[test]
fn test_mixed_text_and_reminder_syntax() {
    // Input: "rem today rem:today 15:02"
    // Expectation:
    //   Summary: "rem today"
    //   Alarm: Absolute at 15:02 today

    let aliases = HashMap::new();
    let t = Task::new("rem today rem:today 15:02", &aliases, None);

    assert_eq!(
        t.summary, "rem today",
        "Bare 'rem' and 'today' should be text, not escaped"
    );

    assert_eq!(t.alarms.len(), 1, "Should have 1 alarm");
    match t.alarms[0].trigger {
        AlarmTrigger::Absolute(dt) => {
            let local = dt.with_timezone(&Local);
            assert_eq!(local.hour(), 15);
            assert_eq!(local.minute(), 2);
            assert_eq!(local.date_naive(), Local::now().date_naive());
        }
        _ => panic!("Expected absolute trigger"),
    }
}

#[test]
fn test_bare_keywords_not_escaped() {
    let aliases = HashMap::new();
    let t = Task::new("Meeting today or tomorrow", &aliases, None);

    // Should NOT be "Meeting \today or \tomorrow"
    assert_eq!(t.summary, "Meeting today or tomorrow");
    assert!(t.due.is_none());

    // But prefixed should work
    let t2 = Task::new("Meeting @today", &aliases, None);
    assert_eq!(t2.summary, "Meeting");
    assert!(t2.due.is_some());
}

#[test]
fn test_reminder_date_and_time_roundtrip() {
    use chrono::Duration;

    let aliases = HashMap::new();

    // 1. Parse "rem:tomorrow 16:00"
    let t = Task::new("test rem:tomorrow 16:00", &aliases, None);

    assert_eq!(t.alarms.len(), 1);
    match t.alarms[0].trigger {
        AlarmTrigger::Absolute(dt) => {
            let local = dt.with_timezone(&Local);
            let tomorrow = Local::now().date_naive() + Duration::days(1);

            assert_eq!(local.date_naive(), tomorrow, "Date should be tomorrow");
            assert_eq!(local.hour(), 16, "Time should be 16:00");
        }
        _ => panic!("Expected absolute trigger"),
    }

    // 2. Check Smart String reconstruction (The Bug Fix)
    let smart = t.to_smart_string();
    // Should reconstruct as "test rem:tomorrow 16:00" (preserving the keyword)
    assert!(
        smart.contains("rem:tomorrow 16:00"),
        "Should preserve 'tomorrow' keyword in roundtrip. Got: {}",
        smart
    );
}

#[test]
fn test_rem_in_syntax() {
    let aliases = HashMap::new();

    // Test "rem:in 5m"
    let t = Task::new("Pizza rem:in 5m", &aliases, None);

    assert_eq!(t.summary, "Pizza");
    assert_eq!(t.alarms.len(), 1);

    match t.alarms[0].trigger {
        AlarmTrigger::Absolute(dt) => {
            let now = Local::now();
            let diff = dt.with_timezone(&Local) - now;
            // Allow small execution delta (e.g. 4m 59s)
            assert!(
                diff.num_seconds() > 290 && diff.num_seconds() <= 300,
                "Should be ~5 mins from now"
            );
        }
        _ => panic!("rem:in should create Absolute alarm"),
    }

    // Test "rem: in 1 hour" (with spaces)
    let t2 = Task::new("Long task rem: in 1 hour", &aliases, None);
    assert_eq!(t2.alarms.len(), 1);
    match t2.alarms[0].trigger {
        AlarmTrigger::Absolute(dt) => {
            let now = Local::now();
            let diff = dt.with_timezone(&Local) - now;
            assert!(diff.num_minutes() >= 59 && diff.num_minutes() <= 60);
        }
        _ => panic!(),
    }
}
