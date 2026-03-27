use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_sync_delete_404_is_success() {
    let ctx = Arc::new(TestContext::new());

    // 1. Mock Server returning 404 Not Found for a DELETE
    let mut server = Server::new_async().await;
    let url = server.url();
    let mock = server
        .mock("DELETE", "/cal/task.ics")
        .with_status(404)
        .create_async()
        .await;

    // 2. Setup Client
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // 3. Add Delete Action to Journal
    let mut task = Task::new("T", &HashMap::new(), None);
    // Note: client.rs uses strip_host, so we ensure the href implies the relative path
    task.href = format!("{}/cal/task.ics", url);
    task.etag = "\"123\"".to_string();
    Journal::push(ctx.as_ref(), Action::Delete(task)).unwrap();

    // 4. Sync
    let res = client.sync_journal().await;

    // 5. Assertions
    // 404 on delete means "already deleted", so sync should succeed (Ok)
    assert!(res.is_ok(), "Sync failed: {:?}", res.err());
    mock.assert();

    // Item should be removed from journal
    let j = Journal::load(ctx.as_ref());
    assert!(j.is_empty(), "Journal should be empty after 404 delete");
}

#[tokio::test]
async fn test_sync_500_keeps_item_in_queue() {
    let ctx = Arc::new(TestContext::new());

    // 1. Mock Server returning 500 Error
    let mut server = Server::new_async().await;
    let url = server.url();
    let mock = server
        .mock("PUT", "/cal/task.ics")
        .with_status(500)
        .create_async()
        .await;

    // 2. Setup Client
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // 3. Add Create Action
    let mut task = Task::new("T", &HashMap::new(), None);
    task.uid = "task".to_string();
    task.calendar_href = "/cal/".to_string();
    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    // 4. Sync
    let res = client.sync_journal().await;

    // 5. Assertions
    // Should return Error
    assert!(res.is_err(), "Sync should have failed due to 500");
    mock.assert();

    // Item should REMAIN in journal because it failed
    let j = Journal::load(ctx.as_ref());
    assert!(
        !j.is_empty(),
        "Journal should still contain the failed item"
    );
    assert_eq!(j.queue.len(), 1);
}

#[tokio::test]
async fn test_sync_ignores_companion_events_to_prevent_multiget_spam() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";

    // 1. Mock the PROPFIND listing returning ONE valid task and ONE companion event
    let mock_list = server
        .mock("PROPFIND", cal_path)
        .match_header("depth", "1")
        .with_status(207)
        .with_body(r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>/cal/valid-task.ics</d:href>
                    <d:propstat><d:prop><d:getetag>"1"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
                </d:response>
                <d:response>
                    <d:href>/cal/evt-valid-task-start.ics</d:href>
                    <d:propstat><d:prop><d:getetag>"2"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
                </d:response>
            </d:multistatus>
        "#)
        .create_async()
        .await;

    // 2. Mock the REPORT (MULTIGET) to fetch the actual task data
    let valid_ics = "BEGIN:VCALENDAR\nVERSION:2.0\nBEGIN:VTODO\nUID:valid-task\nSUMMARY:Test\nSTATUS:NEEDS-ACTION\nEND:VTODO\nEND:VCALENDAR";
    let mock_get = server
        .mock("REPORT", cal_path)
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>/cal/valid-task.ics</d:href>
                    <d:propstat>
                        <d:prop>
                            <cal:calendar-data>{}</cal:calendar-data>
                            <d:getetag>"1"</d:getetag>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
            "#,
            valid_ics
        ))
        .create_async()
        .await;

    // 3. Run the sync
    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();
    let tasks = client.get_tasks(&format!("{}{}", url, cal_path)).await.unwrap();

    // 4. Assertions
    mock_list.assert();
    mock_get.assert(); // If the client tried to request evt-valid-task-start.ics, mockito would panic

    assert_eq!(
        tasks.len(),
        1,
        "Client should have completely ignored the evt- file and only parsed the 1 valid task."
    );
    assert_eq!(tasks[0].uid, "valid-task");
}
