// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for 412 conflict resolution paths in attempt_conflict_resolution.
//!
//! These cover the cases that issue #56 reported as producing spurious
//! "Conflict Copy" tasks:
//!   - a genuine concurrent edit that the 3-way merge *can* resolve (no copy),
//!   - a transient failure to fetch the server version (leave queued, no copy).
use cfait::cache::Cache;
use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

/// Local edits one field, the server edited a different field. The 3-way merge
/// succeeds, the update is retried with the merged task, and no "Conflict Copy"
/// is ever created.
#[tokio::test]
async fn test_412_resolves_via_three_way_merge_no_copy() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "merge-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    // 1. Initial update PUT fails with 412 (server changed since we last fetched).
    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "old-etag")
        .with_status(412)
        .create_async()
        .await;

    // 2. Fetch of the server version (REPORT calendar-multiget) returns a task that
    //    changed a *different* field (description) than the local edit (summary).
    let server_ics = "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:merge-uid\nSUMMARY:Base Title\nDESCRIPTION:Server Description\nEND:VTODO\nEND:VCALENDAR"
        .to_string();
    let mock_fetch = server
        .mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(format!(
            r#"
            <d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav">
                <d:response>
                    <d:href>{}</d:href>
                    <d:propstat>
                        <d:prop>
                            <cal:calendar-data>{}</cal:calendar-data>
                            <d:getetag>"server-etag"</d:getetag>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
            "#,
            task_path, server_ics
        ))
        .create_async()
        .await;

    // 3. Retried PUT with the merged task uses the server's etag and succeeds.
    let mock_retry_ok = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"server-etag\"")
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .create_async()
        .await;

    // A conflict-copy PUT (if it happened) would carry "Conflict Copy" in the body
    // and use If-None-Match: *. Assert it never happens.
    let mock_conflict_copy = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // Base state in the cache: the version both clients started from.
    let mut base_task = Task::new("Base Title", &HashMap::new(), None);
    base_task.uid = task_uid.to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    base_task.description = "Base Description".to_string();
    Cache::save(
        ctx.as_ref(),
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    // Local edit: changed summary only.
    let mut local_task = base_task.clone();
    local_task.summary = "Local Title".to_string();
    local_task.etag = "old-etag".to_string(); // sent raw; server stores quoted

    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let res = tokio::time::timeout(std::time::Duration::from_secs(10), client.sync_journal()).await;
    assert!(res.is_ok(), "Test timed out!");
    let sync_res = res.unwrap();
    assert!(sync_res.is_ok(), "Sync failed: {:?}", sync_res.err());

    mock_412.assert();
    mock_fetch.assert();
    mock_retry_ok.assert();
    mock_conflict_copy.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "Journal should be empty after a successful merged retry"
    );
}

/// When the server version cannot be fetched (slow/flaky server), the update must
/// stay queued for the next sync instead of producing a "Conflict Copy".
#[tokio::test]
async fn test_412_fetch_failure_leaves_queued_no_copy() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "fetch-fail-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    // 1. Update PUT fails with 412.
    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "old-etag")
        .with_status(412)
        .create_async()
        .await;

    // 2. Fetch of the server version fails (e.g., server returns 500 / empty body).
    let mock_fetch_fail = server
        .mock("REPORT", "/cal/")
        .with_status(500)
        .create_async()
        .await;

    // 3. No conflict copy should be created.
    let mock_conflict_copy = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut base_task = Task::new("Fetch Fail Task", &HashMap::new(), None);
    base_task.uid = task_uid.to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    base_task.description = "Base Description".to_string();
    Cache::save(
        ctx.as_ref(),
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    let mut local_task = base_task.clone();
    local_task.summary = "Local Title".to_string();
    local_task.etag = "old-etag".to_string();

    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let res = tokio::time::timeout(std::time::Duration::from_secs(10), client.sync_journal()).await;
    assert!(res.is_ok(), "Test timed out!");
    let sync_res = res.unwrap();

    // The sync returns an error (transient fetch failure stops the loop), but the
    // action stays safely at the front of the queue for the next sync attempt.
    assert!(
        sync_res.is_err(),
        "Sync should surface a transient error, not silently duplicate the task"
    );

    mock_412.assert();
    mock_fetch_fail.assert();
    mock_conflict_copy.assert();

    let journal = Journal::load(ctx.as_ref());
    assert_eq!(
        journal.queue.len(),
        1,
        "The update must remain queued, not be dropped or duplicated"
    );
}
