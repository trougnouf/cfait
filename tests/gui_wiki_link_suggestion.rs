// SPDX-License-Identifier: GPL-3.0-or-later
//! Applying an autocomplete suggestion in the journal editor leaves the caret
//! right after the replacement (not at the end of the document), and clicking
//! a suggestion chip drops the cycle so the banner closes instead of staying
//! open on the applied entry.
#![cfg(feature = "gui")]

use cfait::gui::message::Message;
use cfait::gui::state::{GuiApp, SidebarMode, SuggestionEditor};
use cfait::gui::update;
use cfait::model::Task;
use std::collections::HashMap;

fn app_with_journal(tasks: &[&str]) -> GuiApp {
    // The engine only suggests tasks from collections the user can see.
    let mut app = GuiApp {
        calendars: vec![cfait::model::CalendarListEntry {
            name: "Local".to_string(),
            href: "local://default".to_string(),
            color: None,
            supports_vjournal: Some(true),
        }],
        ..Default::default()
    };
    for (i, summary) in tasks.iter().enumerate() {
        let mut t = Task::new(summary, &HashMap::new(), None);
        t.uid = format!("task-{i}");
        t.calendar_href = "local://default".to_string();
        app.store.add_task(t);
    }
    app.sidebar_mode = SidebarMode::Journal;
    app
}

fn set_journal_text(app: &mut GuiApp, text: &str) {
    app.journal_editor_content = iced::widget::text_editor::Content::with_text(text);
}

fn set_caret(app: &mut GuiApp, line: usize, column: usize) {
    app.journal_editor_content
        .move_to(iced::widget::text_editor::Cursor {
            position: iced::widget::text_editor::Position { line, column },
            selection: None,
        });
}

fn caret(app: &GuiApp) -> (usize, usize) {
    let p = app.journal_editor_content.cursor().position;
    (p.line, p.column)
}

fn byte_offset_of(text: &str, line: usize, column: usize) -> usize {
    let mut offset = 0;
    for (i, l) in text.split('\n').enumerate() {
        if i == line {
            return offset + l.chars().take(column).map(|c| c.len_utf8()).sum::<usize>();
        }
        offset += l.len() + 1;
    }
    panic!("no line {line} in {text:?}");
}

fn banner_at(
    app: &GuiApp,
    line: usize,
    column: usize,
) -> Option<(std::ops::Range<usize>, Vec<String>)> {
    let text = app.journal_editor_content.text();
    let pos = byte_offset_of(&text, line, column);
    cfait::model::autocomplete::suggest(
        &text,
        pos,
        &app.store,
        &app.tag_aliases,
        &app.calendars,
        &app.visible_calendar_hrefs(),
    )
    .map(|(range, suggs)| (range, suggs.into_iter().map(|s| s.replacement).collect()))
}

#[test]
fn apply_suggestion_leaves_caret_after_replacement() {
    let mut app = app_with_journal(&["Ventilation for portable tools"]);
    set_journal_text(
        &mut app,
        "Plan the wiring before the walls go up.\n1. [ ] [[Ventil",
    );

    let (range, suggs) = banner_at(&app, 1, 42).expect("the partial link offers the task");
    assert_eq!(
        suggs,
        vec!["[[Ventilation for portable tools]]".to_string()]
    );

    let _ = update::update(
        &mut app,
        Message::ApplySuggestion(range, "[[Ventilation for portable tools]]".to_string()),
    );

    assert_eq!(
        app.journal_editor_content.text(),
        "Plan the wiring before the walls go up.\n1. [ ] [[Ventilation for portable tools]] ",
        "the token is replaced and a trailing space is appended at the buffer end"
    );
    assert_eq!(
        caret(&app),
        (1, 42),
        "the caret stays right after the replacement, not at the document end"
    );
    assert!(
        banner_at(&app, 1, 42).is_none(),
        "the banner must close once the link is complete"
    );
}

#[test]
fn apply_suggestion_mid_document_keeps_caret_in_place() {
    let mut app = app_with_journal(&["Ventilation for portable tools"]);
    // The link sits on the first line of a two-line document: applying the
    // suggestion must not drag the caret down to the end of the document.
    set_journal_text(&mut app, "1. [ ] [[Ventil\nsecond line of notes");
    set_caret(&mut app, 0, 15);

    let (range, _suggs) = banner_at(&app, 0, 15).expect("the partial link offers the task");
    let _ = update::update(
        &mut app,
        Message::ApplySuggestion(range, "[[Ventilation for portable tools]]".to_string()),
    );

    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for portable tools]]\nsecond line of notes",
        "a token mid-document is replaced in place, with no trailing space"
    );
    assert_eq!(
        caret(&app),
        (0, 41),
        "the caret stays right after the replacement on the same line"
    );
    assert!(
        banner_at(&app, 0, 41).is_none(),
        "the banner must close once the link is complete"
    );
}

