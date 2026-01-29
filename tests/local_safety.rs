use cfait::context::TestContext;
use cfait::journal::Journal;
use cfait::model::Task;
use cfait::storage::LOCAL_CALENDAR_HREF;
use std::collections::HashMap;

#[test]
fn test_local_tasks_are_not_pruned_as_ghosts() {
    let ctx = TestContext::new();

    let mut task = Task::new("Buy Milk", &HashMap::new(), None);
    task.uid = "local-uid-1".to_string();
    task.calendar_href = LOCAL_CALENDAR_HREF.to_string();
    task.href = String::new();
    task.etag = String::new();

    let mut tasks = vec![task.clone()];

    // This calls Journal::apply_to_tasks to replay offline changes and prune ghosts.
    // It should NOT prune the local task because it's in a local calendar.
    Journal::apply_to_tasks(&ctx, &mut tasks, LOCAL_CALENDAR_HREF);

    assert_eq!(
        tasks.len(),
        1,
        "Catastrophic failure: Local task was pruned because it lacked an ETag!"
    );
    assert_eq!(tasks[0].uid, "local-uid-1");
}
