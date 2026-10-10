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
        .match_header("If-Match", "\"old-etag\"")
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
    local_task.etag = "old-etag".to_string(); // bare server etag; cfait sends it quoted

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
        .match_header("If-Match", "\"old-etag\"")
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

#[tokio::test]
async fn test_ghost_update_412_retries_as_create() {
    let ctx = Arc::new(TestContext::new());

    // An entry that never reached the server (ghost: empty href, pending
    // refresh etag). Its update PUT carries an unsatisfiable If-Match and
    // gets 412 forever; the conflict-resolution fetch on an empty href fails
    // deterministically, so the action must be retried as a creation instead
    // of parking at the queue head.
    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "ghost-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    let mock_update_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", mockito::Matcher::Any)
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // The retried creation succeeds.
    let mock_create = server
        .mock("PUT", task_path.as_str())
        .match_header("If-None-Match", "*")
        .with_status(201)
        .with_header("ETag", "\"fresh-etag\"")
        .expect(1)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("water the ferns", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.calendar_href = format!("{}/cal/", url);
    task.href = String::new();
    task.etag = "pending_refresh".to_string();
    Journal::push(ctx.as_ref(), Action::Update(task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());
    mock_update_412.assert();
    mock_create.assert();

    // The queue drains: the ghost was re-created instead of parking.
    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "a ghost update must be retried as a creation"
    );
    let (_, synced) = result.unwrap();
    assert!(
        synced.iter().any(|t| t.uid == task_uid),
        "the re-created entry should be reported as synced"
    );
}

#[tokio::test]
async fn test_412_conflict_fetch_405_makes_conflict_copy() {
    let ctx = Arc::new(TestContext::new());

    // A deterministic 4xx on the server-version fetch (405 here) would park
    // the action forever if treated as transient; the local edits must be
    // preserved as a conflict copy instead.
    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "merge-fetch-405";
    let task_path = format!("/cal/{}.ics", task_uid);

    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"old-etag\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // The server refuses the multiget REPORT on the calendar (405), e.g.
    // because the path cannot serve the request. This fails on every retry.
    let mock_fetch_405 = server
        .mock("REPORT", "/cal/")
        .with_status(405)
        .expect(1)
        .create_async()
        .await;

    // The conflict copy is created on the server.
    let mock_conflict_copy = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_header("If-None-Match", "*")
        .match_body(mockito::Matcher::Regex(r"Conflict Copy".to_string()))
        .with_status(201)
        .with_header("ETag", "\"copy-etag\"")
        .expect(1)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut base_task = Task::new("water the ferns", &HashMap::new(), None);
    base_task.uid = task_uid.to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    Cache::save(
        ctx.as_ref(),
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    let mut local_task = base_task.clone();
    local_task.summary = "repot the ferns".to_string();
    local_task.etag = "old-etag".to_string();
    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let result = client.sync_journal().await;
    assert!(
        result.is_ok(),
        "a deterministic fetch failure must not park the queue: {:?}",
        result.err()
    );
    mock_412.assert();
    mock_fetch_405.assert();
    mock_conflict_copy.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "the conflict copy must be pushed, not parked"
    );
}

/// Rapid done/undo race across sibling frontends sharing one journal: the
/// "done" toggle was already pushed by the other process (server holds it at
/// SEQUENCE 1), the queued "undo" still carries the stale pre-toggle etag and
/// 412s. The undo is the newer link of the same edit chain (SEQUENCE 2), so
/// it must win against the server — no "Conflict Copy" and no silent revert
/// to the done state.
#[tokio::test]
async fn test_412_rapid_toggle_newer_local_sequence_wins_no_copy() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "rapid-toggle-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    // 1. The undo PUT carries the stale pre-toggle etag and 412s.
    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"old-etag\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // 2. The server holds our own "done" link: completed at SEQUENCE 1.
    let server_ics = "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:rapid-toggle-uid\nSUMMARY:water the ferns\nSTATUS:COMPLETED\nPERCENT-COMPLETE:100\nCOMPLETED:20261008T120000Z\nSEQUENCE:1\nEND:VTODO\nEND:VCALENDAR".to_string();
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
        .expect(1)
        .create_async()
        .await;

    // 3. The undo is retried against the server's etag and must carry the
    //    local (undone) content, not a merge back to the done state.
    let mock_retry_ok = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"server-etag\"")
        .match_body(mockito::Matcher::Regex(r"SEQUENCE:2".to_string()))
        .with_status(201)
        .with_header("ETag", "\"new-etag\"")
        .expect(1)
        .create_async()
        .await;

    // 4. No conflict copy, and no PUT that resurrects the done state.
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
    let mock_done_put = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_body(mockito::Matcher::Regex(r"STATUS:COMPLETED".to_string()))
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // The cache still holds the pre-toggle base (a racing fetch rewrote it
    // after the sibling's push persisted the done state), with a percent
    // value the done/undo toggles clobber — the exact input combination
    // that hard-conflicts the 3-way merge.
    let mut base_task = Task::new("water the ferns", &HashMap::new(), None);
    base_task.uid = task_uid.to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    base_task.percent_complete = Some(50);
    Cache::save(
        ctx.as_ref(),
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    // The queued undo: same fields as the base except the toggles bumped
    // SEQUENCE to 2 and reset the percent; its etag is stale.
    let mut local_task = base_task.clone();
    local_task.sequence = 2;
    local_task.percent_complete = None;
    local_task.etag = "old-etag".to_string();
    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());

    mock_412.assert();
    mock_fetch.assert();
    mock_retry_ok.assert();
    mock_conflict_copy.assert();
    mock_done_put.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "the undo must be pushed, not parked or duplicated"
    );
    let (_, synced) = result.unwrap();
    assert!(
        synced.iter().any(|t| t.uid == task_uid),
        "the undo should be reported as synced"
    );
}

