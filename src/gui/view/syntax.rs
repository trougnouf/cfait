// Implements syntax highlighting for the smart input editor.
use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::{Color, Font};
use std::ops::Range;

// 1. Add state field
pub struct SmartInputHighlighter {
    is_dark: bool,
}

impl Default for SmartInputHighlighter {
    fn default() -> Self {
        Self { is_dark: true } // Default to dark if unknown
    }
}



impl Highlighter for SmartInputHighlighter {
    // 2. Change Settings from () to bool (is_dark)
    type Settings = bool;
    type Highlight = highlighter::Format<Font>;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    // 3. Initialize with the setting
    fn new(settings: &Self::Settings) -> Self {
        Self { is_dark: *settings }
    }

    // 4. Update state when view rebuilds
    fn update(&mut self, settings: &Self::Settings) {
        self.is_dark = *settings;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let tokens = tokenize_smart_input(line);

        let spans: Vec<(Range<usize>, Self::Highlight)> = tokens
            .into_iter()
            .map(|t| {
                let format = match t.kind {
                    SyntaxType::Priority => {
                        let text = &line[t.start..t.end];
                        let p = text.trim_start_matches('!').parse::<u8>().unwrap_or(0);

                        // 5. Pass self.is_dark to the color utility
                        let (r, g, b) = color_utils::get_priority_rgb(p, self.is_dark);

                        highlighter::Format {
                            color: Some(Color::from_rgb(r, g, b)),
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        }
                    }
                    SyntaxType::DueDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.6, 1.0)),
                        font: None,
                    },
                    SyntaxType::StartDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.4, 0.8, 0.4)),
                        font: None,
                    },
                    SyntaxType::Recurrence => highlighter::Format {
                        color: Some(Color::from_rgb(0.8, 0.4, 0.8)),
                        font: None,
                    },
                    SyntaxType::Duration => highlighter::Format {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
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
                        color: None,
                        font: None,
                    },
                    SyntaxType::Location => highlighter::Format {
                        color: Some(Color::from_rgb(0.8, 0.5, 0.0)),
                        font: None,
                    },
                    SyntaxType::Url => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.2, 0.8)),
                        font: None,
                    },
                    SyntaxType::Geo => highlighter::Format {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                        font: None,
                    },
                    SyntaxType::Description => highlighter::Format {
                        color: Some(Color::from_rgb(0.6, 0.0, 0.6)),
                        font: None,
                    },
                    SyntaxType::Reminder => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.4, 0.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                };
                (t.start..t.end, format)
            })
            .collect();

        spans.into_iter()
    }

    fn change_line(&mut self, _line: usize) {}
    fn current_line(&self) -> usize {
        0
    }
}
