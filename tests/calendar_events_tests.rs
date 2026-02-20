// Tests for calendar event generation logic.
use cfait::model::{DateType, Task, TaskStatus};
use chrono::{NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

// Helper to create a task with smart input
fn parse(input: &str) -> Task {
    let aliases = HashMap::new();
    Task::new(input, &aliases, None)
}

// ============================================================================
// PARSER TESTS: +cal and -cal syntax
// ============================================================================

#[test]
fn test_parse_plus_cal() {
    let task = parse("Meeting @tomorrow +cal");
    assert_eq!(task.summary, "Meeting");
    assert_eq!(task.create_event, Some(true));
}

#[test]
fn test_parse_minus_cal() {
    let task = parse("Note @tomorrow -cal");
    assert_eq!(task.summary, "Note");
    assert_eq!(task.create_event, Some(false));
}

#[test]
fn test_parse_no_cal_modifier() {
    let task = parse("Task @tomorrow");
    assert_eq!(task.summary, "Task");
    assert_eq!(task.create_event, None);
}

#[test]
fn test_parse_cal_with_other_properties() {
    let task = parse("Meeting @tomorrow 2pm ~1h @@office #work +cal");
    assert_eq!(task.summary, "Meeting");
    assert_eq!(task.create_event, Some(true));
    assert_eq!(task.estimated_duration, Some(60));
    assert_eq!(task.location, Some("office".to_string()));
    assert!(task.categories.contains(&"work".to_string()));
}

// ============================================================================
// TO_SMART_STRING TESTS: Output +cal/-cal
// ============================================================================

#[test]
fn test_to_smart_string_with_plus_cal() {
    let mut task = parse("Meeting @tomorrow");
    task.create_event = Some(true);
    let output = task.to_smart_string();
    assert!(
        output.contains("+cal"),
        "Output should contain +cal: {}",
        output
    );
}

#[test]
fn test_to_smart_string_with_minus_cal() {
    let mut task = parse("Note @tomorrow");
    task.create_event = Some(false);
    let output = task.to_smart_string();
    assert!(
        output.contains("-cal"),
        "Output should contain -cal: {}",
        output
    );
}

#[test]
fn test_to_smart_string_without_cal_modifier() {
    let task = parse("Task @tomorrow");
    let output = task.to_smart_string();
    assert!(
        !output.contains("+cal") && !output.contains("-cal"),
        "Output should not contain cal modifiers: {}",
        output
    );
}

// ============================================================================
// EVENT GENERATION TESTS: to_event_ics()
// ============================================================================

#[test]
fn test_event_generation_no_dates_returns_none() {
    let task = parse("Task without dates");
    let result = task.to_event_ics();
    assert!(
        result.is_none(),
        "Task without dates should not generate event"
    );
}

#[test]
fn test_event_generation_with_due_date() {
    let mut task = parse("Buy milk");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some(), "Task with due date should generate event");

    let (event_uid, ics) = result.unwrap();
    assert_eq!(event_uid, format!("evt-{}", task.uid));
    assert!(ics.contains("BEGIN:VEVENT"));
    assert!(ics.contains("SUMMARY:Buy milk"));
    assert!(ics.contains("DTSTART"));
    assert!(ics.contains("DTEND"));
}

#[test]
fn test_event_generation_with_start_date() {
    let mut task = parse("Project work");
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(
        result.is_some(),
        "Task with start date should generate event"
    );

    let (event_uid, ics) = result.unwrap();
    assert_eq!(event_uid, format!("evt-{}", task.uid));
    assert!(ics.contains("BEGIN:VEVENT"));
    assert!(ics.contains("SUMMARY:Project work"));
}

#[test]
fn test_event_generation_both_dates() {
    let mut task = parse("Conference");
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 17).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("DTSTART"));
    assert!(ics.contains("DTEND"));
    // Should span from start to due
}

#[test]
fn test_event_generation_with_duration() {
    let mut task = parse("Meeting ~30m");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    task.estimated_duration = Some(30);

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    // Should calculate start time as 30 minutes before due
    assert!(ics.contains("DTSTART"));
    assert!(ics.contains("DTEND"));
}

#[test]
fn test_event_generation_all_day_exclusive_dtend() {
    let mut task = parse("Birthday");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    // DTEND for all-day events should be exclusive (next day)
    assert!(ics.contains("VALUE=DATE"));
    assert!(ics.contains("20250216")); // Next day for DTEND
}

