// SPDX-License-Identifier: GPL-3.0-or-later
//! Reproduces the delete-resurrection race between an in-flight `PersistBatch`
//! (a local delete) and an `OfflineRefresh` disk read.
//!
//! The network actor processes actions sequentially. When an `OfflineRefresh`
//! is dequeued *before* a queued `PersistBatch`, its `spawn_blocking` disk read
//! completes before the delete is written, so the resulting `FullStateReloaded`
//! still contains the deleted task. In the real UI the delete's `edit_generation`
//! bump predates the detection, so the `pending_refresh_generation` guard passes
//! and the full replace resurrects the task.
//!
//! These tests pin down that the *ordering* of the two actions is what decides
//! the outcome: bad order resurrects, good order does not.
use cfait::context::TestContext;
use cfait::model::Task;
use cfait::storage::LocalStorage;
use cfait::tui::action::{Action, AppEvent};
use cfait::tui::network::{NetworkActorConfig, run_network_actor};
use mockito::{Server, ServerGuard};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

const LOCAL_HREF: &str = "local://default";

fn make_local_task(uid: &str) -> Task {
    let mut task = Task::new("Water the ferns", &HashMap::new(), None);
    task.uid = uid.to_string();
    task.calendar_href = LOCAL_HREF.to_string();
    task.href = format!("{}/{}.ics", LOCAL_HREF, uid);
    task.etag = String::new();
    task
}

/// Reads events until a `FullStateReloaded` arrives, returning its contents.
async fn wait_for_full_state_reload(
    event_rx: &mut mpsc::Receiver<AppEvent>,
) -> Vec<(String, Vec<Task>)> {
    let mut errors: Vec<String> = Vec::new();
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(10), event_rx.recv()).await {
            Ok(Some(AppEvent::FullStateReloaded(results))) => return results,
            Ok(Some(AppEvent::Error(e))) => {
                errors.push(e);
                continue;
            }
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed before FullStateReloaded"),
            Err(_) => panic!(
                "Timeout waiting for FullStateReloaded. Actor errors: {:?}",
                errors
            ),
        }
    }
}

fn contains_uid(results: &[(String, Vec<Task>)], uid: &str) -> bool {
    results
        .iter()
        .any(|(href, tasks)| href == LOCAL_HREF && tasks.iter().any(|t| t.uid == uid))
}

/// Creates a mock CalDAV server (empty multistatus) and returns it alongside
/// its base URL. The caller must keep the server alive for the test duration.
async fn new_mock_server() -> (ServerGuard, String) {
    let mut server = Server::new_async().await;
    let url = server.url();
    let _m_root = server
        .mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;
    (server, url)
}

/// Spawns the network actor against the given (already-running) mock server
/// URL and waits until it is ready. Returns the action/event handles plus the
/// actor handle.
async fn spawn_ready_actor(
    ctx: Arc<TestContext>,
    url: String,
) -> (
    mpsc::Sender<Action>,
    mpsc::Receiver<AppEvent>,
    tokio::task::JoinHandle<()>,
) {
    let app_config = cfait::config::Config {
        sync_settings: false,
        ..Default::default()
    };
    app_config.save(ctx.as_ref()).unwrap();

    let (action_tx, action_rx) = mpsc::channel(100);
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let config = NetworkActorConfig {
        url,
        user: "user".into(),
        pass: "pass".into(),
        allow_insecure: true,
        enable_local_mode: true,
        default_cal: None,
    };

    let actor_handle = tokio::spawn(async move {
        run_network_actor(ctx.clone(), config, action_rx, event_tx).await;
    });

    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(10), event_rx.recv()).await {
            Ok(Some(AppEvent::Status { key, .. })) if key == "ready" => {
                return (action_tx, event_rx, actor_handle);
            }
            Ok(Some(_)) => continue,
            Ok(None) => panic!("Actor channel closed before ready"),
            Err(_) => panic!("Timeout waiting for actor ready"),
        }
    }
}

/// BAD ORDER: `OfflineRefresh` is dequeued before the delete's `PersistBatch`.
/// The disk read completes before the delete write, so the `FullStateReloaded`
/// still contains the task -> it resurrects in the UI.
#[tokio::test]
async fn offline_refresh_before_delete_resurrects_task() {
    let test_ctx = TestContext::new();
    let ctx = Arc::new(test_ctx);

    // Seed a task on disk as if created by another cfait instance.
    let task = make_local_task("fern-1");
    LocalStorage::save_for_href(ctx.as_ref(), LOCAL_HREF, std::slice::from_ref(&task)).unwrap();

    let (_server, url) = new_mock_server().await;
    let (action_tx, mut event_rx, actor_handle) = spawn_ready_actor(ctx, url).await;

    action_tx.send(Action::OfflineRefresh).await.unwrap();
    action_tx
        .send(Action::PersistBatch(vec![cfait::journal::Action::Delete(
            task.clone(),
        )]))
        .await
        .unwrap();

    let results = wait_for_full_state_reload(&mut event_rx).await;
    assert!(
        contains_uid(&results, "fern-1"),
        "expected the resurrection: OfflineRefresh read disk before the delete write, \
         so the deleted task is still present in the FullStateReloaded"
    );

    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
}

