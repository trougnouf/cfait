// SPDX-License-Identifier: GPL-3.0-or-later
//! `is:flat` / `is:doc` keyword: a VTODO whose description is parsed like a
//! VJOURNAL — Markdown headers and plain list items stay as document text
//! instead of being extracted into subtasks (checkbox items still are).
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, Task};
use cfait::store::{SyncTreeOptions, TaskStore};
use std::collections::HashMap;
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn calendars() -> Vec<CalendarListEntry> {
    vec![CalendarListEntry {
        name: "Local".to_string(),
        href: "local://default".to_string(),
        color: None,
        supports_vjournal: Some(true),
    }]
}

fn children_of(store: &TaskStore, parent: &str) -> Vec<Task> {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.parent_uid.as_deref() == Some(parent))
        .cloned()
        .collect()
}

#[test]
fn flat_keyword_parses_and_roundtrips() {
    let aliases = HashMap::new();

    let t = Task::new("Trip planning is:flat", &aliases, None);
    assert!(t.flat);
    assert!(!t.is_journal, "is:flat must stay a VTODO");
    assert!(!t.is_note, "is:flat does not imply is:note");
    assert_eq!(t.summary, "Trip planning");

    let t_doc = Task::new("Trip planning is:doc", &aliases, None);
    assert!(t_doc.flat, "is:doc is an alias of is:flat");
    assert_eq!(t_doc.summary, "Trip planning");

    // The token round-trips through the smart string.
    let smart = t.to_smart_string();
    assert!(smart.contains("is:flat"), "smart string: {smart}");
    let reparsed = Task::new(&smart, &aliases, None);
    assert!(reparsed.flat);
    assert_eq!(reparsed.summary, "Trip planning");

    // Deleting the token unflattens the task.
    let mut t_unflat = t.clone();
    t_unflat.apply_smart_input("Trip planning", &aliases, None);
    assert!(!t_unflat.flat);

    // The literal words are special tokens (escaped when kept as text).
    assert!(cfait::model::parser::is_special_token("is:flat"));
    assert!(cfait::model::parser::is_special_token("is:doc"));
}

#[test]
fn flat_doc_keeps_headers_and_lists_as_prose() {
    let md = "# Trip notes\nSome prose.\n- Water the tomatoes\n- [ ] Book the campsite";
    let mut media = HashMap::new();

    // Flat-document parsing (journal rules): only the checkbox item extracts.
    let (clean, extracted) = cfait::model::extractor::extract_markdown_tasks(md, true, &mut media);
    assert_eq!(
        extracted.len(),
        1,
        "only the checkbox item is extracted: {extracted:?}"
    );
    assert_eq!(extracted[0].raw_text, "Book the campsite");
    assert!(
        clean.contains("# Trip notes"),
        "header stays prose: {clean}"
    );
    assert!(
        clean.contains("- Water the tomatoes"),
        "plain list item stays prose: {clean}"
    );
    assert!(
        clean.contains("<!-- uid:"),
        "the extracted child leaves a shadow line: {clean}"
    );

    // Plain parsing shreds the header and the plain list item into subtasks.
    let mut media_plain = HashMap::new();
    let (_clean_plain, extracted_plain) =
        cfait::model::extractor::extract_markdown_tasks(md, false, &mut media_plain);
    assert_eq!(
        extracted_plain.len(),
        3,
        "plain parsing extracts header, list item and checkbox: {extracted_plain:?}"
    );
    assert!(
        extracted_plain
            .iter()
            .any(|e| e.raw_text == "Trip notes" && e.is_note)
    );
    assert!(
        extracted_plain
            .iter()
            .any(|e| e.raw_text == "Water the tomatoes")
    );
}

#[test]
fn flat_stored_description_keeps_shadows() {
    let desc = "Prose\n- [ ] Child <!-- uid:abc-123 -->";
    assert_eq!(
        cfait::model::extractor::stored_description(desc, true),
        desc,
        "a flat document keeps its shadow lines"
    );
    assert_eq!(
        cfait::model::extractor::stored_description(desc, false),
        "Prose",
        "a plain task stores prose only"
    );
}

#[test]
fn has_extractable_subtasks_respects_flat_docs() {
    let prose_only = "# Header\n- plain item\njust text";
    assert!(!cfait::model::extractor::has_extractable_subtasks(
        prose_only, true
    ));
    assert!(cfait::model::extractor::has_extractable_subtasks(
        prose_only, false
    ));
    assert!(cfait::model::extractor::has_extractable_subtasks(
        "# Header\n- [ ] real task",
        true
    ));
}

#[test]
fn nested_flat_line_scopes_its_subtree() {
    let md = "- [ ] Project\n    - Doc is:flat\n        # Notes\n        - plain item\n    - [ ] Real child";
    let mut media = HashMap::new();
    let (_, extracted) = cfait::model::extractor::extract_markdown_tasks(md, false, &mut media);

    assert_eq!(
        extracted.len(),
        3,
        "Project, Doc and Real child: {extracted:?}"
    );
    let doc = &extracted[1];
    assert_eq!(doc.raw_text, "Doc is:flat");
    let doc_task = Task::new(&doc.raw_text, &HashMap::new(), None);
    assert!(doc_task.flat, "the nested line parses as a flat doc");
    assert_eq!(doc_task.summary, "Doc");
    assert!(
        doc.description.contains("# Notes"),
        "the header under the flat line stays prose: {:?}",
        doc.description
    );
    assert!(
        doc.description.contains("- plain item"),
        "the plain list item under the flat line stays prose: {:?}",
        doc.description
    );

    let real_child = &extracted[2];
    assert_eq!(real_child.raw_text, "Real child");
    assert_eq!(
        real_child.parent_uid.as_deref(),
        Some(extracted[0].uid.as_str()),
        "the checkbox child stays a sibling of the flat doc, not nested under it"
    );
}