/// A hard conflict at equal SEQUENCE (two devices edited the same field from
/// the same fork) must keep producing a conflict copy: neither edit is
/// provably newer, so both sides are preserved.
#[tokio::test]
async fn test_412_sequence_tie_still_makes_conflict_copy() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "tie-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"old-etag\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // The server edit carries the same SEQUENCE as ours: a genuine tie.
    let server_ics = "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:tie-uid\nSUMMARY:Server Title\nSEQUENCE:1\nEND:VTODO\nEND:VCALENDAR".to_string();
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
        .expect(1)
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
        .with_header("ETag", "\"copy-etag\"")
        .expect(1)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut base_task = Task::new("Base Title", &HashMap::new(), None);
    base_task.uid = task_uid.to_string();
    base_task.href = format!("{}{}", url, task_path);
    base_task.calendar_href = format!("{}/cal/", url);
    base_task.etag = "\"old-etag\"".to_string();
    Cache::save(
        ctx.as_ref(),
        &base_task.calendar_href,
        &[base_task.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    let mut local_task = base_task.clone();
    local_task.summary = "Local Title".to_string();
    local_task.sequence = 1;
    local_task.etag = "old-etag".to_string();
    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());

    mock_412.assert();
    mock_fetch.assert();
    mock_conflict_copy.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "the conflict copy must be pushed, not parked"
    );
}

