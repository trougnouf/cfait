//! Tests for `client/core.rs`, moved from an inline module.
#![cfg(feature = "test_hooks")]
use cfait::client::core::RustyClient;
use cfait::config::{SETTINGS_SUMMARY, SETTINGS_UID, SettingsPayload};
use cfait::journal::Action;
use cfait::journal::Journal;
use cfait::model::{CalendarListEntry, IcsAdapter, Task, TaskStatus};
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
    let cals = client.get_calendars().await.unwrap();
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
            supports_vjournal: Some(true),
            supports_vevent: Some(true),
        });
        LocalCalendarRegistry::save(ctx.as_ref(), &regs).unwrap();
    }

    let cals2 = client.get_calendars().await.unwrap();
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

    let cals3 = client.get_calendars().await.unwrap();
    assert!(
        cals3.iter().any(|c| c.href == "local://recovery"),
        "Recovery calendar should be visible when it has tasks"
    );
}

#[tokio::test]
#[serial]
async fn test_calendar_creation_and_updating() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());
    let mut server = mockito::Server::new_async().await;
    let url = server.url();

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // Mock principal discovery
    let _mock_principal = server.mock("PROPFIND", "/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/</d:href><d:propstat><d:prop><d:current-user-principal><d:href>/user/</d:href></d:current-user-principal></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async().await;

    let _mock_home = server.mock("PROPFIND", "/user/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>/user/</d:href><d:propstat><d:prop><c:calendar-home-set><d:href>/cal/</d:href></c:calendar-home-set></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async().await;

    let _mock_mkcal = server
        .mock("MKCALENDAR", mockito::Matcher::Any)
        .with_status(201)
        .create_async()
        .await;

    let _mock_proppatch = server
        .mock("PROPPATCH", mockito::Matcher::Any)
        .with_status(200)
        .create_async()
        .await;

    let new_cal = client
        .create_calendar("New Cal", Some("#FF0000"))
        .await
        .unwrap();
    assert!(new_cal.starts_with("/cal/"));

    client
        .update_calendar(&new_cal, "Updated Cal", None)
        .await
        .unwrap();
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
    let (orig, mut updated) = store
        .lock()
        .await
        .move_task(&task.uid, dest.to_string())
        .unwrap();
    updated.href = String::new();
    updated.etag = String::new();
    let res = controller
        .persist_changes(vec![
            cfait::journal::Action::Delete(orig),
            cfait::journal::Action::Create(updated),
        ])
        .await;
    assert!(
        res.is_ok(),
        "Expected controller persist_changes to succeed in queuing migration"
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

// --- GET_ALL_TASKS FALLBACK TESTS ---

#[tokio::test]
#[serial]
async fn test_get_all_tasks_falls_back_to_cache_when_server_fails() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());

    // Seed the on-disk cache with a task for the remote calendar. A real
    // cached task was fetched from the server, so it carries an etag; without
    // one it would be treated as an unsynced "ghost" and filtered out.
    let mut cached = Task::new("Water the tomatoes", &HashMap::new(), None);
    cached.uid = "cached-1".to_string();
    cached.calendar_href = "https://example.com/cal/".to_string();
    cached.etag = "etag-1".to_string();
    cfait::cache::Cache::save(
        ctx.as_ref(),
        "https://example.com/cal/",
        std::slice::from_ref(&cached),
        Some("tok-1".to_string()),
    )
    .unwrap();

    // Seed a local calendar too; local fetches never hit the network.
    let mut local = Task::new("Read a chapter", &HashMap::new(), None);
    local.uid = "local-1".to_string();
    local.calendar_href = "local://garden".to_string();
    LocalStorage::save_for_href(ctx.as_ref(), "local://garden", std::slice::from_ref(&local))
        .unwrap();

    // A mock server that answers nothing: every request fails, so the remote
    // fetch must error out and fall back to the cached tasks instead of
    // silently dropping the calendar.
    let server = mockito::Server::new_async().await;
    let client = RustyClient::new(ctx.clone(), &server.url(), "u", "p", true, None).unwrap();

    let calendars = vec![
        CalendarListEntry {
            name: "Garden".to_string(),
            href: "local://garden".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
        CalendarListEntry {
            name: "Remote".to_string(),
            href: "https://example.com/cal/".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
    ];

    let results = client.get_all_tasks(&calendars).await.unwrap();

    let local_tasks = results
        .iter()
        .find(|(href, _)| href == "local://garden")
        .map(|(_, t)| t.clone());
    assert!(
        local_tasks
            .as_ref()
            .is_some_and(|t| t.iter().any(|t| t.uid == "local-1")),
        "Local calendar should be present with its task"
    );

    let remote_tasks = results
        .iter()
        .find(|(href, _)| href == "https://example.com/cal/")
        .map(|(_, t)| t.clone());
    assert!(
        remote_tasks
            .as_ref()
            .is_some_and(|t| t.iter().any(|t| t.uid == "cached-1")),
        "Failed remote calendar should fall back to its cached tasks"
    );
}

// --- SETTINGS CARRIER CONFLICT TESTS ---

/// Build a settings carrier task with the given payload revision and etag.
fn carrier_task(url: &str, updated_at: i64, etag: &str) -> Task {
    let mut task = Task::new(SETTINGS_SUMMARY, &HashMap::new(), None);
    task.uid = SETTINGS_UID.to_string();
    task.calendar_href = format!("{}/cal/", url);
    task.href = format!("{}/cal/{}.ics", url, SETTINGS_UID);
    task.etag = etag.to_string();
    task.is_event = true;
    task.status = TaskStatus::Cancelled;
    task.description = serde_json::to_string(&SettingsPayload {
        updated_at,
        config: Default::default(),
    })
    .expect("serialize payload");
    task
}

/// A calendar-multiget (REPORT) response serving `ics` at `path` with `etag`.
fn multiget_body(path: &str, ics: &str, etag: &str) -> String {
    format!(
        r#"<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav"><d:response><d:href>{}</d:href><d:propstat><d:prop><cal:calendar-data>{}</cal:calendar-data><d:getetag>"{}"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#,
        path, ics, etag
    )
}

/// A 412 on the settings carrier whose server copy carries a newer payload
/// revision must be resolved last-write-wins by adopting the server version:
/// no re-PUT, no conflict copy, no rescue.
#[tokio::test]
#[serial]
async fn test_carrier_412_newer_server_wins() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());
    let mut server = mockito::Server::new_async().await;
    let url = server.url();
    let path = format!("/cal/{}.ics", SETTINGS_UID);

    // 1. The update PUT 412s (another device wrote newer settings).
    let mock_412 = server
        .mock("PUT", path.as_str())
        .match_header("If-Match", "\"local-etag\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // 2. The server copy holds the newer payload revision.
    let server_carrier = carrier_task(&url, 2000, "server-etag");
    let server_ics = IcsAdapter::to_ics(&server_carrier);
    let mock_fetch = server
        .mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(multiget_body(&path, &server_ics, "server-etag"))
        .expect(1)
        .create_async()
        .await;

    // 3. The local copy must not be pushed back (no wildcard re-PUT).
    let mock_reput = server
        .mock("PUT", path.as_str())
        .match_header("If-Match", "*")
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", false, None).unwrap();

    Journal::push(
        ctx.as_ref(),
        Action::Update(carrier_task(&url, 1000, "local-etag")),
    )
    .unwrap();

    let (warnings, synced) = client.sync_journal().await.expect("sync_journal");

    mock_412.assert();
    mock_fetch.assert();
    mock_reput.assert();

    assert!(
        warnings
            .iter()
            .any(|w| w.contains("adopting the server version")),
        "a carrier conflict must report the LWW resolution: {:?}",
        warnings
    );
    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "the action must be popped, not parked or duplicated"
    );

    let adopted = synced
        .iter()
        .find(|t| t.uid == SETTINGS_UID)
        .expect("carrier reported as synced");
    assert_eq!(adopted.etag, "\"server-etag\"");
    let payload: SettingsPayload = serde_json::from_str(&adopted.description).unwrap();
    assert_eq!(
        payload.updated_at, 2000,
        "the newer server payload must win"
    );
}

/// A 412 on the settings carrier whose server copy carries an OLDER payload
/// revision must be resolved by re-PUTting the local payload (wildcard
/// precondition), with no conflict copy.
#[tokio::test]
#[serial]
async fn test_carrier_412_newer_local_reputs() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());
    let mut server = mockito::Server::new_async().await;
    let url = server.url();
    let path = format!("/cal/{}.ics", SETTINGS_UID);

    // 1. The update PUT 412s.
    let mock_412 = server
        .mock("PUT", path.as_str())
        .match_header("If-Match", "\"local-etag\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // 2. The server copy holds an older revision.
    let server_carrier = carrier_task(&url, 500, "server-etag");
    let server_ics = IcsAdapter::to_ics(&server_carrier);
    let mock_fetch = server
        .mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(multiget_body(&path, &server_ics, "server-etag"))
        .expect(1)
        .create_async()
        .await;

    // 3. The local payload wins: re-PUT with If-Match: *.
    let mock_reput = server
        .mock("PUT", path.as_str())
        .match_header("If-Match", "*")
        .match_body(mockito::Matcher::Regex(SETTINGS_UID.to_string()))
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .expect(1)
        .create_async()
        .await;

    // 4. The carrier must never spawn a user-visible conflict copy.
    let mock_conflict_copy = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_body(mockito::Matcher::Regex("Conflict Copy".to_string()))
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", false, None).unwrap();

    Journal::push(
        ctx.as_ref(),
        Action::Update(carrier_task(&url, 1000, "local-etag")),
    )
    .unwrap();

    let (_, synced) = client.sync_journal().await.expect("sync_journal");

    mock_412.assert();
    mock_fetch.assert();
    mock_reput.assert();
    mock_conflict_copy.assert();

    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "the action must be popped, not parked or duplicated"
    );
    let pushed = synced
        .iter()
        .find(|t| t.uid == SETTINGS_UID)
        .expect("carrier reported as synced");
    assert_eq!(pushed.etag, "\"new-etag\"");
    let payload: SettingsPayload = serde_json::from_str(&pushed.description).unwrap();
    assert_eq!(payload.updated_at, 1000, "the newer local payload must win");
}

