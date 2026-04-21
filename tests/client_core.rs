//! Tests for `client/core.rs`, moved from an inline module.
#![cfg(feature = "test_hooks")]
use cfait::client::core::RustyClient;
use cfait::journal::Action;
use cfait::journal::Journal;
use cfait::model::CalendarListEntry;
use cfait::model::Task;
use cfait::storage::{LocalCalendarRegistry, LocalStorage};
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Mutex;

// These hooks are necessary for some legacy tests. New tests should avoid them.
// The library exposes test hooks behind `test_hooks`; import them from there.
// Use the `test_hooks` module path which matches how the hooks are declared in the core module.
use cfait::client::core::{TEST_FETCH_REMOTE_HOOK, TEST_FORCE_SYNC_ERROR};

// --- RECOVERY TESTS (previously in a separate file) ---

#[tokio::test]
#[serial]
async fn test_sync_journal_moves_failed_task_to_recovery() {
    // Use an explicit TestContext for filesystem isolation (no global env var mutation)
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    // Ensure registry does not contain recovery calendar initially
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    regs.retain(|c| c.href != "local://recovery");
    LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();

    // Create a task and push a Create action into the journal
    let mut task = Task::new("WillFail", &HashMap::new(), None);
    task.uid = "sync-err-1".to_string();
    task.calendar_href = "https://example.com/cal/".to_string();
    task.summary = "Should fail and be recovered".to_string();

    Journal::push(ctx.as_ref(), Action::Create(task.clone())).unwrap();

    // Install test force hook to cause sync_journal to treat this action as a 403 error
    // Add explicit type annotations in the initializer so the compiler can infer the OnceLock inner type.
    let hook = TEST_FORCE_SYNC_ERROR.get_or_init(|| {
        Mutex::new(None::<Box<dyn Fn(&Action) -> Option<anyhow::Error> + Send + Sync + 'static>>)
    });
    {
        let mut g = hook.lock().unwrap();
        *g = Some(Box::new(move |action: &Action| match action {
            Action::Create(t) if t.uid == "sync-err-1" => Some(anyhow::anyhow!("403 Forbidden")),
            Action::Update(t) if t.uid == "sync-err-1" => Some(anyhow::anyhow!("403 Forbidden")),
            Action::Move(t, _) if t.uid == "sync-err-1" => Some(anyhow::anyhow!("403 Forbidden")),
            _ => None,
        }));
    }

    let client = RustyClient::new(
        ctx.clone(),
        "http://dummy.test",
        "user",
        "pass",
        false,
        None,
    )
    .unwrap();
    let (warnings, _synced): (Vec<String>, Vec<cfait::model::Task>) =
        client.sync_journal().await.unwrap();

    assert!(
        warnings
            .iter()
            .any(|w| w.contains("Recovery") || w.contains("Local (Recovery)")),
        "Expected a warning about recovery"
    );

    // Recovery storage should contain the task
    let recovered = LocalStorage::load_for_href(ctx.as_ref(), "local://recovery").unwrap();
    assert!(
        recovered.iter().any(|t| t.uid == "sync-err-1"),
        "Task should be moved to recovery"
    );

    // Cleanup hooks
    if let Some(h) = TEST_FETCH_REMOTE_HOOK.get() {
        *h.lock().unwrap() = None;
    }
    if let Some(h) = TEST_FORCE_SYNC_ERROR.get() {
        *h.lock().unwrap() = None;
    }
}

#[tokio::test]
#[serial]
async fn test_recovery_calendar_visibility() {
    // Use an explicit TestContext for filesystem isolation (no global env var mutation)
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    // Ensure registry does not contain recovery calendar initially
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    regs.retain(|c| c.href != "local://recovery");
    LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();

    // Use an offline client for this visibility test; attach the test ctx so client-side code
    // that expects a context can access it if needed.
    let client = RustyClient {
        client: None,
        ctx: ctx.clone(),
    };

    // When recovery calendar is not present and no tasks exist, it should not be visible
    let (cals, _) = client.get_calendars().await.unwrap();
    assert!(
        !cals.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should not be visible when absent and empty"
    );

    // Add recovery calendar entry to registry but no tasks -> still should not be visible
    let mut regs = LocalCalendarRegistry::load(ctx.as_ref()).unwrap_or_default();
    if !regs.iter().any(|c| c.href == "local://recovery") {
        regs.push(CalendarListEntry {
            name: "Local (Recovery)".to_string(),
            href: "local://recovery".to_string(),
            color: Some("#DB4437".to_string()),
        });
        LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();
    }

    let (cals2, _) = client.get_calendars().await.unwrap();
    assert!(
        !cals2.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should not be visible when present but empty"
    );

    // Add a recovered task to the recovery storage -> calendar should become visible
    let mut task = Task::new("Recovered", &HashMap::new(), None);
    task.uid = "rec-1".to_string();
    task.calendar_href = "local://recovery".to_string();
    LocalStorage::save_for_href(
        ctx.as_ref(),
        "local://recovery",
        std::slice::from_ref(&task),
    )
    .unwrap();

    let (cals3, _) = client.get_calendars().await.unwrap();
    assert!(
        cals3.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should be visible when it has tasks"
    );
}

// --- MOVE TASK TESTS (updated to use TaskController) ---

#[tokio::test]
#[serial]
async fn test_controller_move_local_to_remote() {
    // Use explicit TestContext for filesystem isolation
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    // Create an in-memory TaskStore and put a local task into it and into LocalStorage
    let store = std::sync::Arc::new(tokio::sync::Mutex::new(cfait::store::TaskStore::new(
        ctx.clone(),
    )));

    let mut task = Task::new("T1", &std::collections::HashMap::new(), None);
    task.uid = "uid-123".to_string();
    task.calendar_href = "local://src".to_string();

    // Persist to LocalStorage and add to the in-memory store
    cfait::storage::LocalStorage::save_for_href(ctx.as_ref(), "local://src", &[task.clone()])
        .unwrap();
    store.lock().await.add_task(task.clone());

    // Construct a TaskController with no network client (offline)
    let client_arc = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let controller = cfait::controller::TaskController::new(store.clone(), client_arc, ctx.clone());

    let dest = "https://example.com/cal/dest/";
    let res = controller.move_task(&task.uid, dest).await;
    assert!(
        res.is_ok(),
        "Expected controller move_task to succeed in queuing migration"
    );

    // Source should be deleted from local storage
    let src_tasks =
        cfait::storage::LocalStorage::load_for_href(ctx.as_ref(), "local://src").unwrap();
    assert!(
        src_tasks.is_empty(),
        "Source local storage should be deleted after move"
    );

    // Journal should now contain ONLY a Create(remote) because local Deletes
    // are executed immediately on disk and bypass the journal queue.
    let j = cfait::journal::Journal::load(ctx.as_ref());
    assert_eq!(j.queue.len(), 1, "Expected one action in journal (Create)");

    match &j.queue[0] {
        cfait::journal::Action::Create(t) => assert_eq!(t.calendar_href, dest),
        other => panic!("Expected action to be Create(remote), got: {:?}", other),
    }
}
