// SPDX-License-Identifier: GPL-3.0-or-later
//! The GUI background worker must debounce user changes into a single sync
//! that empties the journal (so the unsynced indicator can clear), it must
//! not be starved by a continuous stream of external file events, and it
//! must report failed syncs instead of swallowing them.
#![cfg(feature = "gui")]

use cfait::client::RustyClient;
use cfait::context::{AppContext, TestContext};
use cfait::gui::async_ops::{WorkerCommand, spawn_background_worker};
use cfait::gui::message::Message;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Receives messages until `is_done` matches one or the timeout expires.
/// Returns whether the target message was seen.
async fn wait_for(
    ui_rx: &mut mpsc::Receiver<Message>,
    timeout: std::time::Duration,
    is_done: impl Fn(&Message) -> bool,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let remaining = deadline
            .saturating_duration_since(std::time::Instant::now())
            .max(std::time::Duration::from_millis(1));
        match tokio::time::timeout(remaining, ui_rx.recv()).await {
            Ok(Some(msg)) if is_done(&msg) => return true,
            Ok(Some(_)) | Err(_) => {}
            Ok(None) => return false,
        }
    }
    false
}

/// Waits for `BackgroundSyncComplete` (or a timeout) and returns whether it
/// was seen, whether a failure was seen first, and the messages observed.
async fn wait_for_sync_complete(ui_rx: &mut mpsc::Receiver<Message>) -> (bool, bool, Vec<String>) {
    let mut saw_complete = false;
    let mut saw_failed = false;
    let mut notes = Vec::new();
    for _ in 0..200 {
        match tokio::time::timeout(std::time::Duration::from_millis(100), ui_rx.recv()).await {
            Ok(Some(Message::JournalSaved)) => notes.push("JournalSaved".to_string()),
            Ok(Some(Message::BackgroundSyncComplete(_))) => {
                saw_complete = true;
                break;
            }
            Ok(Some(Message::BackgroundSyncFailed)) => {
                saw_failed = true;
                notes.push("BackgroundSyncFailed".to_string());
            }
            Ok(Some(msg)) => notes.push(format!("{msg:?}")),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    (saw_complete, saw_failed, notes)
}

/// Builds a worker wired to a mock server, sends an Update for a fern task,
/// and returns the handles the tests need.
async fn worker_with_pending_update(
    ctx: Arc<TestContext>,
    server: &mut Server,
    put_status: usize,
) -> (
    mpsc::Sender<WorkerCommand>,
    mpsc::Receiver<Message>,
    String,
    mockito::Mock,
) {
    let task_uid = "fern-watering";
    let task_path = format!("/cal/{task_uid}.ics");
    let mock_put = server
        .mock("PUT", task_path.as_str())
        .with_status(put_status)
        .with_header("ETag", "etag-2")
        .create_async()
        .await;

    let client =
        RustyClient::new(ctx.clone(), &server.url(), "gardener", "seeds", true, None).unwrap();

    let (ui_tx, ui_rx) = mpsc::channel::<Message>(100);
    let worker_tx = spawn_background_worker(ui_tx, ctx.clone());
    worker_tx
        .send(WorkerCommand::UpdateClient(Some(client)))
        .await
        .unwrap();

    // Simulate a user change: updating a remote task.
    let mut task = Task::new("Water the ferns", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = "/cal/".to_string();
    task.href = task_path.clone();
    task.etag = "etag-1".to_string();
    worker_tx
        .send(WorkerCommand::Batch(vec![Action::Update(task)]))
        .await
        .unwrap();

    (worker_tx, ui_rx, task_uid.to_string(), mock_put)
}

#[tokio::test]
async fn test_worker_debounced_sync_empties_journal() {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let (_worker_tx, mut ui_rx, _task_uid, _mock_put) =
        worker_with_pending_update(ctx.clone(), &mut server, 204).await;

    let (saw_complete, saw_failed, notes) = wait_for_sync_complete(&mut ui_rx).await;

    let journal = Journal::load(ctx.as_ref());
    assert!(
        saw_complete,
        "worker never reported a completed sync (saw {notes:?})"
    );
    assert!(!saw_failed, "worker reported a failed sync (saw {notes:?})");
    assert!(
        journal.is_empty(),
        "journal must be empty after the debounced sync, got {:?}",
        journal.queue
    );
}

#[tokio::test]
async fn test_worker_sync_not_starved_by_watcher_events() {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let (_worker_tx, mut ui_rx, _task_uid, _mock_put) =
        worker_with_pending_update(ctx.clone(), &mut server, 204).await;

    // Hammer the cache dir with external .json writes, like a synced folder
    // or another cfait instance. The old worker recreated its 500ms sync
    // sleep on every watcher event, so the stream below reset the window
    // forever and the sync never fired. The stream outlasts the wait window
    // on purpose: once it stops, even the starved worker would eventually
    // sync.
    let decoy = ctx.get_cache_dir().unwrap().join("decoy_external.json");
    let hammer = tokio::spawn(async move {
        for i in 0..60u32 {
            let _ = std::fs::write(&decoy, format!("watering round {i}"));
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    // The fixed worker syncs ~500ms after the batch; the starved worker
    // never does while the stream keeps flowing.
    let saw_complete = wait_for(&mut ui_rx, std::time::Duration::from_secs(5), |m| {
        matches!(m, Message::BackgroundSyncComplete(_))
    })
    .await;
    let _ = hammer.await;

    assert!(
        saw_complete,
        "watcher event stream starved the debounced sync"
    );
    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "journal must be empty after the sync"
    );
}

#[tokio::test]
async fn test_worker_failed_sync_reports_failure_and_keeps_journal() {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let (_worker_tx, mut ui_rx, _task_uid, _mock_put) =
        worker_with_pending_update(ctx.clone(), &mut server, 500).await;

    // The server rejects the update (e.g. a transient 500). The worker must
    // report the failure — previously it only eprintln'd, so the GUI kept
    // showing a quiet, unexplained unsynced state — and keep the change
    // queued for the retry.
    let saw_failed = wait_for(&mut ui_rx, std::time::Duration::from_secs(10), |m| {
        matches!(m, Message::BackgroundSyncFailed)
    })
    .await;

    assert!(saw_failed, "worker never reported the failed sync");
    let journal = Journal::load(ctx.as_ref());
    assert!(
        !journal.is_empty(),
        "failed sync must keep the change queued for retry, journal: {:?}",
        journal.queue
    );
}
