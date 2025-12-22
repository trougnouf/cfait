use cfait::journal::Journal;
use cfait::model::Task;
use cfait::storage::LOCAL_CALENDAR_HREF;
use std::collections::HashMap;
use std::env;
use std::fs;

#[test]
fn test_local_tasks_are_not_pruned_as_ghosts() {
    // 1. Setup isolated environment
    let temp_dir = env::temp_dir().join(format!("cfait_test_local_loss_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // Set the env var so the Journal looks in our temp dir (though we aren't loading from disk here,
    // it's good practice for isolation if internal logic checks paths).
    unsafe {
        env::set_var("CFAIT_TEST_DIR", &temp_dir);
    }

    // 2. Create a "Local" task
    // Local tasks are stored on disk without an ETag.
    let mut task = Task::new("Buy Milk", &HashMap::new());
    task.uid = "local-uid-1".to_string();
    task.calendar_href = LOCAL_CALENDAR_HREF.to_string();
    task.href = String::new(); // Local tasks often have empty hrefs or "local://..."
    task.etag = String::new(); // CRITICAL: Local tasks have empty ETags

    let mut tasks = vec![task.clone()];

    // 3. Simulate the "Focus" / "Refresh" operation
    // This calls Journal::apply_to_tasks to replay offline changes and prune ghosts.
    Journal::apply_to_tasks(&mut tasks, LOCAL_CALENDAR_HREF);

    // 4. Assert Data Integrity
    // BEFORE FIX: This fails because the task is removed (size 0).
    // AFTER FIX: This passes.
    assert_eq!(
        tasks.len(),
        1,
        "Catastrophic failure: Local task was pruned because it lacked an ETag!"
    );
    assert_eq!(tasks[0].uid, "local-uid-1");

    // Cleanup
    unsafe {
        env::remove_var("CFAIT_TEST_DIR");
    }
    let _ = fs::remove_dir_all(&temp_dir);
}
