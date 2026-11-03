// SPDX-License-Identifier: GPL-3.0-or-later
//! RFC 5545 compliance: a VTODO with a recurrence must carry a DTSTART.
//!
//! Regression tests for https://github.com/trougnouf/cfait/issues/124:
//! `cfait add "task @date rec:weekly"` used to serialize a VTODO with DUE
//! and RRULE but no DTSTART, which strict parsers reject (RFC 5545
//! 3.6.1.1 makes DTSTART REQUIRED for VTODO, and 3.8.5.3 requires it to
//! anchor a recurrence).

use cfait::config::{PausedSortBehavior, SortPreset};
use cfait::context::TestContext;
use cfait::model::{DateType, Task};
use cfait::store::{FilterOptions, TaskStore};
use chrono::{Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn parse(input: &str) -> Task {
    let aliases = HashMap::new();
    Task::new(input, &aliases, None)
}

// ============================================================================
// PARSER: `@date rec:...` must behave like `^@date rec:...`
// ============================================================================

#[test]
fn test_due_only_with_recurrence_anchors_start_to_due() {
    let task = parse("Water plants @2026-11-02 rec:weekly");

    let due = DateType::AllDay(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    assert_eq!(task.due, Some(due.clone()), "due date should be parsed");
    assert_eq!(
        task.dtstart,
        Some(due),
        "start should be anchored to the due date when a recurrence is present"
    );
    assert_eq!(task.rrule.as_deref(), Some("FREQ=WEEKLY"));
    assert!(
        task.has_anchored_recurrence_start(),
        "auto-anchored start must be marked X-CFAIT-ANCHORED-START"
    );
}

#[test]
fn test_due_only_with_recurrence_and_time() {
    // `@date time rec:...` yields a Specific due; the start must match it.
    let task = parse("Review @2026-11-02 15:00 rec:daily");

    assert!(
        task.due.is_some()
            && task
                .dtstart
                .as_ref()
                .is_some_and(|s| s.to_date_naive() == task.due.as_ref().unwrap().to_date_naive()),
        "start and due should share the same date, got start={:?} due={:?}",
        task.dtstart,
        task.due
    );
    assert_eq!(task.rrule.as_deref(), Some("FREQ=DAILY"));
}

#[test]
fn test_explicit_start_due_with_recurrence_unchanged() {
    let task = parse("Standup ^@2026-11-02 rec:weekly");
    let d = DateType::AllDay(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    assert_eq!(task.dtstart, Some(d.clone()));
    assert_eq!(task.due, Some(d));
    assert!(
        !task.has_anchored_recurrence_start(),
        "an explicit ^@ start must NOT be marked as an auto-anchor (it keeps deferral semantics)"
    );
}

#[test]
fn test_due_only_without_recurrence_keeps_no_start() {
    // Non-recurring tasks keep their existing semantics (no implicit start),
    // so sorting/display behavior is unchanged.
    let task = parse("Buy milk @2026-11-02");
    assert_eq!(
        task.due,
        Some(DateType::AllDay(
            NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
        ))
    );
    assert!(
        task.dtstart.is_none(),
        "non-recurring due-only tasks must not gain an implicit start"
    );
}

// ============================================================================
// SERIALIZER: VTODO with RRULE must emit DTSTART
// ============================================================================

#[test]
fn test_recurring_task_serializes_dtstart() {
    let task = parse("Water plants @2026-11-02 rec:weekly");
    let ics = task.to_ics();

    assert!(
        ics.contains("DTSTART;VALUE=DATE:20261102"),
        "ICS must carry an all-day DTSTART anchored to the due date. Got:\n{}",
        ics
    );
    assert!(ics.contains("DUE;VALUE=DATE:20261102"));
    assert!(ics.contains("RRULE:FREQ=WEEKLY"));
}

#[test]
fn test_recurring_task_with_datetime_due_serializes_matching_dtstart() {
    let mut task = parse("Review code");
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 11, 2, 15, 0, 0).unwrap(),
    ));
    task.rrule = Some("FREQ=DAILY".to_string());
    let ics = task.to_ics();

    assert!(
        ics.contains("DTSTART:20261102T150000Z"),
        "ICS must carry a DTSTART matching the due datetime. Got:\n{}",
        ics
    );
    assert!(ics.contains("DUE:20261102T150000Z"));
}

