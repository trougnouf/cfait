// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for https://github.com/trougnouf/cfait/issues/120
//!
//! A recurring task whose stored DUE is already in the past should surface as
//! an upcoming item — its next RRULE occurrence — rather than a stale overdue
//! one. The projection is display-only: the stored `due` is never mutated, so
//! the recycling model and calendar sync are preserved.

use cfait::alarm_index::AlarmIndex;
use cfait::config::{PausedSortBehavior, SortPreset};
use cfait::context::TestContext;
use cfait::model::{DateType, IcsAdapter, RecurrenceEngine, Task};
use cfait::store::{FilterOptions, TaskListItem, TaskStore};
use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The exact ICS from https://github.com/trougnouf/cfait/issues/120. Kept with
/// `\n` so the RFC 5545 folded leading spaces survive in source; converted to
/// `\r\n` (as radicale emits) at use time.
const ICS_RAW: &str = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Radicale//NONSGML Version 3.8.3//EN
BEGIN:VTODO
DESCRIPTION:
 update usb stick with backup
DTSTAMP:20261021T100112Z
DUE:20261017T140000Z
RRULE:FREQ=WEEKLY
STATUS:NEEDS-ACTION
SUMMARY:
 update usb stick with backup
UID:5fca396e-e883-4eaf-a159-3348ca08d5ee
END:VTODO
END:VCALENDAR
"#;

fn issue_ics() -> String {
    ICS_RAW.replace("\n", "\r\n")
}

/// The empty filter sets that `FilterOptions` borrows. Owned so the borrow
/// stays valid for the whole test.
#[derive(Default)]
struct FilterSets {
    hidden: HashSet<String>,
    cats: HashSet<String>,
    locs: HashSet<String>,
    done: HashSet<String>,
    tags: HashSet<String>,
    locs2: HashSet<String>,
    collapsed: HashSet<String>,
    aliases: HashMap<String, Vec<String>>,
}

fn make_options<'a>(
    sets: &'a FilterSets,
    search_term: &'a str,
    hide_completed: bool,
) -> FilterOptions<'a> {
    FilterOptions {
        active_cal_href: None,
        hidden_calendars: &sets.hidden,
        calendars: &[],
        selected_categories: &sets.cats,
        selected_locations: &sets.locs,
        match_all_categories: false,
        search_term,
        hide_completed_global: hide_completed,
        hide_fully_completed_tags: false,
        hide_aliases_in_sidebar: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 14,
        sort_standard_by_priority: false,
        sort_preset: SortPreset::default(),
        expanded_done_groups: &sets.done,
        expanded_tags: &sets.tags,
        expanded_locations: &sets.locs2,
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: &sets.aliases,
        search_collapsed_tasks: &sets.collapsed,
        focused_task_uid: None,
        paused_sort_behavior: PausedSortBehavior::default(),
        sort_tiebreak_recent: false,
        default_reminder_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
    }
}

fn find_task<'a>(items: &'a [TaskListItem], needle: &str) -> Option<&'a Task> {
    items
        .iter()
        .find_map(|t| match t {
            TaskListItem::Task(task) if task.summary.contains(needle) => Some(task),
            _ => None,
        })
        .map(|v| &**v)
}

/// A weekly task whose stored due date is far in the past, so its projection
/// always happens regardless of the wall clock.
fn past_due_weekly_task() -> Task {
    let aliases = HashMap::new();
    let mut task = Task::new("water the tomatoes", &aliases, None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap(),
    ));
    task.rrule = Some("FREQ=WEEKLY".to_string());
    task.calendar_href = "cal1".to_string();
    task
}

/// The exact ICS from the issue must parse and the task must be visible in the
/// default view, with its stored due date left untouched.
#[test]
fn issue_120_ics_parses_and_is_visible() {
    let task = IcsAdapter::from_ics(&issue_ics(), "etag".into(), "href".into(), "cal1".into())
        .expect("the issue ICS must parse");

    // Faithful to the issue: past due, weekly, no DTSTART, needs-action.
    assert_eq!(task.summary, "update usb stick with backup");
    assert_eq!(task.rrule.as_deref(), Some("FREQ=WEEKLY"));
    assert!(task.dtstart.is_none());
    assert!(!task.status.is_done());
    assert_eq!(task.uid, "5fca396e-e883-4eaf-a159-3348ca08d5ee");
    let stored_due = task.due.clone().expect("the issue task has a due date");

    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    store.add_task(task);

    let sets = FilterSets::default();
    let options = make_options(&sets, "is:ready", true);
    let items = store.filter(options).items;

    let shown = find_task(&items, "usb stick")
        .expect("the recurring task must be visible in the default is:ready view");
    // The stored due date is never mutated by the display projection.
    assert_eq!(shown.due, Some(stored_due));
}