/// A failure on the settings carrier (5xx, transient network error) must not
/// block the queue: the carrier action is dropped with a warning and the
/// following user tasks keep syncing (head-of-line blocking fix,
/// https://github.com/trougnouf/cfait/issues/123).
#[tokio::test]
#[serial]
async fn test_carrier_failure_does_not_block_user_tasks() {
    let ctx = std::sync::Arc::new(cfait::context::TestContext::new());
    let mut server = mockito::Server::new_async().await;
    let url = server.url();

    // The user task's creation succeeds.
    let mock_user_put = server
        .mock("PUT", "/cal/user-task-1.ics")
        .match_header("If-None-Match", "*")
        .with_status(201)
        .with_header("ETag", "\"user-etag\"")
        .expect(1)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", false, None).unwrap();

    let carrier = carrier_task(&url, 1000, "local-etag");
    let mut user = Task::new("water the ferns", &HashMap::new(), None);
    user.uid = "user-task-1".to_string();
    user.calendar_href = format!("{}/cal/", url);

    Journal::push(ctx.as_ref(), Action::Create(carrier)).unwrap();
    Journal::push(ctx.as_ref(), Action::Create(user)).unwrap();

    // Force a transient failure on the carrier only (no fatal status code in
    // the message, so sync_journal maps it to a plain Err).
    let hook = TEST_FORCE_SYNC_ERROR.get_or_init(|| {
        Mutex::new(None::<Box<dyn Fn(&Action) -> Option<anyhow::Error> + Send + Sync + 'static>>)
    });
    {
        let mut g = hook.lock().unwrap();
        *g = Some(Box::new(move |action: &Action| match action {
            Action::Create(t) if t.uid == SETTINGS_UID => {
                Some(anyhow::anyhow!("carrier upload failed"))
            }
            Action::Update(t) if t.uid == SETTINGS_UID => {
                Some(anyhow::anyhow!("carrier upload failed"))
            }
            _ => None,
        }));
    }

    let (warnings, synced) = client.sync_journal().await.expect("sync_journal");

    mock_user_put.assert();

    // The failed carrier was dropped from the queue…
    let journal = Journal::load(ctx.as_ref());
    assert!(
        !journal.queue.iter().any(|a| matches!(
            a,
            Action::Create(t)
            | Action::Update(t)
            | Action::Delete(t)
            | Action::Move(t, _)
                if t.uid == SETTINGS_UID
        )),
        "the failed carrier must be dropped, not parked at the head"
    );
    // …with a warning, and the user task went through.
    assert!(
        warnings.iter().any(|w| w.contains("settings carrier")),
        "a carrier failure must surface a warning: {:?}",
        warnings
    );
    assert!(
        synced.iter().any(|t| t.uid == "user-task-1"),
        "the user task must sync despite the carrier failure"
    );

    *hook.lock().unwrap() = None;
}
