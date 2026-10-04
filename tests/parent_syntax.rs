// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for the `parent:` smart-syntax token and its store-side resolution.

use std::collections::HashMap;
use std::sync::Arc;

use cfait::context::TestContext;
use cfait::model::CalendarListEntry;
use cfait::model::Task;
use cfait::model::autocomplete::suggest;
use cfait::model::parser::SyntaxType;
use cfait::model::parser::tokenize_smart_input;
use cfait::store::{DependencyWarning, TaskStore};

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn cal_entry(name: &str, href: &str) -> CalendarListEntry {
    CalendarListEntry {
        name: name.to_string(),
        href: href.to_string(),
        color: None,
        supports_vjournal: None,
    }
}

#[test]
fn parent_token_sets_raw_reference() {
    let aliases = HashMap::new();
    let t = Task::new("Repot the fern parent:\"Plant tree\"", &aliases, None);
    assert_eq!(t.summary, "Repot the fern");
    assert_eq!(t.parent_uid.as_deref(), Some("Plant tree"));

    let t = Task::new("Repot the fern parent:abc123", &aliases, None);
    assert_eq!(t.parent_uid.as_deref(), Some("abc123"));
}

#[test]
fn parent_token_is_highlighted_as_its_own_token_kind() {
    let tokens = tokenize_smart_input("Repot the fern parent:\"Plant tree\"", false);
    let parent_token = tokens
        .iter()
        .find(|t| t.kind == SyntaxType::Parent)
        .expect("parent: token should be recognized");
    assert!(parent_token.end > parent_token.start);
}

#[test]
fn edit_without_parent_token_preserves_parent() {
    let aliases = HashMap::new();
    let mut t = Task::new("Repot the fern", &aliases, None);
    t.parent_uid = Some("known-parent-uid".to_string());

    t.apply_smart_input("Repot the big fern @tomorrow", &aliases, None);
    assert_eq!(t.parent_uid.as_deref(), Some("known-parent-uid"));
    assert_eq!(t.summary, "Repot the big fern");
}

#[test]
fn resolve_parent_reference_by_summary() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut parent = Task::new("Plant tree", &aliases, None);
    parent.uid = "parent-uid".to_string();
    parent.calendar_href = "cal1".to_string();
    store.add_task(parent);

    let mut child = Task::new("Repot the fern parent:\"Plant tree\"", &aliases, None);
    child.uid = "child-uid".to_string();
    child.calendar_href = "cal1".to_string();
    let warnings = store.resolve_dependencies(&mut child);

    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    assert_eq!(
        child.parent_uid.as_deref(),
        Some("parent-uid"),
        "The raw parent reference must resolve to the matching task's UID"
    );
}

#[test]
fn resolve_parent_reference_creating_cycle_is_not_applied() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut greenhouse = Task::new("Greenhouse", &aliases, None);
    greenhouse.uid = "greenhouse".to_string();
    greenhouse.calendar_href = "cal1".to_string();
    store.add_task(greenhouse);

    let mut seedling = Task::new("Seedling", &aliases, None);
    seedling.uid = "seedling".to_string();
    seedling.calendar_href = "cal1".to_string();
    seedling.parent_uid = Some("greenhouse".to_string());
    store.add_task(seedling);

    // Re-parenting the greenhouse under its own child would create a cycle:
    // the reference must not be applied. It stays as the raw typed string,
    // which matches no task UID, so the hierarchy remains acyclic.
    let mut moved = Task::new("Greenhouse parent:\"Seedling\"", &aliases, None);
    moved.uid = "greenhouse".to_string();
    moved.calendar_href = "cal1".to_string();
    let warnings = store.resolve_dependencies(&mut moved);

    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, DependencyWarning::InvalidParent { .. })),
        "expected an InvalidParent warning, got: {warnings:?}"
    );
    assert_eq!(
        moved.parent_uid.as_deref(),
        Some("Seedling"),
        "The cyclic reference must not resolve to the child's UID"
    );
}

#[test]
fn resolve_unknown_parent_reference_is_kept_as_raw() {
    let store = make_store();
    let aliases = HashMap::new();

    let mut orphan = Task::new("Repot the fern parent:\"Plant tree\"", &aliases, None);
    orphan.uid = "orphan".to_string();
    orphan.calendar_href = "cal1".to_string();
    let warnings = store.resolve_dependencies(&mut orphan);

    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, DependencyWarning::InvalidParent { .. })),
        "expected an InvalidParent warning, got: {warnings:?}"
    );
    // Like dep:/rel:, an unresolved reference is preserved rather than
    // silently dropping the user's input.
    assert_eq!(orphan.parent_uid.as_deref(), Some("Plant tree"));
}

