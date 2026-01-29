use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_ghost_is_pruned_even_when_ctag_matches() {
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

    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("Ghost", &HashMap::new(), None);
    task.uid = "ghost".to_string();
    task.href = format!("{}{}/ghost.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "".to_string();

    cfait::cache::Cache::save(
        ctx.as_ref(),
        &full_cal_href,
        &[task],
        Some("matching-token".to_string()),
    )
    .unwrap();
    assert!(Journal::load(ctx.as_ref()).is_empty());

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();

    mock_getctag.assert();
    mock_list.assert();

    assert!(
        tasks.is_empty(),
        "Ghost task with empty ETag should be pruned even if CTag matches"
    );
}

#[tokio::test]
async fn test_pending_delete_suppresses_server_item() {
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

    let mock_list = server.mock("PROPFIND", cal_path).match_header("depth", "1").with_status(207).with_body(format!(r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>{}</d:href><d:propstat><d:prop><d:getetag>"server-etag"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#, task_path)).create_async().await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();
    let full_cal_href = format!("{}{}", url, cal_path);

    let mut task = Task::new("Zombie", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "\"old-etag\"".to_string();

    cfait::cache::Cache::save(
        ctx.as_ref(),
        &full_cal_href,
        &[task.clone()],
        Some("cached-token".to_string()),
    )
    .unwrap();

    let calendar_href_to_fetch = task.calendar_href.clone();
    Journal::push(ctx.as_ref(), Action::Delete(task)).unwrap();

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
}

#[tokio::test]
async fn test_server_deletion_updates_cache() {
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

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();
    let full_cal_href = format!("{}{}", url, cal_path);
    let mut task = Task::new("Existing", &HashMap::new(), None);
    task.uid = "existing".to_string();
    task.href = format!("{}{}/task.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "\"123\"".to_string();

    cfait::cache::Cache::save(
        ctx.as_ref(),
        &full_cal_href,
        &[task],
        Some("old-token".to_string()),
    )
    .unwrap();

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();
    mock_list.assert();
    assert!(tasks.is_empty());
    let (cached, _) = cfait::cache::Cache::load(ctx.as_ref(), &full_cal_href).unwrap();
    assert!(cached.is_empty());
}

#[tokio::test]
async fn test_ghost_is_pruned_on_full_sync_with_ctag_mismatch() {
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let full_cal_href = format!("{}{}", url, cal_path);

    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async()
        .await;

    let ctx = Arc::new(TestContext::new());
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("Ghost", &HashMap::new(), None);
    task.uid = "ghost".to_string();
    task.href = format!("{}{}/ghost.ics", url, cal_path);
    task.calendar_href = full_cal_href.clone();
    task.etag = "".to_string();

    cfait::cache::Cache::save(
        ctx.as_ref(),
        &full_cal_href,
        &[task],
        Some("local-token".to_string()),
    )
    .unwrap();

    let tasks = client.get_tasks(&full_cal_href).await.unwrap();

    mock_list.assert();
    assert!(tasks.is_empty(), "Ghost task should be pruned on full sync");
}
