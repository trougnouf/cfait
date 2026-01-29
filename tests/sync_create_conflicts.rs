use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_create_412_handled_gracefully() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "stuck-task";
    let task_path = format!("/cal/{}.ics", task_uid);

    // 1. Mock the specific failure case:
    // The client sends a PUT with "If-None-Match: *" (Create only if not exists).
    // The server returns 412 (meaning it DOES exist).
    let mock_put = server
        .mock("PUT", task_path.as_str())
        .match_header("If-None-Match", "*")
        .with_status(412) // Precondition Failed
        .create_async()
        .await;

    // 2. Setup Client with ctx
    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    // 3. Queue the Create Action with ctx
    let mut task = Task::new("Stuck Task", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = format!("{}/cal/", url);
    task.href = format!("{}{}", url, task_path);

    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    // 4. Run Sync
    let result = client.sync_journal().await;

    // 5. Assertions
    mock_put.assert();

    // The sync technically "succeeded" in processing the queue (by skipping the stuck item)
    assert!(
        result.is_ok(),
        "Sync returned error for 412: {:?}",
        result.err()
    );

    // CRITICAL: The journal must be empty. If it's not, the client is stuck in a loop.
    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "Journal should be empty. The client failed to resolve the 412 conflict."
    );
}

#[tokio::test]
async fn test_create_500_persists() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_path = "/cal/broken-server.ics";

    // Mock a genuine server error
    let mock_put = server
        .mock("PUT", task_path)
        .match_header("If-None-Match", "*")
        .with_status(500)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    let mut task = Task::new("Broken Task", &HashMap::new(), None);
    task.uid = "broken-server".to_string();
    task.calendar_href = format!("{}/cal/", url);
    task.href = format!("{}{}", url, task_path);

    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    let result = client.sync_journal().await;

    mock_put.assert();

    // Sync should fail
    assert!(result.is_err(), "Sync should fail on 500");

    // Journal should KEEP the item (retry later)
    let journal = Journal::load(ctx.as_ref());
    assert!(
        !journal.is_empty(),
        "Journal should preserve items on 500 error"
    );
}

#[tokio::test]
async fn test_move_404_handled_gracefully() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let old_path = "/cal1/task.ics";
    let new_cal = format!("{}/cal2/", url);

    // Mock a MOVE where the source is missing (404)
    // Client should assume it was already moved or deleted and proceed.
    let mock_move = server
        .mock("MOVE", old_path)
        .with_status(404)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    let mut task = Task::new("Moving Task", &HashMap::new(), None);
    task.uid = "moving".to_string();
    task.href = format!("{}{}", url, old_path);
    task.calendar_href = format!("{}/cal1/", url);

    Journal::push(ctx.as_ref(), Action::Move(task, new_cal)).unwrap();

    let result = client.sync_journal().await;

    mock_move.assert();
    assert!(result.is_ok());

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "Journal should drop MOVE actions if source is 404"
    );
}
