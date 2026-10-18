// SPDX-License-Identifier: GPL-3.0-or-later
//! The caret must be drawn at the position the styled layout implies, not at
//! the pre-highlight position, after pasting text that gets stylized (e.g. a
//! bold tag). See https://github.com/trougnouf/cfait/issues/24
//!
//! This drives iced's `Editor` with the same method calls the runtime makes,
//! in the same order, so it guards the patched `iced_graphics` (see the
//! `[patch.crates-io]` section in `Cargo.toml`): its `Editor::highlight()`
//! re-shapes lines with new (bold) attributes and must clear the cached
//! selection, otherwise the caret is drawn at the plain-layout x-offset
//! until the next keystroke.
//!
//! The real per-frame order (iced_winit + iced_widget `text_editor.rs`):
//!   - the app's update handler runs `perform(action)` for the paste,
//!   - the UI rebuild runs the layout phase, i.e. `editor.update()`, which
//!     re-shapes the line with the line's CURRENT (pre-highlight) attrs and
//!     clears the selection cache,
//!   - the widget's `update()` on RedrawRequested calls `input_method()`,
//!     which calls `editor.selection()` BEFORE any highlight,
//!   - the widget's `draw()` calls `highlight()` (re-shapes with the new
//!     bold attrs) and then `editor.selection()` to draw the caret.
#![cfg(feature = "gui")]

use iced::advanced::graphics::text::editor::Editor as GraphicsEditor;
use iced::advanced::graphics::text::font_system;
use iced::advanced::text::editor::Editor as _;
use iced::advanced::text::editor::{self, Action, Cursor, Edit, Selection};
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::advanced::text::{LineHeight, Wrapping};
use iced::{Font, Point, Size};
use std::borrow::Cow;
use std::ops::Range;

const FONT_SIZE: f32 = 16.0;

/// A (regular, bold) font pair with genuinely different metrics. The first
/// pair present on disk is used; if none is found the test is skipped.
fn find_font_pair() -> Option<(String, String)> {
    const CANDIDATES: &[(&str, &str)] = &[
        // Linux
        (
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ),
        (
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
        ),
        // Windows
        (
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\arialbd.ttf",
        ),
        // macOS
        (
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        ),
    ];
    CANDIDATES
        .iter()
        .find(|(regular, bold)| {
            std::path::Path::new(regular).is_file() && std::path::Path::new(bold).is_file()
        })
        .map(|(regular, bold)| (regular.to_string(), bold.to_string()))
}

fn load_fonts(regular: &str, bold: &str) {
    let mut fs = font_system().write().expect("font system lock");
    for path in [regular, bold] {
        fs.load_font(Cow::Owned(std::fs::read(path).expect("missing font")));
    }
}

fn bold_font() -> Font {
    Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    }
}

/// A highlighter that makes one explicit range bold and the rest plain.
struct BoldRange {
    range: Range<usize>,
}

impl Highlighter for BoldRange {
    type Settings = ();
    type Highlight = bool;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(_settings: &()) -> Self {
        Self { range: 0..0 }
    }
    fn update(&mut self, _settings: &()) {}
    fn change_line(&mut self, _line: usize) {}
    fn current_line(&self) -> usize {
        0
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut spans = Vec::new();
        let s = self.range.start.min(line.len());
        let e = self.range.end.min(line.len());
        if s > 0 {
            spans.push((0..s, false));
        }
        if s < e {
            spans.push((s..e, true));
        }
        if e < line.len() {
            spans.push((e..line.len(), false));
        }
        if spans.is_empty() {
            spans.push((0..line.len(), false));
        }
        spans.into_iter()
    }
}

fn format_bold(bold: &bool) -> highlighter::Format<Font> {
    highlighter::Format {
        color: None,
        font: if *bold { Some(bold_font()) } else { None },
    }
}

/// The caret x that the buffer's CURRENT layout actually implies, computed
/// independently of the (possibly stale) selection cache.
fn true_caret_x(editor: &GraphicsEditor) -> f32 {
    let cursor = editor.cursor();
    let buffer = editor.buffer();
    let line = &buffer.lines[cursor.position.line];
    let layout = line.layout_opt().expect("line layout should be cached");
    let index = cursor.position.column;
    // Sum glyph widths whose start is before the cursor index, on the visual
    // line that contains the cursor.
    for vl in layout.iter() {
        let start = vl.glyphs.first().map(|g| g.start).unwrap_or(0);
        let end = vl.glyphs.last().map(|g| g.end).unwrap_or(0);
        if start <= index && index <= end {
            return vl
                .glyphs
                .iter()
                .take_while(|g| index > g.start)
                .map(|g| g.w)
                .sum();
        }
    }
    layout.last().map(|l| l.w).unwrap_or(0.0)
}

