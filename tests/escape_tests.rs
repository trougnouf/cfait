// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for escaping special characters in input.
use cfait::model::{
    Task,
    parser::{extract_inline_aliases, strip_quotes},
};
use std::collections::HashMap;

#[test]
fn test_escaped_location() {
    let aliases = HashMap::new();
    let task = Task::new("\\@@home", &aliases, None);
    assert_eq!(task.summary, "@@home");
    assert!(task.locations.is_empty());
}

#[test]
fn test_escaped_tag() {
    let aliases = HashMap::new();
    let task = Task::new("\\#tag", &aliases, None);
    assert_eq!(task.summary, "#tag");
    assert!(task.categories.is_empty());
}

#[test]
fn test_escaped_date() {
    let aliases = HashMap::new();
    let task = Task::new("\\@tomorrow", &aliases, None);
    assert_eq!(task.summary, "@tomorrow");
    assert!(task.due.is_none());
}

#[test]
fn test_escaped_start_date() {
    let aliases = HashMap::new();
    let task = Task::new("\\^next week", &aliases, None);
    assert_eq!(task.summary, "^next week");
    assert!(task.dtstart.is_none());
}

#[test]
fn test_escaped_keyword() {
    let aliases = HashMap::new();
    let task = Task::new("\\loc:home", &aliases, None);
    assert_eq!(task.summary, "loc:home");
    assert!(task.locations.is_empty());
}

#[test]
fn test_escaped_strict_word() {
    let aliases = HashMap::new();
    let task = Task::new("\\today", &aliases, None);
    assert_eq!(task.summary, "today");
    assert!(task.due.is_none());
}

#[test]
fn test_round_trip_escaping() {
    let aliases = HashMap::new();
    // Input is literal location string
    let task = Task::new("\\@@home", &aliases, None);
    // Task summary should be literal "@@home" (stored without escape)
    assert_eq!(task.summary, "@@home");
    // Smart string should be escaped back to "\@@home" to preserve meaning during edit
    assert_eq!(task.to_smart_string(), "\\@@home");
}

#[test]
fn test_mixed_escaping() {
    let aliases = HashMap::new();
    let task = Task::new("Buy \\#milk #groceries", &aliases, None);
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
    let task = Task::new(input, &aliases, None);
    assert_eq!(task.description, r#"foo " bar"#);

    // Note: to_smart_string() does not currently serialize the description field,
    // so we do not test round-trip for description here.
}

#[test]
fn test_quote_escaping_in_location_round_trip() {
    let aliases = HashMap::new();
    // Input: loc:"New \"York\""
    let input = r#"loc:"New \"York\"""#;
    let task = Task::new(input, &aliases, None);
    assert_eq!(task.locations, vec![r#"New "York""#.to_string()]);

    // Round trip: Should re-quote and escape
    let smart = task.to_smart_string();
    // Expected: @@"New \"York\""
    assert!(smart.contains(r#"@@"New \"York\"""#));
}

#[test]
fn test_escaped_wiki_link_title() {
    let aliases = HashMap::new();
    // An escaped wiki link is a literal link title, not a reference to a task.
    let task = Task::new("\\[[Ventilation for portable tools]]", &aliases, None);
    assert_eq!(task.summary, "[[Ventilation for portable tools]]");
}

#[test]
fn test_plain_wiki_link_still_strips_to_target() {
    let aliases = HashMap::new();
    // A bare wiki link keeps its reference semantics: it names a task.
    let task = Task::new("[[Ventilation for portable tools]]", &aliases, None);
    assert_eq!(task.summary, "Ventilation for portable tools");
}

#[test]
fn test_escaped_wiki_link_round_trip() {
    let aliases = HashMap::new();
    let task = Task::new("\\[[Ventilation for portable tools]]", &aliases, None);
    assert_eq!(task.summary, "[[Ventilation for portable tools]]");
    // The smart string re-escapes the link so it survives an edit round-trip.
    assert_eq!(
        task.to_smart_string(),
        "\\[[Ventilation for portable tools]]"
    );
    // Re-parsing the escaped form yields the same literal title.
    let reparsed = Task::new(&task.to_smart_string(), &aliases, None);
    assert_eq!(reparsed.summary, "[[Ventilation for portable tools]]");
}

#[test]
fn test_escape_summary_pure_wiki_link() {
    use cfait::model::parser::escape_summary;
    // A whole-link summary is escaped; a link buried in prose is not.
    assert_eq!(
        escape_summary("[[Water the tomatoes]]"),
        "\\[[Water the tomatoes]]"
    );
    assert_eq!(
        escape_summary("Water the [[tomatoes]]"),
        "Water the [[tomatoes]]"
    );
}

#[test]
fn test_is_pure_wiki_link() {
    use cfait::model::parser::is_pure_wiki_link;
    assert!(is_pure_wiki_link("[[Water the tomatoes]]"));
    assert!(is_pure_wiki_link("[[Water the tomatoes|water]]"));
    assert!(!is_pure_wiki_link("Water the [[tomatoes]]"));
    assert!(!is_pure_wiki_link("[[Water]] [[the]]"));
    assert!(!is_pure_wiki_link("[[Water"));
    assert!(!is_pure_wiki_link("]]Water"));
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