#[test]
fn resolve_parent_keeps_remote_uuid_untouched() {
    let store = make_store();
    let aliases = HashMap::new();

    // A full UUID that is not in the store: the parent may live on another
    // device, so it must pass through untouched instead of being detached.
    let remote_uid = "9f0b4c1e-6b8f-4f2a-9c1d-2e3f4a5b6c7d".to_string();
    let mut child = Task::new("Repot the fern", &aliases, None);
    child.uid = "child".to_string();
    child.calendar_href = "cal1".to_string();
    child.parent_uid = Some(remote_uid.clone());
    let warnings = store.resolve_dependencies(&mut child);

    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    assert_eq!(child.parent_uid.as_deref(), Some(remote_uid.as_str()));
}

#[test]
fn reparenting_overwrites_an_existing_parent_cleanly() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut parent_c = Task::new("Greenhouse", &aliases, None);
    parent_c.uid = "task-c".to_string();
    parent_c.calendar_href = "cal1".to_string();
    store.add_task(parent_c);

    let mut parent_b = Task::new("Balcony garden", &aliases, None);
    parent_b.uid = "task-b".to_string();
    parent_b.calendar_href = "cal1".to_string();
    store.add_task(parent_b);

    let mut task_a = Task::new("Repot the fern", &aliases, None);
    task_a.uid = "task-a".to_string();
    task_a.calendar_href = "cal1".to_string();
    task_a.parent_uid = Some("task-c".to_string());
    store.add_task(task_a);

    // A is a child of C; making it a child of unrelated B must cleanly
    // replace the old relationship, not duplicate or dangle anything.
    let updated = store
        .set_parent("task-a", Some("task-b".to_string()))
        .expect("re-parenting over an existing parent must succeed");
    assert_eq!(updated.parent_uid.as_deref(), Some("task-b"));

    let a = store.get_task_ref("task-a").unwrap();
    assert_eq!(a.parent_uid.as_deref(), Some("task-b"));

    // set_parent drops a parent's entry entirely once its child list is
    // empty, so C's key must be gone rather than lingering as an empty vec.
    assert_eq!(
        store.children_index.get("task-c"),
        None,
        "A must no longer be listed under its former parent C"
    );
    assert_eq!(
        store.children_index.get("task-b").map(Vec::as_slice),
        Some(&["task-a".to_string()][..]),
        "A must be listed exactly once under its new parent B"
    );
}

#[test]
fn autocomplete_task_lookup_scoped_to_visible_collections() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut visible_task = Task::new("Plant tomatoes", &aliases, None);
    visible_task.uid = "visible-uid".to_string();
    visible_task.calendar_href = "garden".to_string();
    store.add_task(visible_task);

    let mut hidden_task = Task::new("Plant a fig tree", &aliases, None);
    hidden_task.uid = "hidden-uid".to_string();
    hidden_task.calendar_href = "wishlist".to_string();
    store.add_task(hidden_task);

    let calendars = vec![
        cal_entry("Garden", "garden"),
        cal_entry("Secret wishlist", "wishlist"),
    ];
    let visible = vec!["garden".to_string()];

    let input = "dep:plant";
    let (_, suggestions) = suggest(input, input.len(), &store, &aliases, &calendars, &visible)
        .expect("expected suggestions");
    let summaries: Vec<&str> = suggestions.iter().map(|s| s.display.as_str()).collect();
    assert_eq!(summaries, vec!["Plant tomatoes"]);

    // With both collections visible, the hidden task reappears.
    let all_visible = vec!["garden".to_string(), "wishlist".to_string()];
    let (_, suggestions) = suggest(
        input,
        input.len(),
        &store,
        &aliases,
        &calendars,
        &all_visible,
    )
    .expect("expected suggestions");
    assert_eq!(suggestions.len(), 2);
}

#[test]
fn wiki_link_autocomplete_only_while_typing() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut task = Task::new("Machine IA", &aliases, None);
    task.uid = "machine-ia".to_string();
    task.calendar_href = "garden".to_string();
    store.add_task(task);

    let calendars = vec![cal_entry("Garden", "garden")];
    let visible = vec!["garden".to_string()];

    // While the link is still open, the task is suggested.
    let input = "[[Machine";
    let (_, suggestions) = suggest(input, input.len(), &store, &aliases, &calendars, &visible)
        .expect("open [[link should suggest tasks");
    assert_eq!(
        suggestions
            .iter()
            .map(|s| s.replacement.as_str())
            .collect::<Vec<_>>(),
        vec!["[[Machine IA]]"]
    );

    // A completed link must not suggest again, no matter where the cursor
    // sits inside it: following it is the context banner's job.
    let done = "[[Machine IA]]";
    assert!(
        suggest(done, done.len(), &store, &aliases, &calendars, &visible).is_none(),
        "completed [[link]] at cursor end must not autocomplete"
    );
    let mid = "[[Machine IA]]";
    assert!(
        suggest(mid, 2, &store, &aliases, &calendars, &visible).is_none(),
        "cursor right after [[ of a completed link must not autocomplete"
    );
    assert!(
        suggest(
            "[[Machine IA]] and then some",
            13,
            &store,
            &aliases,
            &calendars,
            &visible
        )
        .is_none(),
        "cursor inside a completed link must not autocomplete"
    );
}

