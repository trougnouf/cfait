// File: ./tests/local_safety.rs
use cfait::journal::Journal;
use cfait::model::Task;
use cfait::storage::LOCAL_CALENDAR_HREF;
use std::collections::HashMap;

#[test]
fn test_local_tasks_are_not_pruned_as_ghosts() {
    // 1. Setup a "Local" task (Empty ETag, not in Journal)
    let mut tasks = vec![];
    let mut t1 = Task::new("Local Task 1", &HashMap::new());
    t1.calendar_href = LOCAL_CALENDAR_HREF.to_string();
    t1.etag = "".to_string(); // Typical for local tasks
    tasks.push(t1.clone());

    // 2. Run Journal application on the Local Calendar
    // In the buggy version, this sees (Empty ETag + Not in Journal) -> Prune
    Journal::apply_to_tasks(&mut tasks, LOCAL_CALENDAR_HREF);

    // 3. Assert survival
    assert_eq!(
        tasks.len(),
        1,
        "Local tasks must not be pruned even if they have no ETag!"
    );
    assert_eq!(tasks[0].summary, "Local Task 1");
}

#[test]
fn test_server_ghosts_are_still_pruned() {
    // 1. Setup a "Server" Ghost task (Empty ETag, not in Journal)
    let mut tasks = vec![];
    let mut t1 = Task::new("Ghost Task", &HashMap::new());
    t1.calendar_href = "https://server.com/cal/".to_string();
    t1.etag = "".to_string();
    tasks.push(t1);

    // 2. Run Journal application on the Server Calendar
    Journal::apply_to_tasks(&mut tasks, "https://server.com/cal/");

    // 3. Assert deletion
    assert_eq!(
        tasks.len(),
        0,
        "Server tasks with no ETag (Ghosts) MUST be pruned."
    );
}
