// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for synchronization edge cases.
use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::{Matcher, Server};
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

    // 1. Mock the REPORT (calendar-query for VTODO) listing returning ONE valid task
    // Note: The evt- companion event should be filtered out by the VTODO filter
    let mock_list = server
        .mock("REPORT", cal_path)
        .match_header("depth", "1")
        .match_body(Matcher::Regex("name=\"VTODO\"".to_string()))
        .with_status(207)
        .with_body(r#"
            <d:multistatus xmlns:d="DAV:">
                <d:response>
                    <d:href>/cal/valid-task.ics</d:href>
                    <d:propstat><d:prop><d:getetag>"1"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
                </d:response>
            </d:multistatus>
        "#)
        .create_async()
        .await;

    // 1b. Mock the separate VJOURNAL calendar-query (returns nothing here).
    let mock_journal = server
        .mock("REPORT", cal_path)
        .match_header("depth", "1")
        .match_body(Matcher::Regex("name=\"VJOURNAL\"".to_string()))
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
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
    let tasks = client
        .get_tasks(&format!("{}{}", url, cal_path))
        .await
        .unwrap();

    // 4. Assertions
    mock_list.assert();
    mock_journal.assert();
    mock_get.assert(); // If the client tried to request evt-valid-task-start.ics, mockito would panic

    assert_eq!(
        tasks.len(),
        1,
        "Client should have completely ignored the evt- file and only parsed the 1 valid task."
    );
    assert_eq!(tasks[0].uid, "valid-task");
}

#[tokio::test]
async fn test_sync_delete_prunes_task_from_cache() {
    let ctx = Arc::new(TestContext::new());

    // 1. Mock server accepting the DELETE
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal_path = "/cal/";
    let full_cal_href = format!("{}{}", url, cal_path);
    let mock = server
        .mock("DELETE", mockito::Matcher::Any)
        .with_status(204)
        .create_async()
        .await;

    // 2. Seed the calendar cache as if a previous fetch had run: it still
    //    holds the task being deleted plus an unrelated survivor task.
    let mut deleted = Task::new("deleted task", &HashMap::new(), None);
    deleted.uid = "delete-me".to_string();
    deleted.href = format!("{}{}delete-me.ics", url, cal_path);
    deleted.calendar_href = full_cal_href.clone();
    deleted.etag = "\"1\"".to_string();

    let mut survivor = Task::new("survivor task", &HashMap::new(), None);
    survivor.uid = "keep-me".to_string();
    survivor.href = format!("{}{}keep-me.ics", url, cal_path);
    survivor.calendar_href = full_cal_href.clone();
    survivor.etag = "\"2\"".to_string();

    cfait::cache::Cache::save(
        ctx.as_ref(),
        &full_cal_href,
        &[deleted.clone(), survivor.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    // 3. Queue the delete and sync it
    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();
    Journal::push(ctx.as_ref(), Action::Delete(deleted)).unwrap();
    client.sync_journal().await.unwrap();
    mock.assert();

    // 4. The cache must no longer serve the deleted task: with the Delete
    //    popped from the journal, cache-backed store rebuilds would resurrect
    //    it (and later edits would spawn "(Conflict Copy)" tasks).
    let (cached, token) = cfait::cache::Cache::load(ctx.as_ref(), &full_cal_href).unwrap();
    assert!(cached.iter().all(|t| t.uid != "delete-me"));
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].uid, "keep-me");
    assert_eq!(token, Some("token".to_string()));
}

#[tokio::test]
async fn test_sync_422_rescues_task_to_recovery() {
    let ctx = Arc::new(TestContext::new());

    // A 422 answer is a deterministic rejection: retrying the same PUT later
    // cannot succeed, so the task must be rescued instead of parking at the
    // head of the queue forever.
    let mut server = Server::new_async().await;
    let url = server.url();
    let mock = server
        .mock("PUT", "/cal/task.ics")
        .with_status(422)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("water the ferns", &HashMap::new(), None);
    task.uid = "task".to_string();
    task.calendar_href = "/cal/".to_string();
    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    let res = client.sync_journal().await;
    assert!(res.is_ok(), "Sync failed: {:?}", res.err());
    mock.assert();

    // The queue drains: the rejection was handled, not parked.
    let j = Journal::load(ctx.as_ref());
    assert!(j.is_empty(), "a deterministic 4xx must not stay queued");
    assert!(
        j.last_error.is_none(),
        "no failure reason once the queue is empty"
    );

    // The task survives in local recovery with the reason appended.
    let recovered =
        cfait::storage::LocalStorage::load_for_href(ctx.as_ref(), "local://recovery").unwrap();
    let recovered = recovered
        .iter()
        .find(|t| t.uid == "task")
        .expect("task should be rescued into local recovery");
    assert!(recovered.description.contains("[Sync Error]"));
    assert!(recovered.description.contains("422"));
}

#[tokio::test]
async fn test_sync_503_keeps_item_and_records_reason() {
    let ctx = Arc::new(TestContext::new());

    // A 503 is transient (server overloaded or offline): the action must stay
    // queued no matter how many times it fails, with the reason recorded.
    let mut server = Server::new_async().await;
    let url = server.url();
    let mock = server
        .mock("PUT", "/cal/task.ics")
        .with_status(503)
        .expect(2)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("water the ferns", &HashMap::new(), None);
    task.uid = "task".to_string();
    task.calendar_href = "/cal/".to_string();
    Journal::push(ctx.as_ref(), Action::Create(task)).unwrap();

    // First attempt fails and records the reason.
    let res = client.sync_journal().await;
    assert!(res.is_err(), "503 should fail the sync pass");

    let j = Journal::load(ctx.as_ref());
    assert_eq!(j.queue.len(), 1, "a transient error must keep the action");
    let err = j.last_error.as_ref().expect("failure reason recorded");
    assert_eq!(err.uid, "task");
    assert_eq!(err.summary, "water the ferns");
    assert!(err.message.contains("503"));
    assert_eq!(err.attempts, 1);

    // Second attempt: still queued, attempts incremented.
    let res = client.sync_journal().await;
    assert!(res.is_err());
    mock.assert();
    let j = Journal::load(ctx.as_ref());
    assert_eq!(j.queue.len(), 1, "still nothing is discarded on 503");
    assert_eq!(j.last_error.as_ref().unwrap().attempts, 2);
}
