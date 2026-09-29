// SPDX-License-Identifier: GPL-3.0-or-later
//! Robustness tests for the ICS adapter: malformed input, RFC 5545
//! case-insensitivity, TZID handling, and sibling-component isolation.

use cfait::model::{Alarm, AlarmTrigger, DateType, Task};
use chrono::{DateTime, NaiveDate, Utc};

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    chrono::DateTime::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, 0)
            .unwrap(),
        Utc,
    )
}

fn ics(body: &str) -> String {
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n{body}\r\nEND:VCALENDAR")
}

fn parse(body: &str) -> Task {
    Task::from_ics(&ics(body), "etag".into(), "href".into(), "cal".into()).unwrap()
}

/// A DUE value whose 8th byte falls inside a multi-byte UTF-8 character used
/// to panic the parser on a char boundary.
#[test]
fn non_ascii_due_does_not_panic() {
    // é = 2 bytes; "éééaé" is 10 bytes and byte 8 is inside the last é
    let t = parse("BEGIN:VTODO\r\nUID:u1\r\nSUMMARY:Water the tomatoes\r\nDUE:éééaé\r\nEND:VTODO");
    assert_eq!(t.due, None);
}

#[test]
fn max_duration_without_min_survives_roundtrip() {
    let mut t = Task::new("Read a chapter", &std::collections::HashMap::new(), None);
    t.estimated_duration_max = Some(90);
    let ics = t.to_ics();
    assert!(ics.contains("X-CFAIT-ESTIMATED-DURATION-MAX:PT90M"));
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "c".into()).unwrap();
    assert_eq!(parsed.estimated_duration_max, Some(90));
    assert_eq!(parsed.estimated_duration, None);
}

#[test]
fn missing_uid_gets_deterministic_fallback() {
    let t = parse("BEGIN:VTODO\r\nSUMMARY:Feed the fish\r\nEND:VTODO");
    assert_eq!(t.uid, "cfait-nouid-href-Feed the fish");
}

/// A sibling VTODO carrying RECURRENCE-ID must not leak its relations or
/// alarms into the master task.
#[test]
fn override_sibling_does_not_leak_relations_or_alarms() {
    let body = "BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        RRULE:FREQ=WEEKLY\r\n\
        END:VTODO\r\n\
        BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        RECURRENCE-ID:20260921T090000Z\r\n\
        RELATED-TO:leaky-uid\r\n\
        BEGIN:VALARM\r\n\
        ACTION:DISPLAY\r\n\
        TRIGGER:-PT5M\r\n\
        END:VALARM\r\n\
        END:VTODO";
    let t = parse(body);
    assert!(t.related_to.is_empty());
    assert!(t.dependencies.is_empty());
    // An un-typed RELATED-TO lands in parent_uid, so this also catches a
    // leak from the override's RELATED-TO:leaky-uid.
    assert!(t.parent_uid.is_none());
    assert!(t.alarms.is_empty());
}

/// The master's own relations and alarms must still be parsed.
#[test]
fn master_relations_and_alarms_still_parse() {
    let body = "BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        RELATED-TO:parent-uid\r\n\
        BEGIN:VALARM\r\n\
        ACTION:DISPLAY\r\n\
        TRIGGER:-PT5M\r\n\
        END:VALARM\r\n\
        END:VTODO\r\n\
        BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        RECURRENCE-ID:20260921T090000Z\r\n\
        RELATED-TO:leaky-uid\r\n\
        END:VTODO";
    let t = parse(body);
    // The first un-typed RELATED-TO is routed to parent_uid by design.
    assert_eq!(t.parent_uid, Some("parent-uid".to_string()));
    assert!(t.related_to.is_empty());
    assert_eq!(t.alarms.len(), 1);
}

/// Property and parameter names are case-insensitive per RFC 5545.
#[test]
fn lowercase_property_names_parse() {
    let body = "BEGIN:VTODO\r\n\
        uid:lower-uid\r\n\
        summary:Water the tomatoes\r\n\
        due:20261001\r\n\
        dtstart;value=date:20260924\r\n\
        categories:gardening,reading\r\n\
        X-ESTIMATED-DURATION:PT30M\r\n\
        END:VTODO";
    let t = parse(body);
    assert_eq!(t.uid, "lower-uid");
    assert_eq!(t.summary, "Water the tomatoes");
    assert_eq!(
        t.due,
        Some(DateType::AllDay(
            NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
        ))
    );
    assert_eq!(
        t.dtstart,
        Some(DateType::AllDay(
            NaiveDate::from_ymd_opt(2026, 9, 24).unwrap()
        ))
    );
    assert_eq!(
        t.categories,
        vec!["gardening".to_string(), "reading".to_string()]
    );
    assert_eq!(t.estimated_duration, Some(30));
}

