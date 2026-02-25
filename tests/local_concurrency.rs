// File: tests/local_concurrency.rs
use cfait::context::TestContext;
use cfait::model::Task;
use cfait::storage::LocalStorage;
use serial_test::serial;
use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::thread;

/// This test simulates two concurrent UI instances modifying the same local collection file
/// at the same time to ensure the Read-Modify-Write pattern prevents data loss.
#[test]
#[serial] // Use serial to prevent interference with other I/O tests
fn test_concurrent_local_modifications_are_safe() {
    // 1. Setup: Create an isolated context and an initial local collection file
    let ctx = Arc::new(TestContext::new());
    let href = "local://concurrency-test";

    let mut task_a = Task::new("Task A (to be deleted)", &HashMap::new(), None);
    task_a.uid = "task-a".to_string();
    task_a.calendar_href = href.to_string();

    LocalStorage::save_for_href(ctx.as_ref(), href, &[task_a.clone()]).unwrap();

    // 2. Concurrency Setup: Barrier to synchronize threads for a race
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = vec![];

    // --- Thread 1: Adds a new task ---
    let add_ctx = ctx.clone();
    let add_barrier = barrier.clone();
    let add_handle = thread::spawn(move || {
        let mut task_b = Task::new("Task B (to be added)", &HashMap::new(), None);
        task_b.uid = "task-b".to_string();
        task_b.calendar_href = href.to_string();

        add_barrier.wait(); // Wait for delete thread to be ready

        // Use the safe RMW function to add the new task
        let res = LocalStorage::modify_for_href(add_ctx.as_ref(), href, |tasks| {
            tasks.push(task_b);
        });
        assert!(res.is_ok());
    });
    handles.push(add_handle);

    // --- Thread 2: Deletes the original task ---
    let delete_ctx = ctx.clone();
    let delete_barrier = barrier.clone();
    let delete_handle = thread::spawn(move || {
        delete_barrier.wait(); // Wait for add thread to be ready

        // Use the safe RMW function to delete the original task
        let res = LocalStorage::modify_for_href(delete_ctx.as_ref(), href, |tasks| {
            tasks.retain(|t| t.uid != "task-a");
        });
        assert!(res.is_ok());
    });
    handles.push(delete_handle);

    // 3. Wait for both threads to complete their operations
    for handle in handles {
        handle.join().unwrap();
    }

    // 4. Verification: Load the final state from disk
    let final_tasks = LocalStorage::load_for_href(ctx.as_ref(), href).unwrap();

    // The file lock should have serialized the operations, preserving both changes.
    // The final state should contain only Task B.
    assert_eq!(
        final_tasks.len(),
        1,
        "The final list should contain exactly one task"
    );
    assert_eq!(
        final_tasks[0].uid, "task-b",
        "The remaining task should be Task B"
    );
    assert!(
        !final_tasks.iter().any(|t| t.uid == "task-a"),
        "Task A should have been successfully deleted"
    );
}