/// GOOD ORDER: the delete's `PersistBatch` is dequeued before the
/// `OfflineRefresh`. The delete write completes before the disk read, so the
/// `FullStateReloaded` does NOT contain the task -> no resurrection.
#[tokio::test]
async fn delete_before_offline_refresh_does_not_resurrect() {
    let test_ctx = TestContext::new();
    let ctx = Arc::new(test_ctx);

    let task = make_local_task("fern-2");
    LocalStorage::save_for_href(ctx.as_ref(), LOCAL_HREF, std::slice::from_ref(&task)).unwrap();

    let (_server, url) = new_mock_server().await;
    let (action_tx, mut event_rx, actor_handle) = spawn_ready_actor(ctx, url).await;

    action_tx
        .send(Action::PersistBatch(vec![cfait::journal::Action::Delete(
            task.clone(),
        )]))
        .await
        .unwrap();
    action_tx.send(Action::OfflineRefresh).await.unwrap();

    let results = wait_for_full_state_reload(&mut event_rx).await;
    assert!(
        !contains_uid(&results, "fern-2"),
        "the delete write should complete before the OfflineRefresh disk read, \
         so the task must NOT be present in the FullStateReloaded"
    );

    let _ = action_tx.send(Action::Quit).await;
    let _ = actor_handle.await;
}

#[cfg(feature = "gui")]
mod gui_worker {
    //! The GUI background worker shares the watcher + suppression logic with the
    //! TUI actor, plus its own `FlushAndLoad` disk reload. These tests cover the
    //! worker side: the reload surfaces external disk state, and the watcher
    //! distinguishes our own writes from external ones.
    use super::{LOCAL_HREF, make_local_task};
    use cfait::config::Config;
    use cfait::context::{AppContext, TestContext};
    use cfait::gui::async_ops::{WorkerCommand, spawn_background_worker};
    use cfait::gui::message::Message;
    use cfait::model::Task;
    use cfait::storage::LocalStorage;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn spawn_worker(
        ctx: Arc<TestContext>,
    ) -> (mpsc::Sender<WorkerCommand>, mpsc::Receiver<Message>) {
        let (ui_tx, ui_rx) = mpsc::channel(100);
        let cmd_tx = spawn_background_worker(ui_tx, ctx);
        (cmd_tx, ui_rx)
    }

    /// Receives messages until `ExternalReloaded` arrives (skipping anything
    /// else), with a generous timeout.
    async fn wait_for_external_reloaded(
        ui_rx: &mut mpsc::Receiver<Message>,
    ) -> (
        Box<Config>,
        Vec<cfait::model::CalendarListEntry>,
        Vec<(String, Vec<Task>)>,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            match ui_rx.recv().await {
                Some(Message::ExternalReloaded(cfg, cals, tasks)) => return (cfg, cals, tasks),
                Some(_) => continue,
                None => panic!("Worker channel closed before ExternalReloaded"),
            }
        }
        panic!("Timeout waiting for ExternalReloaded");
    }

    /// `FlushAndLoad` must surface the state another cfait instance wrote to
    /// disk: the config, the calendar list, and the tasks themselves.
    #[tokio::test]
    async fn flush_and_load_reloads_external_disk_state() {
        let ctx = Arc::new(TestContext::new());
        Config {
            enable_local_mode: true,
            sync_settings: false,
            ..Default::default()
        }
        .save(ctx.as_ref())
        .unwrap();

        let (cmd_tx, mut ui_rx) = spawn_worker(ctx.clone());

        // Simulate a task created by another cfait instance (e.g. the CLI).
        let task = make_local_task("fern-gui");
        LocalStorage::save_for_href(ctx.as_ref(), LOCAL_HREF, std::slice::from_ref(&task)).unwrap();

        cmd_tx.send(WorkerCommand::FlushAndLoad).await.unwrap();

        let (cfg, _cals, tasks) = wait_for_external_reloaded(&mut ui_rx).await;
        assert!(
            cfg.enable_local_mode,
            "reloaded config must keep local mode enabled"
        );
        assert!(
            tasks.iter().any(|(href, list)| {
                href == LOCAL_HREF && list.iter().any(|t| t.uid == "fern-gui")
            }),
            "FlushAndLoad must include the externally created task"
        );

        drop(cmd_tx); // stops the worker
    }

    /// The watcher must fire for files written by *other* processes, but must
    /// stay silent for files this process just wrote via `atomic_write`
    /// (per-file suppression). The probe write first proves the watcher is
    /// live, so the silence assertion is not vacuous.
    #[tokio::test]
    async fn watcher_fires_for_external_writes_but_not_own_writes() {
        let ctx = Arc::new(TestContext::new());
        Config {
            enable_local_mode: true,
            sync_settings: false,
            ..Default::default()
        }
        .save(ctx.as_ref())
        .unwrap();

        let (cmd_tx, mut ui_rx) = spawn_worker(ctx.clone());
        let data_dir = ctx.get_data_dir().unwrap();

        // Phase 1: an external write (raw fs write, not recorded as ours) must
        // produce an ExternalChangeDetected. The probe is rewritten at most once
        // per second until the event is seen, so a slow watcher startup cannot
        // make the write land before watching begins.
        let probe = data_dir.join("probe_external.json");
        let mut last_write = std::time::Instant::now() - std::time::Duration::from_secs(5);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut saw_external = false;
        while !saw_external && std::time::Instant::now() < deadline {
            if std::time::Instant::now() - last_write > std::time::Duration::from_secs(1) {
                std::fs::write(&probe, b"[]").unwrap();
                last_write = std::time::Instant::now();
            }
            match tokio::time::timeout(std::time::Duration::from_millis(50), ui_rx.recv()).await {
                Ok(Some(Message::ExternalChangeDetected)) => saw_external = true,
                Ok(Some(_)) => continue,
                Ok(None) => panic!("Worker channel closed"),
                Err(_) => continue,
            }
        }
        assert!(saw_external, "watcher must detect the external file write");

        // Phase 2: a write made by this process (through atomic_write) must NOT
        // produce an ExternalChangeDetected.
        let task = make_local_task("fern-own");
        LocalStorage::save_for_href(ctx.as_ref(), LOCAL_HREF, std::slice::from_ref(&task)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(50), ui_rx.recv()).await {
                Ok(Some(Message::ExternalChangeDetected)) => {
                    panic!("own write must be suppressed, got ExternalChangeDetected")
                }
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        drop(cmd_tx); // stops the worker
    }
}

