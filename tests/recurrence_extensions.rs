// Tests for extended recurrence features (until/except).
// Tests for "until" and "except" recurrence extensions

use cfait::model::{DateType, Task};
use chrono::NaiveDate;
use std::collections::HashMap;

// Helper function to parse task
fn parse(input: &str) -> Task {
    Task::new(input, &HashMap::new(), None)
}

#[test]
fn test_until_basic() {
    let t = parse("Morning walk @daily until 2025-12-31");

    // Should have recurrence rule
    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    // Should contain FREQ=DAILY
    assert!(rrule.contains("FREQ=DAILY"));

    // Should contain UNTIL with date
    assert!(rrule.contains("UNTIL=20251231"));
}

#[test]
fn test_until_with_every_interval() {
    let t = parse("Water plants @every 3 days until 2025-06-30");

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=DAILY"));
    assert!(rrule.contains("INTERVAL=3"));
    assert!(rrule.contains("UNTIL=20250630"));
}

#[test]
fn test_until_with_weekly_preset() {
    let t = parse("Meeting @weekly until 2025-03-15");

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=WEEKLY"));
    assert!(rrule.contains("UNTIL=20250315"));
}

#[test]
fn test_until_with_monthly_preset() {
    let t = parse("Bill payment @monthly until 2026-01-01");

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("UNTIL=20260101"));
}

#[test]
fn test_until_with_yearly_preset() {
    let t = parse("Annual review @yearly until 2030-12-31");

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=YEARLY"));
    assert!(rrule.contains("UNTIL=20301231"));
}

#[test]
fn test_until_with_weekday_recurrence() {
    let t = parse("Team sync @every monday until 2025-06-30");

    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=WEEKLY"));
    assert!(rrule.contains("BYDAY=MO"));
    assert!(rrule.contains("UNTIL=20250630"));
}

#[test]
fn test_until_with_natural_date() {
    let t = parse("Exercise @daily until tomorrow");

    // Should parse until with natural date
    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();

    assert!(rrule.contains("FREQ=DAILY"));
    assert!(rrule.contains("UNTIL="));
}

#[test]
fn test_until_invalid_date_falls_back_to_summary() {
    let t = parse("Task @daily until invalid-date");

    // "until" and "invalid-date" should be part of summary since date parsing fails
    assert!(t.summary.contains("until"));
    assert!(t.summary.contains("invalid-date"));
}

#[test]
fn test_until_without_recurrence_in_summary() {
    // "until <date>" is NOT consumed without a recurrence (new behavior)
    // So it WILL appear in the summary
    let t = parse("Wait until 2025-12-31");

    assert!(t.rrule.is_none());
    // "until 2025-12-31" stays in summary since there's no recurrence
    assert_eq!(t.summary, "Wait until 2025-12-31");
}