fn caret_x(sel: &Selection) -> f32 {
    match sel {
        Selection::Caret(Point { x, .. }) => *x,
        _ => f32::NAN,
    }
}

/// The layout phase, i.e. what `UserInterface::build` runs after the app's
/// update handler processed the message.
fn layout_phase(editor: &mut GraphicsEditor, highlighter: &mut BoldRange) {
    editor.update(
        Size::new(400.0, 45.0),
        Font::DEFAULT,
        iced::Pixels(FONT_SIZE),
        LineHeight::default(),
        Wrapping::None,
        highlighter,
    );
}

/// A plain redraw frame (e.g. cursor blink): no layout phase, just
///   update (RedrawRequested) -> input_method -> editor.selection()
///   draw                     -> highlight, then editor.selection()
/// Returns (caret seen by input_method, caret actually drawn).
fn redraw_frame(editor: &mut GraphicsEditor, highlighter: &mut BoldRange) -> (f32, f32) {
    let im = caret_x(&editor.selection());
    editor.highlight(Font::DEFAULT, highlighter, format_bold);
    let drawn = caret_x(&editor.selection());
    (im, drawn)
}

#[test]
fn caret_matches_layout_after_stylized_paste() {
    let Some((regular, bold)) = find_font_pair() else {
        eprintln!("skipping: no regular+bold font pair found on this system");
        return;
    };
    load_fonts(&regular, &bold);

    // The bold range will cover the pasted word "roses" (10..15) once pasted.
    let mut highlighter = BoldRange { range: 10..15 };

    // Start with "water the " (plain), cursor at the end.
    let mut editor = GraphicsEditor::with_text("water the ");
    layout_phase(&mut editor, &mut highlighter);
    editor.move_to(Cursor {
        position: editor::Position {
            line: 0,
            column: 10,
        },
        selection: None,
    });
    // Steady frame before the paste: the line is highlighted plain.
    let (im0, drawn0) = redraw_frame(&mut editor, &mut highlighter);
    let true0 = true_caret_x(&editor);
    eprintln!(
        "Frame 0 (steady): input_method caret x = {im0:.3}, drawn caret x = {drawn0:.3}, true layout caret x = {true0:.3}"
    );
    assert!(
        (drawn0 - true0).abs() < 0.001,
        "steady frame must draw the correct caret"
    );

    // --- Frame A: the paste ---
    // perform() runs in the app's update handler, then the UI rebuild runs
    // the layout phase, then the RedrawRequested pass calls selection()
    // before draw's highlight().
    editor.perform(Action::Edit(Edit::Paste(std::sync::Arc::new(
        "roses".to_string(),
    ))));
    layout_phase(&mut editor, &mut highlighter);
    let (im_a, drawn_a) = redraw_frame(&mut editor, &mut highlighter);
    let true_a = true_caret_x(&editor);
    eprintln!(
        "Frame A (paste): input_method caret x = {im_a:.3}, drawn caret x = {drawn_a:.3}, true layout caret x = {true_a:.3}, diff = {:.3}",
        drawn_a - true_a
    );

    // --- Frame B: a plain redraw (e.g. cursor blink) ---
    let (im_b, drawn_b) = redraw_frame(&mut editor, &mut highlighter);
    let true_b = true_caret_x(&editor);
    eprintln!(
        "Frame B (redraw): input_method caret x = {im_b:.3}, drawn caret x = {drawn_b:.3}, true layout caret x = {true_b:.3}, diff = {:.3}",
        drawn_b - true_b
    );

    // --- Frame C: typing a character clears the cache via perform() ---
    editor.perform(Action::Edit(Edit::Insert('s')));
    layout_phase(&mut editor, &mut highlighter);
    let (im_c, drawn_c) = redraw_frame(&mut editor, &mut highlighter);
    let true_c = true_caret_x(&editor);
    eprintln!(
        "Frame C (typing): input_method caret x = {im_c:.3}, drawn caret x = {drawn_c:.3}, true layout caret x = {true_c:.3}, diff = {:.3}",
        drawn_c - true_c
    );

    // The bug: after the paste, the drawn caret must match the layout.
    assert!(
        (drawn_a - true_a).abs() < 0.001,
        "frame A draws a stale caret: drawn {drawn_a:.3} but layout says {true_a:.3}"
    );
    assert!(
        (drawn_b - true_b).abs() < 0.001,
        "frame B still draws a stale caret: drawn {drawn_b:.3} but layout says {true_b:.3}"
    );
    assert!(
        (drawn_c - true_c).abs() < 0.001,
        "frame C draws a stale caret: drawn {drawn_c:.3} but layout says {true_c:.3}"
    );
}