#[test]
fn clicking_a_suggestion_closes_the_cycle() {
    let mut app = app_with_journal(&[
        "Ventilation for portable tools",
        "Ventilation for the workshop",
    ]);
    set_journal_text(&mut app, "1. [ ] [[Ventil");
    set_caret(&mut app, 0, 15);

    let (_range, suggs) = banner_at(&app, 0, 15).expect("the partial link offers both tasks");
    assert_eq!(
        suggs,
        vec![
            "[[Ventilation for portable tools]]".to_string(),
            "[[Ventilation for the workshop]]".to_string(),
        ]
    );

    // Tab picks the first entry; the banner stays open on the cycled list.
    let _ = update::update(&mut app, Message::TabPressed(true));
    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for portable tools]] "
    );
    let Some(cycle) = app.suggestion_cycle.as_ref() else {
        panic!("Tab must start a suggestion cycle");
    };
    assert_eq!(cycle.editor, SuggestionEditor::Journal);

    // The user clicks the second chip: the chip carries the cycle's range
    // (the range the banner rendered), the click applies it, and the cycle
    // is dropped so the banner re-runs the engine at the new caret and
    // closes.
    let chip_range = cycle.range.clone();
    let _ = update::update(&mut app, Message::ApplySuggestionAt(chip_range, 1));

    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for the workshop]] ",
        "the clicked entry replaces the cycled one"
    );
    assert!(
        app.suggestion_cycle.is_none(),
        "a click is definitive: the cycle must be dropped so the banner closes"
    );
    assert_eq!(
        caret(&app),
        (0, 39),
        "the caret stays right after the replacement"
    );
    assert!(
        banner_at(&app, 0, 39).is_none(),
        "the banner must be closed after the click"
    );
}

#[test]
fn tab_still_cycles_after_a_click() {
    // A click drops the cycle, but a later Tab press re-runs the engine:
    // with a fresh partial link the cycle starts again from the top.
    let mut app = app_with_journal(&[
        "Ventilation for portable tools",
        "Ventilation for the workshop",
    ]);
    set_journal_text(&mut app, "1. [ ] [[Ventil");
    set_caret(&mut app, 0, 15);

    let (_range, _suggs) = banner_at(&app, 0, 15).expect("the partial link offers both tasks");
    let _ = update::update(&mut app, Message::TabPressed(true));
    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for portable tools]] "
    );

    // Click the second chip (the cycle's range, as the banner renders it).
    let chip_range = app
        .suggestion_cycle
        .as_ref()
        .expect("a cycle is running")
        .range
        .clone();
    let _ = update::update(&mut app, Message::ApplySuggestionAt(chip_range, 1));
    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for the workshop]] "
    );
    assert!(app.suggestion_cycle.is_none());

    // A new partial link starts a fresh cycle from the top.
    set_journal_text(&mut app, "1. [ ] [[Ventil");
    set_caret(&mut app, 0, 15);
    let _ = update::update(&mut app, Message::TabPressed(true));
    assert_eq!(
        app.journal_editor_content.text(),
        "1. [ ] [[Ventilation for portable tools]] ",
        "a new Tab cycle starts from the first suggestion"
    );
}

fn summaries(store: &cfait::store::TaskStore) -> Vec<String> {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.calendar_href != "local://trash")
        .map(|t| t.summary.clone())
        .collect()
}

#[test]
fn submitting_a_pure_wiki_link_to_an_existing_task_creates_a_link_title() {
    // Quick-adding a line that is exactly a [[link]] to an existing task
    // stores the link as a literal, clickable title instead of minting a
    // plain duplicate of the target name.
    let mut app = app_with_journal(&["Ventilation for portable tools"]);
    app.input_value =
        iced::widget::text_editor::Content::with_text("[[Ventilation for portable tools]]");

    let _ = update::update(&mut app, Message::SubmitTask);

    let found = summaries(&app.store);
    assert!(
        found.contains(&"[[Ventilation for portable tools]]".to_string()),
        "the quick-add must create a literal link title: {found:?}"
    );
    assert_eq!(
        found
            .iter()
            .filter(|s| *s == "Ventilation for portable tools")
            .count(),
        1,
        "the original task is untouched and no plain duplicate is minted: {found:?}"
    );
}

#[test]
fn submitting_a_pure_wiki_link_to_an_unknown_task_creates_the_target() {
    // A link naming no existing task keeps the documented jump-to-or-create
    // behavior: it creates a plain task named after the target.
    let mut app = app_with_journal(&["Ventilation for portable tools"]);
    app.input_value =
        iced::widget::text_editor::Content::with_text("[[Fresh start on the compost heap]]");

    let _ = update::update(&mut app, Message::SubmitTask);

    let found = summaries(&app.store);
    assert!(
        found.contains(&"Fresh start on the compost heap".to_string()),
        "an unknown link target must create a plain task: {found:?}"
    );
    assert!(
        !found.contains(&"[[Fresh start on the compost heap]]".to_string()),
        "an unknown link must not become a literal title: {found:?}"
    );
}

#[test]
fn editing_a_task_to_a_fresh_unescaped_link_title_creates_a_link_title() {
    // Re-saving an existing task whose new title is exactly a [[link]] to
    // another existing task stores the link as a literal, clickable title
    // instead of stripping it to a plain duplicate of the target name.
    let mut app = app_with_journal(&[
        "Ventilation for portable tools",
        "Fresh start on the compost heap",
    ]);
    app.editing_uid = Some("task-0".to_string());
    app.input_value =
        iced::widget::text_editor::Content::with_text("[[Fresh start on the compost heap]]");

    let _ = update::update(&mut app, Message::SubmitTask);

    let found = summaries(&app.store);
    assert!(
        found.contains(&"[[Fresh start on the compost heap]]".to_string()),
        "the edit must store a literal link title: {found:?}"
    );
    assert_eq!(
        found
            .iter()
            .filter(|s| *s == "Fresh start on the compost heap")
            .count(),
        1,
        "the target task is untouched and no plain duplicate is minted: {found:?}"
    );
}