#[test]
fn test_except_single_date() {
    let t = parse("Daily task @daily except 2025-01-15");

    // Should have recurrence
    assert!(t.rrule.is_some());

    // Should have one exdate
    assert_eq!(t.exdates.len(), 1);

    // Verify the exdate is correct
    match &t.exdates[0] {
        DateType::AllDay(date) => {
            assert_eq!(*date, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
        }
        _ => panic!("Expected AllDay date type"),
    }
}

#[test]
fn test_except_multiple_dates() {
    let t = parse("Meeting @weekly except 2025-01-15 except 2025-02-20");

    assert!(t.rrule.is_some());
    assert_eq!(t.exdates.len(), 2);

    // Verify both exdates
    match &t.exdates[0] {
        DateType::AllDay(date) => {
            assert_eq!(*date, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
        }
        _ => panic!("Expected AllDay date type"),
    }

    match &t.exdates[1] {
        DateType::AllDay(date) => {
            assert_eq!(*date, NaiveDate::from_ymd_opt(2025, 2, 20).unwrap());
        }
        _ => panic!("Expected AllDay date type"),
    }
}

#[test]
fn test_except_with_natural_date() {
    let t = parse("Task @daily except tomorrow");

    assert!(t.rrule.is_some());
    assert_eq!(t.exdates.len(), 1);

    // Should parse tomorrow as a date
    assert!(matches!(t.exdates[0], DateType::AllDay(_)));
}

#[test]
fn test_except_with_weekday() {
    // Test that weekday names are now supported in "except" clauses
    let t = parse("Task @daily except friday");

    assert!(t.rrule.is_some());
    // "friday" is now parsed as a weekday, converting to WEEKLY with inverted BYDAY
    assert_eq!(t.exdates.len(), 0);
    assert_eq!(t.summary, "Task");
    // Should convert @daily to @weekly excluding Friday
    let rrule = t.rrule.as_ref().unwrap();
    assert!(rrule.contains("FREQ=WEEKLY"));
    assert!(rrule.contains("BYDAY="));
    // Check that all days except Friday are included
    assert!(rrule.contains("MO"));
    assert!(rrule.contains("TU"));
    assert!(rrule.contains("WE"));
    assert!(rrule.contains("TH"));
    assert!(rrule.contains("SA"));
    assert!(rrule.contains("SU"));
    // Check that Friday is NOT in the BYDAY list (not just "FR" substring from "FREQ")
    let byday_part = rrule.split("BYDAY=").nth(1).unwrap_or("");
    assert!(!byday_part.contains("FR"));
}

#[test]
fn test_except_invalid_date_falls_back_to_summary() {
    let t = parse("Task @daily except not-a-date");

    // "except" and "not-a-date" should be part of summary since date parsing fails
    assert!(t.summary.contains("except"));
    assert!(t.summary.contains("not-a-date"));
}

#[test]
fn test_except_without_recurrence() {
    // Except without recurrence is just part of the summary
    let t = parse("Do everything except laundry");

    assert!(t.rrule.is_none());
    assert_eq!(t.exdates.len(), 0);
    assert!(t.summary.contains("except"));
    assert!(t.summary.contains("laundry"));
}

#[test]
fn test_until_and_except_combined() {
    let t = parse("Exercise @daily until 2025-12-31 except 2025-01-20 except 2025-02-15");

    // Should have recurrence with UNTIL
    assert!(t.rrule.is_some());
    let rrule = t.rrule.unwrap();
    assert!(rrule.contains("FREQ=DAILY"));
    assert!(rrule.contains("UNTIL=20251231"));

    // Should have two exception dates
    assert_eq!(t.exdates.len(), 2);
}

#[test]
fn test_until_case_insensitive() {
    let t1 = parse("Task @daily UNTIL 2025-12-31");
    let t2 = parse("Task @daily Until 2025-12-31");
    let t3 = parse("Task @daily until 2025-12-31");

    assert!(t1.rrule.is_some() && t1.rrule.as_ref().unwrap().contains("UNTIL="));
    assert!(t2.rrule.is_some() && t2.rrule.as_ref().unwrap().contains("UNTIL="));
    assert!(t3.rrule.is_some() && t3.rrule.as_ref().unwrap().contains("UNTIL="));
}

#[test]
fn test_except_case_insensitive() {
    let t1 = parse("Task @daily EXCEPT 2025-01-15");
    let t2 = parse("Task @daily Except 2025-01-15");
    let t3 = parse("Task @daily except 2025-01-15");

    assert_eq!(t1.exdates.len(), 1);
    assert_eq!(t2.exdates.len(), 1);
    assert_eq!(t3.exdates.len(), 1);
}

#[test]
fn test_prettify_recurrence_with_until() {
    use cfait::model::parser::prettify_recurrence;

    // Daily with until
    let rrule = "FREQ=DAILY;UNTIL=20251231";
    let pretty = prettify_recurrence(rrule);
    assert!(pretty.contains("@daily"));
    assert!(pretty.contains("until 2025-12-31"));

    // Weekly with until
    let rrule2 = "FREQ=WEEKLY;UNTIL=20250630";
    let pretty2 = prettify_recurrence(rrule2);
    assert!(pretty2.contains("@weekly"));
    assert!(pretty2.contains("until 2025-06-30"));

    // Custom interval with until
    let rrule3 = "FREQ=DAILY;INTERVAL=3;UNTIL=20250315";
    let pretty3 = prettify_recurrence(rrule3);
    assert!(pretty3.contains("@every 3 days"));
    assert!(pretty3.contains("until 2025-03-15"));
}

#[test]
fn test_prettify_recurrence_with_weekday_and_until() {
    use cfait::model::parser::prettify_recurrence;

    let rrule = "FREQ=WEEKLY;BYDAY=MO;UNTIL=20251231";
    let pretty = prettify_recurrence(rrule);
    assert!(pretty.contains("@every monday"));
    assert!(pretty.contains("until 2025-12-31"));
}

#[test]
fn test_prettify_recurrence_raw_format_no_until() {
    use cfait::model::parser::prettify_recurrence;

    // Note: The current prettify implementation doesn't check for COUNT,
    // so it will still use smart format and append until. This is a known limitation.
    let rrule = "FREQ=DAILY;COUNT=10;UNTIL=20251231";
    let pretty = prettify_recurrence(rrule);

    // Currently outputs "@daily until 2025-12-31" even with COUNT present
    // This could be improved in the future to detect COUNT and use rec: format
    assert!(pretty.contains("@daily"));
    assert!(pretty.contains("until 2025-12-31"));
}

#[test]
fn test_to_smart_string_with_exdates() {
    let mut t = Task::new("Task @daily", &HashMap::new(), None);

    // Manually add exdates
    t.exdates.push(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    ));
    t.exdates.push(DateType::AllDay(
        NaiveDate::from_ymd_opt(2025, 2, 20).unwrap(),
    ));

    let smart = t.to_smart_string();

    // Should include except keywords in smart string
    assert!(smart.contains("except 2025-01-15"));
    assert!(smart.contains("except 2025-02-20"));
}

#[test]
fn test_round_trip_with_until() {
    let input = "Morning walk @daily until 2025-12-31";
    let t = parse(input);
    let smart = t.to_smart_string();

    // Should contain the prettified recurrence with until
    assert!(smart.contains("@daily"));
    assert!(smart.contains("until 2025-12-31"));
}

#[test]
fn test_round_trip_with_except() {
    let input = "Meeting @weekly except 2025-01-15";
    let t = parse(input);
    let smart = t.to_smart_string();

    // Should contain the recurrence and except
    assert!(smart.contains("@weekly"));
    assert!(smart.contains("except 2025-01-15"));
}

#[test]
fn test_round_trip_with_until_and_except() {
    let input = "Task @daily until 2025-12-31 except 2025-01-15 except 2025-02-20";
    let t = parse(input);
    let smart = t.to_smart_string();

    // Should contain all components
    assert!(smart.contains("@daily"));
    assert!(smart.contains("until 2025-12-31"));
    assert!(smart.contains("except 2025-01-15"));
    assert!(smart.contains("except 2025-02-20"));
}

#[test]
fn test_until_only_applies_to_existing_rrule() {
    let t = Task::new("Task without recurrence", &HashMap::new(), None);
    assert!(t.rrule.is_none());

    // In the actual parser, "until <date>" gets consumed (consumed = 2) even if
    // there's no rrule to attach it to. So it won't appear in the summary.
    let input = "Task until 2025-12-31";
    let parsed = parse(input);

    // Should not have rrule
    assert!(parsed.rrule.is_none());

    // "until 2025-12-31" stays in summary since there's no recurrence (new behavior)
    assert_eq!(parsed.summary, "Task until 2025-12-31");
}

#[test]
fn test_complex_scenario_with_all_features() {
    // Test a complex task with priority, dates, recurrence, until, except, tags, location
    let input = "Yoga class !2 @2025-01-20 ^2025-01-15 @every monday until 2025-12-31 except 2025-03-10 except 2025-07-04 #health @@studio ~1h";
    let t = parse(input);

    // Verify all components
    assert_eq!(t.summary, "Yoga class");
    assert_eq!(t.priority, 2);
    assert!(t.due.is_some());
    assert!(t.dtstart.is_some());
    assert!(t.rrule.is_some());

    let rrule = t.rrule.unwrap();
    assert!(rrule.contains("FREQ=WEEKLY"));
    assert!(rrule.contains("BYDAY=MO"));
    assert!(rrule.contains("UNTIL=20251231"));

    assert_eq!(t.exdates.len(), 2);
    assert!(t.categories.contains(&"health".to_string()));
    assert_eq!(t.location, Some("studio".to_string()));
    assert_eq!(t.estimated_duration, Some(60));
}

#[test]
fn test_syntax_highlighting_for_until() {
    use cfait::model::parser::{SyntaxType, tokenize_smart_input};

    let input = "Task @daily until 2025-12-31";
    let tokens = tokenize_smart_input(input);

    // Should have a Recurrence token that includes "until"
    let recurrence_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == SyntaxType::Recurrence)
        .collect();

    // There should be recurrence tokens
    assert!(!recurrence_tokens.is_empty());
}

