// SPDX-License-Identifier: GPL-3.0-or-later
//! `is:` autocomplete: typing the status-filter prefix in the task input
//! offers the filter values the matcher accepts, filtered by what has been
//! typed so far, case-insensitively, with the replacement range covering
//! exactly the word being typed.
use cfait::context::TestContext;
use cfait::model::autocomplete::suggest;
use cfait::store::TaskStore;
use std::collections::HashMap;
use std::sync::Arc;

fn store() -> TaskStore {
    TaskStore::new(Arc::new(TestContext::new()))
}

fn suggest_at(input: &str, cursor: usize) -> Option<(std::ops::Range<usize>, Vec<String>)> {
    suggest(input, cursor, &store(), &HashMap::new(), &[], &[])
        .map(|(range, suggs)| (range, suggs.into_iter().map(|s| s.replacement).collect()))
}

fn suggest_with_descriptions(input: &str, cursor: usize) -> Option<Vec<(String, String)>> {
    suggest(input, cursor, &store(), &HashMap::new(), &[], &[]).map(|(_, suggs)| {
        suggs
            .into_iter()
            .map(|s| (s.replacement, s.description))
            .collect()
    })
}

#[test]
fn bare_prefix_lists_every_filter_value_in_documented_order() {
    let (range, values) = suggest_at("is:", 3).expect("a bare is: prefix must offer the filters");
    assert_eq!(range, 0..3, "the range must cover exactly the typed word");
    let expected: Vec<String> = vec![
        "is:done",
        "is:started",
        "is:ongoing",
        "is:active",
        "is:ready",
        "is:blocked",
        "is:note",
        "is:page",
        "is:journal",
        "is:flat",
        "is:doc",
        "is:permanent",
        "is:canceled",
        "is:cancelled",
        "is:recurrent",
        "is:pinned",
        "is:archived",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(values, expected);
}

#[test]
fn typed_value_filters_the_candidates() {
    let (_, values) = suggest_at("is:do", 5).expect("is:do must match done and doc");
    assert_eq!(values, vec!["is:done".to_string(), "is:doc".to_string()]);

    let (_, values) = suggest_at("is:p", 4).expect("is:p must match page, permanent, pinned");
    assert_eq!(
        values,
        vec![
            "is:page".to_string(),
            "is:permanent".to_string(),
            "is:pinned".to_string(),
        ]
    );
}

#[test]
fn matching_is_case_insensitive() {
    let (_, values) = suggest_at("IS:DO", 5).expect("uppercase input must match");
    assert_eq!(
        values,
        vec!["is:done".to_string(), "is:doc".to_string()],
        "replacements keep the canonical lowercase form"
    );
}

#[test]
fn works_mid_line_and_replaces_only_the_word() {
    let input = "Garden path ideas is:pa";
    let (range, values) =
        suggest_at(input, input.len()).expect("a mid-line is: word must offer page");
    assert_eq!(range, 18..23, "the range must cover only the is: word");
    assert_eq!(values, vec!["is:page".to_string()]);
}

#[test]
fn exact_value_is_still_suggested() {
    let (_, values) = suggest_at("is:done", 7).expect("a complete value keeps suggesting itself");
    assert_eq!(values, vec!["is:done".to_string()]);
}

#[test]
fn unknown_value_offers_nothing() {
    assert!(suggest_at("is:zzz", 6).is_none());
}

#[test]
fn negated_filter_is_not_autocompleted() {
    // Consistent with `-#tag`: a leading `-` disables suggestions.
    assert!(suggest_at("-is:do", 6).is_none());
}

#[test]
fn suggestions_carry_a_short_description() {
    let pairs =
        suggest_with_descriptions("is:", 3).expect("a bare is: prefix must offer the filters");
    let by_value: HashMap<&str, &str> = pairs
        .iter()
        .map(|(value, desc)| (value.as_str(), desc.as_str()))
        .collect();
    assert_eq!(by_value.get("is:done"), Some(&"Done"));
    assert_eq!(by_value.get("is:started"), Some(&"Started (in process)"));
    assert_eq!(by_value.get("is:active"), Some(&"Not done"));
    assert_eq!(by_value.get("is:journal"), Some(&"Wiki page"));
    assert_eq!(by_value.get("is:pinned"), Some(&"Pinned"));
    assert_eq!(by_value.get("is:archived"), Some(&"Archived"));
}
