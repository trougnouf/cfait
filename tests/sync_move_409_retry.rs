// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression test: a `MOVE` that fails with 409 Conflict because the
//! destination is already occupied must be retried with `Overwrite: T`,
//! instead of falling back to `ReplaceWith([Create(dst), Delete(src)])`.
//! The fallback re-PUTs to the same occupied destination and can spawn a
//! spurious "Conflict Copy".
use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::Server;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn move_409_retries_with_overwrite() {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let url = server.url();

    let uid = "move-409-retry";
    let src_cal = format!("{}/cal1/", url);
    let dst_cal = format!("{}/cal2/", url);
    let src_path = format!("/cal1/{uid}.ics");
    let dst_path = format!("/cal2/{uid}.ics");

    // 1. First MOVE (Overwrite: F) — destination occupied -> 409 Conflict.
    let mock_first = server
        .mock("MOVE", src_path.as_str())
        .match_header("Overwrite", "F")
        .with_status(409)
        .create_async()
        .await;

    // 2. Retry MOVE (Overwrite: T) -> success.
    let mock_retry = server
        .mock("MOVE", src_path.as_str())
        .match_header("Overwrite", "T")
        .with_status(201)
        .create_async()
        .await;

    // 3. After the move, sync_journal fetches the fresh etag of the new
    //    location (the MOVE outcome carries no etag).
    let mock_etag = server
        .mock("PROPFIND", dst_path.as_str())
        .with_status(207)
        .with_body(format!(
            r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>{}</d:href><d:propstat><d:prop><d:getetag>"moved-etag"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#,
            dst_path
        ))
        .create_async()
        .await;

    // 4 & 5. The Create+Delete fallback must NOT fire: no PUT to the
    //    destination, no DELETE of the source.
    let mock_fallback_create = server
        .mock("PUT", dst_path.as_str())
        .with_status(201)
        .expect(0)
        .create_async()
        .await;
    let mock_fallback_delete = server
        .mock("DELETE", src_path.as_str())
        .with_status(204)
        .expect(0)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task = Task::new("Moving Task", &HashMap::new(), None);
    task.uid = uid.to_string();
    task.href = format!("{url}{src_path}");
    task.calendar_href = src_cal.clone();
    task.etag = "\"old-src-etag\"".to_string();

    Journal::push(ctx.as_ref(), Action::Move(task, dst_cal.clone())).unwrap();

    let res = client.sync_journal().await;
    assert!(res.is_ok(), "sync_journal failed: {:?}", res.err());

    mock_first.assert();
    mock_retry.assert();
    mock_etag.assert();
    mock_fallback_create.assert();
    mock_fallback_delete.assert();

    // The journal must drain with the task moved (not rescued, not duplicated).
    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "journal must be empty after successful move retry"
    );

    // The synced task must carry the fresh etag and href of the new location.
    let (_, synced) = res.unwrap();
    assert_eq!(synced.len(), 1, "moved task should be reported as synced");
    assert_eq!(synced[0].href, format!("{url}{dst_path}"));
    // The etag is stored verbatim from the server, quotes included.
    assert_eq!(synced[0].etag, "\"moved-etag\"");
}
