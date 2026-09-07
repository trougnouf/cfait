// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression test for the "high-CPU spiral of events" reported when deleting
//! task trees with subtasks (issue: adjustable midnight rollover #73, comments).
//!
//! What this guards:
//!   1. No reprocessing spiral. Each queued action must be consumed exactly
//!      once — `sync_journal` must terminate with an empty journal and fire
//!      exactly one main DELETE per task, regardless of tree size. If the
//!      pop step in `sync_journal` ever failed to remove a processed action
//!      (e.g. `actions_match_identity` mismatch), the counts below would
//!      exceed N or the test would hang.
//!   2. A measurable baseline for the companion-event amplification. With the
//!      default config (`create_events_for_tasks = false`, no per-task +cal),
//!      the guard in `sync_companion_event` now skips the blind cleanup
//!      DELETEs entirely when no companion events could exist, so a delete
//!      tree of N tasks costs exactly N HTTP round-trips (one main DELETE per
//!      task). Previously every delete fired 5 extra blind DELETEs (3
//!      standard-suffix probes + 2 session probes) regardless of config,
//!      costing 6N round-trips. When events are enabled the full cleanup
//!      still runs, so orphaned events are never left behind.
//!
//! The two sizes assert the *same* per-task constants, which demonstrates
//! linear (O(N)) request count — not quadratic, not unbounded.
use cfait::client::RustyClient;
use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use mockito::{Matcher, Server};
use std::collections::HashMap;
use std::sync::Arc;

/// Main task DELETE: the per-task `.ics` path. Returns 204 so the delete is
/// treated as successful, which is what triggers the companion-event cleanup.
const MAIN_DELETES_PER_TASK: usize = 1;
/// Companion-event probes fired per delete with the default config
/// (`create_events_for_tasks = false`, no per-task +cal, no
/// delete-on-completion): zero. The guard in `sync_companion_event` skips
/// the blind cleanup DELETEs when no companion events could exist, which is
/// the common case and the dominant cost when deleting large task trees.
/// If events are enabled (config on, +cal, or delete-on-completion) the full
/// cleanup still runs and this constant would rise again for those tasks.
const COMPANION_DELETES_PER_TASK: usize = 0;

/// Push `n` Action::Delete entries (simulating the remote-bound output of a
/// deleted task tree), sync, and assert exact request counts.
async fn assert_delete_tree_costs(n: usize) {
    let ctx = Arc::new(TestContext::new());
    let mut server = Server::new_async().await;
    let url = server.url();
    let cal = "/cal/";

    // Main DELETE: 204 success -> triggers sync_companion_event.
    let mock_main = server
        .mock(
            "DELETE",
            Matcher::Regex(r"^/cal/task-\d+\.ics$".to_string()),
        )
        .with_status(204)
        .expect(n * MAIN_DELETES_PER_TASK)
        .create_async()
        .await;

    // Companion DELETEs: 404 so the session-probe loop terminates after two
    // consecutive 404s. Returning 204 here would loop forever.
    let mock_companion = server
        .mock("DELETE", Matcher::Regex(r"^/cal/evt-.*\.ics$".to_string()))
        .with_status(404)
        .expect(n * COMPANION_DELETES_PER_TASK)
        .create_async()
        .await;

    let client = RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    // Simulate the remote-bound output of deleting a task tree: one
    // Action::Delete per task (the local://trash Create copies are routed to
    // local storage by persist_changes and never reach the journal).
    for i in 0..n {
        let uid = format!("task-{i}");
        let mut task = Task::new(&format!("Subtask {i}"), &HashMap::new(), None);
        task.uid = uid.clone();
        task.calendar_href = cal.to_string();
        task.href = format!("{url}{cal}{uid}.ics");
        // Empty etag -> force (unconditional) DELETE; no If-Match header.
        task.etag = String::new();
        Journal::push(ctx.as_ref(), Action::Delete(task)).unwrap();
    }

    let res = client.sync_journal().await;
    assert!(res.is_ok(), "sync_journal failed: {:?}", res.err());

    // Journal must be fully drained — no action reprocessed, no spiral.
    assert!(
        Journal::load(ctx.as_ref()).is_empty(),
        "journal must be empty after syncing {n} deletes"
    );

    // Exact request counts: linear in N, one main DELETE per task.
    mock_main.assert();
    mock_companion.assert();
}

#[tokio::test]
async fn delete_tree_is_linear_no_reprocessing() {
    // Small and large sizes share identical per-task constants -> linear.
    assert_delete_tree_costs(20).await;
    assert_delete_tree_costs(80).await;
}
