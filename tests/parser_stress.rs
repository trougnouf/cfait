// File: tests/parser_stress.rs
use cfait::model::Task;
use chrono::{Datelike, Duration, Local};
use std::collections::HashMap;

fn parse(input: &str) -> Task {
    Task::new(input, &HashMap::new(), None)
}

// --- RECURRENCE TESTS ---

#[test]
fn test_recurrence_permutations() {
    assert_eq!(parse("Task @daily").rrule, Some("FREQ=DAILY".to_string()));
    assert_eq!(parse("Task @weekly").rrule, Some("FREQ=WEEKLY".to_string()));
    assert_eq!(
        parse("Task @monthly").rrule,
        Some("FREQ=MONTHLY".to_string())
    );
    assert_eq!(parse("Task @yearly").rrule, Some("FREQ=YEARLY".to_string()));
    assert_eq!(
        parse("Task rec:daily").rrule,
        Some("FREQ=DAILY".to_string())
    );
    assert_eq!(
        parse("@every 3 days").rrule,
        Some("FREQ=DAILY;INTERVAL=3".to_string())
    );
    assert_eq!(
        parse("@every 2 weeks").rrule,
        Some("FREQ=WEEKLY;INTERVAL=2".to_string())
    );
    assert_eq!(
        parse("@every 5d").rrule,
        Some("FREQ=DAILY;INTERVAL=5".to_string())
    );
    assert_eq!(
        parse("@every 2w").rrule,
        Some("FREQ=WEEKLY;INTERVAL=2".to_string())
    );
    assert_eq!(
        parse("rec:every 1 month").rrule,
        Some("FREQ=MONTHLY;INTERVAL=1".to_string())
    );
    assert_eq!(
        parse("rec:3d").rrule,
        Some("FREQ=DAILY;INTERVAL=3".to_string())
    );
    assert_eq!(
        parse("rec:2mo").rrule,
        Some("FREQ=MONTHLY;INTERVAL=2".to_string())
    );
    assert_eq!(
        parse("@every two years").rrule,
        Some("FREQ=YEARLY;INTERVAL=2".to_string())
    );
}

#[test]
fn test_recurrence_weekday() {
    let t = parse("Weekly meeting @every wednesday");
    assert_eq!(t.rrule, Some("FREQ=WEEKLY;BYDAY=WE".to_string()));
    let s = t.to_smart_string();
    assert!(s.contains("@every wednesday"));
}

// --- DATE TESTS ---

#[test]
fn test_date_iso_explicit() {
    let t = parse("@2025-12-31");
    let d = t.due.unwrap();
    // FIX: use to_date_naive()
    assert_eq!(d.to_date_naive().year(), 2025);
    assert_eq!(d.to_date_naive().month(), 12);
    assert_eq!(d.to_date_naive().day(), 31);
}

#[test]
fn test_date_keywords() {
    assert!(parse("Task today").due.is_none());
    assert!(parse("Task tomorrow").due.is_none());
    assert!(parse("Task @today").due.is_some());
    assert!(parse("Task @tomorrow").due.is_some());
    assert!(parse("Task ^tomorrow").dtstart.is_some());
    assert!(!parse("Task rem:tomorrow").alarms.is_empty());
    assert!(parse("Task @tomorrow").alarms.is_empty());
    assert!(parse("Task due:today").due.is_some());
    let t_wed = parse("Meeting @wednesday");
    assert!(t_wed.due.is_some());
    // FIX: use to_date_naive()
    assert_eq!(
        t_wed.due.unwrap().to_date_naive().weekday(),
        chrono::Weekday::Wed
    );
}

#[test]
fn test_start_date_permutations() {
    assert!(parse("^today").dtstart.is_some());
    assert!(parse("^2025-01-01").dtstart.is_some());
    assert!(parse("start:tomorrow").dtstart.is_some());
    let t = parse("^3d");
    let now = Local::now().date_naive();
    let expected = now + Duration::days(3);
    // FIX: use to_date_naive()
    assert_eq!(t.dtstart.unwrap().to_date_naive(), expected);
}

#[test]
fn test_relative_offsets() {
    let now = Local::now().date_naive();
    let t1 = parse("@in 5 days");
    // FIX: use to_date_naive()
    assert_eq!(t1.due.unwrap().to_date_naive(), now + Duration::days(5));
    let t2 = parse("@in 5d");
    assert_eq!(t2.due.unwrap().to_date_naive(), now + Duration::days(5));
    let t3 = parse("@1w");
    assert_eq!(t3.due.unwrap().to_date_naive(), now + Duration::days(7));
}

#[test]
fn test_next_weekday() {
    let t = parse("@next friday");
    assert!(t.due.is_some());
    let due = t.due.unwrap();
    // FIX: use to_date_naive()
    assert_eq!(due.to_date_naive().weekday(), chrono::Weekday::Fri);
    // FIX: Wrap Utc::now() in DateType for comparison
    assert!(due > cfait::model::DateType::Specific(chrono::Utc::now()));
}

// --- METADATA TESTS ---

#[test]
fn test_priority_formats() {
    assert_eq!(parse("!1").priority, 1);
    assert_eq!(parse("!9").priority, 9);
    let _t_inv = parse("!0");
    assert_eq!(parse("!99").priority, 99);
    let t_text = parse("!important");
    assert_eq!(t_text.priority, 0);
    assert!(t_text.summary.contains("!important"));
}