#[test]
fn resolve_parent_scoped_to_visible_collections() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut visible_parent = Task::new("test task", &aliases, None);
    visible_parent.uid = "visible-uid".to_string();
    visible_parent.calendar_href = "tests".to_string();
    store.add_task(visible_parent);

    let mut hidden_parent = Task::new("test task", &aliases, None);
    hidden_parent.uid = "hidden-uid".to_string();
    hidden_parent.calendar_href = "archive".to_string();
    store.add_task(hidden_parent);

    let mut child = Task::new("sub-task parent:\"test task\"", &aliases, None);
    child.uid = "child-uid".to_string();
    child.calendar_href = "tests".to_string();
    let warnings = store.resolve_dependencies_scoped(&mut child, &["tests".to_string()]);

    assert!(warnings.is_empty(), "warnings: {warnings:?}");
    assert_eq!(
        child.parent_uid.as_deref(),
        Some("visible-uid"),
        "resolution must pick the only visible 'test task', ignoring hidden collections"
    );

    // Without a scope, both collections match and the reference is ambiguous.
    let mut child = Task::new("sub-task parent:\"test task\"", &aliases, None);
    child.uid = "child-uid-2".to_string();
    child.calendar_href = "tests".to_string();
    let warnings = store.resolve_dependencies(&mut child);
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, DependencyWarning::InvalidParent { .. })),
        "unscoped resolution must be ambiguous, got: {warnings:?}"
    );
}

#[test]
fn ambiguous_parent_warning_lists_candidates_with_collections() {
    let mut store = make_store();
    let aliases = HashMap::new();

    for (uid, href) in [("parent-a", "garden"), ("parent-b", "balcony")] {
        let mut t = Task::new("test task", &aliases, None);
        t.uid = uid.to_string();
        t.calendar_href = href.to_string();
        store.add_task(t);
    }

    let mut child = Task::new("sub-task parent:\"test task\"", &aliases, None);
    child.uid = "child-uid".to_string();
    child.calendar_href = "garden".to_string();
    let warnings = store
        .resolve_dependencies_scoped(&mut child, &["garden".to_string(), "balcony".to_string()]);

    let candidates = warnings
        .iter()
        .find_map(|w| match w {
            DependencyWarning::InvalidParent { candidates, .. } => Some(candidates),
            _ => None,
        })
        .expect("an InvalidParent warning carrying candidates");
    assert_eq!(candidates.len(), 2);
    let hrefs: Vec<&str> = candidates
        .iter()
        .map(|(_, _, href)| href.as_str())
        .collect();
    assert!(hrefs.contains(&"garden") && hrefs.contains(&"balcony"));
    assert!(
        warnings.iter().any(|w| w.to_string().contains("abc12345")),
        "the warning must tell the user how to disambiguate: {warnings:?}"
    );
    // The ambiguous reference is kept as typed, not dropped.
    assert_eq!(child.parent_uid.as_deref(), Some("test task"));
}

#[test]
fn autocomplete_duplicate_summaries_offer_short_uids() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut garden_task = Task::new("Plant tomatoes", &aliases, None);
    garden_task.uid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string();
    garden_task.calendar_href = "garden".to_string();
    store.add_task(garden_task);

    let mut balcony_task = Task::new("Plant tomatoes", &aliases, None);
    balcony_task.uid = "99999999-bbbb-cccc-dddd-eeeeeeeeeeee".to_string();
    balcony_task.calendar_href = "balcony".to_string();
    store.add_task(balcony_task);

    let calendars = vec![
        cal_entry("Garden", "garden"),
        cal_entry("Balcony", "balcony"),
    ];
    let visible = vec!["garden".to_string(), "balcony".to_string()];

    let input = "parent:plant";
    let (_, suggestions) = suggest(input, input.len(), &store, &aliases, &calendars, &visible)
        .expect("expected suggestions for duplicate summaries");
    assert_eq!(suggestions.len(), 2);
    for s in &suggestions {
        assert_eq!(
            s.replacement,
            format!("parent:{}", &s.replacement["parent:".len()..]),
            "duplicates must insert a short UID reference"
        );
        assert_eq!(s.replacement.len(), "parent:".len() + 8);
        assert!(
            s.display.contains(" — "),
            "duplicate rows must be labelled with their collection: {}",
            s.display
        );
    }

    // A unique summary keeps the quoted-summary reference.
    let mut unique = Task::new("Plant a fig tree", &aliases, None);
    unique.uid = "11111111-bbbb-cccc-dddd-eeeeeeeeeeee".to_string();
    unique.calendar_href = "garden".to_string();
    store.add_task(unique);
    let input = "parent:fig";
    let (_, suggestions) = suggest(input, input.len(), &store, &aliases, &calendars, &visible)
        .expect("expected suggestions");
    let s = suggestions
        .iter()
        .find(|s| s.display.contains("fig"))
        .expect("fig tree suggestion");
    assert_eq!(s.replacement, "parent:\"Plant a fig tree\"");
}
