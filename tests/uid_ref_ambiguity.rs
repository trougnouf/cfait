// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for short-UID reference resolution.
//!
//! Bug: a quoted summary can legitimately contain UID text verbatim (e.g. a
//! pasted task line kept as-is: `"Fix overcut main beam corner !1 +pin
//! dep:cbc9bdfc-c617-41ce-937b-174857a2c5d3"`). The resolver pooled
//! UID-prefix matches with fuzzy summary matches, so the short UID
//! `cbc9bdfc` matched both the real target task and the pasted line,
//! turning every save of the tree into an "Ambiguous reference" error.
//!
//! Fix: UID-prefix matches take priority over fuzzy summary matches; the
//! summary pool is only consulted when no UID-prefix match exists.

use std::collections::HashMap;
use std::sync::Arc;

use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::{DependencyRefError, TaskStore};

const TARGET_UID: &str = "cbc9bdfc-c617-41ce-937b-174857a2c5d3";

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn add_task(store: &mut TaskStore, uid: &str, summary: &str) {
    let aliases = HashMap::new();
    let mut task = Task::new(summary, &aliases, None);
    task.uid = uid.to_string();
    task.calendar_href = "cal1".to_string();
    store.add_task(task);
}

/// A short UID that is a unique prefix of exactly one task must resolve to
/// that task even when another task's summary contains the UID text.
#[test]
fn uid_prefix_wins_over_summary_containing_uid_text() {
    let mut store = make_store();
    add_task(
        &mut store,
        TARGET_UID,
        "Waterproof the shed floor before the rain",
    );
    // A pasted task line, quoted so the tokens stay literal text.
    add_task(
        &mut store,
        "695af3a1-cf26-4ef8-8025-a02f3223b187",
        "\"Fix overcut main beam corner !1 +pin dep:cbc9bdfc-c617-41ce-937b-174857a2c5d3\"",
    );

    let result = store.resolve_dependency_ref("cbc9bdfc", None);
    assert_eq!(result.unwrap(), TARGET_UID);
}

/// The dependency warning candidate list must not surface the pasted-line
/// summary as a second candidate for a unique UID prefix.
#[test]
fn candidates_exclude_summary_matches_for_uid_prefix() {
    let mut store = make_store();
    add_task(
        &mut store,
        TARGET_UID,
        "Waterproof the shed floor before the rain",
    );
    add_task(
        &mut store,
        "695af3a1-cf26-4ef8-8025-a02f3223b187",
        "\"Fix overcut main beam corner !1 +pin dep:cbc9bdfc-c617-41ce-937b-174857a2c5d3\"",
    );

    let candidates = store.get_dependency_candidates("cbc9bdfc", None, None);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, TARGET_UID);
}

/// `resolve_dependencies` must heal a raw short UID stored in the
/// dependency list into the full UID without warnings.
#[test]
fn resolve_dependencies_heals_short_uid_dep() {
    let mut store = make_store();
    add_task(
        &mut store,
        TARGET_UID,
        "Waterproof the shed floor before the rain",
    );
    add_task(
        &mut store,
        "695af3a1-cf26-4ef8-8025-a02f3223b187",
        "\"Fix overcut main beam corner !1 +pin dep:cbc9bdfc-c617-41ce-937b-174857a2c5d3\"",
    );

    let aliases = HashMap::new();
    let mut task = Task::new("Screw the joist hangers", &aliases, None);
    task.calendar_href = "cal1".to_string();
    task.dependencies = vec!["cbc9bdfc".to_string()];

    let warnings = store.resolve_dependencies(&mut task);
    assert!(warnings.is_empty(), "got warnings: {warnings:?}");
    assert_eq!(task.dependencies, vec![TARGET_UID.to_string()]);
}

/// Two tasks genuinely sharing a UID prefix are still reported ambiguous.
#[test]
fn genuine_uid_prefix_collision_stays_ambiguous() {
    let mut store = make_store();
    add_task(
        &mut store,
        "aaa11111-0000-0000-0000-000000000001",
        "Brew the porter",
    );
    add_task(
        &mut store,
        "aaa22222-0000-0000-0000-000000000002",
        "Bottle the porter",
    );

    match store.resolve_dependency_ref("aaa", None) {
        Err(DependencyRefError::Ambiguous { reference, matches }) => {
            assert_eq!(reference, "aaa");
            assert_eq!(matches.len(), 2);
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

/// Fuzzy summary matching still works when no UID-prefix match exists.
#[test]
fn summary_match_still_works_without_uid_prefix() {
    let mut store = make_store();
    add_task(
        &mut store,
        "11111111-0000-0000-0000-000000000001",
        "Mix one part lime putty with three parts sand",
    );

    let result = store.resolve_dependency_ref("lime putty", None);
    assert_eq!(result.unwrap(), "11111111-0000-0000-0000-000000000001");
}
