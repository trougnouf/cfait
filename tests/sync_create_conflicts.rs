// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for synchronization create conflicts.
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

    // 1b. The client verifies the 412 really means "it exists" by fetching
    // the object's etag; only then does it treat the creation as a success.
    let mock_etag = server
        .mock("PROPFIND", task_path.as_str())
        .with_status(207)
        .with_body(
            r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
    <d:response>
        <d:href>/cal/stuck-task.ics</d:href>
        <d:propstat><d:prop><d:getetag>"etag-1"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
    </d:response>
</d:multistatus>"#,
        )
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
    mock_etag.assert();

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
        .mock("MOVE", mockito::Matcher::Any)
        .with_status(404)
        .create_async()
        .await;

    let mock_create = server
        .mock("PUT", mockito::Matcher::Any)
        .with_status(201)
        .with_header("ETag", "\"new-etag\"") // Tell the client the new ETag so it doesn't fallback to PROPFIND
        .create_async()
        .await;

    let mock_delete = server
        .mock("DELETE", mockito::Matcher::Any)
        .with_status(404) // Source is gone, so delete gets 404 and is happily discarded
        .expect_at_least(1) // Because of the ANY matcher, it will absorb the task delete + companion event cleanup deletes
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
    mock_create.assert();
    mock_delete.assert();
    assert!(result.is_ok());

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "Journal should be empty after falling back to Create+Delete"
    );
}

#[tokio::test]
async fn test_create_412_without_object_rescues_to_recovery() {
    let ctx = Arc::new(TestContext::new());

    // Nextcloud-style rejection: the calendar refuses the component type and
    // answers 412 to the creation PUT, without creating any object.
    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "rejected-task";
    let task_path = format!("/cal/{}.ics", task_uid);

    let mock_put = server
        .mock("PUT", task_path.as_str())
        .match_header("If-None-Match", "*")
        .with_status(412)
        .create_async()
        .await;

    // The verification fetch finds nothing at the path.
    let mock_etag = server
        .mock("PROPFIND", task_path.as_str())
        .with_status(404)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    let mut task = Task::new("water the ferns", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = format!("{}/cal/", url);
    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());
    mock_put.assert();
    mock_etag.assert();

    // The rejection must not park at the queue head as an assumed success
    // (that leaves an unsynced ghost whose updates can never succeed).
    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "a 412 without an existing object must be handled, not parked"
    );

    let recovered =
        cfait::storage::LocalStorage::load_for_href(ctx.as_ref(), "local://recovery").unwrap();
    assert!(
        recovered.iter().any(|t| t.uid == task_uid),
        "the rejected entry must be rescued into local recovery"
    );
}
