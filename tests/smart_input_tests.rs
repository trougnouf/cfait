use cfait::model::{
    Task,
    parser::{SyntaxType, tokenize_smart_input},
    validate_alias_integrity,
};
use chrono::{Duration, Local};
use std::collections::HashMap;

#[test]
fn test_basic_parsing() {
    let aliases = HashMap::new();
    let task = Task::new("Buy milk @@Kroger url:google.com", &aliases, None);

    assert_eq!(task.summary, "Buy milk");
    assert_eq!(task.location, Some("Kroger".to_string()));
    assert_eq!(task.url, Some("google.com".to_string()));
}

#[test]
fn test_quoting_and_escaping() {
    let aliases = HashMap::new();
    // Input: @@"San Francisco" desc:"Line 1\"Line 2"
    // Expected Location: San Francisco
    // Expected Desc: Line 1"Line 2 (Escaped quote becomes literal quote)
    let task = Task::new(
        "@@\"San Francisco\" desc:\"Line 1\\\"Line 2\"",
        &aliases,
        None,
    );

    assert_eq!(task.location, Some("San Francisco".to_string()));
    assert_eq!(task.description, "Line 1\"Line 2".to_string());
}

#[test]
fn test_alias_expansion() {
    let mut aliases = HashMap::new();
    // #shop -> expands to @@mall and adds #buy tag
    aliases.insert(
        "shop".to_string(),
        vec!["@@mall".to_string(), "#buy".to_string()],
    );

    let task = Task::new("Get shoes #shop", &aliases, None);

    // Summary strips tags that were processed
    assert_eq!(task.summary, "Get shoes");
    assert_eq!(task.location, Some("mall".to_string()));
    assert!(task.categories.contains(&"shop".to_string()));
    assert!(task.categories.contains(&"buy".to_string()));
}

#[test]
fn test_alias_precedence() {
    let mut aliases = HashMap::new();
    // #home sets implicit location
    aliases.insert("home".to_string(), vec!["@@home_addr".to_string()]);

    // User explicitly overrides location while using the #home tag
    let task = Task::new("@@work_addr #home", &aliases, None);

    // User input must win over alias expansion
    assert_eq!(task.location, Some("work_addr".to_string()));
    assert!(task.categories.contains(&"home".to_string()));
}

#[test]
fn test_recursive_alias_expansion() {
    let mut aliases = HashMap::new();
    // #a -> #b
    // #b -> @@final_dest
    aliases.insert("a".to_string(), vec!["#b".to_string()]);
    aliases.insert("b".to_string(), vec!["@@final_dest".to_string()]);

    let task = Task::new("Go #a", &aliases, None);

    assert_eq!(task.location, Some("final_dest".to_string()));
    assert!(task.categories.contains(&"a".to_string()));
    assert!(task.categories.contains(&"b".to_string()));
}

#[test]
fn test_cycle_detection_runtime() {
    let mut aliases = HashMap::new();
    // #a -> #b, #b -> #a
    aliases.insert("a".to_string(), vec!["#b".to_string()]);
    aliases.insert("b".to_string(), vec!["#a".to_string()]);

    // Should not hang or crash
    let task = Task::new("Task #a", &aliases, None);

    // Should contain both tags involved in cycle
    assert!(task.categories.contains(&"a".to_string()));
    assert!(task.categories.contains(&"b".to_string()));
}

#[test]
fn test_integrity_validation() {
    let mut aliases = HashMap::new();
    aliases.insert("a".to_string(), vec!["#b".to_string()]);

    // Valid update
    assert!(validate_alias_integrity("b", &["#c".to_string()], &aliases).is_ok());

    // Cyclic update (#b -> #a, while #a -> #b)
    let res = validate_alias_integrity("b", &["#a".to_string()], &aliases);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        "Circular dependency: '#b' leads back to itself."
    );

    // Self reference
    let res_self = validate_alias_integrity("x", &["#x".to_string()], &aliases);
    assert!(res_self.is_err());
}

#[test]
fn test_geo_and_description() {
    let aliases = HashMap::new();
    let task = Task::new(
        "Visit geo:50.12,4.32 desc:\"View from top\"",
        &aliases,
        None,
    );

    assert_eq!(task.geo, Some("50.12,4.32".to_string()));
    assert_eq!(task.description, "View from top");
}

#[test]
fn test_alias_subtag_expansion() {
    let mut aliases = HashMap::new();
    // Alias: #cfait -> #dev
    aliases.insert("cfait".to_string(), vec!["#dev".to_string()]);

    // Input: #cfait:gui (Sub-tag of alias key)
    let task = Task::new("Task #cfait:gui", &aliases, None);

    // Expectation:
    // 1. #cfait:gui should remain (User input)
    // 2. #dev should be added (Alias match on parent 'cfait')
    assert!(task.categories.contains(&"cfait:gui".to_string()));
    assert!(task.categories.contains(&"dev".to_string()));
}

