// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression test: removing the last `dep:` token from a line in the tree
//! editor must remove the dependency in the store.
//!
//! The tree buffer serializes `dep:` tokens, so an absent token is an
//! explicit removal. But `apply_smart_input` restores the saved
//! dependencies when the input carries no `dep:` token (that guard exists
//! because `to_smart_string()` omits them in the single-line editor), and
//! the tree sync then only merged the parsed deps on top — so removing the
//! last dependency silently bounced back on every save.

use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::{SyncTreeOptions, TaskStore};
use std::collections::HashMap;
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn task(uid: &str, summary: &str, parent: Option<&str>, deps: &[&str]) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = "cal1".to_string();
    t.parent_uid = parent.map(str::to_string);
    t.dependencies = deps.iter().map(|s| s.to_string()).collect();
    t
}

fn options<'a>(aliases: &'a HashMap<String, Vec<String>>, calendars: &'a [cfait::model::CalendarListEntry]) -> SyncTreeOptions<'a> {
    SyncTreeOptions {
        aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars,
    }
}

/// The bench scenario: stain-base -> decking -> frame -> ledger -> stain-base
/// cycle; the user removes `dep:decking` from the stain-base line.
#[test]
fn tree_sync_removes_last_dep_token() {
    let mut store = make_store();
    store.add_task(task("bench", "Long bench", None, &[]));
    store.add_task(task("stain-base", "**Prep & Pre-Stain Base:**", Some("bench"), &["decking"]));
    store.add_task(task("decking", "**Stretchers & Decking:**", Some("bench"), &["frame"]));
    store.add_task(task("frame", "**Build Frame:**", Some("bench"), &["ledger"]));
    store.add_task(task("ledger", "**Mount Ledger:**", Some("bench"), &["stain-base"]));

    let aliases = HashMap::new();
    let calendars: Vec<cfait::model::CalendarListEntry> = Vec::new();
    let md = cfait::model::extractor::serialize_task_tree(&store, "bench", &calendars, false);
    let line = md.lines().find(|l| l.contains("uid:stain-base")).expect("stain-base line");
    assert!(line.contains("dep:decking"), "serialized line must carry the dep: {line}");
    let edited = md.replace(" dep:decking", "");
    assert!(edited != md);

    let opts = options(&aliases, &calendars);
    let (_actions, _warnings, _mints) = store
        .sync_tree_from_markdown("bench", &edited, &opts, false, true)
        .expect("tree sync should succeed");

    let deps = &store.get_task_ref("stain-base").unwrap().dependencies;
    assert!(
        !deps.iter().any(|d| d == "decking" || d.contains("decking")),
        "the removed dep must not come back: {deps:?}"
    );

    // Round-trip: re-serializing must not re-add the token.
    let md2 = cfait::model::extractor::serialize_task_tree(&store, "bench", &calendars, false);
    let line2 = md2.lines().find(|l| l.contains("uid:stain-base")).unwrap();
    assert!(!line2.contains("dep:decking"), "re-serialized line: {line2}");
}

/// Removing one of several deps must drop exactly that one.
#[test]
fn tree_sync_removes_one_of_several_deps() {
    let mut store = make_store();
    store.add_task(task("root", "Root", None, &[]));
    store.add_task(task("a", "Step A", Some("root"), &["b", "c"]));
    store.add_task(task("b", "Step B", Some("root"), &[]));
    store.add_task(task("c", "Step C", Some("root"), &[]));

    let aliases = HashMap::new();
    let calendars: Vec<cfait::model::CalendarListEntry> = Vec::new();
    let md = cfait::model::extractor::serialize_task_tree(&store, "root", &calendars, false);
    let edited = md.replace(" dep:b", "");
    assert!(edited != md);

    let opts = options(&aliases, &calendars);
    store
        .sync_tree_from_markdown("root", &edited, &opts, false, true)
        .expect("tree sync should succeed");

    let deps = &store.get_task_ref("a").unwrap().dependencies;
    assert!(!deps.iter().any(|d| d == "b"), "dep b must be gone: {deps:?}");
    assert!(deps.iter().any(|d| d == "c"), "dep c must stay: {deps:?}");
}

/// Adding a dep token must still work (the merge direction must survive).
#[test]
fn tree_sync_adds_dep_token() {
    let mut store = make_store();
    store.add_task(task("root", "Root", None, &[]));
    store.add_task(task("a", "Step A", Some("root"), &[]));
    store.add_task(task("b", "Step B", Some("root"), &[]));

    let aliases = HashMap::new();
    let calendars: Vec<cfait::model::CalendarListEntry> = Vec::new();
    let md = cfait::model::extractor::serialize_task_tree(&store, "root", &calendars, false);
    let edited = md.replacen("<!-- uid:a -->", "dep:b <!-- uid:a -->", 1);

    let opts = options(&aliases, &calendars);
    store
        .sync_tree_from_markdown("root", &edited, &opts, false, true)
        .expect("tree sync should succeed");

    let deps = &store.get_task_ref("a").unwrap().dependencies;
    assert!(deps.iter().any(|d| d == "b"), "dep b must be added: {deps:?}");
}
