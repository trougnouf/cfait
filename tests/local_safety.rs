// File: tests/local_safety.rs
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use cfait::storage::LOCAL_CALENDAR_HREF;
use std::collections::HashMap;

// --- TEST 1: Data Loss Prevention ---
#[test]
fn test_local_tasks_are_not_pruned_as_ghosts() {
    let mut tasks = vec![];
    let mut t1 = Task::new("Local Task 1", &HashMap::new(), None);
    t1.calendar_href = LOCAL_CALENDAR_HREF.to_string();
    t1.etag = "".to_string();
    tasks.push(t1);

    // This panicked/deleted items in the old code
    Journal::apply_to_tasks(&mut tasks, LOCAL_CALENDAR_HREF);

    assert_eq!(tasks.len(), 1, "Local tasks must persist!");
}

// --- TEST 2: Journal Compaction (Fixes Stuck Loading) ---
#[test]
fn test_journal_compaction_squashes_updates() {
    let mut j = Journal::default();
    let uid = "task-explosion";

    // Simulate the explosion: Create + 3 redundant updates
    let mut t = Task::new("Base", &HashMap::new(), None);
    t.uid = uid.to_string();
    j.queue.push(Action::Create(t.clone()));

    t.summary = "Update 1".to_string();
    j.queue.push(Action::Update(t.clone()));

    t.summary = "Update 2".to_string();
    j.queue.push(Action::Update(t.clone()));

    t.summary = "Final State".to_string();
    j.queue.push(Action::Update(t.clone()));

    assert_eq!(j.queue.len(), 4);

    // Apply compaction
    j.compact();

    // Should result in a single CREATE action with the Final State
    assert_eq!(j.queue.len(), 1);
    match &j.queue[0] {
        Action::Create(res) => assert_eq!(res.summary, "Final State"),
        _ => panic!("Should have squashed into the initial Create"),
    }
}
