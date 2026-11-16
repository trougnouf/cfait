// SPDX-License-Identifier: GPL-3.0-or-later
//! A ringing reminder for an occurrence that was completed on another device
//! must be invalidated even when the local master task has not advanced
//! (its sync can be masked by a pending local edit), because the history
//! snapshot syncs as a new uid and always lands.
use cfait::context::TestContext;
use cfait::model::{DateType, RecurrenceEngine, Task, TaskStatus};
use cfait::store::TaskStore;
use chrono::{Duration, NaiveTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

fn default_time() -> NaiveTime {
    NaiveTime::from_hms_opt(8, 0, 0).unwrap()
}

fn weekly_garbage_task() -> Task {
    let mut t = Task::new("Garbage & PMC collection", &HashMap::new(), None);
    // due this morning (2h ago), as if the laptop slept through it
    let dt = Utc::now() - Duration::hours(2);
    t.due = Some(DateType::Specific(dt));
    t.dtstart = Some(DateType::Specific(dt));
    t.rrule = Some("FREQ=WEEKLY".to_string());
    t.calendar_href = "https://cal.example/cal/".to_string();
    t
}

fn store_with(task: Task) -> TaskStore {
    let mut store = TaskStore::new(Arc::new(TestContext::new()));
    store.add_task(task);
    store
}

#[test]
fn completed_occurrence_is_detected_via_history() {
    let task = weekly_garbage_task();
    let old_ts = task
        .due
        .as_ref()
        .unwrap()
        .to_utc_with_default_time(default_time());
    let master_uid = task.uid.clone();

    let mut store = store_with(task.clone());
    // Not completed anywhere yet: the ringing reminder stays valid
    assert!(!store.occurrence_completed_at(&master_uid, old_ts, default_time()));
    assert!(RecurrenceEngine::is_due_moment(
        store.get_task_ref(&master_uid).unwrap(),
        old_ts,
        default_time()
    ));

    // Mark done (as the desktop GUI would): recycle creates a history snapshot
    // and advances the master in the store.
    store.toggle_task(&master_uid);
    assert!(store.occurrence_completed_at(&master_uid, old_ts, default_time()));

    // Once the master advanced, the old due moment is no longer valid either
    let master = store.get_task_ref(&master_uid).unwrap();
    assert!(!RecurrenceEngine::is_due_moment(
        master,
        old_ts,
        default_time()
    ));
}

#[test]
fn stale_master_but_synced_history_still_invalidates() {
    // Laptop view: the master still has the old due (its sync was masked by a
    // pending local edit), but the history snapshot arrived from the server
    // as a new uid.
    let task = weekly_garbage_task();
    let old_ts = task
        .due
        .as_ref()
        .unwrap()
        .to_utc_with_default_time(default_time());
    let master_uid = task.uid.clone();

    let (history, _advanced) = task.recycle(TaskStatus::Completed, false);

    let mut store = store_with(task); // stale master, old due
    store.add_task(history); // history synced from the server
    assert!(store.occurrence_completed_at(&master_uid, old_ts, default_time()));
}

#[test]
fn pending_occurrence_is_not_completed() {
    let task = weekly_garbage_task();
    let old_ts = task
        .due
        .as_ref()
        .unwrap()
        .to_utc_with_default_time(default_time());
    let master_uid = task.uid.clone();
    let store = store_with(task);
    assert!(!store.occurrence_completed_at(&master_uid, old_ts, default_time()));
}

#[test]
fn is_due_moment_respects_exdate_on_stored_due() {
    let mut task = weekly_garbage_task();
    let ts = task
        .due
        .as_ref()
        .unwrap()
        .to_utc_with_default_time(default_time());
    assert!(RecurrenceEngine::is_due_moment(&task, ts, default_time()));
    // A foreign client cancels the instance via EXDATE without advancing DUE
    task.exdates.push(task.due.clone().unwrap());
    assert!(!RecurrenceEngine::is_due_moment(&task, ts, default_time()));
}

#[test]
fn projected_dtstart_follows_projected_due() {
    // Stale recurring master: stored start/due in the past (the morning
    // instance), as after a completion whose advance was clobbered in sync.
    let task = weekly_garbage_task();
    let now = Utc::now();
    let stored_due = task.due.clone().unwrap();
    let stored_start = task.dtstart.clone().unwrap();

    // Due is projected forward...
    let proj_due = RecurrenceEngine::projected_due(&task, now).unwrap();
    assert!(proj_due.to_comparison_time() > now);
    // ...and the start must be projected as a pair, not left in the past.
    let proj_start = RecurrenceEngine::projected_dtstart(&task, now).unwrap();
    assert!(proj_start.to_start_comparison_time() > now);
    // Same next occurrence: the start->due gap is preserved.
    let gap = |d: &DateType, s: &DateType| {
        d.to_comparison_time()
            .signed_duration_since(s.to_start_comparison_time())
    };
    assert_eq!(gap(&proj_due, &proj_start), gap(&stored_due, &stored_start));

    // Ongoing task (start past, due future) must NOT be projected.
    let mut ongoing = weekly_garbage_task();
    ongoing.dtstart = Some(DateType::Specific(now - Duration::hours(2)));
    ongoing.due = Some(DateType::Specific(now + Duration::hours(2)));
    assert_eq!(
        RecurrenceEngine::projected_dtstart(&ongoing, now).unwrap(),
        ongoing.dtstart.clone().unwrap()
    );
    assert_eq!(
        RecurrenceEngine::projected_due(&ongoing, now).unwrap(),
        ongoing.due.clone().unwrap()
    );

    // Non-recurring tasks are never projected.
    let mut plain = weekly_garbage_task();
    plain.rrule = None;
    assert_eq!(
        RecurrenceEngine::projected_dtstart(&plain, now).unwrap(),
        plain.dtstart.clone().unwrap()
    );
}

#[test]
fn stale_recurring_master_is_deferred_like_a_future_start() {
    use cfait::config::SortPreset;
    use chrono::NaiveTime;

    let task = weekly_garbage_task();
    let now = Utc::now();
    let default_time = NaiveTime::from_hms_opt(8, 0, 0).unwrap();

    // With the raw (past) start, the task escapes the future-start deferral
    // and ranks as a dated task...
    let mut raw = task.clone();
    raw.effective_due = RecurrenceEngine::projected_due(&task, now);
    raw.effective_dtstart = task.dtstart.clone();
    let rank_raw = raw.calculate_base_rank(
        None,
        1,
        1,
        1,
        false,
        SortPreset::UrgentDueStarted,
        default_time,
    );

    // ...but with the projected start (next occurrence, beyond grace) it is
    // deferred to the future group, matching its projected due display.
    let mut projected = task.clone();
    projected.effective_due = RecurrenceEngine::projected_due(&task, now);
    projected.effective_dtstart = RecurrenceEngine::projected_dtstart(&task, now);
    let rank_projected = projected.calculate_base_rank(
        None,
        1,
        1,
        1,
        false,
        SortPreset::UrgentDueStarted,
        default_time,
    );

    assert_eq!(rank_raw, 4);
    assert_eq!(rank_projected, 7);
}
