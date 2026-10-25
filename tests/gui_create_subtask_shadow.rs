// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression test for https://git.disroot.org/trougnouf/cfait/issues/23
//!
//! Creating a task with subtask markdown in the description box extracts the
//! subtasks as children. A plain task stores a prose-only description: the
//! extracted lines live on as child tasks, never as uid-tagged shadow text
//! in the parent's description (journal pages are the exception — their
//! description is the document, so they keep their shadows). The previews
//! still strip shadow lines via `description_prose`, so even a legacy
//! description carrying them (saved by 1.2.0) renders cleanly, a parent
//! created from subtask-only markdown shows no description text, and
//! toggling details never toggles a phantom description.
//!
//! These tests pin the storage policy and the view invariant it relies on:
//! after the create (or a no-op edit) of a parent with subtasks, the parent's
//! stored description holds only prose, and the children are neither
//! duplicated nor lost.
#![cfg(feature = "gui")]

use cfait::gui::message::Message;
use cfait::gui::state::GuiApp;
use cfait::gui::update;
use cfait::model::extractor::description_prose;
use cfait::model::{CalendarListEntry, Task};

fn app_with_local_calendar() -> GuiApp {
    GuiApp {
        calendars: vec![CalendarListEntry {
            name: "Local".to_string(),
            href: "local://default".to_string(),
            color: None,
            supports_vjournal: Some(true),
            supports_vevent: Some(true),
        }],
        active_cal_href: Some("local://default".to_string()),
        ..Default::default()
    }
}

fn submit_new_task(app: &mut GuiApp, title: &str, description: &str) {
    app.input_value = iced::widget::text_editor::Content::with_text(title);
    app.description_value = iced::widget::text_editor::Content::with_text(description);
    let _ = update::update(app, Message::SubmitTask);
}

fn all_tasks(app: &GuiApp) -> Vec<Task> {
    app.store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .cloned()
        .collect()
}

fn children_of(app: &GuiApp, parent_uid: &str) -> Vec<Task> {
    let mut children: Vec<Task> = all_tasks(app)
        .into_iter()
        .filter(|t| t.parent_uid.as_deref() == Some(parent_uid))
        .collect();
    children.sort_by(|a, b| a.summary.cmp(&b.summary));
    children
}

#[test]
fn create_with_subtask_markdown_stores_prose_only_description_on_parent() {
    let mut app = app_with_local_calendar();
    submit_new_task(
        &mut app,
        "water the garden",
        "- [ ] buy seeds\n- [ ] dig the beds",
    );

    let parent_uid = app
        .selected_uid
        .clone()
        .expect("the new parent should be selected");
    let parent = app
        .store
        .get_task_ref(&parent_uid)
        .expect("the parent task should exist")
        .clone();

    assert_eq!(parent.summary, "water the garden");

    // The subtasks were pulled out of the description into child tasks.
    let children = children_of(&app, &parent_uid);
    assert_eq!(
        children
            .iter()
            .map(|t| t.summary.as_str())
            .collect::<Vec<_>>(),
        vec!["buy seeds", "dig the beds"]
    );
    assert!(children.iter().all(|t| t.description.is_empty()));

    // A plain task stores prose only: the description left over from a
    // subtask-only input is empty — no uid-tagged shadow lines, so no
    // preview surface can show them as description text (this is what made
    // the row render the subtasks as a bogus description and made Ctrl+Enter
    // toggle it instead of the details) and third-party clients see a clean
    // DESCRIPTION.
    assert!(
        parent.description.is_empty(),
        "a VTODO must store a prose-only description, got: {:?}",
        parent.description
    );
    assert!(
        description_prose(&parent.description).is_empty(),
        "parent prose must be empty, got: {:?}",
        description_prose(&parent.description)
    );
}

#[test]
fn create_with_prose_and_subtasks_keeps_only_the_prose() {
    let mut app = app_with_local_calendar();
    submit_new_task(
        &mut app,
        "water the garden",
        "remember the hose\n- [ ] buy seeds",
    );

    let parent_uid = app.selected_uid.clone().expect("parent selected");
    let parent = app.store.get_task_ref(&parent_uid).unwrap().clone();

    // Only the prose is stored; the subtask line is gone from the description.
    assert_eq!(parent.description, "remember the hose");
    assert_eq!(description_prose(&parent.description), "remember the hose");
    let children = children_of(&app, &parent_uid);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].summary, "buy seeds");
}

#[test]
fn noop_edit_of_parent_with_subtasks_does_not_duplicate_or_lose_prose() {
    let mut app = app_with_local_calendar();
    submit_new_task(&mut app, "water the garden", "- [ ] buy seeds");

    let parent_uid = app.selected_uid.clone().expect("parent selected");
    let child_uids: Vec<String> = children_of(&app, &parent_uid)
        .iter()
        .map(|t| t.uid.clone())
        .collect();
    assert_eq!(child_uids.len(), 1);

    // Open the summary/description editor (what `e` does) and save unchanged.
    let idx = app
        .find_task_index_by_uid(&parent_uid)
        .expect("parent should be in the visible list");
    let _ = update::update(&mut app, Message::EditTaskStart(idx));
    assert_eq!(app.editing_uid.as_deref(), Some(parent_uid.as_str()));
    let _ = update::update(&mut app, Message::SubmitTask);

    // The shadow-bound child is re-bound in place: no duplicate, no loss.
    let children = children_of(&app, &parent_uid);
    assert_eq!(
        children.iter().map(|t| t.uid.as_str()).collect::<Vec<_>>(),
        child_uids.as_slice()
    );
    assert_eq!(
        all_tasks(&app).len(),
        2,
        "parent + one child, no duplicates"
    );

    // The parent's stored description is still prose-only: the merged view
    // the editor loaded (prose + uid-tagged children) must not surface the
    // subtask line as description text.
    let parent = app.store.get_task_ref(&parent_uid).unwrap().clone();
    assert!(
        parent.description.is_empty(),
        "parent description must stay prose-only after a no-op edit, got: {:?}",
        parent.description
    );
    assert!(
        description_prose(&parent.description).is_empty(),
        "parent prose must stay empty after a no-op edit, got: {:?}",
        description_prose(&parent.description)
    );
}