#[test]
fn test_legacy_recurring_item_without_start_is_repaired_on_write() {
    // Items created by older cfait versions (or imported) may have a
    // recurrence with a due but no start; serializing must repair them.
    let mut task = parse("Legacy recurring task");
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(),
    ));
    task.rrule = Some("FREQ=WEEKLY".to_string());
    assert!(task.dtstart.is_none());

    let ics = task.to_ics();
    assert!(
        ics.contains("DTSTART;VALUE=DATE:20261102"),
        "Legacy item must be serialized with a DTSTART derived from DUE. Got:\n{}",
        ics
    );
}

#[test]
fn test_recurring_item_without_any_date_gets_today_anchor() {
    let mut task = parse("Dateless recurring task");
    task.rrule = Some("FREQ=DAILY".to_string());
    assert!(task.dtstart.is_none() && task.due.is_none());

    let ics = task.to_ics();
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y%m%d")
        .to_string();
    assert!(
        ics.contains(&format!("DTSTART;VALUE=DATE:{}", today)),
        "Dateless recurring item must get a DTSTART anchored to today. Got:\n{}",
        ics
    );
}

#[test]
fn test_non_recurring_due_only_task_still_omits_dtstart() {
    // Existing behavior for non-recurring tasks must not change.
    let task = parse("Buy milk @2026-11-02");
    let ics = task.to_ics();

    assert!(ics.contains("DUE;VALUE=DATE:20261102"));
    assert!(
        !ics.contains("DTSTART"),
        "Non-recurring due-only tasks must not gain a DTSTART. Got:\n{}",
        ics
    );
}

// ============================================================================
// ROUNDTRIP
// ============================================================================

#[test]
fn test_roundtrip_recurring_all_day_task() {
    let original = parse("Water plants @2026-11-02 rec:weekly");
    let ics = original.to_ics();

    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .expect("ICS should round-trip");

    let d = DateType::AllDay(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    assert_eq!(
        restored.dtstart,
        Some(d.clone()),
        "DTSTART should survive round-trip"
    );
    assert_eq!(restored.due, Some(d), "DUE should survive round-trip");
    assert_eq!(restored.rrule.as_deref(), Some("FREQ=WEEKLY"));
}

// ============================================================================
// DAY CONTEXT: a ^@ item (start+due on the same day) is due-only for display
// ============================================================================

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

#[test]
fn test_day_context_same_day_start_due_not_double_listed() {
    let date = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
    let mut store = make_store();
    let d = DateType::AllDay(date);

    // ^@ item: start and due on the same day (recurring weekly)
    let mut same_day = Task::new("Same day recurring", &HashMap::new(), None);
    same_day.uid = "same-day".to_string();
    same_day.calendar_href = "cal1".to_string();
    same_day.dtstart = Some(d.clone());
    same_day.due = Some(d.clone());
    same_day.rrule = Some("FREQ=WEEKLY".to_string());
    store.add_task(same_day);

    // Start-only item
    let mut start_only = Task::new("Start only", &HashMap::new(), None);
    start_only.uid = "start-only".to_string();
    start_only.calendar_href = "cal1".to_string();
    start_only.dtstart = Some(d.clone());
    store.add_task(start_only);

    // Due-only item
    let mut due_only = Task::new("Due only", &HashMap::new(), None);
    due_only.uid = "due-only".to_string();
    due_only.calendar_href = "cal1".to_string();
    due_only.due = Some(d.clone());
    store.add_task(due_only);

    let visible: HashSet<String> = HashSet::from(["cal1".to_string()]);
    let ctx = store.get_day_context(date, &visible);

    let due_uids: Vec<&str> = ctx.due_tasks.iter().map(|t| t.uid.as_str()).collect();
    let started_uids: Vec<&str> = ctx.started_tasks.iter().map(|t| t.uid.as_str()).collect();

    assert!(
        due_uids.contains(&"same-day"),
        "^@ item must appear under due today"
    );
    assert!(
        due_uids.contains(&"due-only"),
        "due-only item must appear under due today"
    );
    assert!(
        !due_uids.contains(&"start-only"),
        "start-only item must not appear under due today"
    );

    assert!(
        started_uids.contains(&"start-only"),
        "start-only item must appear under started today"
    );
    assert!(
        !started_uids.contains(&"same-day"),
        "^@ item must not be double-listed under started today"
    );
    assert!(
        !started_uids.contains(&"due-only"),
        "due-only item must not appear under started today"
    );
}

// ============================================================================
// READINESS: an anchored recurrence start (start == due) is not a "future
// start" — `@date rec:...` tasks must stay in is:ready and keep their
// due-based rank, instead of being hidden until the first occurrence.
// ============================================================================

fn static_empty_set() -> &'static HashSet<String> {
    static SET: std::sync::OnceLock<HashSet<String>> = std::sync::OnceLock::new();
    SET.get_or_init(HashSet::new)
}

