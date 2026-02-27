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
        Mutex::new(None::<Box<dyn Fn(&Action) -> Option<String> + Send + Sync + 'static>>)
    });
    {
        let mut g = hook.lock().unwrap();
        *g = Some(Box::new(move |action: &Action| match action {
            Action::Create(t) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
            Action::Update(t) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
            Action::Move(t, _) if t.uid == "sync-err-1" => Some("403 Forbidden".to_string()),
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
    let warnings = client.sync_journal().await.unwrap();

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

// --- MOVE TASK TESTS (previously inline) ---

#[tokio::test]
#[serial]
async fn test_move_task_verify_success_preserves_remote_and_deletes_local() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    let mut server = mockito::Server::new_async().await;
    let url = server.url();

    let mut task = Task::new("T1", &HashMap::new(), None);
    task.uid = "uid-123".to_string();
    task.calendar_href = "local://src".to_string();
    LocalStorage::save_for_href(ctx.as_ref(), "local://src", &[task.clone()]).unwrap();

    let dest = format!("{}/dest/", url);
    let expected_remote_href = format!("{}/{}.ics", dest.trim_end_matches('/'), task.uid);

    let _mock_put = server
        .mock("PUT", mockito::Matcher::Any)
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .create_async()
        .await;

    // Ensure the initializer has an explicit type so OnceLock can infer the contained Mutex<Option<...>> type.
    let hook = TEST_FETCH_REMOTE_HOOK.get_or_init(|| {
        Mutex::new(None::<Box<dyn Fn(&str) -> Option<Task> + Send + Sync + 'static>>)
    });
    {
        let mut guard = hook.lock().unwrap();
        let t_clone = task.clone();
        let expected_href_clone = expected_remote_href.clone();
        let dest_clone = dest.clone();
        *guard = Some(Box::new(move |href: &str| {
            if href == expected_href_clone {
                let mut rt = t_clone.clone();
                rt.calendar_href = dest_clone.clone();
                rt.href = expected_href_clone.clone();
                Some(rt)
            } else {
                None
            }
        }));
    }

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();
    let res = client.move_task(&task, &dest).await.unwrap();

    let src_tasks = LocalStorage::load_for_href(ctx.as_ref(), "local://src").unwrap();
    assert!(src_tasks.is_empty(), "Source should be deleted");
    assert_eq!(res.0.uid, "uid-123");

    if let Some(h) = TEST_FETCH_REMOTE_HOOK.get() {
        *h.lock().unwrap() = None;
    }
}

#[tokio::test]
#[serial]
async fn test_move_task_verify_failure_preserves_local() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    let mut server = mockito::Server::new_async().await;
    let url = server.url();

    let mut task = Task::new("T2", &HashMap::new(), None);
    task.uid = "uid-456".to_string();
    task.calendar_href = "local://src2".to_string();
    LocalStorage::save_for_href(ctx.as_ref(), "local://src2", &[task.clone()]).unwrap();

    let dest = format!("{}/dest2/", url);
    let expected_remote_href = format!("{}/{}.ics", dest.trim_end_matches('/'), task.uid);

    let _mock_put = server
        .mock("PUT", mockito::Matcher::Any)
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .create_async()
        .await;

    // Ensure the initializer has an explicit type so OnceLock can infer the contained Mutex<Option<...>> type.
    let hook = TEST_FETCH_REMOTE_HOOK.get_or_init(|| {
        Mutex::new(None::<Box<dyn Fn(&str) -> Option<Task> + Send + Sync + 'static>>)
    });
    {
        let mut guard = hook.lock().unwrap();
        let expected_href_clone = expected_remote_href.clone();
        *guard = Some(Box::new(move |href: &str| {
            assert_eq!(href, expected_href_clone);
            None
        }));
    }

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();
    let res = client.move_task(&task, &dest).await;
    assert!(res.is_err(), "Move should fail verification and return Err");

    let src_tasks = LocalStorage::load_for_href(ctx.as_ref(), "local://src2").unwrap();
    assert_eq!(src_tasks.len(), 1);
    assert_eq!(src_tasks[0].uid, "uid-456");

    if let Some(h) = TEST_FETCH_REMOTE_HOOK.get() {
        *h.lock().unwrap() = None;
    }
}
