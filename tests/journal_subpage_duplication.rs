//! Repro: an `is:page` line in a journal's markdown carves out a sub-page, and
//! re-saving the same raw text (without `<!-- uid -->` comments) accumulates
//! duplicate sub-pages because every extraction mints a fresh UUID while the
//! soft-delete guard keeps `is_journal` descendants.
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, DateType, Task};
use cfait::store::{SyncTreeOptions, TaskStore};
use std::collections::HashMap;
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn daily_root(date: chrono::NaiveDate) -> Task {
    let mut t = Task::new("", &HashMap::new(), None);
    t.uid = "root-daily".to_string();
    t.calendar_href = "local://default".to_string();
    t.is_journal = true;
    t.dtstart = Some(DateType::AllDay(date));
    t.summary = date.format("%Y-%m-%d").to_string();
    t
}

/// A daily-note document whose first line is a bullet tagged `is:page`, with the
/// rest of the body indented under it (as a pasted document would be).
const GARDENING_DOC: &str = "- Morning ritual: check the tomato stakes is:page\n  Water the seedlings before the heat builds up.\n  Mulch the beds after each watering so the soil stays cool.";

#[test]
fn is_page_line_becomes_subpage_and_resave_is_idempotent() {
    let mut store = make_store();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    store.add_task(daily_root(date));

    let calendars = [CalendarListEntry {
        name: "Local".to_string(),
        href: "local://default".to_string(),
        color: None,
        supports_vjournal: Some(true),
    }];
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &calendars,
    };

    // First save: the `is:page` line should become a single sub-page.
    let (_actions, warnings) = store
        .sync_tree_from_markdown("root-daily", GARDENING_DOC, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());

    let mut children: Vec<_> = store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.parent_uid.as_deref() == Some("root-daily"))
        .collect();
    assert_eq!(children.len(), 1, "expected one sub-page");
    let child = children.pop().unwrap();
    assert!(child.is_journal, "sub-page should be a journal component");
    assert!(
        child.summary.contains("check the tomato stakes"),
        "summary: {}",
        child.summary
    );
    assert!(
        child.description.contains("Water the seedlings"),
        "body should live in the sub-page: {}",
        child.description
    );

    // The root's own description is emptied because the body moved into the child.
    let root = store.get_task_ref("root-daily").unwrap();
    assert!(
        root.description.trim().is_empty(),
        "root description: {:?}",
        root.description
    );

    // Second save of the identical raw text (no uid comments). This must not
    // mint a second sub-page.
    let (_actions2, _warnings2) = store
        .sync_tree_from_markdown("root-daily", GARDENING_DOC, &opts, true)
        .unwrap();

    let children2: Vec<_> = store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.parent_uid.as_deref() == Some("root-daily"))
        .collect();
    assert_eq!(
        children2.len(),
        1,
        "re-saving the same text must not create duplicate sub-pages, got {}",
        children2.len()
    );
}