fn static_empty_alias_map() -> &'static HashMap<String, Vec<String>> {
    static MAP: std::sync::OnceLock<HashMap<String, Vec<String>>> = std::sync::OnceLock::new();
    MAP.get_or_init(HashMap::new)
}

fn ready_filter_options() -> FilterOptions<'static> {
    FilterOptions {
        active_cal_href: None,
        hidden_calendars: static_empty_set(),
        calendars: &[],
        selected_categories: static_empty_set(),
        selected_locations: static_empty_set(),
        match_all_categories: false,
        search_term: "is:ready",
        hide_completed_global: true,
        hide_fully_completed_tags: false,
        hide_aliases_in_sidebar: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 5,
        default_priority: 5,
        start_grace_period_days: 1,
        sort_standard_by_priority: false,
        sort_preset: SortPreset::default(),
        expanded_done_groups: static_empty_set(),
        expanded_tags: static_empty_set(),
        expanded_locations: static_empty_set(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: static_empty_alias_map(),
        search_collapsed_tasks: static_empty_set(),
        focused_task_uid: None,
        paused_sort_behavior: PausedSortBehavior::default(),
        sort_tiebreak_recent: false,
        default_reminder_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
    }
}

fn summary_of(items: &[cfait::store::TaskListItem], needle: &str) -> Option<Task> {
    items.iter().find_map(|item| match item {
        cfait::store::TaskListItem::Task(t) if t.summary.contains(needle) => {
            Some(t.as_ref().clone())
        }
        _ => None,
    })
}

fn store_task(mut t: Task, uid: &str) -> Task {
    t.uid = uid.to_string();
    t.calendar_href = "cal1".to_string();
    t
}

#[test]
fn test_anchored_recurrence_start_stays_in_is_ready() {
    // `@date rec:...` with a future due date anchors the start to the due
    // date (RFC compliance) but must NOT be treated as starting in the
    // future: the task stays in is:ready, ranked by its due date.
    let mut store = make_store();
    let anchored = store_task(parse("Anchored recurring @5d rec:weekly"), "anchored");
    assert!(anchored.has_anchored_recurrence_start());
    store.add_task(anchored);

    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Anchored").is_some(),
        "anchored recurring task (@date rec:) must stay in is:ready"
    );
}

#[test]
fn test_anchored_recurrence_start_is_not_deferred_in_ranking() {
    let mut store = make_store();
    store.add_task(store_task(
        parse("Anchored recurring @10d rec:weekly"),
        "anchored",
    ));
    store.add_task(store_task(parse("Plain due task @10d"), "plain"));

    let filtered = store.filter(ready_filter_options());
    let anchored_rank = summary_of(&filtered.items, "Anchored").map(|t| t.sort_rank);
    let plain_rank = summary_of(&filtered.items, "Plain").map(|t| t.sort_rank);

    assert_eq!(
        anchored_rank,
        Some(4),
        "anchored recurring task must keep its due-based rank, not be deferred to 7"
    );
    assert_eq!(
        anchored_rank, plain_rank,
        "anchored and plain due tasks must rank alike"
    );
}

#[test]
fn test_explicit_same_day_start_due_recurrence_still_deferred() {
    // `^@date rec:...` is an explicit scheduling constraint: unlike
    // `@date rec:...`, it must stay out of is:ready until the start arrives.
    let mut store = make_store();
    let explicit = store_task(parse("Explicit ^@5d rec:weekly"), "explicit");
    assert!(explicit.dtstart.is_some() && explicit.due.is_some());
    assert!(!explicit.has_anchored_recurrence_start());
    store.add_task(explicit);

    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Explicit").is_none(),
        "explicit ^@date rec: task with a future start must stay out of is:ready"
    );
}

#[test]
fn test_non_recurring_same_day_start_due_still_deferred() {
    // One-off `^@date` tasks keep their scheduling semantics: an explicit
    // future start still removes them from is:ready.
    let mut store = make_store();
    store.add_task(store_task(parse("One off ^@5d"), "one-off"));

    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "One off").is_none(),
        "non-recurring ^@ task with a future start must stay out of is:ready"
    );
}

#[test]
fn test_recurring_future_start_without_due_still_deferred() {
    // A recurring task with a genuine future start (no due) keeps the
    // existing deferral behavior.
    let mut store = make_store();
    store.add_task(store_task(
        parse("Start only recurring ^7d rec:weekly"),
        "start-only",
    ));

    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Start only").is_none(),
        "recurring task with a future start and no due must stay out of is:ready"
    );
}