#[test]
fn test_alias_preserves_sigils() {
    let mut aliases = HashMap::new();
    // Alias: #home -> @@"123 Main St"
    aliases.insert("home".to_string(), vec!["@@\"123 Main St\"".to_string()]);

    let task = Task::new("Go #home", &aliases, None);

    assert_eq!(task.location, Some("123 Main St".to_string()));
    assert!(task.categories.contains(&"home".to_string()));
}

#[test]
fn test_natural_date_parsing_in_keyword() {
    let aliases = HashMap::new();
    let t1 = Task::new("Meeting @in 2 weeks", &aliases, None);
    let now = Local::now();
    let expected = now + Duration::days(14);

    assert!(t1.due.is_some());
    let due = t1.due.unwrap();

    assert_eq!(due.to_date_naive(), expected.date_naive());

    let t2 = Task::new("Work ^in 3 days", &aliases, None);
    assert!(t2.dtstart.is_some());
    let start = t2.dtstart.unwrap();
    assert_eq!(
        start.to_date_naive(),
        (now + Duration::days(3)).date_naive()
    );

    // 3. Edge Case: "in" used as a preposition (not a date)
    let t3 = Task::new("Turn report in", &aliases, None);
    assert!(t3.due.is_none());
    assert!(t3.dtstart.is_none());
    assert_eq!(t3.summary, "Turn report in");

    // 4. Edge Case: "in" followed by non-number
    let t4 = Task::new("Go in room", &aliases, None);
    assert!(t4.due.is_none());
}

#[test]
fn test_syntax_highlighting_tokens_for_in_keyword() {
    let input = "Buy food @in 2 days #chores";
    let tokens = tokenize_smart_input(input);

    let due_token = tokens.iter().find(|t| t.kind == SyntaxType::DueDate);
    assert!(due_token.is_some(), "DueDate token for '@in' was not found");
    let t = due_token.unwrap();
    assert_eq!(&input[t.start..t.end], "@in 2 days");
}

#[test]
fn test_reminder_syntax_highlighting_with_time() {
    // Test rem:tomorrow 16:00
    let input = "test rem:tomorrow 16:00";
    let tokens = tokenize_smart_input(input);

    let reminder_token = tokens.iter().find(|t| t.kind == SyntaxType::Reminder);
    assert!(
        reminder_token.is_some(),
        "Reminder token for 'rem:tomorrow 16:00' was not found"
    );
    let t = reminder_token.unwrap();
    assert_eq!(
        &input[t.start..t.end],
        "rem:tomorrow 16:00",
        "Reminder should include both date and time"
    );
}

#[test]
fn test_reminder_syntax_highlighting_with_full_date() {
    // Test rem:2025-12-27 09:00
    let input = "meeting rem:2025-12-27 09:00";
    let tokens = tokenize_smart_input(input);

    let reminder_token = tokens.iter().find(|t| t.kind == SyntaxType::Reminder);
    assert!(
        reminder_token.is_some(),
        "Reminder token for 'rem:2025-12-27 09:00' was not found"
    );
    let t = reminder_token.unwrap();
    assert_eq!(
        &input[t.start..t.end],
        "rem:2025-12-27 09:00",
        "Reminder should include full date and time"
    );
}

#[test]
fn test_reminder_syntax_highlighting_rem_in() {
    // Test rem:in 5m
    let input = "Pizza rem:in 5m";
    let tokens = tokenize_smart_input(input);

    let reminder_token = tokens.iter().find(|t| t.kind == SyntaxType::Reminder);
    assert!(
        reminder_token.is_some(),
        "Reminder token for 'rem:in 5m' was not found"
    );
    let t = reminder_token.unwrap();
    assert_eq!(
        &input[t.start..t.end],
        "rem:in 5m",
        "Reminder should include 'in' keyword and duration"
    );
}

#[test]
fn test_reminder_syntax_highlighting_with_space() {
    // Test rem: tomorrow 16:00 (space after colon)
    let input = "test rem: tomorrow 16:00";
    let tokens = tokenize_smart_input(input);

    let reminder_token = tokens.iter().find(|t| t.kind == SyntaxType::Reminder);
    assert!(
        reminder_token.is_some(),
        "Reminder token for 'rem: tomorrow 16:00' was not found"
    );
    let t = reminder_token.unwrap();
    assert_eq!(
        &input[t.start..t.end],
        "rem: tomorrow 16:00",
        "Reminder with space after colon should include date and time"
    );
}

#[test]
fn test_reminder_syntax_highlighting_time_only() {
    // Test rem:today (just time)
    let input = "call rem:14:30";
    let tokens = tokenize_smart_input(input);

    let reminder_token = tokens.iter().find(|t| t.kind == SyntaxType::Reminder);
    assert!(
        reminder_token.is_some(),
        "Reminder token for 'rem:14:30' was not found"
    );
    let t = reminder_token.unwrap();
    assert_eq!(&input[t.start..t.end], "rem:14:30");
}