/// A recurring task whose stored due date is in the past is projected forward
/// to its next occurrence in the main list and in the day planner.
#[test]
fn past_due_recurring_task_is_projected_upcoming() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    store.add_task(past_due_weekly_task());

    let sets = FilterSets::default();
    let options = make_options(&sets, "is:ready", true);
    let items = store.filter(options).items;

    let shown =
        find_task(&items, "water the tomatoes").expect("the recurring task must be visible");
    let stored_due = shown.due.clone().expect("stored due date");
    let eff_due = shown.effective_due.clone().expect("projected due date");

    // Projected forward to a later weekly occurrence.
    assert!(
        eff_due.to_date_naive() > stored_due.to_date_naive(),
        "effective_due {} should be after stored due {}",
        eff_due.to_date_naive(),
        stored_due.to_date_naive()
    );
    assert_eq!(
        (eff_due.to_date_naive() - stored_due.to_date_naive()).num_days() % 7,
        0,
        "a weekly recurrence projects to a whole number of weeks later"
    );
    // No longer flagged overdue now that it is an upcoming item.
    assert!(
        !shown.is_overdue,
        "a projected recurring task is upcoming, not overdue"
    );

    // The day planner / journal shows it on the projected (upcoming) date.
    let projected_date = eff_due.to_date_naive();
    let visible_cals: HashSet<String> = HashSet::from(["cal1".to_string()]);
    let day_ctx = store.get_day_context(projected_date, &visible_cals);
    assert!(
        day_ctx
            .due_tasks
            .iter()
            .any(|t| t.summary.contains("water the tomatoes")),
        "the day planner should show the recurring task on its next occurrence date"
    );
}

/// `projected_due` returns the stored due date for a non-recurring task, even
/// when it is in the past.
#[test]
fn projected_due_non_recurring_keeps_stored() {
    let aliases = HashMap::new();
    let mut task = Task::new("read a chapter", &aliases, None);
    let due = DateType::Specific(Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap());
    task.due = Some(due.clone());
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert_eq!(RecurrenceEngine::projected_due(&task, now), Some(due));
}

/// `projected_due` returns the stored due date for a recurring task whose due
/// date is still in the future.
#[test]
fn projected_due_future_due_keeps_stored() {
    let aliases = HashMap::new();
    let mut task = Task::new("hike the ridge", &aliases, None);
    let due = DateType::Specific(Utc.with_ymd_and_hms(2030, 1, 1, 9, 0, 0).unwrap());
    task.due = Some(due.clone());
    task.rrule = Some("FREQ=WEEKLY".to_string());
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert_eq!(RecurrenceEngine::projected_due(&task, now), Some(due));
}

/// `projected_due` moves a past-due recurring task to a future weekly
/// occurrence.
#[test]
fn projected_due_past_due_recurring_is_future() {
    let aliases = HashMap::new();
    let mut task = Task::new("water the tomatoes", &aliases, None);
    let stored = DateType::Specific(Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap());
    task.due = Some(stored.clone());
    task.rrule = Some("FREQ=WEEKLY".to_string());
    let projected = RecurrenceEngine::projected_due(&task, Utc::now()).expect("projected");
    assert!(projected.to_date_naive() > stored.to_date_naive());
    assert_eq!(
        (projected.to_date_naive() - stored.to_date_naive()).num_days() % 7,
        0
    );
}

/// `is_due_moment` accepts the stored due date of a non-recurring task, even
/// when it is in the past (the "Due now" reminder of a plain overdue task).
#[test]
fn is_due_moment_accepts_stored_due() {
    let aliases = HashMap::new();
    let mut task = Task::new("read a chapter", &aliases, None);
    let due = DateType::Specific(Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap());
    task.due = Some(due.clone());
    let default_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let moment = due.to_utc_with_default_time(default_time);
    assert!(RecurrenceEngine::is_due_moment(&task, moment, default_time));
    // A moment that is not the due date is not a due moment.
    assert!(!RecurrenceEngine::is_due_moment(
        &task,
        moment + chrono::Duration::days(3),
        default_time
    ));
}