/// A TZID'd DUE is converted to UTC with the zone's offset.
#[test]
fn tzid_due_converts_to_utc() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Sip tea\r\nDUE;TZID=America/New_York:20260701T090000\r\nEND:VTODO",
    );
    // July 1 is EDT (UTC-4)
    assert_eq!(t.due, Some(DateType::Specific(utc(2026, 7, 1, 13, 0))));
}

/// A quoted TZID value is accepted too.
#[test]
fn quoted_tzid_due_converts_to_utc() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Sip tea\r\nDUE;TZID=\"America/New_York\":20260701T090000\r\nEND:VTODO",
    );
    assert_eq!(t.due, Some(DateType::Specific(utc(2026, 7, 1, 13, 0))));
}

/// A TZID'd DUE in a DST gap resolves with the same convention as
/// safe_local_to_utc (advance one hour, subtract one hour in UTC).
#[test]
fn tzid_due_in_dst_gap_does_not_panic_or_drop() {
    // 2026-03-08: US springs forward, 02:00-03:00 does not exist
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Stretch\r\nDUE;TZID=America/New_York:20260308T023000\r\nEND:VTODO",
    );
    // 03:30 EDT = 07:30 UTC, minus 1h = 06:30 UTC
    assert_eq!(t.due, Some(DateType::Specific(utc(2026, 3, 8, 6, 30))));
}

/// A TZID'd DTSTART converts to UTC as well.
#[test]
fn tzid_dtstart_converts_to_utc() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Sip tea\r\nDTSTART;TZID=Europe/Paris:20260115T080000\r\nEND:VTODO",
    );
    // January 15 is CET (UTC+1)
    assert_eq!(t.dtstart, Some(DateType::Specific(utc(2026, 1, 15, 7, 0))));
}

/// A floating RRULE UNTIL is converted to UTC using the DTSTART timezone.
#[test]
fn floating_rrule_until_converts_with_dtstart_tzid() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Morning stretch\r\nDTSTART;TZID=America/New_York:20260701T090000\r\nRRULE:FREQ=DAILY;UNTIL=20260703T090000\r\nEND:VTODO",
    );
    assert_eq!(
        t.rrule.as_deref(),
        Some("FREQ=DAILY;UNTIL=20260703T130000Z")
    );
}

/// A date-only RRULE UNTIL is left untouched.
#[test]
fn date_only_rrule_until_untouched() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Morning stretch\r\nDTSTART;TZID=America/New_York:20260701T090000\r\nRRULE:FREQ=DAILY;UNTIL=20260703\r\nEND:VTODO",
    );
    assert_eq!(t.rrule.as_deref(), Some("FREQ=DAILY;UNTIL=20260703"));
}

/// EXDATE with a TZID converts each value to UTC.
#[test]
fn tzid_exdate_converts_to_utc() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Morning stretch\r\nDTSTART;TZID=America/New_York:20260701T090000\r\nRRULE:FREQ=DAILY;COUNT=5\r\nEXDATE;TZID=America/New_York:20260702T090000,20260703T090000\r\nEND:VTODO",
    );
    assert_eq!(
        t.exdates,
        vec![
            DateType::Specific(utc(2026, 7, 2, 13, 0)),
            DateType::Specific(utc(2026, 7, 3, 13, 0)),
        ]
    );
}

/// A TRIGGER with seconds rounds up to the nearest minute instead of
/// firing immediately.
#[test]
fn trigger_with_seconds_rounds_up() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Check the oven\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:PT30S\r\nEND:VALARM\r\nEND:VTODO",
    );
    assert_eq!(t.alarms.len(), 1);
    assert!(matches!(t.alarms[0].trigger, AlarmTrigger::Relative(1)));
}

#[test]
fn trigger_with_full_minutes_rounds_up() {
    let t = parse(
        "BEGIN:VTODO\r\nUID:u\r\nSUMMARY:Check the oven\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT90S\r\nEND:VALARM\r\nEND:VTODO",
    );
    assert!(matches!(t.alarms[0].trigger, AlarmTrigger::Relative(-2)));
}

/// The VJOURNAL path shares the case-insensitive raw property scan.
#[test]
fn journal_lowercase_properties_parse() {
    let body = "BEGIN:VJOURNAL\r\n\
        uid:journal-1\r\n\
        summary:Reading log\r\n\
        dtstart:20260920\r\n\
        END:VJOURNAL";
    let t = parse(body);
    assert!(t.is_journal);
    assert_eq!(t.uid, "journal-1");
    assert_eq!(t.summary, "Reading log");
    assert_eq!(
        t.dtstart,
        Some(DateType::AllDay(
            NaiveDate::from_ymd_opt(2026, 9, 20).unwrap()
        ))
    );
}

