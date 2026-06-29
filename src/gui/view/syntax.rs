// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/view/syntax.rs
// Implements syntax highlighting for the smart input editor.
use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::{Color, Font};
use std::ops::Range;

// 1. Add state field
pub struct SmartInputHighlighter {
    is_dark: bool,
    is_search: bool,
}

impl Default for SmartInputHighlighter {
    fn default() -> Self {
        Self {
            is_dark: true,
            is_search: false,
        } // Default: dark=true, search=false
    }
}

impl Highlighter for SmartInputHighlighter {
    // Settings: (is_dark, is_search)
    type Settings = (bool, bool); // (is_dark, is_search)
    type Highlight = highlighter::Format<Font>;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            is_dark: settings.0,
            is_search: settings.1,
        }
    }

    fn update(&mut self, settings: &Self::Settings) {
        self.is_dark = settings.0;
        self.is_search = settings.1;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        // Pass context to tokenizer
        let tokens = tokenize_smart_input(line, self.is_search);

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
                    SyntaxType::WikiLink => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.7, 1.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
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
                    SyntaxType::Filter => highlighter::Format {
                        color: Some(Color::from_rgb(0.0, 0.8, 0.8)), // Cyan
                        font: None,
                    },
                    SyntaxType::Operator => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.0, 1.0)), // Magenta for boolean ops
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    SyntaxType::Goal => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.8, 0.6)), // Sea Green
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    SyntaxType::Calendar => highlighter::Format {
                        // Added handler
                        color: Some(Color::from_rgb(0.91, 0.11, 0.38)), // #E91E63 Pink
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    SyntaxType::Pin => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.4, 0.0)), // Orange for pin
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
pub struct SessionHighlighter {
    is_dark: bool,
}

impl Default for SessionHighlighter {
    fn default() -> Self {
        Self { is_dark: true }
    }
}

impl Highlighter for SessionHighlighter {
    type Settings = bool;
    type Highlight = highlighter::Format<Font>;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self { is_dark: *settings }
    }

    fn update(&mut self, settings: &Self::Settings) {
        self.is_dark = *settings;
    }

    fn change_line(&mut self, _line: usize) {}
    fn current_line(&self) -> usize {
        0
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut spans = Vec::new();
        let mut cursor = 0;

        let lex_guard = crate::model::parser::LEXICON.read().unwrap();
        let lex = &*lex_guard;

        for word in line.split_whitespace() {
            let start = line[cursor..].find(word).unwrap() + cursor;
            let end = start + word.len();

            if start > cursor {
                spans.push((
                    cursor..start,
                    highlighter::Format {
                        color: None,
                        font: None,
                    },
                ));
            }

            let lower = word.to_lowercase();
            let format = if crate::model::parser::parse_duration_with_lex(&lower, lex).is_some() {
                // Duration matches
                highlighter::Format {
                    color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                    font: None,
                }
            } else if crate::model::parser::parse_smart_date_with_lex(&lower, lex).is_some()
                || crate::model::parser::parse_weekday_code_with_lex(&lower, lex).is_some()
            {
                // Date matches
                highlighter::Format {
                    color: Some(Color::from_rgb(0.2, 0.6, 1.0)),
                    font: None,
                }
            } else if lower.contains(':') && (lower.contains('-') || lower.len() <= 5) {
                // Time or Time Range matches
                highlighter::Format {
                    color: Some(Color::from_rgb(0.4, 0.8, 0.4)),
                    font: None,
                }
            } else {
                // Default text
                highlighter::Format {
                    color: None,
                    font: None,
                }
            };

            spans.push((start..end, format));
            cursor = end;
        }

        if cursor < line.len() {
            spans.push((
                cursor..line.len(),
                highlighter::Format {
                    color: None,
                    font: None,
                },
            ));
        }

        spans.into_iter()
    }
}

pub struct MarkdownHighlighter {
    is_dark: bool,
}

impl Default for MarkdownHighlighter {
    fn default() -> Self {
        Self { is_dark: true }
    }
}

impl Highlighter for MarkdownHighlighter {
    type Settings = bool;
    type Highlight = highlighter::Format<Font>;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self { is_dark: *settings }
    }

    fn update(&mut self, settings: &Self::Settings) {
        self.is_dark = *settings;
    }

    fn change_line(&mut self, _line: usize) {}
    fn current_line(&self) -> usize {
        0
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut spans = Vec::new();

        let header_color = Some(Color::from_rgb(1.0, 0.6, 0.0)); // Orange
        let link_color = Some(Color::from_rgb(0.2, 0.7, 1.0)); // Cyan
        let dim_color = Some(Color::from_rgb(0.4, 0.4, 0.4)); // Dark Gray
        let checkbox_color = Some(Color::from_rgb(0.4, 0.8, 0.4)); // Greenish

        let trimmed = line.trim_start();
        let is_header = trimmed.starts_with('#');
        let is_list =
            trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ [");

        let mut cursor = 0;

        // Base format for the line
        let base_format = if is_header {
            highlighter::Format {
                color: header_color,
                font: Some(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            }
        } else {
            highlighter::Format {
                color: None,
                font: None,
            }
        };

        // Scan for inline elements (Links and UIDs)
        while cursor < line.len() {
            let remaining = &line[cursor..];

            if let Some(uid_start) = remaining.find("<!-- uid:")
                && let Some(uid_end_offset) = remaining[uid_start..].find("-->")
            {
                let abs_start = cursor + uid_start;
                let abs_end = abs_start + uid_end_offset + 3;

                if abs_start > cursor {
                    spans.push((cursor..abs_start, base_format));
                }
                spans.push((
                    abs_start..abs_end,
                    highlighter::Format {
                        color: dim_color,
                        font: Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        }),
                    },
                ));
                cursor = abs_end;
                continue;
            }

            if let Some(link_start) = remaining.find("[[")
                && let Some(link_end_offset) = remaining[link_start..].find("]]")
            {
                let abs_start = cursor + link_start;
                let abs_end = abs_start + link_end_offset + 2;

                if abs_start > cursor {
                    // Apply checkbox color if it's the start of a list item
                    if is_list && cursor == 0 && abs_start <= line.find('[').unwrap_or(0) + 4 {
                        spans.push((
                            cursor..abs_start,
                            highlighter::Format {
                                color: checkbox_color,
                                font: None,
                            },
                        ));
                    } else {
                        spans.push((cursor..abs_start, base_format));
                    }
                }
                spans.push((
                    abs_start..abs_end,
                    highlighter::Format {
                        color: link_color,
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                ));
                cursor = abs_end;
                continue;
            }

            // No more inline elements, push the rest
            if is_list && cursor == 0 {
                let box_end = line.find(']').unwrap_or(0) + 1;
                if box_end > 0 {
                    spans.push((
                        0..box_end,
                        highlighter::Format {
                            color: checkbox_color,
                            font: None,
                        },
                    ));
                    cursor = box_end;
                    continue;
                }
            }

            spans.push((cursor..line.len(), base_format));
            break;
        }

        spans.into_iter()
    }
}
