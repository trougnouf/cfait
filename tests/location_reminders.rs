// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for location-based reminders (`rem:@@loc[:leave][:Nm]`):
//! parsing, canonical round-trip, ICS storage, and geofence evaluation.
use cfait::alarm_index::{
    AlarmIndex, LocationAlarmEntry, LocationAlarmState, LocationEval, evaluate_location_alarms,
    haversine_m,
};
use cfait::model::Task;
use cfait::model::item::{AlarmTrigger, LocationEvent};
use chrono::Utc;
use std::collections::HashMap;

fn parse(input: &str) -> Task {
    Task::new(input, &HashMap::new(), None)
}

#[test]
fn parse_rem_at_location() {
    let t = parse("Buy milk rem:@@store");
    assert_eq!(t.summary, "Buy milk");
    assert_eq!(t.alarms.len(), 1);
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            event,
        } => {
            assert_eq!(locations, &vec!["store".to_string()]);
            assert_eq!(*radius_m, None);
            assert_eq!(*event, LocationEvent::Enter);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    // Canonical round-trip
    assert_eq!(t.to_smart_string(), "Buy milk rem:@@store");
}

#[test]
fn parse_rem_location_with_radius() {
    let t = parse("Call mom rem:@@home:500m");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location { radius_m, .. } => assert_eq!(*radius_m, Some(500)),
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(t.to_smart_string(), "Call mom rem:@@home:500m");
}

#[test]
fn parse_rem_location_leave() {
    let t = parse("Take umbrella rem:@@home:leave");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            event, radius_m, ..
        } => {
            assert_eq!(*event, LocationEvent::Leave);
            assert_eq!(*radius_m, None);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(t.to_smart_string(), "Take umbrella rem:@@home:leave");
}

#[test]
fn parse_rem_location_multi_and_both_modifiers() {
    // Modifiers may appear in any order after the location list.
    let t = parse("Groceries rem:@@aldi|auchan:200m:leave");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            event,
        } => {
            assert_eq!(locations, &vec!["aldi".to_string(), "auchan".to_string()]);
            assert_eq!(*radius_m, Some(200));
            assert_eq!(*event, LocationEvent::Leave);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(
        t.to_smart_string(),
        "Groceries rem:@@aldi|auchan:leave:200m"
    );
}

#[test]
fn parse_rem_loc_prefix_alias() {
    let t = parse("Dentist rem:loc:clinic:100m");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            ..
        } => {
            assert_eq!(locations, &vec!["clinic".to_string()]);
            assert_eq!(*radius_m, Some(100));
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
}

#[test]
fn parse_rem_location_quoted_name() {
    let t = parse("Visit rem:@@\"my place\":250m");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            ..
        } => {
            assert_eq!(locations, &vec!["my place".to_string()]);
            assert_eq!(*radius_m, Some(250));
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
}

#[test]
fn invalid_modifier_is_not_a_location_reminder() {
    // An unrecognized modifier must not silently create a wrong alarm.
    let t = parse("Task rem:@@home:banana");
    assert!(t.alarms.is_empty());
    assert!(t.to_smart_string().contains("rem:@@home:banana"));
}

#[test]
fn plain_reminders_still_parse() {
    // With a due date, a bare duration is relative to it.
    let t = parse("Task @tomorrow rem:10m");
    assert_eq!(t.alarms.len(), 1);
    assert!(matches!(t.alarms[0].trigger, AlarmTrigger::Relative(_)));
    // Without a date it falls back to an absolute alarm from now.
    let t = parse("Task rem:10m");
    assert_eq!(t.alarms.len(), 1);
    assert!(matches!(t.alarms[0].trigger, AlarmTrigger::Absolute(_)));
}

#[test]
fn ics_roundtrip_location_alarm() {
    let mut t = parse("Buy milk rem:@@store:300m @@errands");
    let ics = t.to_ics();
    assert!(ics.contains("X-CFAIT-LOC-ALARM:"));
    // Location alarms must not be emitted as VALARM triggers.
    assert!(!ics.contains("BEGIN:VALARM"));

    let t2 = Task::from_ics(&ics, "e".into(), "h".into(), "cal".into()).unwrap();
    assert_eq!(t2.alarms.len(), 1);
    match &t2.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            event,
        } => {
            assert_eq!(locations, &vec!["store".to_string()]);
            assert_eq!(*radius_m, Some(300));
            assert_eq!(*event, LocationEvent::Enter);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(t2.to_smart_string(), t.to_smart_string());
    t.alarms.clear();
}

