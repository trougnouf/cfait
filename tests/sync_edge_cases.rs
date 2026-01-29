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
