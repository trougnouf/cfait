// File: ./tests/sync_safety.rs
/*
 Migration of sync safety tests to use TestContext and ctx-aware APIs.

 These tests create an isolated TestContext for filesystem operations (journal,
 cache, etc.) and pass it explicitly to APIs that accept a context. Network
 interactions are driven via mockito Server.
*/

use cfait::client::RustyClient;
use cfait::context::{AppContext, TestContext}; // Import AppContext trait
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::fs;
use tokio::sync::Mutex;

static SAFETY_TEST_MUTEX: Mutex<()> = Mutex::const_new(());

// RAII guard for test environment setup and teardown.
struct TestEnvGuard {
    ctx: TestContext,
    original_env: Option<String>,
}

impl TestEnvGuard {
    fn new(_suffix: &str) -> Self {
        // The context creates a unique temp directory.
        let ctx = TestContext::new();

        // Set the environment variable so that code using the default context
        // (like client.sync_journal -> Journal::load) will use this temp directory.
        let original_env = std::env::var("CFAIT_TEST_DIR").ok();
        unsafe {
            std::env::set_var("CFAIT_TEST_DIR", &ctx.root);
        }

        // Perform initial cleanup using the context.
        if let Some(p) = Journal::get_path_with_ctx(&ctx)
            && p.exists()
        {
            let _ = fs::remove_file(p);
        }
        if let Ok(cache_dir) = ctx.get_cache_dir()
            && cache_dir.exists() {
                let _ = fs::remove_dir_all(&cache_dir);
                let _ = fs::create_dir_all(&cache_dir);
            }

        Self { ctx, original_env }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        // Restore the original environment variable.
        unsafe {
            if let Some(val) = &self.original_env {
                std::env::set_var("CFAIT_TEST_DIR", val);
            } else {
                std::env::remove_var("CFAIT_TEST_DIR");
            }
        }
        // The TestContext's Drop implementation will remove the temp directory.
    }
}

#[tokio::test]
async fn test_safety_resurrection_on_404() {
    let _guard = SAFETY_TEST_MUTEX.lock().await;
    let env_guard = TestEnvGuard::new("resurrect");
    let ctx = &env_guard.ctx;

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_path = "/cal/important_task.ics";

    let mock_update_404 = server
        .mock("PUT", task_path)
        .with_status(404)
        .create_async()
        .await;

    let mock_resurrect = server
        .mock("PUT", task_path)
        .match_header("If-Match", mockito::Matcher::Missing)
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .create_async()
        .await;

    // Client construction does not require AppContext in this codebase.
    let client = RustyClient::new(&url, "u", "p", true, None).unwrap();

    let mut task = Task::new("Important Work", &HashMap::new(), None);
    task.uid = "important_task".to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = format!("{}/cal/", url);
    task.etag = "\"old-etag\"".to_string();

    // Push into journal using the test context
    Journal::push_with_ctx(ctx, Action::Update(task)).unwrap();

    // Run sync; the client implementation is expected to consult the journal
    // (in this codebase it may use the default context internally). The test
    // still asserts the expected network interactions.
    let res = client.sync_journal().await;

    assert!(
        res.is_ok(),
        "Sync should succeed by converting Update to Create"
    );
    mock_update_404.assert();
    mock_resurrect.assert();

    // env_guard is dropped here and temp files are removed
}

#[tokio::test]
async fn test_safety_conflict_copy_on_hard_412() {
    let _guard = SAFETY_TEST_MUTEX.lock().await;
    let env_guard = TestEnvGuard::new("conflict");
    let ctx = &env_guard.ctx;

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_path = "/cal/conflict.ics";

    let mock_412 = server
        .mock("PUT", task_path)
        .with_status(412)
        .create_async()
        .await;

    let _mock_get = server
        .mock("PROPFIND", "/cal/")
        .match_header("depth", "1")
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop><d:getetag>"server-etag"</d:getetag></d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            task_path
        ))
        .create_async()
        .await;

    let server_ics = "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:conflict\nSUMMARY:Local Version\nDESCRIPTION:Server Change\nEND:VTODO\nEND:VCALENDAR".to_string();

    let _mock_fetch_item = server
        .mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop>
                            <cal:calendar-data>{}</cal:calendar-data>
                            <d:getetag>"server-etag"</d:getetag>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            task_path, server_ics
        ))
        .create_async()
        .await;

    let mock_create_copy = server
        .mock("PUT", mockito::Matcher::Regex(r"/cal/.*\.ics".to_string()))
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .create_async()
        .await;

    let client = RustyClient::new(&url, "u", "p", true, None).unwrap();

    let mut base_task = Task::new("Local Version", &HashMap::new(), None);
    base_task.uid = "conflict".to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    base_task.description = "Base Description".to_string();

    // Save remote cache using the test context so client-side cache reads (if context-aware) will pick it up.
    cfait::cache::Cache::save_with_ctx(
        ctx,
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    let mut local_task = base_task.clone();
    local_task.description = "Local Change".to_string();

    // Push local update in the test context journal
    Journal::push_with_ctx(ctx, Action::Update(local_task)).unwrap();

    let res = tokio::time::timeout(std::time::Duration::from_secs(5), client.sync_journal()).await;

    assert!(
        res.is_ok(),
        "Test timed out! Infinite loop detected (Merge incorrectly succeeded)."
    );
    let sync_res = res.unwrap();
    assert!(sync_res.is_ok(), "Sync failed: {:?}", sync_res.err());

    mock_412.assert();
    mock_create_copy.assert();

    // TestContext cleanup happens automatically.
}
