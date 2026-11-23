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

type Coords = HashMap<String, (f64, f64, Option<u32>)>;

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
    let home = ("home".to_string(), (50.0, 4.0, None));
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
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0, None))]);
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
    // Per-location radius (500m) beats the global default (300m)...
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0, Some(500)))]);
    let entries = vec![entry("a1", &["home"], None, "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let now = Utc::now();
    // 400m away: outside a 300m default, inside the 500m location radius.
    let d = haversine_m(50.0, 4.0, 50.0036, 4.0);
    assert!(d > 300.0 && d < 500.0, "distance setup: {}", d);
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0036, 4.0, &mut state);
    assert_eq!(fired.len(), 1, "per-location radius should apply");

    // ...but an explicit per-reminder radius (100m) beats both.
    let entries = vec![entry("a2", &["home"], Some(100), "enter")];
    let mut state: HashMap<String, LocationAlarmState> = HashMap::new();
    let fired =
        evaluate_location_alarms(&entries, &eval(&coords, 300, now), 50.0036, 4.0, &mut state);
    assert!(fired.is_empty(), "per-reminder radius should win");
}

#[test]
fn evaluate_skips_locations_without_coords() {
    let coords: HashMap<String, (f64, f64, Option<u32>)> = HashMap::new();
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
        ("aldi".to_string(), (50.0, 4.0, None)),
        ("auchan".to_string(), (51.0, 5.0, None)),
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
    let coords = HashMap::from([("home".to_string(), (50.0, 4.0, None))]);
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
