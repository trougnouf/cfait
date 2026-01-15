// Tests for duration range functionality
//
// This tests the new duration range feature that allows tasks to have
// both a minimum and maximum estimated duration (e.g., ~30m-1h).

use cfait::model::parser::parse_duration_range;
use cfait::model::{DateType, Task};
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

fn parse(input: &str) -> Task {
    let aliases = HashMap::new();
    Task::new(input, &aliases, None)
}

// ============================================================================
// PARSER TESTS: Duration range parsing
// ============================================================================

#[test]
fn test_parse_duration_range_single_value() {
    // Simple single values
    assert_eq!(parse_duration_range("30m"), Some((30, None)));
    assert_eq!(parse_duration_range("1h"), Some((60, None)));
    assert_eq!(parse_duration_range("2d"), Some((2880, None)));
}

#[test]
fn test_parse_duration_range_valid_range() {
    // Valid ranges
    assert_eq!(parse_duration_range("30m-1h"), Some((30, Some(60))));
    assert_eq!(parse_duration_range("1h-2h"), Some((60, Some(120))));
    assert_eq!(parse_duration_range("30m-90m"), Some((30, Some(90))));
}

#[test]
fn test_parse_duration_range_equal_values() {
    // Range where min == max should still be a valid range
    assert_eq!(parse_duration_range("30m-30m"), Some((30, Some(30))));
}

#[test]
fn test_parse_duration_range_invalid_order() {
    // Invalid ranges (max < min) should fallback to single value
    assert_eq!(parse_duration_range("1h-30m"), Some((60, None)));
}

#[test]
fn test_parse_duration_range_invalid_input() {
    // Invalid inputs
    assert_eq!(parse_duration_range(""), None);
    assert_eq!(parse_duration_range("invalid"), None);
    assert_eq!(parse_duration_range("-"), None);
}

#[test]
fn test_parse_duration_range_smart_input_single() {
    let t = parse("Task ~30m");
    assert_eq!(t.estimated_duration, Some(30));
    assert_eq!(t.estimated_duration_max, None);
}

#[test]
fn test_parse_duration_range_smart_input_valid_range() {
    let t = parse("Task ~30m-1h");
    assert_eq!(t.estimated_duration, Some(30));
    assert_eq!(t.estimated_duration_max, Some(60));
}

#[test]
fn test_parse_duration_range_smart_input_with_est() {
    let t = parse("Task est:30m-1h");
    assert_eq!(t.estimated_duration, Some(30));
    assert_eq!(t.estimated_duration_max, Some(60));
}

// ============================================================================
// MATCHER TESTS: Overlap-based filtering
// ============================================================================

#[test]
fn test_duration_filter_point_query_in_range() {
    // Task [30m, 1h] should match ~30m, ~45m, ~1h
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(
        t.matches_search_term("~30m"),
        "Should match ~30m (in range)"
    );
    assert!(
        t.matches_search_term("~45m"),
        "Should match ~45m (in range)"
    );
    assert!(t.matches_search_term("~1h"), "Should match ~1h (in range)");
}

#[test]
fn test_duration_filter_point_query_outside_range() {
    // Task [30m, 1h] should NOT match ~15m, ~2h
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(
        !t.matches_search_term("~15m"),
        "Should NOT match ~15m (below range)"
    );
    assert!(
        !t.matches_search_term("~2h"),
        "Should NOT match ~2h (above range)"
    );
}

#[test]
fn test_duration_filter_point_estimate() {
    // Task with single duration (point estimate) should match exact value only
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = None; // Point estimate

    assert!(t.matches_search_term("~30m"), "Should match ~30m (exact)");
    assert!(!t.matches_search_term("~29m"), "Should NOT match ~29m");
    assert!(!t.matches_search_term("~31m"), "Should NOT match ~31m");
}

#[test]
fn test_duration_filter_less_than() {
    // Query ~<1h: Match if task can be shorter than 1h
    // Task [30m, 1h] should match (min=30 < 60)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(t.matches_search_term("~<1h"), "Task [30m-1h] can be < 1h");
}

#[test]
fn test_duration_filter_less_than_fail() {
    // Query ~<30m: Match if task can be shorter than 30m
    // Task [30m, 1h] should NOT match (min=30 >= 30)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(
        !t.matches_search_term("~<30m"),
        "Task [30m-1h] cannot be < 30m"
    );
}

#[test]
fn test_duration_filter_greater_than() {
    // Query ~>30m: Match if task can be longer than 30m
    // Task [30m, 1h] should match (max=60 > 30)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(t.matches_search_term("~>30m"), "Task [30m-1h] can be > 30m");
}

