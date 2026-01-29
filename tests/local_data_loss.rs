// Tests ensuring no data loss in local storage logic.
use cfait::context::TestContext;
use cfait::journal::Journal;
use cfait::model::Task;
use cfait::storage::LOCAL_CALENDAR_HREF;
use std::collections::HashMap;

#[test]
fn test_local_tasks_are_not_pruned_as_ghosts() {
    // 1. Create an isolated TestContext (handles temp dir lifecycle)
    let ctx = TestContext::new();

    // 2. Create a "Local" task
    // Local tasks are stored on disk without an ETag.
    let mut task = Task::new("Buy Milk", &HashMap::new(), None);
    task.uid = "local-uid-1".to_string();
    task.calendar_href = LOCAL_CALENDAR_HREF.to_string();
    task.href = String::new(); // Local tasks often have empty hrefs or "local://..."
    task.etag = String::new(); // CRITICAL: Local tasks have empty ETags

    let mut tasks = vec![task.clone()];

    // 3. Simulate the "Focus" / "Refresh" operation
    // Use the context-aware Journal API to replay offline changes and prune ghosts.
    Journal::apply_to_tasks(&ctx, &mut tasks, LOCAL_CALENDAR_HREF);

    // 4. Assert Data Integrity
    assert_eq!(
        tasks.len(),
        1,
        "Catastrophic failure: Local task was pruned because it lacked an ETag!"
    );
    assert_eq!(tasks[0].uid, "local-uid-1");
}
