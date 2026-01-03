// Tests for additional smart input features.
use cfait::model::Task;
use std::collections::HashMap;

#[test]
fn test_recurrence_presets() {
    let aliases = HashMap::new();

    // Test @daily
    let t1 = Task::new("Standup @daily", &aliases, None);
    assert_eq!(t1.rrule, Some("FREQ=DAILY".to_string()));

    // Test @weekly
    let t2 = Task::new("Review @weekly", &aliases, None);
    assert_eq!(t2.rrule, Some("FREQ=WEEKLY".to_string()));

    // Test @monthly
    let t3 = Task::new("Pay bills @monthly", &aliases, None);
    assert_eq!(t3.rrule, Some("FREQ=MONTHLY".to_string()));

    // Test @yearly
    let t4 = Task::new("Birthday @yearly", &aliases, None);
    assert_eq!(t4.rrule, Some("FREQ=YEARLY".to_string()));
}

#[test]
fn test_recurrence_every_x_units() {
    let aliases = HashMap::new();

    // Test @every 3 days
    let t1 = Task::new("Water plants @every 3 days", &aliases, None);
    // Note: The parser implementation builds this string manually
    assert_eq!(t1.rrule, Some("FREQ=DAILY;INTERVAL=3".to_string()));

    // Test @every 2 weeks
    let t2 = Task::new("Sprint Planning @every 2 weeks", &aliases, None);
    assert_eq!(t2.rrule, Some("FREQ=WEEKLY;INTERVAL=2".to_string()));

    // Test @every 6 months
    let t3 = Task::new("Dentist @every 6 months", &aliases, None);
    assert_eq!(t3.rrule, Some("FREQ=MONTHLY;INTERVAL=6".to_string()));
}

#[test]
fn test_recurrence_raw_input() {
    let aliases = HashMap::new();

    // Test manual raw input (e.g. pasting from advanced editor)
    // This ensures the parser doesn't treat "FREQ=..." as a text title
    let t1 = Task::new("Complex Task rec:FREQ=MONTHLY;BYDAY=MO", &aliases, None);
    assert_eq!(t1.rrule, Some("FREQ=MONTHLY;BYDAY=MO".to_string()));
}

#[test]
fn test_duration_units() {
    let aliases = HashMap::new();

    // Minutes
    let t1 = Task::new("Quick task ~15m", &aliases, None);
    assert_eq!(t1.estimated_duration, Some(15));

    // Hours
    let t2 = Task::new("Deep work ~2h", &aliases, None);
    assert_eq!(t2.estimated_duration, Some(120));

    // Days
    let t3 = Task::new("Project ~3d", &aliases, None);
    assert_eq!(t3.estimated_duration, Some(3 * 24 * 60));

    // Weeks
    let t4 = Task::new("Sabbatical ~1w", &aliases, None);
    assert_eq!(t4.estimated_duration, Some(7 * 24 * 60));
}

#[test]
fn test_start_date_syntax() {
    let aliases = HashMap::new();

    // Caret syntax
    let t1 = Task::new("Future work ^tomorrow", &aliases, None);
    assert!(t1.dtstart.is_some());

    // Explicit syntax
    let t2 = Task::new("Future work start:tomorrow", &aliases, None);
    assert!(t2.dtstart.is_some());

    assert_eq!(t1.dtstart, t2.dtstart);
}

