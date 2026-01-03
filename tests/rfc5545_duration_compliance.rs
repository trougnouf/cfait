// File: tests/rfc5545_duration_compliance.rs
//
// Tests to ensure RFC 5545 compliance: VTODO components cannot contain both
// DUE and DURATION properties simultaneously.
//
// Background: RFC 5545 section 3.6.2 states that a VTODO can have either a
// DUE or DURATION property, but not both. When we have an estimated_duration
// field and a due date, we must serialize estimated_duration as a custom
// property (X-ESTIMATED-DURATION) to avoid the conflict.

use cfait::model::{DateType, Task};
use chrono::{NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

fn parse(input: &str) -> Task {
    let aliases = HashMap::new();
    Task::new(input, &aliases, None)
}

// ============================================================================
// SERIALIZATION TESTS: Ensure correct property usage
// ============================================================================

#[test]
fn test_task_with_due_and_duration_uses_x_estimated() {
    let mut task = parse("Meeting");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    task.estimated_duration = Some(60);

    let ics = task.to_ics();

    // Should contain DUE
    assert!(ics.contains("DUE:"), "ICS should contain DUE property");

    // Should contain X-ESTIMATED-DURATION, not DURATION
    assert!(
        ics.contains("X-ESTIMATED-DURATION:PT60M"),
        "ICS should contain X-ESTIMATED-DURATION when DUE is present. Got:\n{}",
        ics
    );

    // Should NOT contain standard DURATION property (check for line break to avoid matching X-ESTIMATED-DURATION)
    assert!(
        !ics.contains("\nDURATION:"),
        "ICS should NOT contain DURATION when DUE is present (RFC 5545 violation). Got:\n{}",
        ics
    );
}

#[test]
fn test_task_with_only_duration_uses_standard_property() {
    let mut task = parse("Task");
    task.estimated_duration = Some(120);
    // No due date set

    let ics = task.to_ics();

    // Should NOT contain DUE
    assert!(!ics.contains("DUE:"), "ICS should not contain DUE property");

    // Should contain standard DURATION (not custom property)
    assert!(
        ics.contains("DURATION:PT120M"),
        "ICS should contain standard DURATION when no DUE is present. Got:\n{}",
        ics
    );

    // Should NOT contain X-ESTIMATED-DURATION
    assert!(
        !ics.contains("X-ESTIMATED-DURATION:"),
        "ICS should not contain X-ESTIMATED-DURATION when DUE is absent. Got:\n{}",
        ics
    );
}

#[test]
fn test_task_with_allday_due_and_duration_uses_x_estimated() {
    let mut task = parse("Event");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
    ));
    task.estimated_duration = Some(90);

    let ics = task.to_ics();

    // Should contain DUE with VALUE=DATE
    assert!(
        ics.contains("DUE;VALUE=DATE:20250310"),
        "ICS should contain all-day DUE property. Got:\n{}",
        ics
    );

    // Should use X-ESTIMATED-DURATION
    assert!(
        ics.contains("X-ESTIMATED-DURATION:PT90M"),
        "ICS should contain X-ESTIMATED-DURATION with all-day DUE. Got:\n{}",
        ics
    );

    // Should NOT contain DURATION (check for line break to avoid matching X-ESTIMATED-DURATION)
    assert!(
        !ics.contains("\nDURATION:"),
        "ICS should NOT contain DURATION with all-day DUE. Got:\n{}",
        ics
    );
}

#[test]
fn test_task_without_duration_only_due() {
    let mut task = parse("Task");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 4, 1, 10, 30, 0).unwrap(),
    ));
    // No estimated_duration

    let ics = task.to_ics();

    // Should contain DUE
    assert!(ics.contains("DUE:"), "ICS should contain DUE");

    // Should NOT contain any DURATION or X-ESTIMATED-DURATION
    assert!(
        !ics.contains("\nDURATION:"),
        "ICS should not contain DURATION when no estimated_duration is set"
    );
    assert!(
        !ics.contains("\nX-ESTIMATED-DURATION:"),
        "ICS should not contain X-ESTIMATED-DURATION when no estimated_duration is set"
    );
}

// ============================================================================
// DESERIALIZATION TESTS: Ensure backward compatibility
// ============================================================================

#[test]
fn test_parse_ics_with_x_estimated_duration() {
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//cfait//cfait//EN
BEGIN:VTODO
UID:test-123
SUMMARY:Task with custom duration
DUE:20250215T140000Z
X-ESTIMATED-DURATION:PT45M
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag123".to_string(),
        "href123".to_string(),
        "cal123".to_string(),
    )
    .expect("Failed to parse ICS");

    assert_eq!(task.summary, "Task with custom duration");
    assert!(task.due.is_some(), "Should have DUE date");
    assert_eq!(
        task.estimated_duration,
        Some(45),
        "Should parse X-ESTIMATED-DURATION"
    );
}

