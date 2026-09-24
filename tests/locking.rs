// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for locking mechanism.
use cfait::client::RustyClient;
use cfait::context::{AppContext, TestContext};
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use cfait::storage::SyncLock;
use mockito::Server;
use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

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

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_sync_journal_waits_for_cross_process_lock() {
    use fs2::FileExt;

    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_path = "/cal/water-ferns.ics";

    let mock_put = server
        .mock("PUT", task_path)
        .expect(1)
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .create_async()
        .await;

    // Hold the cross-process sync lock from a separate thread, simulating
    // another cfait instance (e.g. the GUI) that is mid-sync.
    let lock_path = ctx.get_data_dir().unwrap().join("sync.lock");
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let holder = thread::spawn(move || {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        file.lock_exclusive().unwrap();
        ready_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        // Dropping the file releases the lock.
    });
    ready_rx.recv().unwrap();

    let mut task = Task::new("Water the ferns", &HashMap::new(), None);
    task.uid = "water-ferns".to_string();
    task.calendar_href = format!("{}/cal/", url);

    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();
    let sync = tokio::spawn(async move { client.sync_journal().await });

    // The sync must be blocked on the cross-process lock, not applying the
    // action to the server. (The mock's `.expect(1)` + final `assert()` then
    // proves the action was applied exactly once, after the release.)
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert!(
        !sync.is_finished(),
        "sync_journal should wait for the cross-process lock"
    );

    // Release the lock; the sync should now proceed and finish.
    release_tx.send(()).unwrap();
    holder.join().unwrap();

    let res = tokio::time::timeout(Duration::from_secs(30), sync)
        .await
        .expect("sync_journal did not finish after the lock was released")
        .expect("sync_journal panicked");
    assert!(res.is_ok(), "Sync failed: {:?}", res.err());
    mock_put.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.queue.is_empty(),
        "Journal should be empty after sync"
    );
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_sync_lock_gives_up_after_timeout() {
    use fs2::FileExt;

    let ctx = TestContext::new();

    // Hold the lock from a separate thread for the whole test.
    let lock_path = ctx.get_data_dir().unwrap().join("sync.lock");
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let holder = thread::spawn(move || {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        file.lock_exclusive().unwrap();
        ready_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    ready_rx.recv().unwrap();

    // A short timeout must give up (None) rather than block forever.
    let started = std::time::Instant::now();
    let lock = SyncLock::acquire(&ctx, Duration::from_millis(500)).await;
    assert!(lock.is_none(), "acquire should give up after the timeout");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "acquire should not block far beyond the timeout"
    );

    release_tx.send(()).unwrap();
    holder.join().unwrap();
}
