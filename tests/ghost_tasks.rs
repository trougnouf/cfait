// Tests handling of "ghost" tasks in synchronization.
use cfait::client::RustyClient;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::env;
use std::fs;
use tokio::sync::Mutex;

// Lock for environment isolation
static GHOST_TEST_MUTEX: Mutex<()> = Mutex::const_new(());

fn setup_ghost_env(suffix: &str) -> std::path::PathBuf {
    let temp_dir = env::temp_dir().join(format!("cfait_ghost_{}_{}", suffix, std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    unsafe {
        env::set_var("CFAIT_TEST_DIR", &temp_dir);
    }
    if let Some(p) = Journal::get_path()
        && p.exists()
    {
        let _ = fs::remove_file(p);
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
async fn test_ghost_is_pruned_even_when_ctag_matches() {
    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("ctag_match_prune");

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let full_cal_href = format!("{}{}", url, cal_path);

    let mock_getctag = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "0")
        .match_body(mockito::Matcher::Regex("getctag".to_string()))
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/cal/</d:href><d:propstat><d:prop><cs:getctag xmlns:cs="http://calendarserver.org/ns/">"matching-token"</cs:getctag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async()
        .await;

    let _mock_synctoken = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "0")
        .match_body(mockito::Matcher::Regex("sync-token".to_string()))
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/cal/</d:href><d:propstat><d:prop><d:sync-token>"matching-token"</d:sync-token></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async()
        .await;

    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let client = RustyClient::new(&url, "u", "p", true).unwrap();

    let mut task = Task::new("Ghost", &HashMap::new(), None);
    task.uid = "ghost".to_string();
    task.href = format!("{}{}/ghost.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "".to_string();

    cfait::cache::Cache::save(&full_cal_href, &[task], Some("matching-token".to_string())).unwrap();
    assert!(Journal::load().is_empty());

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();

    mock_getctag.assert();
    mock_list.assert();

    assert!(
        tasks.is_empty(),
        "Ghost task with empty ETag should be pruned even if CTag matches"
    );

    teardown(temp_dir);
}

#[tokio::test]
async fn test_pending_delete_suppresses_server_item() {
    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("suppress");
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let task_path = "/cal/zombie.ics";
    let task_uid = "zombie-task";

    let mock_delete = server
        .mock("DELETE", task_path)
        .with_status(500)
        .create_async()
        .await;

    let _mock_ctag = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "0")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/cal/</d:href><d:propstat><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async()
        .await;

    let mock_list = server.mock("PROPFIND", cal_path).match_header("depth", "1").with_status(207).with_body(format!(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>{}</d:href><d:propstat><d:prop><d:getetag>"server-etag"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#, task_path)).create_async().await;

    let client = RustyClient::new(&url, "u", "p", true).unwrap();
    let full_cal_href = format!("{}{}", url, cal_path);

    let mut task = Task::new("Zombie", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "\"old-etag\"".to_string();

    // FIX: Save task to cache so the client knows it exists locally.
    // This allows the client to match the server item (via href) to the local item,
    // recognize the pending delete in the journal, and prune it instead of trying to fetch it.
    cfait::cache::Cache::save(
        &full_cal_href,
        &[task.clone()],
        Some("cached-token".to_string()),
    )
    .unwrap();

    let calendar_href_to_fetch = task.calendar_href.clone();
    Journal::push(Action::Delete(task)).unwrap();

    let tasks = client.get_tasks(&calendar_href_to_fetch).await;

    assert!(
        tasks.is_ok(),
        "get_tasks should now succeed even if sync_journal fails"
    );
    let list = tasks.unwrap();

    mock_delete.assert();
    mock_list.assert();

    assert!(
        list.is_empty(),
        "The zombie task should have been filtered out"
    );

    teardown(temp_dir);
}

#[tokio::test]
async fn test_server_deletion_updates_cache() {
    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("server_del");
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;
    let client = RustyClient::new(&url, "u", "p", true).unwrap();
    let full_cal_href = format!("{}{}", url, cal_path);
    let mut task = Task::new("Existing", &HashMap::new(), None);
    task.uid = "existing".to_string();
    task.href = format!("{}{}/task.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "\"123\"".to_string();
    cfait::cache::Cache::save(&full_cal_href, &[task], Some("old-token".to_string())).unwrap();
    let tasks = client.get_tasks(&full_cal_href).await.unwrap();
    mock_list.assert();
    assert!(tasks.is_empty());
    let (cached, _) = cfait::cache::Cache::load(&full_cal_href).unwrap();
    assert!(cached.is_empty());
    teardown(temp_dir);
}

#[tokio::test]
async fn test_ghost_is_pruned_on_full_sync_with_ctag_mismatch() {
    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("full_sync_prune");

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let full_cal_href = format!("{}{}", url, cal_path);

    let _mock_ctag = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "0")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:propstat><d:prop><cs:getctag xmlns:cs="http://calendarserver.org/ns/">"server-token"</cs:getctag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#)
        .create_async()
        .await;

    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let client = RustyClient::new(&url, "u", "p", true).unwrap();

    let mut task = Task::new("Ghost", &HashMap::new(), None);
    task.uid = "ghost".to_string();
    task.href = format!("{}{}/ghost.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "".to_string();

    cfait::cache::Cache::save(&full_cal_href, &[task], Some("local-token".to_string())).unwrap();

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();

    mock_list.assert();
    assert!(tasks.is_empty(), "Ghost task should be pruned on full sync");

    teardown(temp_dir);
}