#[test]
fn test_duration_formats() {
    assert_eq!(parse("~30m").estimated_duration, Some(30));
    assert_eq!(parse("est:30m").estimated_duration, Some(30));
    assert_eq!(parse("~2h").estimated_duration, Some(120));
    assert_eq!(parse("~1d").estimated_duration, Some(1440));
    let t = parse("~huge");
    assert!(t.estimated_duration.is_none());
    assert!(t.summary.contains("~huge"));
}

// --- NEGATIVE TESTS ---

#[test]
fn test_negative_plain_numbers() {
    let t = parse("Project 5d");
    assert!(t.dtstart.is_none());
    assert!(t.due.is_none());
    assert_eq!(t.summary, "Project 5d");
}

#[test]
fn test_negative_email_addresses() {
    let t = parse("Email bob@example.com");
    assert!(t.due.is_none());
    assert!(t.summary.contains("bob@example.com"));
}

#[test]
fn test_negative_twitter_handles() {
    let t = parse("Follow @rustlang");
    assert!(t.due.is_none());
    assert!(t.summary.contains("@rustlang"));
}

#[test]
fn test_negative_search_operators() {
    let t1 = parse("Search !<2");
    assert_eq!(t1.priority, 0);
    assert!(t1.summary.contains("!<2"));
    let t2 = parse("Filter @>2023-01-01");
    assert!(t2.due.is_none());
    assert!(t2.summary.contains("@>2023-01-01"));
}

#[test]
fn test_negative_orphaned_sigils() {
    let t1 = parse("Meet @ 5");
    assert!(t1.due.is_none());
    assert!(t1.summary.contains("@"));
    let t2 = parse("Is it ^ ?");
    assert!(t2.dtstart.is_none());
    assert!(t2.summary.contains("^"));
}

#[test]
fn test_negative_locations() {
    let t = parse("Look at @@");
    assert!(t.location.is_none());
    assert_eq!(t.summary, "Look at @@");
    let t2 = parse("Go to loc:");
    assert!(t2.location.is_none());
    assert_eq!(t2.summary, "Go to loc:");
}
// --- EXTENDED COVERAGE ---

#[test]
fn test_case_insensitivity() {
    // Recurrence
    assert_eq!(parse("@DAILY").rrule, Some("FREQ=DAILY".to_string()));
    assert_eq!(parse("@Weekly").rrule, Some("FREQ=WEEKLY".to_string()));

    // Keywords
    assert!(parse("@ToDaY").due.is_some());

    // Explicit keys
    assert_eq!(
        parse("URL:example.com").url,
        Some("example.com".to_string())
    );
    assert_eq!(parse("LOC:Home").location, Some("Home".to_string()));
}

#[test]
fn test_start_date_weekdays() {
    // "^monday"
    let t = parse("Start ^monday");
    assert!(t.dtstart.is_some());
    // FIX: use to_date_naive()
    assert_eq!(
        t.dtstart.unwrap().to_date_naive().weekday(),
        chrono::Weekday::Mon
    );

    // "start:friday"
    let t2 = parse("Start start:friday");
    assert!(t2.dtstart.is_some());
    // FIX: use to_date_naive()
    assert_eq!(
        t2.dtstart.unwrap().to_date_naive().weekday(),
        chrono::Weekday::Fri
    );
}

#[test]
fn test_quoted_values() {
    // Tags with spaces
    let t1 = parse("Work on #\"Project X\"");
    assert!(t1.categories.contains(&"Project X".to_string()));

    // Location with spaces
    let t2 = parse("Meeting @@\"Conference Room B\"");
    assert_eq!(t2.location, Some("Conference Room B".to_string()));

    // Description with spaces
    let t3 = parse("desc:\"Call mom back\"");
    assert_eq!(t3.description, "Call mom back");
}

#[test]
fn test_url_complexity() {
    // URL with query parameters and special chars
    let url = "https://example.com/path?query=1&param=2";
    let t = parse(&format!("Check url:{}", url));
    assert_eq!(t.url, Some(url.to_string()));

    // URL bracket syntax (often used for org-mode style links, though we strip brackets)
    let t2 = parse("[[https://example.com]]");
    assert_eq!(t2.url, Some("https://example.com".to_string()));
}

#[test]
fn test_negative_implicit_next() {
    // "next friday" without @ or ^ should be TEXT, not a date.
    // We don't want "The next step is..." to trigger a due date.
    let t = parse("See you next friday");
    assert!(t.due.is_none());
    assert!(t.dtstart.is_none());
    assert_eq!(t.summary, "See you next friday");
}

#[test]
fn test_kitchen_sink() {
    // A task containing almost every field types
    let input =
        "Big Task !1 @tomorrow ^today ~2h #work #urgent @@Office url:github.com desc:\"Check PR\"";
    let t = parse(input);

    assert_eq!(t.summary, "Big Task");
    assert_eq!(t.priority, 1);
    assert!(t.due.is_some());
    assert!(t.dtstart.is_some());
    assert_eq!(t.estimated_duration, Some(120));
    assert!(t.categories.contains(&"work".to_string()));
    assert!(t.categories.contains(&"urgent".to_string()));
    assert_eq!(t.location, Some("Office".to_string()));
    assert_eq!(t.url, Some("github.com".to_string()));
    assert_eq!(t.description, "Check PR");
}

#[test]
fn test_recurrence_mixed_casing_and_spacing() {
    // "@every 2 Months" (Mixed case unit)
    let t = parse("Bill @every 2 Months");
    assert_eq!(t.rrule, Some("FREQ=MONTHLY;INTERVAL=2".to_string()));
}
