use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::TaskStore;

/// Verifies the blocking reverse index is built and updated correctly.
///
/// Scenario:
///  - Create tasks A, B, C where B and C depend on A.
///  - Ensure get_tasks_blocking("A") returns B and C.
///  - Add task D without dependency, then call add_dependency(D, A) and ensure D appears.
///  - Remove dependency from B to A and ensure B is no longer returned.
#[test]
fn test_blocking_index_basic() {
    // Setup store with TestContext
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);

    let aliases: HashMap<String, Vec<String>> = HashMap::new();

    // Task A (the blocker)
    let mut a = Task::new("Task A", &aliases, None);
    a.uid = "A".to_string();
    a.calendar_href = "cal1".to_string();
    a.summary = "A".to_string();

    // Task B depends on A
    let mut b = Task::new("Task B", &aliases, None);
    b.uid = "B".to_string();
    b.calendar_href = "cal1".to_string();
    b.summary = "B".to_string();
    b.dependencies.push("A".to_string());

    // Task C depends on A
    let mut c = Task::new("Task C", &aliases, None);
    c.uid = "C".to_string();
    c.calendar_href = "cal1".to_string();
    c.summary = "C".to_string();
    c.dependencies.push("A".to_string());

    // Insert tasks into store
    store.add_task(a);
    store.add_task(b);
    store.add_task(c);

    // Check blocking for A: should contain B and C
    let blocking = store.get_tasks_blocking("A");
    let uids: HashSet<String> = blocking.into_iter().map(|(uid, _)| uid).collect();

    assert_eq!(uids.len(), 2, "A should block exactly 2 tasks initially");
    assert!(uids.contains("B"), "A should block B");
    assert!(uids.contains("C"), "A should block C");

    // Add task D without deps, then add dependency D -> A via add_dependency
    let mut d = Task::new("Task D", &aliases, None);
    d.uid = "D".to_string();
    d.calendar_href = "cal1".to_string();
    d.summary = "D".to_string();
    store.add_task(d);

    // Initially D should not be in blocking list
    let before = store.get_tasks_blocking("A");
    let before_uids: HashSet<String> = before.into_iter().map(|(uid, _)| uid).collect();
    assert!(
        !before_uids.contains("D"),
        "D should not yet be blocked by A"
    );

    // Add dependency D -> A (D depends on A => A blocks D)
    let added = store.add_dependency("D", "A".to_string());
    assert!(
        added.is_some(),
        "add_dependency should return Some when adding new dependency"
    );

    let after = store.get_tasks_blocking("A");
    let after_uids: HashSet<String> = after.into_iter().map(|(uid, _)| uid).collect();
    assert_eq!(after_uids.len(), 3, "A should block 3 tasks after adding D");
    assert!(after_uids.contains("D"), "A should now block D");

    // Now remove dependency B -> A and ensure B is no longer returned
    let removed = store.remove_dependency("B", "A");
    assert!(
        removed.is_some(),
        "remove_dependency should return Some when dependency existed"
    );

    let final_blocking = store.get_tasks_blocking("A");
    let final_uids: HashSet<String> = final_blocking.into_iter().map(|(uid, _)| uid).collect();
    assert_eq!(
        final_uids.len(),
        2,
        "After removing B->A there should be 2 blockers left"
    );
    assert!(
        !final_uids.contains("B"),
        "B should no longer be blocked by A"
    );
    assert!(final_uids.contains("C"), "C should still be blocked by A");
    assert!(final_uids.contains("D"), "D should still be blocked by A");
}