type Coords = HashMap<String, (f64, f64)>;

fn eval<'a>(
    coords: &'a Coords,
    default_radius_m: u32,
    now: chrono::DateTime<Utc>,
) -> LocationEval<'a> {
    LocationEval {
        coords,
        default_radius_m,
        now,
    }
}

fn entry(uid: &str, locations: &[&str], radius_m: Option<u32>, event: &str) -> LocationAlarmEntry {
    LocationAlarmEntry {
        alarm_uid: uid.to_string(),
        task_uid: format!("task-{}", uid),
        task_title: "Task".to_string(),
        calendar_href: "local://default".to_string(),
        locations: locations.iter().map(|s| s.to_string()).collect(),
        radius_m,
        event: event.to_string(),
        description: None,
    }
}

#[test]
fn haversine_distance() {
    // ~111 m per 0.001 degrees of latitude.
    let d = haversine_m(50.0, 4.0, 50.001, 4.0);
    assert!((d - 111.2).abs() < 1.0, "got {}", d);
}

#[test]
fn evaluate_fires_on_enter_transition_only() {
    let home = ("home".to_string(), (50.0, 4.0));
    let coords = HashMap::from([home]);
    let entries = vec![entry("a1", &["home"], None, "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();

    // Outside: no fire, state recorded as outside.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.01, 4.0, &mut state);
    assert!(fired.is_empty());
    assert!(!state["a1"].inside);

    // Entering: fires once.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0001, 4.0, &mut state);
    assert_eq!(fired.len(), 1);
    assert!(state["a1"].inside);

    // Still inside: does not re-fire.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0002, 4.0, &mut state);
    assert!(fired.is_empty());

    // Leaving: enter reminder does not fire; state flips to outside.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.01, 4.0, &mut state);
    assert!(fired.is_empty());
    assert!(!state["a1"].inside);

    // Re-entering within the 24h window: suppressed by the re-fire window.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0001, 4.0, &mut state);
    assert!(fired.is_empty(), "re-fire window should suppress");
}

#[test]
fn evaluate_leave_fires_on_exit() {
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0))]);
    let entries = vec![entry("a1", &["home"], None, "leave")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();

    // Unknown previous state: a leave alarm must not fire before we know we were inside.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.01, 4.0, &mut state);
    assert!(fired.is_empty());

    // Move inside (no fire for a leave alarm), then outside: fires once.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0001, 4.0, &mut state);
    assert!(fired.is_empty());
    assert!(state["a1"].inside);
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.01, 4.0, &mut state);
    assert_eq!(fired.len(), 1);
    assert!(!state["a1"].inside);
    // Staying outside: no re-fire.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.02, 4.0, &mut state);
    assert!(fired.is_empty());
}