#[test]
fn test_parse_ics_with_standard_duration() {
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//cfait//cfait//EN
BEGIN:VTODO
UID:test-456
SUMMARY:Task with standard duration
DURATION:PT2H
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag456".to_string(),
        "href456".to_string(),
        "cal456".to_string(),
    )
    .expect("Failed to parse ICS");

    assert_eq!(task.summary, "Task with standard duration");
    assert!(task.due.is_none(), "Should not have DUE date");
    assert_eq!(
        task.estimated_duration,
        Some(120),
        "Should parse standard DURATION as minutes"
    );
}

#[test]
fn test_parse_ics_prefers_x_estimated_over_duration() {
    // If both are present (shouldn't happen, but test parser priority)
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//cfait//cfait//EN
BEGIN:VTODO
UID:test-789
SUMMARY:Task with both properties
DUE:20250301T120000Z
DURATION:PT1H
X-ESTIMATED-DURATION:PT30M
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag789".to_string(),
        "href789".to_string(),
        "cal789".to_string(),
    )
    .expect("Failed to parse ICS");

    // Parser should prefer X-ESTIMATED-DURATION
    assert_eq!(
        task.estimated_duration,
        Some(30),
        "Should prefer X-ESTIMATED-DURATION over DURATION"
    );
}

// ============================================================================
// ROUNDTRIP TESTS: Ensure data integrity through serialize/deserialize cycles
// ============================================================================

#[test]
fn test_roundtrip_task_with_due_and_duration() {
    let mut original = parse("Project task");
    original.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 5, 20, 16, 0, 0).unwrap(),
    ));
    original.estimated_duration = Some(180); // 3 hours

    // Serialize
    let ics = original.to_ics();

    // Verify RFC compliance in serialized form
    assert!(ics.contains("DUE:"));
    assert!(ics.contains("X-ESTIMATED-DURATION:PT180M"));
    assert!(!ics.contains("\nDURATION:"));

    // Deserialize
    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .expect("Failed to parse roundtrip ICS");

    // Verify data integrity
    assert_eq!(restored.summary, original.summary);
    assert!(restored.due.is_some());
    assert_eq!(restored.estimated_duration, Some(180));
}

#[test]
fn test_roundtrip_task_with_only_duration() {
    let mut original = parse("Background task");
    original.estimated_duration = Some(240); // 4 hours
    // No due date

    // Serialize
    let ics = original.to_ics();

    // Should use standard DURATION
    assert!(!ics.contains("DUE:"));
    assert!(ics.contains("\nDURATION:PT240M"));
    assert!(!ics.contains("\nX-ESTIMATED-DURATION:"));

    // Deserialize
    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .expect("Failed to parse roundtrip ICS");

    // Verify data integrity
    assert_eq!(restored.summary, original.summary);
    assert!(restored.due.is_none());
    assert_eq!(restored.estimated_duration, Some(240));
}

#[test]
fn test_roundtrip_preserves_property_choice() {
    // Test 1: Task with due -> X-ESTIMATED-DURATION
    let mut task1 = parse("Task 1");
    task1.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
    ));
    task1.estimated_duration = Some(60);

    let ics1 = task1.to_ics();
    let restored1 =
        Task::from_ics(&ics1, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();

    assert_eq!(restored1.estimated_duration, Some(60));

    // Test 2: Task without due -> DURATION
    let mut task2 = parse("Task 2");
    task2.estimated_duration = Some(90);

    let ics2 = task2.to_ics();
    let restored2 =
        Task::from_ics(&ics2, "e2".to_string(), "h2".to_string(), "c2".to_string()).unwrap();

    assert_eq!(restored2.estimated_duration, Some(90));
}

// ============================================================================
// REGRESSION TEST: The actual bug from the issue
// ============================================================================

#[test]
fn test_regression_radicale_serialization_error() {
    // This test reproduces the exact error scenario from the issue:
    // "VTODO components cannot contain both DUE and DURATION components"

    let mut task = parse("Task that caused radicale error");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 1, 3, 12, 0, 0).unwrap(),
    ));
    task.estimated_duration = Some(30);

    let ics = task.to_ics();

    // Critical: Verify we don't violate RFC 5545
    let has_due = ics.contains("DUE:");
    let has_x_estimated = ics.contains("X-ESTIMATED-DURATION:");

    // Re-check with proper pattern matching to avoid false positives
    let has_duration_line = ics.contains("\nDURATION:");

    assert!(has_due, "Task should have DUE property");
    assert!(has_x_estimated, "Task should have X-ESTIMATED-DURATION");
    assert!(
        !has_duration_line,
        "Task MUST NOT have DURATION when DUE is present (RFC 5545 violation)"
    );

    // Verify radicale would accept this by checking the ICS is parseable
    let parsed = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    );
    assert!(parsed.is_ok(), "ICS should be parseable without errors");
}