#[test]
fn test_event_generation_with_location() {
    let mut task = parse("Meeting @@office");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.location = Some("office".to_string());

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("LOCATION:office"));
}

#[test]
fn test_event_generation_with_url() {
    let mut task = parse("Video call url:https://meet.example.com");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.url = Some("https://meet.example.com".to_string());

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("URL:https://meet.example.com"));
}

#[test]
fn test_event_generation_with_description() {
    let mut task = parse("Task desc:Important");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.description = "Important".to_string();

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("DESCRIPTION:"));
    // Should contain disclaimer parts (checking short words that won't be split by line folding)
    assert!(ics.contains("automatically"));
    assert!(ics.contains("task"));
    assert!(ics.contains("overwritten"));
    // Should contain user's description at the beginning
    assert!(ics.contains("Important"));
}

#[test]
fn test_event_generation_disclaimer_present() {
    let mut task = parse("Task");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();

    // The disclaimer is in the DESCRIPTION field, which may be encoded
    // Just check that key phrases appear somewhere in the ICS
    assert!(ics.contains("Cfait"), "Should mention Cfait");
    assert!(
        ics.contains("automatically"),
        "Should mention automatic behavior"
    );
    assert!(
        ics.contains("DESCRIPTION"),
        "Should have a description field"
    );
}

#[test]
fn test_event_generation_status_mapping() {
    let mut task = parse("Task");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    // Test Cancelled status
    task.status = TaskStatus::Cancelled;
    let result = task.to_event_ics();
    assert!(result.is_some(), "Cancelled task should generate an event");
    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("STATUS:CANCELLED"));

    // Test Completed status
    task.status = TaskStatus::Completed;
    let result = task.to_event_ics();
    assert!(result.is_some());
    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("STATUS:CONFIRMED"));

    // Test other statuses
    task.status = TaskStatus::NeedsAction;
    let result = task.to_event_ics();
    assert!(result.is_some());
    let (_uid, ics) = result.unwrap();
    assert!(ics.contains("STATUS:CONFIRMED"));
}

#[test]
fn test_event_generation_deterministic_uid() {
    let mut task1 = parse("Task");
    task1.uid = "test-uid-123".to_string();
    task1.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result1 = task1.to_event_ics();
    assert!(result1.is_some());
    let (uid1, _) = result1.unwrap();

    // Generate again - should get same UID
    let result2 = task1.to_event_ics();
    assert!(result2.is_some());
    let (uid2, _) = result2.unwrap();

    assert_eq!(uid1, uid2, "Event UID should be deterministic");
    assert_eq!(uid1, "evt-test-uid-123");
}

// ============================================================================
// ICS SERIALIZATION TESTS: X-CFAIT-CREATE-EVENT property
// ============================================================================

#[test]
fn test_ics_serialization_with_create_event_true() {
    let mut task = parse("Task +cal");
    task.create_event = Some(true);
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let ics = task.to_ics();
    assert!(
        ics.contains("X-CFAIT-CREATE-EVENT:TRUE"),
        "ICS should contain X-CFAIT-CREATE-EVENT:TRUE"
    );
}

#[test]
fn test_ics_serialization_with_create_event_false() {
    let mut task = parse("Task -cal");
    task.create_event = Some(false);
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let ics = task.to_ics();
    assert!(
        ics.contains("X-CFAIT-CREATE-EVENT:FALSE"),
        "ICS should contain X-CFAIT-CREATE-EVENT:FALSE"
    );
}

#[test]
fn test_ics_serialization_without_create_event() {
    let mut task = parse("Task");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let ics = task.to_ics();
    assert!(
        !ics.contains("X-CFAIT-CREATE-EVENT"),
        "ICS should not contain X-CFAIT-CREATE-EVENT when None"
    );
}

#[test]
fn test_ics_deserialization_create_event_true() {
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:test-123
SUMMARY:Test Task
DUE;VALUE=DATE:20250215
X-CFAIT-CREATE-EVENT:TRUE
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag123".to_string(),
        "/test.ics".to_string(),
        "/calendar/".to_string(),
    )
    .expect("Should parse ICS");

    assert_eq!(task.create_event, Some(true));
}

#[test]
fn test_ics_deserialization_create_event_false() {
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:test-123
SUMMARY:Test Task
DUE;VALUE=DATE:20250215
X-CFAIT-CREATE-EVENT:FALSE
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag123".to_string(),
        "/test.ics".to_string(),
        "/calendar/".to_string(),
    )
    .expect("Should parse ICS");

    assert_eq!(task.create_event, Some(false));
}