/// The icalendar crate already unescapes Text properties (SUMMARY,
/// DESCRIPTION, LOCATION) when parsing, so the adapter must not unescape
/// them a second time: literal backslash sequences such as `\n` in a
/// recipe note or `C:\new` in a path would be corrupted on every round trip.
#[test]
fn vtodo_text_with_literal_backslashes_roundtrips() {
    let mut t = Task::new(
        "Water the tomatoes",
        &std::collections::HashMap::new(),
        None,
    );
    t.summary = "Soak the beans \\n overnight".to_string();
    t.description = "recipe: 2 cups flour, 1 tsp salt\\nrest 10 min".to_string();
    t.locations = vec!["greenhouse, row C:\\new".to_string()];
    let ics = t.to_ics();
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "c".into()).unwrap();
    assert_eq!(parsed.summary, t.summary);
    assert_eq!(parsed.description, t.description);
    assert_eq!(parsed.locations, t.locations);
}

/// A category containing a literal comma is escaped as `\,` on the wire;
/// once the crate unescapes it, splitting the value again would tear the
/// category in two.
#[test]
fn category_with_literal_comma_roundtrips() {
    let mut t = Task::new("Weed the beds", &std::collections::HashMap::new(), None);
    t.categories = vec!["garden, front bed".to_string()];
    let ics = t.to_ics();
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "c".into()).unwrap();
    assert_eq!(parsed.categories, vec!["garden, front bed".to_string()]);
}

/// Alarm DESCRIPTION is read from raw lines (not crate properties), so it
/// still needs unescaping — and to_ics escapes it, so round trips must be
/// lossless for commas and newlines.
#[test]
fn alarm_description_with_newline_and_comma_roundtrips() {
    let mut t = Task::new("Check the oven", &std::collections::HashMap::new(), None);
    let mut alarm = Alarm::new_relative(5);
    alarm.description = Some("Preheat to 180C, then\ncheck the crust\\nsmell".to_string());
    t.alarms = vec![alarm];
    let ics = t.to_ics();
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "c".into()).unwrap();
    assert_eq!(parsed.alarms.len(), 1);
    assert_eq!(
        parsed.alarms[0].description,
        Some("Preheat to 180C, then\ncheck the crust\\nsmell".to_string())
    );
}

/// URL is a URI type: it is never escaped in ICS, so a literal backslash
/// sequence in the URL must survive the round trip untouched.
#[test]
fn vtodo_url_with_backslash_sequence_survives() {
    let mut t = Task::new("Read the recipe", &std::collections::HashMap::new(), None);
    t.url = Some("https://example.com/recipe\\nrev2".to_string());
    let ics = t.to_ics();
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "c".into()).unwrap();
    assert_eq!(
        parsed.url,
        Some("https://example.com/recipe\\nrev2".to_string())
    );
}

/// Same for the VJOURNAL path, which reads the raw URL value.
#[test]
fn journal_url_with_backslash_sequence_survives() {
    let t = parse(
        "BEGIN:VJOURNAL\r\n\
            UID:j1\r\n\
            SUMMARY:Reading log\r\n\
            URL:https://example.com/notes\\n2026\r\n\
            END:VJOURNAL",
    );
    assert_eq!(t.url, Some("https://example.com/notes\\n2026".to_string()));
}

/// The VJOURNAL path reads raw text, so its unescaping must keep working.
#[test]
fn journal_text_properties_unescape() {
    let t = parse(
        "BEGIN:VJOURNAL\r\n\
            UID:j1\r\n\
            SUMMARY:Morning pages\\nsecond line\r\n\
            DESCRIPTION:notes\\, with comma\r\n\
            LOCATION:desk, window seat\r\n\
            CATEGORIES:reading\\, fiction\r\n\
            END:VJOURNAL",
    );
    assert_eq!(t.summary, "Morning pages\nsecond line");
    assert_eq!(t.description, "notes, with comma");
    assert_eq!(t.locations, vec!["desk, window seat".to_string()]);
    assert_eq!(t.categories, vec!["reading, fiction".to_string()]);
}

/// A sibling override that precedes the master must not contribute its
/// CATEGORIES to the master task.
#[test]
fn categories_from_override_do_not_leak_into_master() {
    let body = "BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        RECURRENCE-ID:20260921T090000Z\r\n\
        CATEGORIES:override-cat\r\n\
        END:VTODO\r\n\
        BEGIN:VTODO\r\n\
        UID:master\r\n\
        SUMMARY:Weekly reading\r\n\
        CATEGORIES:master-cat\r\n\
        END:VTODO";
    let t = parse(body);
    assert_eq!(t.categories, vec!["master-cat".to_string()]);
}

