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