#[cfg(feature = "gui")]
mod gui_coalesce {
    //! The GUI must never drop a load-triggering message while a load is in
    //! flight: `ExternalChangeDetected` and `Refresh` arriving during a load
    //! are coalesced and re-run once the load finishes. These tests drive the
    //! message handler directly (no iced runtime needed).
    use cfait::context::TestContext;
    use cfait::gui::message::Message;
    use cfait::gui::state::GuiApp;
    use cfait::gui::update::network::handle;
    use std::sync::Arc;

    fn new_app() -> GuiApp {
        // The default context is a real StandardContext; swap in a sandboxed
        // one so no test ever touches the user's data dir.
        GuiApp {
            ctx: Arc::new(TestContext::new()),
            ..Default::default()
        }
    }

    /// A load is in flight when an external change is detected: the change is
    /// remembered, not dropped, and the in-flight load is allowed to finish.
    #[test]
    fn external_change_during_load_is_coalesced() {
        let mut app = new_app();
        app.loading = true;

        let task = handle(&mut app, Message::ExternalChangeDetected);
        assert!(app.loading, "in-flight load must not be disturbed");
        assert!(app.pending_external_reload, "change must be remembered");

        // The in-flight load finishes: the coalesced reload must start.
        let follow_up = handle(&mut app, Message::LocalLoaded(Ok((vec![], vec![]))));
        assert!(!app.pending_external_reload, "flag must be consumed");
        assert!(
            app.loading,
            "coalesced external reload must now be in flight"
        );
        let _ = (task, follow_up);
    }

    /// An external change detected while idle starts a reload immediately.
    #[test]
    fn external_change_when_idle_reloads_immediately() {
        let mut app = new_app();
        app.loading = false;

        let task = handle(&mut app, Message::ExternalChangeDetected);
        assert!(!app.pending_external_reload);
        assert!(app.loading, "reload must start immediately");
        let _ = task;
    }

    /// A manual refresh requested during a load is coalesced and re-dispatched
    /// when the load finishes.
    #[test]
    fn refresh_during_load_is_coalesced() {
        let mut app = new_app();
        app.loading = true;

        let task = handle(&mut app, Message::Refresh);
        assert!(app.loading);
        assert!(app.pending_refresh);

        // The load finishes; iced then processes the re-dispatched refresh,
        // which we simulate by handling the message directly.
        let follow_up = handle(&mut app, Message::LocalLoaded(Ok((vec![], vec![]))));
        assert!(!app.pending_refresh, "flag must be consumed");
        assert!(
            !app.loading,
            "loading released until the re-dispatched refresh runs"
        );
        let refresh_task = handle(&mut app, Message::Refresh);
        assert!(app.loading, "coalesced refresh must start a new load");
        let _ = (task, follow_up, refresh_task);
    }
}