/// Multiple CATEGORIES lines on the master all merge into one list.
#[test]
fn multiple_categories_lines_merge() {
    let t = parse(
        "BEGIN:VTODO\r\n\
            UID:u\r\n\
            SUMMARY:Read a chapter\r\n\
            CATEGORIES:gardening\r\n\
            CATEGORIES:reading\r\n\
            END:VTODO",
    );
    assert_eq!(
        t.categories,
        vec!["gardening".to_string(), "reading".to_string()]
    );
}

// --- Settings carrier (1970 CANCELLED VEVENT) ---

fn settings_carrier(description: &str) -> Task {
    let mut t = Task::new(
        cfait::config::SETTINGS_SUMMARY,
        &std::collections::HashMap::new(),
        None,
    );
    t.uid = cfait::config::SETTINGS_UID.to_string();
    t.status = cfait::model::TaskStatus::Cancelled;
    t.description = description.to_string();
    t.categories = vec![cfait::config::SETTINGS_CATEGORY.to_string()];
    t.is_event = true;
    t
}

/// The settings fallback carrier must serialize as a CANCELLED VEVENT pinned
/// at the Unix epoch so time-range filters hide it, and the payload must
/// survive the round trip losslessly (JSON is full of commas and quotes).
#[test]
fn settings_vevent_carrier_roundtrips() {
    let json = r#"{"updated_at":1759000000,"config":{"default_calendar":"/calendars/u/main","tag_aliases":{"work":["work, home"]},"goals":{"g1":{"label":"Read, a lot"}}}}"#;
    let mut t = settings_carrier(json);
    t.sequence = 7;
    let ics = t.to_ics();
    assert!(ics.contains("BEGIN:VEVENT"), "carrier must be a VEVENT");
    assert!(ics.contains("END:VEVENT"));
    assert!(!ics.contains("VTODO"), "carrier must not be a VTODO");
    assert!(ics.contains("UID:cfait-global-settings-v1"));
    assert!(ics.contains("STATUS:CANCELLED"));
    assert!(ics.contains("DTSTART:19700101T000000Z"));
    assert!(ics.contains("DTEND:19700101T000001Z"));

    let parsed =
        Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).unwrap();
    assert!(parsed.is_event);
    assert!(!parsed.is_journal);
    assert_eq!(parsed.uid, cfait::config::SETTINGS_UID);
    assert_eq!(parsed.status, cfait::model::TaskStatus::Cancelled);
    assert_eq!(parsed.summary, cfait::config::SETTINGS_SUMMARY);
    assert_eq!(parsed.description, json);
    assert_eq!(parsed.categories, vec![cfait::config::SETTINGS_CATEGORY.to_string()]);
    assert_eq!(parsed.sequence, 7);
    assert_eq!(
        parsed.dtstart,
        Some(DateType::Specific(
            DateTime::from_timestamp(0, 0).unwrap()
        ))
    );
}

/// A hand-written 1970 CANCELLED VEVENT (as stored by another device) parses
/// back into an event carrier with the epoch start.
#[test]
fn settings_vevent_carrier_parses_from_wire() {
    let t = parse(
        "BEGIN:VEVENT\r\n\
            UID:cfait-global-settings-v1\r\n\
            SUMMARY:⚙ Cfait Settings (Do not delete)\r\n\
            DTSTAMP:20260929T000000Z\r\n\
            SEQUENCE:3\r\n\
            STATUS:CANCELLED\r\n\
            DTSTART:19700101T000000Z\r\n\
            DTEND:19700101T000001Z\r\n\
            CATEGORIES:cfait-internal\r\n\
            END:VEVENT",
    );
    assert!(t.is_event);
    assert_eq!(t.uid, "cfait-global-settings-v1");
    assert_eq!(t.status, cfait::model::TaskStatus::Cancelled);
    assert_eq!(
        t.dtstart,
        Some(DateType::Specific(DateTime::from_timestamp(0, 0).unwrap()))
    );
    assert_eq!(t.sequence, 3);
    assert_eq!(t.categories, vec!["cfait-internal".to_string()]);
}

/// A VEVENT without STATUS is still treated as the cancelled carrier (the
/// summary marker is what identifies it).
#[test]
fn settings_vevent_without_status_defaults_cancelled() {
    let t = parse(
        "BEGIN:VEVENT\r\n\
            UID:cfait-global-settings-v1\r\n\
            SUMMARY:⚙ Cfait Settings (Do not delete)\r\n\
            DTSTART:19700101T000000Z\r\n\
            DTEND:19700101T000001Z\r\n\
            END:VEVENT",
    );
    assert_eq!(t.status, cfait::model::TaskStatus::Cancelled);
    assert!(t.is_event);
}