#[test]
fn evaluate_radius_precedence() {
    // Without a per-reminder radius, the global default applies.
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0))]);
    let entries = vec![entry("a1", &["home"], None, "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    // 400m away: outside the 300m default.
    let d = haversine_m(50.0, 4.0, 50.0036, 4.0);
    assert!(d > 300.0 && d < 500.0, "distance setup: {}", d);
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0036, 4.0, &mut state);
    assert!(fired.is_empty(), "400m is outside the 300m default");

    // An explicit per-reminder radius (500m) beats the default.
    let entries = vec![entry("a2", &["home"], Some(500), "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0036, 4.0, &mut state);
    assert_eq!(fired.len(), 1, "per-reminder radius should win");
}

#[test]
fn evaluate_skips_locations_without_coords() {
    let coords: Coords = HashMap::new();
    let entries = vec![entry("a1", &["nowhere"], None, "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let fired = evaluate_location_alarms(
        &entries,
        &eval(&coords, 300, Utc::now()),
        50.0,
        4.0,
        &mut state,
    );
    assert!(fired.is_empty());
    assert!(state.is_empty(), "no state change without coordinates");
}

#[test]
fn evaluate_multi_location_any_inside() {
    let coords = HashMap::from([
        ("aldi".to_string(), (50.0, 4.0)),
        ("auchan".to_string(), (51.0, 5.0)),
    ]);
    let entries = vec![entry("a1", &["aldi", "auchan"], None, "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    // Near auchan only: fires.
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 51.0001, 5.0, &mut state);
    assert_eq!(fired.len(), 1);
}

#[test]
fn evaluate_location_alert_uses_system_transition() {
    use cfait::alarm_index::evaluate_location_alert;
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0))]);
    let entries = vec![
        entry("a1", &["home"], None, "enter"),
        entry("a2", &["home"], Some(100), "enter"), // different radius: own alert
    ];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();

    // Alert at default radius 300, entering: fires a1 only (a2 resolved 100).
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "home",
        300,
        true,
        &mut state,
    );
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].alarm_uid, "a1");
    // Repeated entering (no exit): suppressed by transition rule.
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "home",
        300,
        true,
        &mut state,
    );
    assert!(fired.is_empty());
    // Leaving at radius 300: enter-alarm does not fire.
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "home",
        300,
        false,
        &mut state,
    );
    assert!(fired.is_empty());
    assert!(!state["a1"].inside);
    // Alert at radius 100, entering: fires a2 now.
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "home",
        100,
        true,
        &mut state,
    );
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].alarm_uid, "a2");
}

#[test]
fn alarm_index_collects_location_alarms() {
    let t = parse("Buy milk rem:@@store:300m");
    let mut task_map = HashMap::new();
    task_map.insert(t.uid.clone(), t);
    let calendars = HashMap::from([("local://default".to_string(), task_map)]);
    let index = AlarmIndex::rebuild_from_tasks(&calendars, &[], true, "08:00");
    assert!(index.alarms.is_empty(), "no VALARM trigger expected");
    assert_eq!(index.get_location_alarms().len(), 1);
    let e = &index.get_location_alarms()[0];
    assert_eq!(e.locations, vec!["store".to_string()]);
    assert_eq!(e.radius_m, Some(300));
    assert_eq!(e.event, "enter");
    assert!(index.has_location_alarms());
    // Time-based next-alarm scheduling ignores location alarms.
    assert!(index.get_next_alarm_timestamp().is_none());
}

// --- Inline `rem:geo:lat,lon[:leave][:Nm]` and alias-defined coordinates ---

use cfait::alarm_index::evaluate_location_alert;
use cfait::model::parser::{location_coords_from_aliases, parse_geo_coords};

#[test]
fn parse_geo_coords_valid() {
    assert_eq!(
        parse_geo_coords("geo:50.1953,4.5353"),
        Some((50.1953, 4.5353))
    );
    assert_eq!(parse_geo_coords("50.1953,4.5353"), Some((50.1953, 4.5353)));
    assert_eq!(
        parse_geo_coords("geo:-33.86,151.21"),
        Some((-33.86, 151.21))
    );
    assert_eq!(parse_geo_coords("GEO:1,2"), Some((1.0, 2.0)));
    assert_eq!(parse_geo_coords("geo:90,180"), Some((90.0, 180.0)));
}

#[test]
fn parse_geo_coords_invalid() {
    assert_eq!(parse_geo_coords("geo:91,0"), None, "lat out of range");
    assert_eq!(parse_geo_coords("geo:0,181"), None, "lon out of range");
    assert_eq!(parse_geo_coords("geo:1"), None, "missing lon");
    assert_eq!(parse_geo_coords("geo:1,2,3"), None);
    assert_eq!(parse_geo_coords("geo:abc,2"), None);
    assert_eq!(
        parse_geo_coords("geo:here"),
        None,
        "no dynamic 'here' in reminders"
    );
    assert_eq!(parse_geo_coords("geo:"), None);
}

