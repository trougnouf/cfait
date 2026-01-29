use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn test_concurrent_journal_writes() {
    // 1. Setup Isolation
    // TestContext creates a unique temp dir and cleans it up on drop
    let ctx = Arc::new(TestContext::new());

    // 2. Setup Barrier to ensure threads start writing exactly at the same time
    let thread_count = 10;
    let barrier = Arc::new(Barrier::new(thread_count));

    let mut handles = vec![];

    for i in 0..thread_count {
        let b = barrier.clone();
        let thread_ctx = ctx.clone();
        let handle = thread::spawn(move || {
            b.wait(); // Wait for everyone to be ready

            let mut task = Task::new(&format!("Task {}", i), &HashMap::new(), None);
            task.uid = format!("uid-{}", i);

            // Pass the context explicitly
            let res = Journal::push(thread_ctx.as_ref(), Action::Create(task));
            assert!(res.is_ok(), "Journal push failed in thread {}", i);
        });
        handles.push(handle);
    }

    // 3. Wait for all threads
    for h in handles {
        h.join().unwrap();
    }

    // 4. Verify Data Integrity
    let journal = Journal::load(ctx.as_ref());

    assert_eq!(
        journal.queue.len(),
        thread_count,
        "Journal should contain exactly {} items",
        thread_count
    );

    // Verify no duplicates and all UIDs present
    let uids: Vec<String> = journal
        .queue
        .iter()
        .map(|a| match a {
            Action::Create(t) => t.uid.clone(),
            _ => "".to_string(),
        })
        .collect();

    for i in 0..thread_count {
        assert!(
            uids.contains(&format!("uid-{}", i)),
            "Journal missing uid-{}",
            i
        );
    }
}
