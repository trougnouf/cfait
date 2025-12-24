// File: ./tests/escape_tests.rs
use cfait::model::{
    Task,
    parser::{extract_inline_aliases, strip_quotes},
};
use std::collections::HashMap;

#[test]
fn test_escaped_location() {
    let aliases = HashMap::new();
    let task = Task::new("\\@@home", &aliases);
    assert_eq!(task.summary, "@@home");
    assert!(task.location.is_none());
}

#[test]
fn test_escaped_tag() {
    let aliases = HashMap::new();
    let task = Task::new("\\#tag", &aliases);
    assert_eq!(task.summary, "#tag");
    assert!(task.categories.is_empty());
}

#[test]
fn test_escaped_date() {
    let aliases = HashMap::new();
    let task = Task::new("\\@tomorrow", &aliases);
    assert_eq!(task.summary, "@tomorrow");
    assert!(task.due.is_none());
}

#[test]
fn test_escaped_start_date() {
    let aliases = HashMap::new();
    let task = Task::new("\\^next week", &aliases);
    assert_eq!(task.summary, "^next week");
    assert!(task.dtstart.is_none());
}

#[test]
fn test_escaped_keyword() {
    let aliases = HashMap::new();
    let task = Task::new("\\loc:home", &aliases);
    assert_eq!(task.summary, "loc:home");
    assert!(task.location.is_none());
}

#[test]
fn test_escaped_strict_word() {
    let aliases = HashMap::new();
    let task = Task::new("\\today", &aliases);
    assert_eq!(task.summary, "today");
    assert!(task.due.is_none());
}

#[test]
fn test_round_trip_escaping() {
    let aliases = HashMap::new();
    // Input is literal location string
    let task = Task::new("\\@@home", &aliases);
    // Task summary should be literal "@@home" (stored without escape)
    assert_eq!(task.summary, "@@home");
    // Smart string should be escaped back to "\@@home" to preserve meaning during edit
    assert_eq!(task.to_smart_string(), "\\@@home");
}

#[test]
fn test_mixed_escaping() {
    let aliases = HashMap::new();
    let task = Task::new("Buy \\#milk #groceries", &aliases);
    assert_eq!(task.summary, "Buy #milk");
    assert!(task.categories.contains(&"groceries".to_string()));
    assert!(!task.categories.contains(&"milk".to_string()));
}

#[test]
fn test_quote_escaping_in_desc() {
    let aliases = HashMap::new();
    // Input: desc:"foo \" bar"
    // The parser should unescape the internal quote
    let input = r#"desc:"foo \" bar""#;
    let task = Task::new(input, &aliases);
    assert_eq!(task.description, r#"foo " bar"#);

    // Note: to_smart_string() does not currently serialize the description field,
    // so we do not test round-trip for description here.
}

#[test]
fn test_quote_escaping_in_location_round_trip() {
    let aliases = HashMap::new();
    // Input: loc:"New \"York\""
    let input = r#"loc:"New \"York\"""#;
    let task = Task::new(input, &aliases);
    assert_eq!(task.location, Some(r#"New "York""#.to_string()));

    // Round trip: Should re-quote and escape
    let smart = task.to_smart_string();
    // Expected: @@"New \"York\""
    assert!(smart.contains(r#"@@"New \"York\"""#));
}

#[test]
fn test_strip_quotes_unescape() {
    assert_eq!(strip_quotes(r#""foo \" bar""#), r#"foo " bar"#);
    assert_eq!(strip_quotes(r#"foo \# bar"#), r#"foo # bar"#);
}

#[test]
fn test_alias_escaping() {
    let (clean, aliases) = extract_inline_aliases(r#"\#key:=val"#);
    assert_eq!(clean, r#"\#key:=val"#);
    assert!(aliases.is_empty());
}