#[test]
fn test_syntax_highlighting_for_except() {
    use cfait::model::parser::{SyntaxType, tokenize_smart_input};

    let input = "Task @daily except 2025-01-15";
    let tokens = tokenize_smart_input(input);

    // Should have recurrence tokens
    let recurrence_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == SyntaxType::Recurrence)
        .collect();

    assert!(!recurrence_tokens.is_empty());
}

#[test]
fn test_recurrence_without_explicit_date_simple() {
    // New behavior: recurrence without dates auto-sets both start and due to first occurrence
    let t = parse("Morning routine @daily");

    // The task parses correctly and gets default dates
    assert_eq!(t.summary, "Morning routine");
    assert!(t.rrule.is_some());
    assert!(t.rrule.as_ref().unwrap().contains("FREQ=DAILY"));

    // Should have both start and due dates (both default to first occurrence)
    assert!(
        t.dtstart.is_some(),
        "Recurrence without explicit dates should auto-set start date"
    );
    assert!(
        t.due.is_some(),
        "Recurrence without explicit dates should auto-set due date"
    );
}

#[test]
fn test_recurrence_without_explicit_date_with_until() {
    // New behavior: recurrence with until but no explicit dates auto-sets both start and due
    let t = parse("Morning routine @daily until 2030-12-31");

    assert_eq!(t.summary, "Morning routine");
    assert!(t.rrule.is_some());
    assert!(t.rrule.as_ref().unwrap().contains("FREQ=DAILY"));
    assert!(t.rrule.as_ref().unwrap().contains("UNTIL=20301231"));

    // Should have both start and due dates (defaults to first occurrence)
    assert!(
        t.dtstart.is_some(),
        "Recurrence with until should have default start date"
    );
    assert!(
        t.due.is_some(),
        "Recurrence with until should have default due date"
    );
}