#[test]
fn test_ics_deserialization_without_create_event() {
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:test-123
SUMMARY:Test Task
DUE;VALUE=DATE:20250215
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(
        ics,
        "etag123".to_string(),
        "/test.ics".to_string(),
        "/calendar/".to_string(),
    )
    .expect("Should parse ICS");

    assert_eq!(task.create_event, None);
}

#[test]
fn test_ics_roundtrip_preserves_create_event() {
    let mut original = parse("Task +cal");
    original.create_event = Some(true);
    original.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let ics = original.to_ics();
    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "/test.ics".to_string(),
        "/cal/".to_string(),
    )
    .expect("Should parse");

    assert_eq!(restored.create_event, original.create_event);
}

// ============================================================================
// OVERRIDE LOGIC TESTS: Priority of per-task vs global config
// ============================================================================

#[test]
fn test_override_priority_explicit_true_over_global() {
    // Task explicitly says +cal
    let task = parse("Task +cal @tomorrow");
    assert_eq!(task.create_event, Some(true));

    // Should create event even if global config is false
    // (This is tested in sync logic, but we verify the field is set)
}

#[test]
fn test_override_priority_explicit_false_over_global() {
    // Task explicitly says -cal
    let task = parse("Task -cal @tomorrow");
    assert_eq!(task.create_event, Some(false));

    // Should NOT create event even if global config is true
    // (This is tested in sync logic, but we verify the field is set)
}

#[test]
fn test_override_priority_none_uses_global() {
    // Task has no explicit override
    let task = parse("Task @tomorrow");
    assert_eq!(task.create_event, None);

    // Should use global config setting (tested in sync logic)
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_multiple_cal_modifiers_last_wins() {
    let task = parse("Task +cal -cal @tomorrow");
    // Parser processes left to right, last one should win
    assert_eq!(task.create_event, Some(false));
}

#[test]
fn test_cal_modifier_with_completed_task() {
    let mut task = parse("Task +cal @tomorrow");
    task.status = TaskStatus::Completed;
    assert_eq!(task.create_event, Some(true));

    // Even with +cal, sync logic should delete event for completed tasks
    // (Tested in sync_companion_event logic)
}

#[test]
fn test_cal_modifier_persists_through_edit() {
    let task1 = parse("Original +cal @tomorrow");
    assert_eq!(task1.create_event, Some(true));

    let smart_str = task1.to_smart_string();
    let task2 = parse(&smart_str);
    assert_eq!(
        task2.create_event,
        Some(true),
        "create_event should persist through to_smart_string roundtrip"
    );
}

#[test]
fn test_event_generation_with_timed_dates() {
    let mut task = parse("Meeting");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 30, 0).unwrap(),
    ));
    task.estimated_duration = Some(90); // 1.5 hours

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    // Should have specific time format (not VALUE=DATE)
    assert!(ics.contains("20250215T"));
    assert!(!ics.contains("VALUE=DATE"));
}

#[test]
fn test_event_default_duration_is_one_hour() {
    let mut task = parse("Task");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    // No estimated_duration set

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();
    // Should calculate start time as 1 hour before due (default)
    // Event should be from 13:00 to 14:00
    assert!(ics.contains("DTSTART:20250215T130000Z"));
    assert!(ics.contains("DTEND:20250215T140000Z"));
}

#[test]
fn test_event_long_span_split_into_start_and_due() {
    let mut task = parse("Long project");
    // Set start date: Jan 1, 2025
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ));
    // Set due date: Feb 15, 2025
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();

    // With new logic, we expect TWO events for this all-day range.
    // One for start (Jan 1) and one for due (Feb 15).

    // Check for Start Event (Jan 1)
    assert!(ics.contains("DTSTART;VALUE=DATE:20250101"));
    assert!(ics.contains("DTEND;VALUE=DATE:20250102")); // Exclusive end

    // Check for Due Event (Feb 15)
    assert!(ics.contains("DTSTART;VALUE=DATE:20250215"));
    assert!(ics.contains("DTEND;VALUE=DATE:20250216")); // Exclusive end

    // Ensure we have two distinct VEVENT blocks
    let event_count = ics.matches("BEGIN:VEVENT").count();
    assert_eq!(
        event_count, 2,
        "Should have two VEVENTs for separate start/due dates"
    );
}