#[test]
fn test_recurrence_advance_keeps_anchored_task_ready() {
    // After completing an `@date rec:...` task, the next occurrence still
    // carries the anchor marker, so it must remain ready (not deferred).
    let mut task = store_task(parse("Anchored recurring @1d rec:weekly"), "anchored");
    assert!(task.advance_recurrence(), "recurrence should advance");
    assert!(
        task.has_anchored_recurrence_start(),
        "advanced occurrence must still carry an anchored start"
    );

    let mut store = make_store();
    store.add_task(task);
    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Anchored").is_some(),
        "advanced anchored occurrence must stay in is:ready"
    );
}

// ============================================================================
// MARKER ROUND-TRIP: X-CFAIT-ANCHORED-START must survive ICS round-trips, so
// `@date rec:` (anchored, stays ready) and `^@date rec:` (explicit, deferred)
// remain distinguishable after a sync.
// ============================================================================

#[test]
fn test_anchored_marker_survives_ics_roundtrip() {
    let task = parse("Anchored recurring @5d rec:weekly");
    let ics = task.to_ics();
    assert!(
        ics.contains("X-CFAIT-ANCHORED-START"),
        "anchored task must serialize the marker. Got:\n{}",
        ics
    );

    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal1".to_string(),
    )
    .expect("ICS should round-trip");
    assert!(
        restored.has_anchored_recurrence_start(),
        "marker must survive the ICS round-trip"
    );

    let mut store = make_store();
    store.add_task(store_task(restored, "anchored"));
    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Anchored").is_some(),
        "re-parsed anchored task must stay in is:ready"
    );
}

#[test]
fn test_explicit_start_due_recurrence_roundtrips_as_deferred() {
    let task = parse("Explicit ^@5d rec:weekly");
    assert!(!task.has_anchored_recurrence_start());
    let ics = task.to_ics();
    assert!(
        !ics.contains("X-CFAIT-ANCHORED-START"),
        "explicit ^@ must not carry the anchor marker. Got:\n{}",
        ics
    );

    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal1".to_string(),
    )
    .expect("ICS should round-trip");
    assert!(
        !restored.has_anchored_recurrence_start(),
        "explicit ^@ must stay non-anchored after round-trip"
    );

    let mut store = make_store();
    store.add_task(store_task(restored, "explicit"));
    let filtered = store.filter(ready_filter_options()).items;
    assert!(
        summary_of(&filtered, "Explicit").is_none(),
        "re-parsed explicit ^@ rec: task must stay out of is:ready"
    );
}

#[test]
fn test_legacy_item_repaired_with_anchor_marker() {
    // Pre-fix items (rrule + due, no dtstart) get a synthetic DTSTART on
    // write; the marker must be emitted too, or the item would flip to
    // deferred once re-parsed after a sync.
    let mut task = parse("Legacy recurring");
    let future = Utc::now().date_naive() + Duration::days(5);
    task.due = Some(DateType::AllDay(future));
    task.rrule = Some("FREQ=WEEKLY".to_string());
    assert!(task.dtstart.is_none() && !task.has_anchored_recurrence_start());

    let ics = task.to_ics();
    assert!(ics.contains("DTSTART"), "legacy item must gain a DTSTART");
    assert!(
        ics.contains("X-CFAIT-ANCHORED-START"),
        "legacy item's synthetic DTSTART must be marked as anchored. Got:\n{}",
        ics
    );

    let restored = Task::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal1".to_string(),
    )
    .expect("ICS should round-trip");
    assert!(
        restored.has_anchored_recurrence_start(),
        "repaired legacy item must be treated as anchored"
    );
}

#[test]
fn test_to_smart_string_keeps_anchor_rendered_as_due() {
    // Editing an anchored task must not turn `@date rec:` into `^@date rec:`.
    let anchored = parse("Anchored recurring @5d rec:weekly");
    let s = anchored.to_smart_string();
    assert!(
        s.contains(" @"),
        "anchored task must render as @date. Got: {}",
        s
    );
    assert!(
        !s.contains("^@"),
        "anchored task must not render as ^@. Got: {}",
        s
    );

    let explicit = parse("Explicit ^@5d rec:weekly");
    let s = explicit.to_smart_string();
    assert!(
        s.contains("^@"),
        "explicit ^@ must keep rendering as ^@. Got: {}",
        s
    );
}