#[test]
fn parse_rem_geo_location() {
    let t = parse("Buy milk rem:geo:50.1953,4.5353");
    assert_eq!(t.summary, "Buy milk");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            event,
        } => {
            assert_eq!(locations, &vec!["geo:50.1953,4.5353".to_string()]);
            assert_eq!(*radius_m, None);
            assert_eq!(*event, LocationEvent::Enter);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(t.to_smart_string(), "Buy milk rem:geo:50.1953,4.5353");
}

#[test]
fn parse_rem_geo_with_modifiers_roundtrip() {
    let t = parse("Call rem:geo:50.1953,4.5353:leave:200m");
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            event,
        } => {
            assert_eq!(locations, &vec!["geo:50.1953,4.5353".to_string()]);
            assert_eq!(*radius_m, Some(200));
            assert_eq!(*event, LocationEvent::Leave);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    // Canonical order: location, :leave, :Nm — re-parseable.
    assert_eq!(
        t.to_smart_string(),
        "Call rem:geo:50.1953,4.5353:leave:200m"
    );
    let t2 = parse(&t.to_smart_string());
    assert_eq!(t2.to_smart_string(), t.to_smart_string());
}

#[test]
fn parse_rem_geo_invalid_is_not_a_location_reminder() {
    // Out-of-range latitude: the token is not a reminder, leaks to the summary.
    let t = parse("Task rem:geo:95,4");
    assert!(t.alarms.is_empty());
    assert!(t.to_smart_string().contains("rem:geo:95,4"));
}

#[test]
fn parse_rem_geo_ics_roundtrip() {
    let t = parse("Dentist rem:geo:50.1953,4.5353:100m");
    let ics = t.to_ics();
    assert!(ics.contains("X-CFAIT-LOC-ALARM:"));
    let t2 = Task::from_ics(&ics, "e".into(), "h".into(), "cal".into()).unwrap();
    match &t2.alarms[0].trigger {
        AlarmTrigger::Location { locations, .. } => {
            assert_eq!(locations, &vec!["geo:50.1953,4.5353".to_string()]);
        }
        other => panic!("expected location trigger, got {:?}", other),
    }
    assert_eq!(t2.to_smart_string(), t.to_smart_string());
}

#[test]
fn evaluate_inline_geo_location() {
    // No coords map at all: the inline geo: reference resolves by itself.
    let coords: Coords = HashMap::new();
    let entries = vec![entry("g1", &["geo:50.1953,4.5353"], Some(500), "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    let fired = evaluate_location_alarms(
        &entries,
        &eval(&coords, 300, now),
        50.196,
        4.536,
        &mut state,
    );
    assert_eq!(fired.len(), 1, "near the inline geo point should fire");
    let fired = evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.5, 4.5, &mut state);
    assert!(fired.is_empty(), "far away must not fire");
}

