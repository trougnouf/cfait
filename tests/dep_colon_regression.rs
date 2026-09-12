// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for dependency resolution and preservation.
//!
//! Bug 1: Task summaries containing ':' (e.g. markdown headers like
//! "**Bardage (Horizontal):**") broke `resolve_dependency_ref` because
//! the parser strips quotes before the value reaches the resolver, and
//! `split_path_respecting_quotes` then splits on the ':' inside the
//! summary, creating false path segments.
//!
//! Bug 2: `to_smart_string()` does not serialize `dep:`/`rel:` tokens, but
//! `apply_smart_input` unconditionally cleared `dependencies` and
//! `related_to`. So editing a task title silently lost its dependencies.

use std::collections::HashMap;
use std::sync::Arc;

use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::TaskStore;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

// ── Bug 1: dep: with ':' in the summary ───────────────────────────────

#[test]
fn resolve_dep_with_colon_in_summary() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut target = Task::new(
        "Bardage (Horizontal): cladding instructions",
        &aliases,
        None,
    );
    target.uid = "target-uid".to_string();
    target.calendar_href = "cal1".to_string();
    store.add_task(target);

    // The value that would be stored after the parser strips quotes from
    // `dep:"Bardage (Horizontal): cladding instructions"`.
    let raw = "Bardage (Horizontal): cladding instructions";
    let result = store.resolve_dependency_ref(raw, None);
    assert!(
        result.is_ok(),
        "should resolve dep with ':' in summary, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "target-uid");
}

#[test]
fn resolve_dep_with_multiple_colons_in_summary() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let summary = "Step 1: mix: apply 20% limonene + 80% tung oil";
    let mut target = Task::new(summary, &aliases, None);
    target.uid = "multi-colon-uid".to_string();
    target.calendar_href = "cal1".to_string();
    store.add_task(target);

    let result = store.resolve_dependency_ref(summary, None);
    assert!(
        result.is_ok(),
        "should resolve dep with multiple ':' in summary, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "multi-colon-uid");
}

#[test]
fn resolve_dep_with_markdown_colon_in_summary() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let summary = "**Bardage (Horizontal):** Screw your 18x130 mm Douglas cladding";
    let mut target = Task::new(summary, &aliases, None);
    target.uid = "markdown-uid".to_string();
    target.calendar_href = "cal1".to_string();
    store.add_task(target);

    let result = store.resolve_dependency_ref(summary, None);
    assert!(
        result.is_ok(),
        "should resolve dep with markdown ':' in summary, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "markdown-uid");
}

// ── Bug 2: apply_smart_input preserves deps/relations ─────────────────

#[test]
fn apply_smart_input_preserves_dependencies_on_edit() {
    let aliases = HashMap::new();

    let mut task = Task::new("Water the garden", &aliases, None);
    task.dependencies.push("some-blocker-uid".to_string());
    task.related_to.push("some-sibling-uid".to_string());

    // Simulate editing: to_smart_string() does NOT include dep:/rel: tokens.
    let smart = task.to_smart_string();
    assert!(
        !smart.contains("dep:"),
        "to_smart_string should not serialize dep: tokens"
    );
    assert!(
        !smart.contains("rel:"),
        "to_smart_string should not serialize rel: tokens"
    );

    // apply_smart_input with the edited text (no dep:/rel: tokens) should
    // preserve the existing dependencies and relations.
    task.apply_smart_input(&smart, &aliases, None);

    assert!(
        task.dependencies.contains(&"some-blocker-uid".to_string()),
        "dependencies should be preserved when input has no dep: tokens"
    );
    assert!(
        task.related_to.contains(&"some-sibling-uid".to_string()),
        "related_to should be preserved when input has no rel: tokens"
    );
}

#[test]
fn apply_smart_input_updates_dependencies_when_dep_token_present() {
    let aliases = HashMap::new();

    let mut task = Task::new("Water the garden", &aliases, None);
    task.dependencies.push("old-blocker".to_string());

    // Edit with a new dep: token — should replace, not merge.
    task.apply_smart_input("Water the garden dep:new-blocker", &aliases, None);

    assert!(
        task.dependencies.contains(&"new-blocker".to_string()),
        "new dependency should be set"
    );
    assert!(
        !task.dependencies.contains(&"old-blocker".to_string()),
        "old dependency should be replaced when dep: token is present"
    );
}

#[test]
fn apply_smart_input_preserves_relations_on_edit() {
    let aliases = HashMap::new();

    let mut task = Task::new("Prune the roses", &aliases, None);
    task.related_to.push("sibling-1".to_string());
    task.related_to.push("sibling-2".to_string());

    let smart = task.to_smart_string();
    task.apply_smart_input(&smart, &aliases, None);

    assert_eq!(
        task.related_to.len(),
        2,
        "both relations should be preserved"
    );
    assert!(task.related_to.contains(&"sibling-1".to_string()));
    assert!(task.related_to.contains(&"sibling-2".to_string()));
}

#[test]
fn new_task_with_dep_token_still_works() {
    let aliases = HashMap::new();

    let task = Task::new("Paint the fence dep:\"Build the fence\"", &aliases, None);
    assert_eq!(task.dependencies.len(), 1);
    assert_eq!(task.dependencies[0], "Build the fence");
}