#[test]
fn test_duration_filter_greater_than_fail() {
    // Query ~>1h: Match if task can be longer than 1h
    // Task [30m, 1h] should NOT match (max=60 <= 60)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(
        !t.matches_search_term("~>1h"),
        "Task [30m-1h] cannot be > 1h"
    );
}

#[test]
fn test_duration_filter_less_than_or_equal() {
    // Query ~<=1h: Match if task can be 1h or shorter
    // Task [30m, 1h] should match (min=30 <= 60)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(t.matches_search_term("~<=1h"), "Task [30m-1h] can be <= 1h");
}

#[test]
fn test_duration_filter_less_than_or_equal_fail() {
    // Query ~<=30m: Match if task can be 30m or shorter
    // Task [31m, 1h] should NOT match (min=31 > 30)
    let mut t = parse("Task");
    t.estimated_duration = Some(31);
    t.estimated_duration_max = Some(60);

    assert!(
        !t.matches_search_term("~<=30m"),
        "Task [31m-1h] cannot be <= 30m"
    );
}

#[test]
fn test_duration_filter_greater_than_or_equal() {
    // Query ~>=30m: Match if task can be 30m or longer
    // Task [30m, 1h] should match (max=60 >= 30)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    assert!(
        t.matches_search_term("~>=30m"),
        "Task [30m-1h] can be >= 30m"
    );
}

#[test]
fn test_duration_filter_greater_than_or_equal_fail() {
    // Query ~>=1h: Match if task can be 1h or longer
    // Task [30m, 59m] should NOT match (max=59 < 60)
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(59);

    assert!(
        !t.matches_search_term("~>=1h"),
        "Task [30m-59m] cannot be >= 1h"
    );
}

#[test]
fn test_duration_filter_no_duration() {
    // Task without duration should NOT match duration queries
    let t = parse("Task");

    assert!(
        !t.matches_search_term("~30m"),
        "Task without duration should NOT match ~30m"
    );
    assert!(
        !t.matches_search_term("~<1h"),
        "Task without duration should NOT match ~<1h"
    );
}

// ============================================================================
// ICS ROUNDTRIP TESTS: estimated_duration_max persistence
// ============================================================================

