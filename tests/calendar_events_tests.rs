// File: tests/calendar_events_tests.rs
use cfait::model::{DateType, Task, TaskStatus};
use chrono::{NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

// Helper to create a task with smart input
fn parse(input: &str) -> Task {
    let aliases = HashMap::new();
    Task::new(input, &aliases, None)
}

#[test]
fn test_event_generation_both_dates_short_span() {
    let mut task = parse("Conference");
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 16).unwrap(),
    ));

    let result = task.to_event_ics();
    assert_eq!(
        result.len(),
        1,
        "Consecutive all-day dates should merge into one multi-day event"
    );

    let (suffix, ics) = &result[0];
    assert_eq!(suffix, "");
    assert!(ics.contains("DTSTART;VALUE=DATE:20250215"));
    assert!(ics.contains("DTEND;VALUE=DATE:20250217")); // Exclusive end (+1 day)
}

#[test]
fn test_event_generation_both_dates_long_span_splits() {
    let mut task = parse("Long gap");
    // Start: Jan 1
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ));
    // Due: Feb 15
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert_eq!(
        result.len(),
        2,
        "Dates >1 day apart should split into -start and -due events"
    );

    let start_event = result.iter().find(|(s, _)| s == "-start").unwrap();
    assert!(start_event.1.contains("DTSTART;VALUE=DATE:20250101"));

    let due_event = result.iter().find(|(s, _)| s == "-due").unwrap();
    assert!(due_event.1.contains("DTSTART;VALUE=DATE:20250215"));
}

#[test]
fn test_event_generation_with_duration() {
    let mut task = parse("Meeting ~30m");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    task.estimated_duration = Some(30);

    let result = task.to_event_ics();
    assert!(!result.is_empty());

    let (suffix, ics) = result.first().unwrap();
    assert_eq!(suffix, "");
    // Should calculate start time as 30 minutes before due
    assert!(ics.contains("DTSTART"));
    assert!(ics.contains("DTEND"));
}

#[test]
fn test_event_long_span_with_specific_times() {
    let mut task = parse("Timed long project");
    // Start: Jan 1, 2025 at 9:00 AM
    task.dtstart = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
    ));
    // Due: Feb 15, 2025 at 5:00 PM
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 17, 0, 0).unwrap(),
    ));

    let result = task.to_event_ics();
    assert_eq!(result.len(), 2, "Specific times > 24h apart should split");

    // 1. Check Start Event (Jan 1, 09:00 to 10:00 (default 1h))
    let start_event = result.iter().find(|(s, _)| s == "-start").unwrap();
    assert!(start_event.1.contains("SUMMARY:Timed long project (start)"));
    assert!(start_event.1.contains("DTSTART:20250101T090000Z"));
    assert!(start_event.1.contains("DTEND:20250101T100000Z"));

    // 2. Check Due Event (Feb 15, ending at 17:00. Start = 17:00 - 60m = 16:00)
    let due_event = result.iter().find(|(s, _)| s == "-due").unwrap();
    assert!(due_event.1.contains("SUMMARY:Timed long project (due)"));
    assert!(due_event.1.contains("DTSTART:20250215T160000Z"));
    assert!(due_event.1.contains("DTEND:20250215T170000Z"));
}

#[test]
fn test_event_generation_cancelled_with_sessions() {
    let mut task = parse("Cancelled project");
    task.status = TaskStatus::Cancelled;
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.sessions.push(cfait::model::item::WorkSession {
        start: 1735689600, // 2025-01-01 00:00:00 UTC
        end: 1735693200,   // 2025-01-01 01:00:00 UTC
    });

    let result = task.to_event_ics();
    assert!(
        !result.is_empty(),
        "Cancelled task with sessions should generate an event"
    );
    assert_eq!(
        result.len(),
        2,
        "Should have 2 VEVENTs (session + cancelled plan)"
    );

    let session_event = result.iter().find(|(s, _)| s == "-session-0").unwrap();
    assert!(session_event.1.contains("SUMMARY:⚙ Cancelled project"));

    let main_event = result.iter().find(|(s, _)| s.is_empty()).unwrap();
    assert!(main_event.1.contains("SUMMARY:Cancelled project"));
    assert!(main_event.1.contains("STATUS:CANCELLED"));
}
