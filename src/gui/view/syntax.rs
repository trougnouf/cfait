// File: src/gui/view/syntax.rs
use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::{Color, Font};
use std::ops::Range;

#[derive(Default)]
pub struct SmartInputHighlighter;

impl Highlighter for SmartInputHighlighter {
    type Settings = ();
    type Highlight = highlighter::Format<Font>;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self::default()
    }

    fn update(&mut self, _settings: &Self::Settings) {
        // No-op as we are stateless
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let tokens = tokenize_smart_input(line);

        let spans: Vec<(Range<usize>, Self::Highlight)> = tokens
            .into_iter()
            .map(|t| {
                let format = match t.kind {
                    SyntaxType::Priority => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.2, 0.2)), // Red
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    SyntaxType::DueDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.6, 1.0)), // Blue
                        font: None,
                    },
                    SyntaxType::StartDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.8, 0.8)), // Cyan
                        font: None,
                    },
                    SyntaxType::Recurrence => highlighter::Format {
                        color: Some(Color::from_rgb(0.4, 0.4, 1.0)), // Purple-ish
                        font: None,
                    },
                    SyntaxType::Duration => highlighter::Format {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)), // Grey
                        font: None,
                    },
                    SyntaxType::Tag => {
                        let text = &line[t.start..t.end];
                        let tag_name = text.trim_start_matches('#');
                        let (r, g, b) = color_utils::generate_color(tag_name);
                        highlighter::Format {
                            color: Some(Color::from_rgb(r, g, b)),
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        }
                    }
                    SyntaxType::Text => highlighter::Format {
                        color: None, // Use default text color from the theme
                        font: None,
                    },
                };
                (t.start..t.end, format)
            })
            .collect();

        spans.into_iter()
    }

    // These are for stateful highlighters; ours is stateless.
    fn change_line(&mut self, _line: usize) {}
    fn current_line(&self) -> usize {
        0
    }
}
