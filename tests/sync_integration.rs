// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration tests for synchronization.
use cfait::cache::Cache;
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
    let task_path = format!("/cal/{}.ics", task_uid); // Use the exact task path

    // 1. Mock the initial PUT that fails with a 412
    let mock_412 = server
        .mock("PUT", &*task_path)
        .match_header("If-Match", "old-etag")
        .with_status(412)
        .create_async()
        .await;

    // 2. Mock the REPORT to fetch server state, which we'll say is 404
    // This mocks the fetch_remote_task call, which sends a REPORT to the parent directory.
    // Returning 404 causes the 3-way merge to fail, triggering the "Conflict Copy" fallback.
    let mock_report_fetch = server
        .mock("REPORT", "/cal/")
        .with_status(404)
        .create_async()
        .await;

    // 3. Mock the successful creation of the "Conflict Copy"
    let mock_conflict_copy = server
        .mock("PUT", mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()))
        .match_header("If-None-Match", "*")
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .with_header("ETag", "new-conflict-etag")
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "user", "pass", true, None).unwrap();

    let mut task = Task::new("Local Title", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = "/cal/".to_string();
    task.href = task_path.clone();
    task.description = "Local Description".to_string();
    task.etag = "old-etag".to_string();

    // Save the task to the cache so attempt_conflict_resolution can find it
    Cache::save(
        ctx.as_ref(),
        "/cal/",
        &[task.clone()],
        Some("sync-token".to_string()),
    )
    .unwrap();

    Journal::push(ctx.as_ref(), Action::Update(task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync should succeed");

    mock_412.assert();
    mock_report_fetch.assert();
    mock_conflict_copy.assert();

    let j = Journal::load(ctx.as_ref());
    assert!(
        j.is_empty(),
        "Journal should be empty after successful sync"
    );
}
