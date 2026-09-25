// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for disk-based trash operations.
//!
//! `empty_trash` and `prune_trash` must enumerate from disk rather than from the
//! in-memory store, so items created by another cfait instance (the CLI, the
//! background daemon) are purged too, and disk failures surface to the caller
//! instead of silently no-op'ing.
use cfait::config::Config;
use cfait::context::TestContext;
use cfait::controller::TaskController;
use cfait::model::{RawProperty, Task};
use cfait::storage::LocalStorage;
use cfait::store::TaskStore;
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Arc;

const TRASH: &str = "local://trash";

fn trash_task(uid: &str) -> Task {
    let mut task = Task::new("water the tomatoes", &HashMap::new(), None);
    task.uid = uid.to_string();
    task.calendar_href = TRASH.to_string();
    task
}

fn trashed_task(uid: &str, age_days: i64) -> Task {
    let mut task = trash_task(uid);
    let when = chrono::Utc::now() - chrono::Duration::days(age_days);
    task.unmapped_properties.push(RawProperty {
        key: "X-TRASHED-DATE".to_string(),
        value: when.to_rfc3339(),
        params: vec![],
    });
    task
}

/// A controller with a deliberately empty in-memory store: the disk is the
/// source of truth for trash operations.
fn build_controller(ctx: Arc<TestContext>) -> TaskController {
    let store = Arc::new(tokio::sync::Mutex::new(TaskStore::new(ctx.clone())));
    let client = Arc::new(tokio::sync::Mutex::new(None));
    TaskController::new(store, client, ctx)
}

#[tokio::test]
#[serial]
async fn empty_trash_purges_disk_items_even_when_store_is_empty() {
    let ctx = Arc::new(TestContext::new());
    // Two items dropped in the trash by "another instance" — our store never
    // saw them.
    LocalStorage::save_for_href(ctx.as_ref(), TRASH, &[trash_task("a"), trash_task("b")]).unwrap();
    let controller = build_controller(ctx.clone());

    let count = controller.empty_trash().await.unwrap();
    assert_eq!(count, 2);
    let remaining = LocalStorage::load_for_href(ctx.as_ref(), TRASH).unwrap();
    assert!(remaining.is_empty(), "trash file should be emptied on disk");
}

#[tokio::test]
#[serial]
async fn empty_trash_on_empty_trash_returns_zero() {
    let ctx = Arc::new(TestContext::new());
    let controller = build_controller(ctx.clone());

    let count = controller.empty_trash().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[serial]
async fn prune_trash_respects_retention_and_keeps_undated_items() {
    let ctx = Arc::new(TestContext::new());
    let config = Config {
        trash_retention_days: 7,
        ..Default::default()
    };
    config.save(ctx.as_ref()).unwrap();

    // One item past the 7-day retention, one fresh, one without a
    // X-TRASHED-DATE (must survive — a missing property never causes loss).
    LocalStorage::save_for_href(
        ctx.as_ref(),
        TRASH,
        &[
            trashed_task("old", 10),
            trashed_task("fresh", 1),
            trash_task("undated"),
        ],
    )
    .unwrap();
    let controller = build_controller(ctx.clone());

    let count = controller.prune_trash().await.unwrap();
    assert_eq!(count, 1);
    let remaining = LocalStorage::load_for_href(ctx.as_ref(), TRASH).unwrap();
    let uids: Vec<&str> = remaining.iter().map(|t| t.uid.as_str()).collect();
    assert_eq!(uids.len(), 2);
    assert!(uids.contains(&"fresh"));
    assert!(uids.contains(&"undated"));
}

#[tokio::test]
#[serial]
async fn prune_trash_is_disabled_when_retention_is_zero() {
    let ctx = Arc::new(TestContext::new());
    let config = Config {
        trash_retention_days: 0,
        ..Default::default()
    };
    config.save(ctx.as_ref()).unwrap();
    LocalStorage::save_for_href(ctx.as_ref(), TRASH, &[trashed_task("old", 365)]).unwrap();
    let controller = build_controller(ctx.clone());

    let count = controller.prune_trash().await.unwrap();
    assert_eq!(count, 0);
    let remaining = LocalStorage::load_for_href(ctx.as_ref(), TRASH).unwrap();
    assert_eq!(remaining.len(), 1, "retention=0 must never purge");
}