#[test]
fn evaluate_location_alert_with_inline_geo_name() {
    let coords: Coords = HashMap::new();
    let entries = vec![entry("g1", &["geo:50.1953,4.5353"], Some(500), "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    // The geofence key is the location string itself ("geo:50.1953,4.5353").
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "geo:50.1953,4.5353",
        500,
        true,
        &mut state,
    );
    assert_eq!(fired.len(), 1);
    // A mismatched alert radius does not fire the entry.
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let fired = evaluate_location_alert(
        &entries,
        &eval(&coords, 300, now),
        "geo:50.1953,4.5353",
        300,
        true,
        &mut state,
    );
    assert!(fired.is_empty());
}

#[test]
fn location_coords_from_alias_geo_values() {
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();
    aliases.insert(
        "@@aldi".to_string(),
        vec![
            "#groceries:aldi".to_string(),
            "col:wl".to_string(),
            "geo:50.1953,4.5353".to_string(),
            "rem:@@aldi:500m".to_string(),
        ],
    );
    // Chained alias: @@a -> @@aldi -> geo:...
    aliases.insert("@@a".to_string(), vec!["@@aldi".to_string()]);
    aliases.insert("#shop".to_string(), vec!["no-geo-here".to_string()]);

    let coords = location_coords_from_aliases(&aliases);
    assert_eq!(coords.get("aldi"), Some(&(50.1953, 4.5353)));
    assert_eq!(coords.get("a"), Some(&(50.1953, 4.5353)), "alias chain");
    assert!(!coords.contains_key("shop"), "tag aliases have no coords");
}

#[test]
fn full_alias_example_end_to_end() {
    // The documented example alias: @@aldi:=#groceries:aldi,col:wl,geo:50.1953,4.5353,rem:@@aldi:500m
    let (cleaned, new_aliases) = cfait::model::extract_inline_aliases(
        "@@aldi:=#groceries:aldi,col:wl,geo:50.1953,4.5353,rem:@@aldi:500m",
    );
    assert!(cleaned.contains("@@aldi"));
    assert_eq!(new_aliases.len(), 1);
    let values = &new_aliases["@@aldi"];
    // The comma inside geo:50.1953,4.5353 must not split the value.
    assert!(
        values.contains(&"geo:50.1953,4.5353".to_string()),
        "got {:?}",
        values
    );
    assert!(values.contains(&"rem:@@aldi:500m".to_string()));
    // No useless-value warnings for col:/rem:/geo: values.
    assert!(cfait::model::find_useless_alias_values(values).is_empty());

    // A task tagged @@aldi gets the tag, the color, the geo property, and the reminder.
    let t = Task::new("Buy milk @@aldi", &new_aliases, None);
    assert_eq!(t.summary, "Buy milk");
    assert!(t.categories.iter().any(|c| c == "groceries:aldi"));
    assert!(t.locations.iter().any(|l| l == "aldi"));
    assert_eq!(t.geo.as_deref(), Some("50.1953,4.5353"));
    assert_eq!(t.target_collection.as_deref(), Some("wl"));
    match &t.alarms[0].trigger {
        AlarmTrigger::Location {
            locations,
            radius_m,
            ..
        } => {
            assert_eq!(locations, &vec!["aldi".to_string()]);
            assert_eq!(*radius_m, Some(500));
        }
        other => panic!("expected location trigger, got {:?}", other),
    }

    // The reminder resolves through the alias geo and fires near the store.
    let coords = location_coords_from_aliases(&new_aliases);
    assert_eq!(coords.get("aldi"), Some(&(50.1953, 4.5353)));
    let mut task_map = HashMap::new();
    task_map.insert(t.uid.clone(), t);
    let calendars = HashMap::from([("local://default".to_string(), task_map)]);
    let index = AlarmIndex::rebuild_from_tasks(&calendars, &[], true, "08:00");
    let entries = index.get_location_alarms();
    assert_eq!(entries.len(), 1);
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    let fired =
        evaluate_location_alarms(entries, &eval(&coords, 300, now), 50.196, 4.536, &mut state);
    assert_eq!(fired.len(), 1, "standing at the aldi should fire");
    let fired = evaluate_location_alarms(entries, &eval(&coords, 300, now), 51.0, 5.0, &mut state);
    assert!(fired.is_empty());
}

#[test]
fn plain_location_tag_alone_creates_no_reminder() {
    // Without a rem: value in the alias, a bare @@aldi tag does not remind.
    let aliases: HashMap<String, Vec<String>> =
        HashMap::from([("@@aldi".to_string(), vec!["geo:50.1953,4.5353".to_string()])]);
    let t = Task::new("Buy milk @@aldi", &aliases, None);
    assert!(t.alarms.is_empty());
    assert!(t.locations.iter().any(|l| l == "aldi"));
    // But rem:@@aldi on any task still resolves via the alias geo.
    let t2 = Task::new("Buy milk rem:@@aldi", &HashMap::new(), None);
    assert_eq!(t2.alarms.len(), 1);
    let coords = location_coords_from_aliases(&aliases);
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let mut task_map = HashMap::new();
    task_map.insert(t2.uid.clone(), t2);
    let calendars = HashMap::from([("local://default".to_string(), task_map)]);
    let index = AlarmIndex::rebuild_from_tasks(&calendars, &[], true, "08:00");
    let fired = evaluate_location_alarms(
        index.get_location_alarms(),
        &eval(&coords, 300, Utc::now()),
        50.196,
        4.536,
        &mut state,
    );
    assert_eq!(fired.len(), 1);
}