#[test]
fn test_prettify_round_trip() {
    let aliases = HashMap::new();

    // 1. Create a task with a raw RRULE (simulating loaded from disk)
    let mut t = Task::new("Base", &aliases, None);
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

#[test]
fn test_recurrence_with_until() {
    let aliases = HashMap::new();

    // Test until with preset recurrence
    let t1 = Task::new("Daily standup @daily until 2025-12-31", &aliases, None);
    assert!(t1.rrule.is_some());
    let rrule1 = t1.rrule.unwrap();
    assert!(rrule1.contains("FREQ=DAILY"));
    assert!(rrule1.contains("UNTIL=20251231"));

    // Test until with custom interval
    let t2 = Task::new("Review @every 2 weeks until 2026-06-30", &aliases, None);
    assert!(t2.rrule.is_some());
    let rrule2 = t2.rrule.unwrap();
    assert!(rrule2.contains("FREQ=WEEKLY"));
    assert!(rrule2.contains("INTERVAL=2"));
    assert!(rrule2.contains("UNTIL=20260630"));
}

#[test]
fn test_recurrence_with_except() {
    use cfait::model::DateType;
    use chrono::NaiveDate;

    let aliases = HashMap::new();

    // Test single exception
    let t1 = Task::new("Meeting @weekly except 2025-01-20", &aliases, None);
    assert!(t1.rrule.is_some());
    assert_eq!(t1.exdates.len(), 1);
    match &t1.exdates[0] {
        DateType::AllDay(date) => {
            assert_eq!(*date, NaiveDate::from_ymd_opt(2025, 1, 20).unwrap());
        }
        _ => panic!("Expected AllDay date type"),
    }

    // Test multiple exceptions
    let t2 = Task::new(
        "Standup @daily except 2025-12-25 except 2026-01-01",
        &aliases,
        None,
    );
    assert_eq!(t2.exdates.len(), 2);
}

#[test]
fn test_recurrence_with_until_and_except() {
    let aliases = HashMap::new();

    // Test combining until and except
    let t = Task::new(
        "Team sync @daily until 2025-12-31 except 2025-12-25",
        &aliases,
        None,
    );

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();
    assert!(rrule.contains("FREQ=DAILY"));
    assert!(rrule.contains("UNTIL=20251231"));

    assert_eq!(t.exdates.len(), 1);
}

#[test]
fn test_until_and_except_round_trip() {
    use cfait::model::DateType;
    use chrono::NaiveDate;

    let aliases = HashMap::new();

    // Create task with until
    let input1 = "Task @weekly until 2025-12-31";
    let t1 = Task::new(input1, &aliases, None);
    let smart1 = t1.to_smart_string();
    assert!(smart1.contains("@weekly"));
    assert!(smart1.contains("until 2025-12-31"));

    // Create task with except
    let mut t2 = Task::new("Task @daily", &aliases, None);
    t2.exdates.push(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    ));
    let smart2 = t2.to_smart_string();
    assert!(smart2.contains("@daily"));
    assert!(smart2.contains("except 2025-01-15"));

    // Create task with both
    let input3 = "Task @daily until 2025-12-31 except 2025-01-20";
    let t3 = Task::new(input3, &aliases, None);
    let smart3 = t3.to_smart_string();
    assert!(smart3.contains("@daily"));
    assert!(smart3.contains("until 2025-12-31"));
    assert!(smart3.contains("except 2025-01-20"));
}

#[test]
fn test_complex_task_with_recurrence_extensions() {
    let aliases = HashMap::new();

    // Test a complex real-world scenario
    let input = "Team meeting !2 @2025-01-20 @every monday until 2025-12-31 except 2025-07-04 #work @@office ~1h rem:10m";
    let t = Task::new(input, &aliases, None);

    // Verify all components
    assert_eq!(t.summary, "Team meeting");
    assert_eq!(t.priority, 2);
    assert!(t.due.is_some());
    assert!(t.rrule.is_some());

    let rrule = t.rrule.unwrap();
    assert!(rrule.contains("FREQ=WEEKLY"));
    assert!(rrule.contains("BYDAY=MO"));
    assert!(rrule.contains("UNTIL=20251231"));

    assert_eq!(t.exdates.len(), 1);
    assert!(t.categories.contains(&"work".to_string()));
    assert_eq!(t.location, Some("office".to_string()));
    assert_eq!(t.estimated_duration, Some(60));
    assert!(!t.alarms.is_empty());
}
