// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression test for the compaction-once-before-loop change (commit d18d6ae7).
//!
//! When a `Move` fails and falls back to `ReplaceWith([Create(dst), Delete(src)])`,
//! a pending `Update` to the *target* calendar (same uid) is still sitting
//! further back in the queue. The old per-iteration `compact()` would merge
//! the reinserted `Create(dst)` with that `Update(dst)` into a single
//! `Create`; the new code processes them separately. The `Update` carries a
//! stale etag and would 412 — producing a spurious conflict copy — *except*
//! that etag propagation (`sync.rs` pop block) overwrites the stale etag
//! with the fresh one returned by the `Create` PUT before the `Update` is
//! processed.
//!
//! This test pins that guarantee: with the server returning an ETag on the
//! Create PUT, the subsequent Update must arrive with the propagated (fresh)
//! etag and succeed, so no conflict copy is ever created and the journal
//! drains cleanly.
use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::{Matcher, Server};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn failed_move_with_pending_update_no_conflict_copy() {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let url = server.url();

    let uid = "move-pending-update";
    let src_cal = format!("{}/cal1/", url);
    let dst_cal = format!("{}/cal2/", url);
    let src_path = format!("/cal1/{uid}.ics");
    let dst_path = format!("/cal2/{uid}.ics");
    let fresh_etag = "\"fresh-from-create\"";

    // 1. MOVE fails (non-412, e.g. 500) → triggers ReplaceWith fallback.
    //    Two attempts: first overwrite=F, then retry with overwrite=T (412 path
    //    is also retried in handle_move), both fail so ReplaceWith fires.
    server
        .mock("MOVE", Matcher::Any)
        .with_status(500)
        .expect_at_least(1)
        .create_async()
        .await;

    // 2. DELETE the source (from ReplaceWith). 204 success.
    let mock_delete = server
        .mock("DELETE", src_path.as_str())
        .with_status(204)
        .create_async()
        .await;

    // 3. CREATE at destination (from ReplaceWith). Returns the fresh ETag so
    //    etag propagation can overwrite the stale Update etag. Use If-None-Match
    //    matcher to distinguish from the Update PUT.
    let mock_create = server
        .mock("PUT", dst_path.as_str())
        .match_header("If-None-Match", "*")
        .with_status(201)
        .with_header("ETag", fresh_etag)
        .create_async()
        .await;

    // 4. UPDATE at destination — the pending edit that was queued *before* the
    //    sync. With etag propagation it must arrive with the fresh etag (not the
    //    stale one), so it succeeds and no conflict copy is produced.
    let mock_update = server
        .mock("PUT", dst_path.as_str())
        .match_header("If-Match", fresh_etag)
        .with_status(204)
        .with_header("ETag", fresh_etag)
        .create_async()
        .await;

    // 5. If a conflict copy were produced, the client would issue a *second*
    //    Create PUT to a different path (/cal2/<uuid>.ics) with If-None-Match:*.
    //    Fail loudly if that ever happens.
    let mock_conflict_copy = server
        .mock(
            "PUT",
            Matcher::Regex(r"^/cal2/[0-9a-f-]+\.ics$".to_string()),
        )
        .with_status(418)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // Queue a Move and a pending Update to the destination calendar. The
    // Update's etag is deliberately stale — the server's current etag will be
    // fresh_etag (returned by the Create).
    let mut task = Task::new("Moving Task", &HashMap::new(), None);
    task.uid = uid.to_string();
    task.href = format!("{url}{src_path}");
    task.calendar_href = src_cal.clone();
    task.etag = "\"old-src-etag\"".to_string();

    let mut moved_edit = task.clone();
    moved_edit.calendar_href = dst_cal.clone();
    moved_edit.href = format!("{url}{dst_path}");
    moved_edit.etag = "\"stale-dst-etag\"".to_string();
    moved_edit.summary = "Moving Task (edited)".to_string();

    Journal::push(ctx.as_ref(), Action::Move(task, dst_cal.clone())).unwrap();
    Journal::push(ctx.as_ref(), Action::Update(moved_edit)).unwrap();

    let res = client.sync_journal().await;
    assert!(res.is_ok(), "sync_journal failed: {:?}", res.err());

    mock_delete.assert();
    mock_create.assert();
    mock_update.assert();
    mock_conflict_copy.assert();

    // Journal must be fully drained — no conflict copy left behind, no spiral.
    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "journal must be empty after failed move + pending update"
    );
}
