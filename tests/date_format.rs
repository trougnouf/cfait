// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for the `date_format` setting: how typed dates are interpreted.
//!
//! Parsing tests build their own `ParserLexicon` so they never race on the
//! global `LEXICON`; one dedicated test exercises the global setter.
use cfait::config::DateFormat;
use cfait::model::DateType;
use cfait::model::parser::ParserLexicon;
use chrono::NaiveDate;

fn lex_with(formats: &[&str]) -> ParserLexicon {
    let mut lex = ParserLexicon::build();
    lex.configured_date_formats = formats.iter().map(|s| s.to_string()).collect();
    lex
}

fn all_day(y: i32, m: u32, d: u32) -> DateType {
    DateType::AllDay(NaiveDate::from_ymd_opt(y, m, d).unwrap())
}

#[test]
fn iso_and_compact_always_win() {
    // Even with DMY forced, ISO 8601 and compact dates parse unambiguously.
    let lex = lex_with(&["%d-%m-%Y", "%d/%m/%Y"]);
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("2026-01-02", &lex),
        Some(all_day(2026, 1, 2))
    );
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("20260102", &lex),
        Some(all_day(2026, 1, 2))
    );
}

#[test]
fn each_explicit_format_parses_both_separators() {
    // Note: for `Ydm` the dash variant is not a valid ISO date (day 25),
    // so it cannot be swallowed by the ISO rule.
    let cases = [
        (
            DateFormat::Ymd,
            "2026-01-02",
            "2026/01/02",
            all_day(2026, 1, 2),
        ),
        (
            DateFormat::Dmy,
            "02-01-2026",
            "02/01/2026",
            all_day(2026, 1, 2),
        ),
        (
            DateFormat::Mdy,
            "01-02-2026",
            "01/02/2026",
            all_day(2026, 1, 2),
        ),
        (
            DateFormat::Ydm,
            "2026-25-01",
            "2026/25/01",
            all_day(2026, 1, 25),
        ),
        (
            DateFormat::Myd,
            "01-2026-02",
            "01/2026/02",
            all_day(2026, 1, 2),
        ),
        (
            DateFormat::Dym,
            "02-2026-01",
            "02/2026/01",
            all_day(2026, 1, 2),
        ),
    ];
    for (fmt, dash, slash, expected) in cases {
        // Both separator variants are registered together.
        let formats = fmt.chrono_formats();
        let lex = lex_with(&formats);
        assert_eq!(
            cfait::model::parser::parse_smart_date_with_lex(dash, &lex),
            Some(expected.clone()),
            "{fmt:?} dash variant {dash}"
        );
        assert_eq!(
            cfait::model::parser::parse_smart_date_with_lex(slash, &lex),
            Some(expected),
            "{fmt:?} slash variant {slash}"
        );
    }
}

#[test]
fn ambiguous_input_follows_the_chosen_order() {
    // 01/02/2026 is Feb 1 in DMY (day first) and Jan 2 in MDY (month first).
    let dmy = lex_with(&["%d-%m-%Y", "%d/%m/%Y"]);
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("01/02/2026", &dmy),
        Some(all_day(2026, 2, 1))
    );
    let mdy = lex_with(&["%m-%d-%Y", "%m/%d/%Y"]);
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("01/02/2026", &mdy),
        Some(all_day(2026, 1, 2))
    );
}

#[test]
fn invalid_dates_are_rejected() {
    let dmy = lex_with(&["%d-%m-%Y", "%d/%m/%Y"]);
    // Feb 31 does not exist.
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("31/02/2026", &dmy),
        None
    );
    // Month 13 does not exist.
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("01/13/2026", &dmy),
        None
    );
}

#[test]
fn auto_adds_no_formats() {
    assert!(DateFormat::Auto.chrono_formats().is_empty());
    // With no configured formats and the default (en) lexicon, 01/02/2026
    // is not a known format and does not parse as a date.
    let lex = lex_with(&[]);
    assert_eq!(
        cfait::model::parser::parse_smart_date_with_lex("01/02/2026", &lex),
        None
    );
}

#[test]
fn name_and_fromstr_roundtrip() {
    for fmt in [
        DateFormat::Auto,
        DateFormat::Ymd,
        DateFormat::Dmy,
        DateFormat::Mdy,
        DateFormat::Ydm,
        DateFormat::Myd,
        DateFormat::Dym,
    ] {
        let name = fmt.name();
        assert_eq!(name.parse::<DateFormat>().unwrap(), fmt);
        // Case-insensitive, like other config enums.
        assert_eq!(name.to_uppercase().parse::<DateFormat>().unwrap(), fmt);
    }
    assert!("nonsense".parse::<DateFormat>().is_err());
}

#[test]
fn config_roundtrip_keeps_date_format() {
    // Config is serialized as TOML, so round-trip through TOML.
    let mut cfg = cfait::config::Config::default();
    assert_eq!(cfg.date_format, DateFormat::Auto);
    cfg.date_format = DateFormat::Dmy;
    let toml_str = toml::to_string(&cfg).unwrap();
    assert!(toml_str.contains("date_format = \"dmy\""));
    let back: cfait::config::Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(back.date_format, DateFormat::Dmy);

    // Missing key falls back to the default (old configs).
    let stripped = toml_str.replace("date_format = \"dmy\"\n", "");
    let back: cfait::config::Config = toml::from_str(&stripped).unwrap();
    assert_eq!(back.date_format, DateFormat::Auto);
}

#[test]
fn global_setter_feeds_the_lexicon() {
    // The only test in this file touching the global LEXICON.
    cfait::model::parser::set_configured_date_formats(vec!["%d/%m/%Y"]);
    cfait::model::parser::rebuild_lexicon();
    assert_eq!(
        cfait::model::parser::parse_smart_date("02/01/2026"),
        Some(all_day(2026, 1, 2))
    );
    // ISO keeps working through the global path too.
    assert_eq!(
        cfait::model::parser::parse_smart_date("2026-01-02"),
        Some(all_day(2026, 1, 2))
    );

    // Restore the default so later tests in this process are unaffected.
    cfait::model::parser::set_configured_date_formats(Vec::new());
    cfait::model::parser::rebuild_lexicon();
}
