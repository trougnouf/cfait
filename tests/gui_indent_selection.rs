// SPDX-License-Identifier: GPL-3.0-or-later
//! Tab / Shift+Tab in the multi-line editors (journal page, task
//! description) must indent or outdent every line of the active selection,
//! not just the caret's line, and must preserve the selection instead of
//! eating the selected text.
#![cfg(feature = "gui")]

use iced::widget::text_editor::{Content, Cursor, Position};

fn select(content: &mut Content, anchor: (usize, usize), position: (usize, usize)) {
    content.move_to(Cursor {
        position: Position {
            line: position.0,
            column: position.1,
        },
        selection: Some(Position {
            line: anchor.0,
            column: anchor.1,
        }),
    });
}

/// (line, column, selection anchor) of the caret.
fn caret(content: &Content) -> (usize, usize, Option<(usize, usize)>) {
    let cursor = content.cursor();
    (
        cursor.position.line,
        cursor.position.column,
        cursor.selection.map(|p| (p.line, p.column)),
    )
}

#[test]
fn tab_indents_only_the_carets_line_without_a_selection() {
    let mut content = Content::with_text("water the tomatoes\nprune the roses");
    content.move_to(Cursor {
        position: Position { line: 0, column: 4 },
        selection: None,
    });

    cfait::gui::update::common::indent_line(&mut content);

    assert_eq!(content.text(), "  water the tomatoes\nprune the roses");
    assert_eq!(caret(&content), (0, 6, None));
}

#[test]
fn tab_indents_every_selected_line_and_keeps_the_selection() {
    let mut content = Content::with_text(
        "water the tomatoes\n  prune the roses\n    deadhead the peonies\nweeding",
    );
    // Selected upwards: caret at the top of the range, anchor at the bottom.
    select(&mut content, (2, 10), (0, 4));

    cfait::gui::update::common::indent_line(&mut content);

    assert_eq!(
        content.text(),
        "  water the tomatoes\n    prune the roses\n      deadhead the peonies\nweeding"
    );
    // Selection preserved, shifted by the two inserted spaces on each end.
    assert_eq!(caret(&content), (0, 6, Some((2, 12))));
}

#[test]
fn shift_tab_outdents_every_selected_line() {
    let mut content = Content::with_text(
        "  water the tomatoes\n    prune the roses\n deadhead the peonies\nweeding",
    );
    select(&mut content, (0, 3), (2, 9));

    cfait::gui::update::common::outdent_line(&mut content);

    // Two spaces off the first line, two off the second, one off the third
    // (it only had one to spare), the fourth line untouched.
    assert_eq!(
        content.text(),
        "water the tomatoes\n  prune the roses\ndeadhead the peonies\nweeding"
    );
    assert_eq!(caret(&content), (2, 8, Some((0, 1))));
}

#[test]
fn shift_tab_is_a_noop_when_no_line_has_leading_spaces() {
    let mut content = Content::with_text("water the tomatoes\nprune the roses");
    select(&mut content, (0, 2), (1, 5));

    cfait::gui::update::common::outdent_line(&mut content);

    assert_eq!(content.text(), "water the tomatoes\nprune the roses");
}

#[test]
fn tab_does_not_eat_the_selected_text() {
    let mut content = Content::with_text("plant the basil\n  trim the mint");
    select(&mut content, (0, 6), (1, 10));

    cfait::gui::update::common::indent_line(&mut content);

    assert_eq!(content.text(), "  plant the basil\n    trim the mint");
}