#[test]
fn sync_tree_from_markdown_parses_flat_root_as_document() {
    let mut store = make_store();
    let aliases = HashMap::new();

    let mut root = Task::new("Garden plans is:flat", &aliases, None);
    root.uid = "flat-1".to_string();
    root.calendar_href = "local://default".to_string();
    root.description = "Spring notes:\n# Tasks\n- Water the tomatoes".to_string();
    store.add_task(root);

    let cals = calendars();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };

    // The tree editor serializes the flat task with its checkbox line; the
    // description body follows indented.
    let md = cfait::model::extractor::serialize_task_tree(&store, "flat-1", &cals, false);
    assert!(
        md.starts_with("- [ ] Garden plans is:flat"),
        "a flat task keeps its checkbox line:\n{md}"
    );

    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("flat-1", &md, &opts, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "re-syncing an unchanged flat tree is a no-op: {actions:?}"
    );

    let root_after = store.get_task_ref("flat-1").unwrap();
    assert!(
        root_after.flat,
        "the is:flat token from the root line re-parses"
    );
    assert_eq!(
        root_after.description, "Spring notes:\n# Tasks\n- Water the tomatoes",
        "headers and plain list items stay prose in the stored description"
    );
    assert!(
        children_of(&store, "flat-1").is_empty(),
        "no subtasks are shredded out of the document"
    );

    // An explicit checkbox line still becomes a child, and its shadow line
    // stays in the document.
    let md_with_child = format!("{md}\n  - [ ] Buy seeds <!-- uid:child-1 -->\n");
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("flat-1", &md_with_child, &opts, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.uid == "child-1")),
        "the checkbox line creates a child: {actions:?}"
    );

    let children = children_of(&store, "flat-1");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].summary, "Buy seeds");
    assert!(!children[0].is_note);

    let root_after = store.get_task_ref("flat-1").unwrap();
    assert!(
        root_after
            .description
            .contains("- [ ] Buy seeds <!-- uid:child-1 -->"),
        "the child's shadow line stays in the document: {:?}",
        root_after.description
    );

    // Round-trip: the re-serialized tree re-syncs to no actions.
    let md2 = cfait::model::extractor::serialize_task_tree(&store, "flat-1", &cals, false);
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("flat-1", &md2, &opts, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "the flat tree round-trip is idempotent: {actions2:?}"
    );
}

#[test]
fn search_is_flat_matches_flat_tasks_only() {
    let store = make_store();
    let aliases = HashMap::new();

    let flat = Task::new("Garden plans is:flat", &aliases, None);
    let flat_doc = Task::new("Garden plans is:doc", &aliases, None);
    let plain = Task::new("Garden plans", &aliases, None);
    let page = Task::new("Garden plans is:page", &aliases, None);

    assert!(flat.matches_search_term("is:flat", &store));
    assert!(flat.matches_search_term("is:doc", &store));
    assert!(flat_doc.matches_search_term("is:flat", &store));
    assert!(!plain.matches_search_term("is:flat", &store));
    assert!(plain.matches_search_term("-is:flat", &store));
    assert!(
        !page.matches_search_term("is:flat", &store),
        "journal pages are matched by is:page, not is:flat"
    );
    assert!(page.matches_search_term("is:page", &store));
}

#[test]
fn ics_roundtrip_preserves_flat() {
    let aliases = HashMap::new();
    let mut t = Task::new("Garden plans is:flat", &aliases, None);
    t.uid = "flat-ics".to_string();

    let ics = t.to_ics();
    assert!(
        ics.contains("BEGIN:VTODO"),
        "a flat task stays a VTODO:\n{ics}"
    );
    assert!(!ics.contains("BEGIN:VJOURNAL"));
    assert!(
        ics.contains("X-CFAIT-FLAT:TRUE"),
        "flat flag is stored:\n{ics}"
    );

    let parsed = Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).unwrap();
    assert!(parsed.flat);
    assert!(!parsed.is_journal);
    assert_eq!(parsed.summary, "Garden plans");
}

#[test]
fn flat_task_extracts_transient_description_metadata() {
    let aliases = HashMap::new();
    let mut t = Task::new("Garden plans is:flat", &aliases, None);
    t.description = "Ideas for #gardening at @@home".to_string();
    t.extract_transient_metadata(&aliases);
    assert!(
        t.transient_desc_tags.contains(&"gardening".to_string()),
        "tags in a flat document body surface transiently: {:?}",
        t.transient_desc_tags
    );
    assert!(
        t.transient_desc_locs.contains(&"home".to_string()),
        "locations in a flat document body surface transiently: {:?}",
        t.transient_desc_locs
    );
    assert_eq!(
        t.cached_has_subtasks,
        Some(false),
        "a prose-only flat document has no extractable subtasks"
    );
}