#[test]
fn test_recurrence_without_explicit_date_with_except() {
    // New behavior: recurrence with except but no explicit start date defaults to starting today
    let t = parse("Morning routine @daily except 2030-12-25");

    assert_eq!(t.summary, "Morning routine");
    assert!(t.rrule.is_some());
    assert_eq!(t.exdates.len(), 1);

    // Should have a start date (defaults to today)
    assert!(
        t.dtstart.is_some(),
        "Recurrence with except should have a default start date"
    );
}

#[test]
fn test_prettify_except_weekdays_single() {
    // Single weekday exception should prettify nicely
    let t = parse("Task @daily except monday");
    let smart = t.to_smart_string();

    assert!(
        smart.contains("@daily except monday"),
        "Should prettify to '@daily except monday', got: {}",
        smart
    );
}

#[test]
fn test_prettify_except_weekdays_multiple() {
    // Multiple weekday exceptions should prettify nicely
    let t = parse("Task @daily except monday,tuesday");
    let smart = t.to_smart_string();

    assert!(
        smart.contains("@daily except monday,tuesday"),
        "Should prettify to '@daily except monday,tuesday', got: {}",
        smart
    );
}

#[test]
fn test_prettify_except_weekdays_with_until() {
    // Exception with until should prettify both
    let t = parse("Task @daily until 2025-12-31 except monday");
    let smart = t.to_smart_string();

    assert!(smart.contains("@daily"), "Should contain @daily");
    assert!(
        smart.contains("until 2025-12-31"),
        "Should contain until clause"
    );
    assert!(
        smart.contains("except monday"),
        "Should contain except clause"
    );
}

