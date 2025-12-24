// File: tests/sync_safety.rs
use cfait::client::RustyClient;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::env;
use std::fs;
use tokio::sync::Mutex;

static SAFETY_TEST_MUTEX: Mutex<()> = Mutex::const_new(());

fn setup_safety_env(suffix: &str) -> std::path::PathBuf {
    let temp_dir = env::temp_dir().join(format!("cfait_safety_{}_{}", suffix, std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    unsafe {
        env::set_var("CFAIT_TEST_DIR", &temp_dir);
    }
    if let Some(p) = Journal::get_path()
        && p.exists() {
            let _ = fs::remove_file(p);
        }
    if let Ok(cache_dir) = cfait::paths::AppPaths::get_cache_dir()
        && cache_dir.exists() {
            let _ = fs::remove_dir_all(&cache_dir);
            let _ = fs::create_dir_all(&cache_dir);
        }
    temp_dir
}

fn teardown(path: std::path::PathBuf) {
    unsafe {
        env::remove_var("CFAIT_TEST_DIR");
    }
    let _ = fs::remove_dir_all(path);
}

#[tokio::test]
async fn test_safety_resurrection_on_404() {
    let _guard = SAFETY_TEST_MUTEX.lock().await;
    let temp_dir = setup_safety_env("resurrect");

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

    let client = RustyClient::new(&url, "u", "p", true).unwrap();

    let mut task = Task::new("Important Work", &HashMap::new());
    task.uid = "important_task".to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = format!("{}/cal/", url);
    task.etag = "\"old-etag\"".to_string();

    Journal::push(Action::Update(task)).unwrap();

    let res = client.sync_journal().await;

    assert!(
        res.is_ok(),
        "Sync should succeed by converting Update to Create"
    );
    mock_update_404.assert();
    mock_resurrect.assert();

    teardown(temp_dir);
}

#[tokio::test]
async fn test_safety_conflict_copy_on_hard_412() {
    let _guard = SAFETY_TEST_MUTEX.lock().await;
    let temp_dir = setup_safety_env("conflict");

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

    let client = RustyClient::new(&url, "u", "p", true).unwrap();

    let mut base_task = Task::new("Local Version", &HashMap::new());
    base_task.uid = "conflict".to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    base_task.description = "Base Description".to_string();

    cfait::cache::Cache::save(
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    let mut local_task = base_task.clone();
    local_task.description = "Local Change".to_string();
    Journal::push(Action::Update(local_task)).unwrap();

    let res = tokio::time::timeout(std::time::Duration::from_secs(5), client.sync_journal()).await;

    assert!(
        res.is_ok(),
        "Test timed out! Infinite loop detected (Merge incorrectly succeeded)."
    );
    let sync_res = res.unwrap();
    assert!(sync_res.is_ok(), "Sync failed: {:?}", sync_res.err());

    mock_412.assert();
    mock_create_copy.assert();

    teardown(temp_dir);
}