/// `is_due_moment` accepts a projected occurrence at the moment its reminder
/// fires — even though the projection has already advanced to the following
/// occurrence by then (the fire-time race).
#[test]
fn is_due_moment_accepts_projected_occurrence_at_fire_time() {
    let task = past_due_weekly_task();
    let default_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

    let next = RecurrenceEngine::next_occurrence(&task).expect("a next occurrence exists");
    let fire_at = next
        .due
        .clone()
        .expect("the occurrence has a due date")
        .to_utc_with_default_time(default_time);

    assert!(RecurrenceEngine::is_due_moment(
        &task,
        fire_at,
        default_time
    ));
}

/// `is_due_moment` accepts the projected occurrence of an all-day recurring
/// task, whose reminder fires at the default time on the occurrence date.
#[test]
fn is_due_moment_accepts_all_day_projected_occurrence() {
    let aliases = HashMap::new();
    let mut task = Task::new("turn over the compost", &aliases, None);
    task.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
    ));
    task.rrule = Some("FREQ=WEEKLY".to_string());
    let default_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

    let next = RecurrenceEngine::next_occurrence(&task).expect("a next occurrence exists");
    let fire_at = next
        .due
        .clone()
        .expect("the occurrence has a due date")
        .to_utc_with_default_time(default_time);

    assert!(RecurrenceEngine::is_due_moment(
        &task,
        fire_at,
        default_time
    ));
}

/// `is_due_moment` rejects moments that are not an occurrence of the rule,
/// and tasks without a due date have no due moments at all.
#[test]
fn is_due_moment_rejects_non_occurrences() {
    let task = past_due_weekly_task();
    let default_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

    let next = RecurrenceEngine::next_occurrence(&task).expect("a next occurrence exists");
    let fire_at = next
        .due
        .clone()
        .unwrap()
        .to_utc_with_default_time(default_time);

    // Three days off the weekly grid is not a due moment.
    assert!(!RecurrenceEngine::is_due_moment(
        &task,
        fire_at + chrono::Duration::days(3),
        default_time
    ));

    // A recurring task without a due date has no due moments at all.
    let mut no_due = past_due_weekly_task();
    no_due.due = None;
    assert!(!RecurrenceEngine::is_due_moment(
        &no_due,
        fire_at,
        default_time
    ));
}

/// The alarm index schedules the "Due now" reminder of a past-due recurring
/// task at its projected (upcoming) occurrence, not at the stale stored due.
#[test]
fn alarm_index_schedules_projected_due_reminder() {
    let tasks = HashMap::from([(
        "cal1".to_string(),
        HashMap::from([("t1".to_string(), past_due_weekly_task())]),
    )]);

    let index = AlarmIndex::rebuild_from_tasks(&tasks, &[], true, "09:00");

    let entry = index
        .alarms
        .iter()
        .find(|a| a.is_implicit && a.alarm_uid.starts_with("implicit_due:"))
        .expect("a 'Due now' reminder must be indexed for the projected occurrence");
    let trigger = chrono::DateTime::from_timestamp_millis(entry.trigger_ms)
        .expect("a valid trigger timestamp");
    let now = Utc::now();
    assert!(
        trigger > now,
        "the reminder must target the future occurrence"
    );
    assert!(
        (trigger - now).num_days() <= 8,
        "a weekly reminder must fall within the next week, got {} days away",
        (trigger - now).num_days()
    );
}

/// Projecting must not invent a reminder for a past-due task without an
/// RRULE: it stays unindexed, as before.
#[test]
fn alarm_index_skips_past_due_non_recurring() {
    let aliases = HashMap::new();
    let mut task = Task::new("read a chapter", &aliases, None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2020, 1, 1, 9, 0, 0).unwrap(),
    ));
    task.calendar_href = "cal1".to_string();
    let uid = task.uid.clone();

    let tasks = HashMap::from([("cal1".to_string(), HashMap::from([(uid.clone(), task)]))]);

    let index = AlarmIndex::rebuild_from_tasks(&tasks, &[], true, "09:00");
    assert!(
        index.alarms.iter().all(|a| a.task_uid != uid),
        "a past-due non-recurring task must not gain a reminder"
    );
}