#[test]
fn test_prettify_weekday_only_no_except() {
    // Single weekday should use @every format, not except
    let t = parse("Meeting @every monday");
    let smart = t.to_smart_string();

    assert!(
        smart.contains("@every monday"),
        "Should prettify to '@every monday', got: {}",
        smart
    );
    assert!(!smart.contains("except"), "Should not contain 'except'");
}

#[test]
fn test_every_multiple_weekdays_two() {
    // Two weekdays should use @every format
    let t = parse("Gym @every monday,wednesday");
    let smart = t.to_smart_string();

    assert!(
        t.rrule.as_ref().unwrap().contains("FREQ=WEEKLY"),
        "Should have WEEKLY frequency"
    );
    assert!(
        t.rrule.as_ref().unwrap().contains("BYDAY=MO,WE"),
        "Should have MO,WE in BYDAY"
    );
    assert!(
        smart.contains("@every monday,wednesday"),
        "Should prettify to '@every monday,wednesday', got: {}",
        smart
    );
}

#[test]
fn test_every_multiple_weekdays_three() {
    // Three weekdays should use @every format
    let t = parse("Meeting @every monday,tuesday,friday");
    let smart = t.to_smart_string();

    assert!(
        t.rrule.as_ref().unwrap().contains("FREQ=WEEKLY"),
        "Should have WEEKLY frequency"
    );
    assert!(
        t.rrule.as_ref().unwrap().contains("BYDAY=MO,TU,FR"),
        "Should have MO,TU,FR in BYDAY"
    );
    assert!(
        smart.contains("@every monday,tuesday,friday"),
        "Should prettify to '@every monday,tuesday,friday', got: {}",
        smart
    );
}

#[test]
fn test_every_multiple_weekdays_with_until() {
    // Multiple weekdays with until should preserve both
    let t = parse("Class @every monday,wednesday until 2025-12-31");
    let smart = t.to_smart_string();

    assert!(
        t.rrule.as_ref().unwrap().contains("UNTIL=20251231"),
        "Should have UNTIL clause"
    );
    assert!(
        smart.contains("@every monday,wednesday"),
        "Should contain '@every monday,wednesday'"
    );
    assert!(
        smart.contains("until 2025-12-31"),
        "Should contain 'until 2025-12-31'"
    );
}

#[test]
fn test_every_all_seven_weekdays() {
    // All 7 weekdays should simplify to @daily
    let t = parse("Task @every monday,tuesday,wednesday,thursday,friday,saturday,sunday");
    let smart = t.to_smart_string();

    assert!(
        t.rrule.as_ref().unwrap().contains("FREQ=WEEKLY"),
        "Should have WEEKLY frequency"
    );
    assert!(
        smart.contains("@daily"),
        "Should simplify to '@daily', got: {}",
        smart
    );
    assert!(
        !smart.contains("except"),
        "Should not contain 'except' when all days present"
    );
}

#[test]
fn test_every_versus_except_threshold() {
    // 2-3 days use @every format
    let t_two = parse("Task @every monday,tuesday");
    assert!(t_two.to_smart_string().contains("@every monday,tuesday"));

    let t_three = parse("Task @every monday,tuesday,wednesday");
    assert!(
        t_three
            .to_smart_string()
            .contains("@every monday,tuesday,wednesday")
    );

    // 4+ days use @daily except format (cleaner to list exclusions)
    let t_four = parse("Task @daily except monday,tuesday,wednesday");
    assert!(t_four.to_smart_string().contains("@daily except"));
}

