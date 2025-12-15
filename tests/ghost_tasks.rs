// File: ./tests/ghost_tasks.rs
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
    if let Some(p) = Journal::get_path() {
        if p.exists() {
            let _ = fs::remove_file(p);
        }
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
async fn test_pending_delete_suppresses_server_item() {
    // SCENARIO:
    // 1. User deletes task. Action::Delete is in Journal.
    // 2. Sync runs. It attempts to DELETE on server but fails (e.g. 500 error).
    // 3. Sync continues to FETCH list (get_tasks).
    // 4. Server LIST still contains the task (since delete failed).
    // 5. Client MUST NOT add this task back to the list (Zombie/Ghost).

    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("suppress");

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let task_path = "/cal/zombie.ics";
    let task_uid = "zombie-task";

    // Mock 1: The DELETE attempt fails
    let mock_delete = server
        .mock("DELETE", task_path)
        .with_status(500)
        .create_async()
        .await;

    // Mock 2: The LIST (PROPFIND) returns the task (because delete failed)
    // FIX: Match Depth: 1 to distinguish from CTag checks (Depth: 0)
    let mock_list = server
        .mock("PROPFIND", cal_path)
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

    // Mock 3: The FETCH (REPORT) returns the task body (if client decides to fetch it)
    let _mock_fetch = server
        .mock("REPORT", cal_path)
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop>
                            <cal:calendar-data>
BEGIN:VCALENDAR
BEGIN:VTODO
UID:{}
SUMMARY:I am a Zombie
END:VTODO
END:VCALENDAR
                            </cal:calendar-data>
                            <d:getetag>"server-etag"</d:getetag>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
        "#,
            task_path, task_uid
        ))
        .create_async()
        .await;

    let client = RustyClient::new(&url, "u", "p", true).unwrap();

    // Setup Journal with Delete action
    let mut task = Task::new("Zombie", &HashMap::new());
    task.uid = task_uid.to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = format!("{}{}", url, cal_path);
    task.etag = "\"old-etag\"".to_string();

    // Clone before move to avoid E0382
    let calendar_href_to_fetch = task.calendar_href.clone();

    Journal::push(Action::Delete(task)).unwrap();

    // Action: Get Tasks (which triggers sync_journal, then fetch)
    // We expect sync_journal to fail (500), but get_tasks should proceed to fetch.
    let tasks = client.get_tasks(&calendar_href_to_fetch).await;

    assert!(
        tasks.is_ok(),
        "Get tasks should succeed even if sync partially failed"
    );
    let list = tasks.unwrap();

    // Verify mocks were hit
    mock_delete.assert();
    mock_list.assert();

    // Verify Ghost is GONE
    assert!(
        list.is_empty(),
        "The list should be empty. The zombie task was filtered out by the pending delete."
    );

    teardown(temp_dir);
}

#[tokio::test]
async fn test_server_deletion_updates_cache() {
    // SCENARIO:
    // 1. Task exists in Local Cache.
    // 2. Task is DELETED on Server (by another client).
    // 3. Client syncs.
    // 4. Task should be removed from Cache/List.

    let _guard = GHOST_TEST_MUTEX.lock().await;
    let temp_dir = setup_ghost_env("server_del");

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";

    // Mock: LIST returns EMPTY list (Task is gone on server)
    // FIX: Match Depth: 1
    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(
            r#"
            <d:multistatus xmlns:d="DAV:">
                <!-- No resources -->
            </d:multistatus>
        "#,
        )
        .create_async()
        .await;

    let client = RustyClient::new(&url, "u", "p", true).unwrap();
    let full_cal_href = format!("{}{}", url, cal_path);

    // Setup Cache with existing task
    let mut task = Task::new("Existing", &HashMap::new());
    task.uid = "existing".to_string();
    task.href = format!("{}{}/task.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "\"123\"".to_string();

    cfait::cache::Cache::save(&full_cal_href, &[task], Some("old-token".to_string())).unwrap();

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();

    mock_list.assert();
    assert!(
        tasks.is_empty(),
        "Task should be removed from list because it is missing on server"
    );

    // Double check cache persistence
    let (cached, _) = cfait::cache::Cache::load(&full_cal_href).unwrap();
    assert!(
        cached.is_empty(),
        "Cache file should have been updated to empty"
    );

    teardown(temp_dir);
}