#[test]
fn test_ics_roundtrip_with_max_duration() {
    // Task with max duration should persist through ICS roundtrip
    let mut original = parse("Task with range");
    original.estimated_duration = Some(30);
    original.estimated_duration_max = Some(60);

    let ics = original.to_ics();

    // Should contain standard DURATION (no due date) and X-CFAIT-ESTIMATED-DURATION-MAX
    assert!(
        ics.contains("DURATION:PT30M"),
        "ICS should contain DURATION:PT30M. Got:\n{}",
        ics
    );
    assert!(
        ics.contains("X-CFAIT-ESTIMATED-DURATION-MAX:PT60M"),
        "ICS should contain X-CFAIT-ESTIMATED-DURATION-MAX:PT60M. Got:\n{}",
        ics
    );

    // Parse back and verify
    let restored =
        Task::from_ics(&ics, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();
    assert_eq!(
        restored.estimated_duration,
        Some(30),
        "Restored estimated_duration should be 30"
    );
    assert_eq!(
        restored.estimated_duration_max,
        Some(60),
        "Restored estimated_duration_max should be 60"
    );
}

#[test]
fn test_ics_roundtrip_without_max_duration() {
    // Task without max duration should work normally
    let mut original = parse("Task with single duration");
    original.estimated_duration = Some(30);
    original.estimated_duration_max = None;

    let ics = original.to_ics();

    // Should NOT contain X-CFAIT-ESTIMATED-DURATION-MAX
    assert!(
        !ics.contains("X-CFAIT-ESTIMATED-DURATION-MAX"),
        "ICS should NOT contain X-CFAIT-ESTIMATED-DURATION-MAX when max is None. Got:\n{}",
        ics
    );

    // Should contain standard DURATION (no due date)
    assert!(
        ics.contains("DURATION:PT30M"),
        "ICS should contain DURATION:PT30M. Got:\n{}",
        ics
    );

    let restored =
        Task::from_ics(&ics, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();
    assert_eq!(restored.estimated_duration, Some(30));
    assert_eq!(restored.estimated_duration_max, None);
}

#[test]
fn test_parse_ics_with_max_duration() {
    // Parse ICS containing X-CFAIT-ESTIMATED-DURATION-MAX
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Cfait//Test//EN
BEGIN:VTODO
UID:test123
SUMMARY:Task with range
X-ESTIMATED-DURATION:PT30M
X-CFAIT-ESTIMATED-DURATION-MAX:PT60M
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(ics, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();

    assert_eq!(task.summary, "Task with range");
    assert_eq!(task.estimated_duration, Some(30));
    assert_eq!(task.estimated_duration_max, Some(60));
}

#[test]
fn test_parse_ics_without_max_duration() {
    // Parse ICS without X-CFAIT-ESTIMATED-DURATION-MAX
    let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Cfait//Test//EN
BEGIN:VTODO
UID:test123
SUMMARY:Task with single duration
X-ESTIMATED-DURATION:PT30M
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR"#;

    let task = Task::from_ics(ics, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();

    assert_eq!(task.summary, "Task with single duration");
    assert_eq!(task.estimated_duration, Some(30));
    assert_eq!(task.estimated_duration_max, None);
}

#[test]
fn test_ics_roundtrip_with_max_and_due_date() {
    // Task with max duration AND due date should use X-ESTIMATED-DURATION
    let mut original = parse("Task with range and due");
    original.estimated_duration = Some(30);
    original.estimated_duration_max = Some(60);
    original.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2025, 2, 15, 14, 0, 0).unwrap(),
    ));

    let ics = original.to_ics();

    // Should contain X-ESTIMATED-DURATION (not DURATION) when due is present
    assert!(
        ics.contains("X-ESTIMATED-DURATION:PT30M"),
        "ICS should contain X-ESTIMATED-DURATION:PT30M when due is present. Got:\n{}",
        ics
    );
    assert!(
        ics.contains("X-CFAIT-ESTIMATED-DURATION-MAX:PT60M"),
        "ICS should contain X-CFAIT-ESTIMATED-DURATION-MAX:PT60M. Got:\n{}",
        ics
    );
    // Should NOT contain standard DURATION
    assert!(
        !ics.contains("\nDURATION:"),
        "ICS should NOT contain DURATION when due is present. Got:\n{}",
        ics
    );

    // Parse back and verify
    let restored =
        Task::from_ics(&ics, "e1".to_string(), "h1".to_string(), "c1".to_string()).unwrap();
    assert_eq!(restored.estimated_duration, Some(30));
    assert_eq!(restored.estimated_duration_max, Some(60));
}

// ============================================================================
// SMART STRING SERIALIZATION TESTS
// ============================================================================

#[test]
fn test_to_smart_string_with_range() {
    // Task with range should serialize to smart string correctly
    let mut t = parse("Task");
    t.summary = "Task with range".to_string();
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    let smart = t.to_smart_string();

    // Should contain ~30m-1h
    assert!(
        smart.contains("~30m-1h"),
        "Smart string should contain ~30m-1h. Got: {}",
        smart
    );
}

#[test]
fn test_to_smart_string_single_duration() {
    // Task with single duration should not show range
    let mut t = parse("Task");
    t.summary = "Task with single".to_string();
    t.estimated_duration = Some(30);
    t.estimated_duration_max = None;

    let smart = t.to_smart_string();

    // Should contain ~30m but NOT range syntax
    assert!(
        smart.contains("~30m") && !smart.contains("~30m-"),
        "Smart string should contain ~30m but NOT range syntax. Got: {}",
        smart
    );
}

#[test]
fn test_to_smart_string_no_duration() {
    // Task without duration should not show duration
    let t = parse("Task without duration");

    let smart = t.to_smart_string();

    // Should NOT contain ~ at all
    assert!(
        !smart.contains("~"),
        "Smart string should NOT contain ~ when no duration. Got: {}",
        smart
    );
}

// ============================================================================
// FORMAT DURATION SHORT TESTS
// ============================================================================

#[test]
fn test_format_duration_short_range() {
    // format_duration_short should display range
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = Some(60);

    let formatted = t.format_duration_short();

    assert_eq!(formatted, "[~30m-1h]");
}

#[test]
fn test_format_duration_short_single() {
    // format_duration_short should display single value
    let mut t = parse("Task");
    t.estimated_duration = Some(30);
    t.estimated_duration_max = None;

    let formatted = t.format_duration_short();

    assert_eq!(formatted, "[~30m]");
}

#[test]
fn test_format_duration_short_none() {
    // format_duration_short should be empty when no duration
    let t = parse("Task");

    let formatted = t.format_duration_short();

    assert_eq!(formatted, "");
}