#[test]
fn test_syntax_highlighting_for_multiple_weekdays() {
    use cfait::model::parser::{SyntaxType, tokenize_smart_input};

    let input = "Meeting @every monday,tuesday,friday #work";
    let tokens = tokenize_smart_input(input);

    // Should have recurrence tokens
    let recurrence_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == SyntaxType::Recurrence)
        .collect();

    assert_eq!(recurrence_tokens.len(), 1, "Should have 1 recurrence token");

    // The recurrence token should span @every and the weekday list
    let rec_token = recurrence_tokens[0];
    let rec_text = &input[rec_token.start..rec_token.end];
    assert_eq!(
        rec_text, "@every monday,tuesday,friday",
        "Recurrence token should include @every and all weekdays"
    );
}

#[test]
fn test_except_months_basic() {
    let input = "Water plants @monthly except dec,jan,feb";
    let task = parse(input);

    assert!(task.rrule.is_some());
    let rrule = task.rrule.unwrap();

    // Should have BYMONTH with 9 months (all except dec, jan, feb)
    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("BYMONTH="));

    // Should include months 3-11 (mar-nov)
    assert!(rrule.contains("BYMONTH=3,4,5,6,7,8,9,10,11"));
}

#[test]
fn test_except_months_many_excluded() {
    let input = "Water peyote @monthly except oct,nov,dec,jan,feb,mar";
    let task = parse(input);

    assert!(task.rrule.is_some());
    let rrule = task.rrule.unwrap();

    // Should have BYMONTH with 6 months (apr-sep)
    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("BYMONTH=4,5,6,7,8,9"));
}

#[test]
fn test_prettify_except_months() {
    use cfait::model::parser::prettify_recurrence;

    // 5 excluded months (7 included) -> should show except
    let rrule = "FREQ=MONTHLY;BYMONTH=3,4,5,6,7,8,9,10,11";
    let pretty = prettify_recurrence(rrule);
    assert_eq!(pretty, "@monthly except jan,feb,dec");
}

#[test]
fn test_prettify_except_months_few_excluded() {
    use cfait::model::parser::prettify_recurrence;

    // 2 excluded months (10 included) -> should show except
    let rrule = "FREQ=MONTHLY;BYMONTH=1,2,3,4,5,6,7,8,9,10";
    let pretty = prettify_recurrence(rrule);
    assert_eq!(pretty, "@monthly except nov,dec");
}

#[test]
fn test_prettify_except_months_many_excluded() {
    use cfait::model::parser::prettify_recurrence;

    // 8 excluded months (4 included) -> should still show except format
    let rrule = "FREQ=MONTHLY;BYMONTH=6,7,8,9";
    let pretty = prettify_recurrence(rrule);
    assert_eq!(pretty, "@monthly except jan,feb,mar,apr,may,oct,nov,dec");
}

#[test]
fn test_prettify_except_months_all_included() {
    use cfait::model::parser::prettify_recurrence;

    // All 12 months -> should show @monthly
    let rrule = "FREQ=MONTHLY;BYMONTH=1,2,3,4,5,6,7,8,9,10,11,12";
    let pretty = prettify_recurrence(rrule);
    assert_eq!(pretty, "@monthly");
}

#[test]
fn test_round_trip_except_months() {
    // Parse input with month exceptions
    let input = "Task @monthly except jan,feb,dec";
    let task = parse(input);

    assert!(task.rrule.is_some());
    let rrule = task.rrule.as_ref().unwrap();

    // Prettify it back
    use cfait::model::parser::prettify_recurrence;
    let pretty = prettify_recurrence(rrule);

    // Should get back a similar format (9 months included = 3 excluded)
    assert_eq!(pretty, "@monthly except jan,feb,dec");
}

#[test]
fn test_except_months_with_until() {
    let input = "Task @monthly except jan,feb until 2025-12-31";
    let task = parse(input);

    assert!(task.rrule.is_some());
    let rrule = task.rrule.unwrap();

    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("BYMONTH="));
    assert!(rrule.contains("UNTIL=20251231"));
}