/// The recorded base fixes the silent-revert variant of the rapid-toggle race:
/// the queued undo forked from the done state the previous push recorded
/// (etag match), the shared cache is stale (a racing fetch rewrote it), and the
/// server additionally holds a foreign description edit. The merge must use
/// the recorded base so the undo survives AND the foreign edit is preserved —
/// neither the stale cache base (reverts the undo) nor a local-wins fallback
/// (drops the foreign edit) is correct.
#[tokio::test]
async fn test_412_recorded_base_merges_undo_and_foreign_edit() {
    use cfait::model::{RawProperty, TaskStatus};

    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "recorded-base-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    // 1. The undo PUT carries the etag the store learned from the done push
    //    and 412s because the server has since gained a foreign edit.
    let mock_412 = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"e1\"")
        .with_status(412)
        .expect(1)
        .create_async()
        .await;

    // 2. The server holds our done link plus a foreign description edit.
    let server_ics = "BEGIN:VCALENDAR\nBEGIN:VTODO\nUID:recorded-base-uid\nSUMMARY:water the ferns\nDESCRIPTION:foreign edit from colleague\nSTATUS:COMPLETED\nPERCENT-COMPLETE:100\nCOMPLETED:20261008T120000Z\nSEQUENCE:1\nEND:VTODO\nEND:VCALENDAR".to_string();
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
                            <d:getetag>"e2"</d:getetag>
                        </d:prop>
                        <d:status>HTTP/1.1 200 OK</d:status>
                    </d:propstat>
                </d:response>
            </d:multistatus>
            "#,
            task_path, server_ics
        ))
        .expect(1)
        .create_async()
        .await;

    // 3. The merged retry keeps the foreign description and the undo.
    let mock_retry_ok = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"e2\"")
        .match_body(mockito::Matcher::Regex(
            r"foreign edit from colleague".to_string(),
        ))
        .with_status(201)
        .with_header("ETag", "\"e3\"")
        .expect(1)
        .create_async()
        .await;

    // 4. No conflict copy, and no retry that resurrects the done state.
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
    let mock_done_put = server
        .mock(
            "PUT",
            mockito::Matcher::Regex(r"^/cal/.*\.ics$".to_string()),
        )
        .match_body(mockito::Matcher::Regex(r"PERCENT-COMPLETE:100".to_string()))
        .with_status(201)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // The done state our own previous push landed, recorded in the journal.
    let mut done_base = Task::new("water the ferns", &HashMap::new(), None);
    done_base.uid = task_uid.to_string();
    done_base.href = format!("{}{}", url, task_path);
    done_base.calendar_href = format!("{}/cal/", url);
    done_base.etag = "\"e1\"".to_string();
    done_base.status = TaskStatus::Completed;
    done_base.percent_complete = Some(100);
    done_base.sequence = 1;
    done_base.unmapped_properties.push(RawProperty {
        key: "COMPLETED".to_string(),
        value: "20261008T120000Z".to_string(),
        params: vec![],
    });
    Journal::modify(ctx.as_ref(), |journal| {
        journal.set_base(task_uid, done_base.clone())
    })
    .unwrap();

    // The shared cache went stale (a fetch in flight before the push landed).
    let mut stale_cache = Task::new("water the ferns", &HashMap::new(), None);
    stale_cache.uid = task_uid.to_string();
    stale_cache.href = format!("{}{}", url, task_path);
    stale_cache.calendar_href = format!("{}/cal/", url);
    stale_cache.etag = "\"e0\"".to_string();
    stale_cache.percent_complete = Some(50);
    Cache::save(
        ctx.as_ref(),
        &stale_cache.calendar_href,
        &[stale_cache.clone()],
        Some("token".to_string()),
    )
    .unwrap();

    // The queued undo: undone again, sequence bumped twice, etag from the
    // done push.
    let mut local_task = Task::new("water the ferns", &HashMap::new(), None);
    local_task.uid = task_uid.to_string();
    local_task.href = format!("{}{}", url, task_path);
    local_task.calendar_href = format!("{}/cal/", url);
    local_task.etag = "\"e1\"".to_string();
    local_task.sequence = 2;
    Journal::push(ctx.as_ref(), Action::Update(local_task)).unwrap();

    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());

    mock_412.assert();
    mock_fetch.assert();
    mock_retry_ok.assert();
    mock_conflict_copy.assert();
    mock_done_put.assert();

    let journal = Journal::load(ctx.as_ref());
    assert!(
        journal.is_empty(),
        "the merged undo must be pushed, not parked or duplicated"
    );
    // The retry's own push recorded the new server-agreed state.
    let recorded = journal.base_for(task_uid).expect("base recorded for uid");
    assert_eq!(recorded.etag, "\"e3\"");
    assert!(!recorded.status.is_done());
}

/// A successful update push records the server-agreed state (fresh etag) in
/// the journal; a pushed delete forgets it.
#[tokio::test]
async fn test_pushed_update_records_and_delete_forgets_base() {
    let ctx = Arc::new(TestContext::new());

    let mut server = Server::new_async().await;
    let url = server.url();
    let task_uid = "push-writer-uid";
    let task_path = format!("/cal/{}.ics", task_uid);

    let mock_put = server
        .mock("PUT", task_path.as_str())
        .match_header("If-Match", "\"e0\"")
        .with_status(201)
        .with_header("ETag", "\"fresh-etag\"")
        .expect(1)
        .create_async()
        .await;

    let mock_delete = server
        .mock("DELETE", task_path.as_str())
        .match_header("If-Match", "\"e0\"")
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("mulch the garden beds", &HashMap::new(), None);
    task.uid = task_uid.to_string();
    task.href = format!("{}{}", url, task_path);
    task.calendar_href = format!("{}/cal/", url);
    task.etag = "e0".to_string();

    Journal::push(ctx.as_ref(), Action::Update(task.clone())).unwrap();
    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());
    mock_put.assert();

    let journal = Journal::load(ctx.as_ref());
    let recorded = journal
        .base_for(task_uid)
        .expect("successful push records the server-agreed state");
    assert_eq!(recorded.etag, "\"fresh-etag\"");
    assert_eq!(recorded.summary, "mulch the garden beds");

    Journal::push(ctx.as_ref(), Action::Delete(task)).unwrap();
    let result = client.sync_journal().await;
    assert!(result.is_ok(), "Sync failed: {:?}", result.err());
    mock_delete.assert();

    assert!(
        Journal::load(ctx.as_ref()).base_for(task_uid).is_none(),
        "a pushed delete must forget the recorded base"
    );
}