#[test]
fn test_event_short_span_split_into_start_and_due() {
    let mut task = parse("Short project");
    // Set start date: Jan 1, 2025
    task.dtstart = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ));
    // Set due date: Jan 5, 2025 (Different day)
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
    ));

    let result = task.to_event_ics();
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();

    // Per new logic: "if there is no time and the start and due day differ, make it two calendar events"
    // So we expect Jan 1 and Jan 5, NOT a span from 1 to 5.

    assert!(ics.contains("DTSTART;VALUE=DATE:20250101"));
    assert!(ics.contains("DTSTART;VALUE=DATE:20250105"));

    // Ensure we have two events
    let event_count = ics.matches("BEGIN:VEVENT").count();
    assert_eq!(
        event_count, 2,
        "Should have two VEVENTs for different start/due days"
    );
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
    assert!(result.is_some());

    let (_uid, ics) = result.unwrap();

    // Logic: "if there is a time and the difference is greater than 24h, make a 1h event starting at the start and a 1h event ending at the due datetime."

    // 1. Check Start Event (Jan 1, 09:00 to 10:00 (default 1h))
    assert!(ics.contains("DTSTART:20250101T090000Z"));
    // Since task has no duration, start event gets 1h duration added to start time for end?
    // Wait, the logic implemented matches `DateType::Specific(dt) => DateType::Specific(*dt + chrono::Duration::minutes(60))` for start event end.
    // So Jan 1 10:00
    assert!(ics.contains("DTEND:20250101T100000Z"));

    // 2. Check Due Event (Feb 15, ending at 17:00. Start = 17:00 - 60m = 16:00)
    assert!(ics.contains("DTSTART:20250215T160000Z"));
    assert!(ics.contains("DTEND:20250215T170000Z"));

    // Ensure we have two events
    let event_count = ics.matches("BEGIN:VEVENT").count();
    assert_eq!(
        event_count, 2,
        "Should have two VEVENTs for start and due times > 24h apart"
    );
}

#[test]
fn test_override_priority_deletion_when_global_off() {
    // Test that events are deleted when global setting is OFF and task has no override
    let mut task = parse("Task with dates");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    task.create_event = None; // No per-task override

    // With global setting ON, should generate event
    let result_on = task.to_event_ics();
    assert!(result_on.is_some());

    // This test verifies the logic in sync_companion_event would delete
    // We can't directly test the async function here, but we verify that:
    // 1. Task with dates CAN generate an event
    // 2. When should_create_events = false (global OFF), should_delete = true
    // The actual deletion logic is tested via the backfill behavior

    // Verify that a task with +cal override would NOT be deleted even with global OFF
    let mut task_with_override = parse("Task +cal");
    task_with_override.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));
    assert_eq!(task_with_override.create_event, Some(true));
    let result_override = task_with_override.to_event_ics();
    assert!(result_override.is_some()); // Should still generate event
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
        result.is_some(),
        "Cancelled task with sessions should generate an event"
    );
    let (_uid, ics) = result.unwrap();

    // Should contain the VEVENT for the session
    assert!(ics.contains("SUMMARY:⚙ Cancelled project"));
    assert!(
        ics.matches("BEGIN:VEVENT").count() == 2,
        "Should have 2 VEVENT components (session + cancelled plan)"
    );

    // Should also contain the main cancelled VEVENT
    assert!(ics.contains("SUMMARY:Cancelled project"));
    assert!(ics.contains("STATUS:CANCELLED"));
}

#[test]
fn test_event_generation_completed_with_sessions_no_summary() {
    let mut task = parse("Completed project");
    task.status = TaskStatus::Completed;
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 15).unwrap(),
    ));
    task.sessions.push(cfait::model::item::WorkSession {
        start: 1735689600, // 2025-01-01 00:00:00 UTC
        end: 1735693200,   // 2025-01-01 01:00:00 UTC
    });

    let result = task.to_event_ics();
    assert!(
        result.is_some(),
        "Completed task with sessions should generate an event"
    );
    let (_uid, ics) = result.unwrap();

    // Should contain ONLY the VEVENT for the session
    assert!(ics.contains("SUMMARY:⚙ Completed project"));
    assert!(
        ics.matches("BEGIN:VEVENT").count() == 1,
        "Should have only 1 VEVENT component (the session)"
    );

    // Should NOT contain the generic "✓ Task Completed" summary event
    assert!(!ics.contains("✓ Task Completed"));
    // Check with newline to avoid matching the session summary's "Completed project" part
    assert!(!ics.contains("SUMMARY:Completed project\r\n"));
}
