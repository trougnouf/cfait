use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_sync_recovers_from_412() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "test-uid";

    let mock_412 = server
        .mock("PUT", "/cal/test-uid.ics")
        .match_header("If-Match", "old-etag")
        .with_status(412)
        .create_async()
        .await;

    let mock_conflict_copy = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_header("If-None-Match", "*")
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    let mut task = Task::new("Local Title", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = "/cal/".to_string();
    task.href = format!("/cal/{}.ics", task_uid);
    task.description = "Local Description".to_string();
    task.etag = "old-etag".to_string();

    Journal::push(ctx.as_ref(), Action::Update(task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync should succeed");

    mock_412.assert();
    mock_conflict_copy.assert();

    let j = Journal::load(ctx.as_ref());
    assert!(
        j.is_empty(),
        "Journal should be empty after successful sync"
    );
}
