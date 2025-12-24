// File: tests/smart_input_extensions.rs
use cfait::model::Task;
use std::collections::HashMap;

#[test]
fn test_recurrence_presets() {
    let aliases = HashMap::new();

    // Test @daily
    let t1 = Task::new("Standup @daily", &aliases);
    assert_eq!(t1.rrule, Some("FREQ=DAILY".to_string()));

    // Test @weekly
    let t2 = Task::new("Review @weekly", &aliases);
    assert_eq!(t2.rrule, Some("FREQ=WEEKLY".to_string()));

    // Test @monthly
    let t3 = Task::new("Pay bills @monthly", &aliases);
    assert_eq!(t3.rrule, Some("FREQ=MONTHLY".to_string()));

    // Test @yearly
    let t4 = Task::new("Birthday @yearly", &aliases);
    assert_eq!(t4.rrule, Some("FREQ=YEARLY".to_string()));
}

#[test]
fn test_recurrence_every_x_units() {
    let aliases = HashMap::new();

    // Test @every 3 days
    let t1 = Task::new("Water plants @every 3 days", &aliases);
    // Note: The parser implementation builds this string manually
    assert_eq!(t1.rrule, Some("FREQ=DAILY;INTERVAL=3".to_string()));

    // Test @every 2 weeks
    let t2 = Task::new("Sprint Planning @every 2 weeks", &aliases);
    assert_eq!(t2.rrule, Some("FREQ=WEEKLY;INTERVAL=2".to_string()));

    // Test @every 6 months
    let t3 = Task::new("Dentist @every 6 months", &aliases);
    assert_eq!(t3.rrule, Some("FREQ=MONTHLY;INTERVAL=6".to_string()));
}

#[test]
fn test_recurrence_raw_input() {
    let aliases = HashMap::new();

    // Test manual raw input (e.g. pasting from advanced editor)
    // This ensures the parser doesn't treat "FREQ=..." as a text title
    let t1 = Task::new("Complex Task rec:FREQ=MONTHLY;BYDAY=MO", &aliases);
    assert_eq!(t1.rrule, Some("FREQ=MONTHLY;BYDAY=MO".to_string()));
}

#[test]
fn test_duration_units() {
    let aliases = HashMap::new();

    // Minutes
    let t1 = Task::new("Quick task ~15m", &aliases);
    assert_eq!(t1.estimated_duration, Some(15));

    // Hours
    let t2 = Task::new("Deep work ~2h", &aliases);
    assert_eq!(t2.estimated_duration, Some(120));

    // Days
    let t3 = Task::new("Project ~3d", &aliases);
    assert_eq!(t3.estimated_duration, Some(3 * 24 * 60));

    // Weeks
    let t4 = Task::new("Sabbatical ~1w", &aliases);
    assert_eq!(t4.estimated_duration, Some(1 * 7 * 24 * 60));
}

#[test]
fn test_start_date_syntax() {
    let aliases = HashMap::new();

    // Caret syntax
    let t1 = Task::new("Future work ^tomorrow", &aliases);
    assert!(t1.dtstart.is_some());

    // Explicit syntax
    let t2 = Task::new("Future work start:tomorrow", &aliases);
    assert!(t2.dtstart.is_some());

    assert_eq!(t1.dtstart, t2.dtstart);
}

#[test]
fn test_prettify_round_trip() {
    let aliases = HashMap::new();

    // 1. Create a task with a raw RRULE (simulating loaded from disk)
    let mut t = Task::new("Base", &aliases);
    t.rrule = Some("FREQ=DAILY".to_string());

    // 2. Convert to smart string
    let smart = t.to_smart_string();

    // 3. Assert it uses the pretty syntax, not the raw syntax
    assert!(smart.contains("@daily"));
    assert!(!smart.contains("FREQ=DAILY"));

    // 4. Test "Every X" round trip
    t.rrule = Some("FREQ=WEEKLY;INTERVAL=2".to_string());
    let smart_every = t.to_smart_string();
    assert!(smart_every.contains("@every 2 weeks"));
}

#[test]
fn test_inline_alias_definition() {
    // README says: #gardening:=#fun,@@home
    // This tests the `extract_inline_aliases` logic indirectly via Task::new logic if integrated,
    // or directly via the parser module.

    let input = "Plant tree #gardening:=#fun,@@home";

    // Verify extraction logic directly
    let (clean, map) = cfait::model::parser::extract_inline_aliases(input);

    assert_eq!(clean, "Plant tree #gardening");
    assert!(map.contains_key("gardening"));

    let values = map.get("gardening").unwrap();
    assert!(values.contains(&"#fun".to_string()));
    assert!(values.contains(&"@@home".to_string()));
}