#[test]
fn test_prettify_except_months_with_until() {
    use cfait::model::parser::prettify_recurrence;

    let rrule = "FREQ=MONTHLY;BYMONTH=3,4,5,6,7,8,9,10,11;UNTIL=20251231";
    let pretty = prettify_recurrence(rrule);
    assert_eq!(pretty, "@monthly until 2025-12-31 except jan,feb,dec");
}

#[test]
fn test_user_scenario_water_peyote() {
    // This test replicates the exact user scenario reported:
    // Input: "Water peyote @monthly except oct,nov,dec,jan,feb,mar"
    // Expected edit display: Should show the except format back

    let input = "Water peyote @monthly except oct,nov,dec,jan,feb,mar";
    let task = parse(input);

    assert_eq!(task.summary, "Water peyote");
    assert!(task.rrule.is_some());

    // Should have BYMONTH=4,5,6,7,8,9 (6 included months: Apr-Sep)
    let rrule = task.rrule.as_ref().unwrap();
    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("BYMONTH=4,5,6,7,8,9"));

    // Now test the round-trip: convert back to smart string
    use cfait::model::parser::prettify_recurrence;
    let pretty = prettify_recurrence(rrule);

    // With 6 months included (6 excluded), should show except format
    // The 6 excluded months are: jan,feb,mar,oct,nov,dec
    assert_eq!(pretty, "@monthly except jan,feb,mar,oct,nov,dec");

    // Also test with fewer exclusions
    let input2 = "Water peyote @monthly except oct,nov,dec";
    let task2 = parse(input2);
    let rrule2 = task2.rrule.as_ref().unwrap();
    let pretty2 = prettify_recurrence(rrule2);
    assert_eq!(pretty2, "@monthly except oct,nov,dec");
}

#[test]
fn test_prettify_except_months_various_counts() {
    use cfait::model::parser::prettify_recurrence;

    // Test various exclusion counts - all should show except format

    // 6 included months (6 excluded)
    let rrule_6_included = "FREQ=MONTHLY;BYMONTH=1,2,3,4,5,6";
    let pretty_6 = prettify_recurrence(rrule_6_included);
    assert_eq!(pretty_6, "@monthly except jul,aug,sep,oct,nov,dec");

    // 5 included months (7 excluded)
    let rrule_5_included = "FREQ=MONTHLY;BYMONTH=1,2,3,4,5";
    let pretty_5 = prettify_recurrence(rrule_5_included);
    assert_eq!(pretty_5, "@monthly except jun,jul,aug,sep,oct,nov,dec");

    // 1 included month (11 excluded)
    let rrule_1_included = "FREQ=MONTHLY;BYMONTH=6";
    let pretty_1 = prettify_recurrence(rrule_1_included);
    assert_eq!(
        pretty_1,
        "@monthly except jan,feb,mar,apr,may,jul,aug,sep,oct,nov,dec"
    );
}

#[test]
fn test_user_scenario_eight_excluded_months() {
    // User reported: "test task @monthly except oct,nov,dec,jan,feb,mar,apr,may"
    // Was showing: "test task ^2026-01-03 @2026-01-03 rec:FREQ=MONTHLY;BYMONTH=6,7,8,9"
    // Should show: "test task @monthly except jan,feb,mar,apr,may,oct,nov,dec"

    let input = "test task @monthly except oct,nov,dec,jan,feb,mar,apr,may";
    let task = parse(input);

    assert_eq!(task.summary, "test task");
    assert!(task.rrule.is_some());

    // Should have BYMONTH=6,7,8,9 (4 included months: Jun-Sep)
    let rrule = task.rrule.as_ref().unwrap();
    assert!(rrule.contains("FREQ=MONTHLY"));
    assert!(rrule.contains("BYMONTH=6,7,8,9"));

    // Now test the round-trip: convert back to smart string
    use cfait::model::parser::prettify_recurrence;
    let pretty = prettify_recurrence(rrule);

    // With 4 months included (8 excluded), should show except format
    // The 8 excluded months are: jan,feb,mar,apr,may,oct,nov,dec
    assert_eq!(pretty, "@monthly except jan,feb,mar,apr,may,oct,nov,dec");
}
