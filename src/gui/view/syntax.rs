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
                    SyntaxType::Dependency => highlighter::Format {
                        color: Some(Color::from_rgb(0.9, 0.6, 0.2)), // Orange
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    SyntaxType::Relation => highlighter::Format {
                        color: Some(Color::from_rgb(0.4, 0.6, 0.9)), // Soft Blue
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
                    SyntaxType::Collection => highlighter::Format {
                        color: Some(Color::from_rgb(0.9, 0.4, 0.4)), // Soft red
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
        let dim_color = Some(Color::from_rgba(0.5, 0.5, 0.5, 0.3)); // Very transparent gray
        let checkbox_color = Some(Color::from_rgb(0.4, 0.8, 0.4)); // Greenish

        let trimmed = line.trim_start();
        let is_header = trimmed.starts_with('#');
        let _is_list =
            trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ [");
        let is_table = trimmed.starts_with('|') && trimmed[1..].contains('|');
        let table_color = Some(Color::from_rgb(0.3, 0.7, 0.5)); // Greenish

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
        } else if is_table {
            highlighter::Format {
                color: table_color,
                font: Some(Font::MONOSPACE),
            }
        } else {
            highlighter::Format {
                color: None,
                font: None,
            }
        };

        let mut after_marker = 0;

        // Find end of markdown marker
        if is_header {
            if let Some(idx) = line.find("# ") {
                after_marker = idx + 2;
            } else if let Some(idx) = line.find("## ") {
                after_marker = idx + 3;
            } else if let Some(idx) = line.find("### ") {
                after_marker = idx + 4;
            }
        } else {
            if let Some(idx) = line.find("- ") {
                after_marker = idx + 2;
            } else if let Some(idx) = line.find("* ") {
                after_marker = idx + 2;
            } else if let Some(idx) = line.find("+ ") {
                after_marker = idx + 2;
            } else if let Some(idx) = line.find(". ") {
                after_marker = idx + 2;
            }
        }

        // Check for checkbox right after marker
        let mut checkbox_end = 0;
        if after_marker > 0 && line.len() >= after_marker + 4 {
            let slice = &line[after_marker..after_marker + 4];
            if slice.starts_with('[') && slice.ends_with("] ") {
                checkbox_end = after_marker + 4;
            }
        }

        // Push prefix and checkbox
        if checkbox_end > 0 {
            if after_marker > 0 {
                spans.push((0..after_marker, base_format));
            }
            spans.push((
                after_marker..checkbox_end,
                highlighter::Format {
                    color: checkbox_color,
                    font: None,
                },
            ));

            let rest_of_line = &line[checkbox_end..];

            if rest_of_line.is_empty() {
                return spans.into_iter();
            }

            let mut byte_formats = vec![base_format; rest_of_line.len()];

            // 1. Apply Markdown to rest_of_line
            let mut cursor = 0;
            while cursor < rest_of_line.len() {
                let remaining = &rest_of_line[cursor..];

                let markers = [
                    (
                        "<!-- uid:",
                        "-->",
                        9,
                        3,
                        highlighter::Format {
                            color: dim_color,
                            font: Some(Font {
                                style: iced::font::Style::Italic,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "[[",
                        "]]",
                        2,
                        2,
                        highlighter::Format {
                            color: link_color,
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "**",
                        "**",
                        2,
                        2,
                        highlighter::Format {
                            color: None,
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "__",
                        "__",
                        2,
                        2,
                        highlighter::Format {
                            color: None,
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "~~",
                        "~~",
                        2,
                        2,
                        highlighter::Format {
                            color: None,
                            font: None,
                        },
                    ),
                    (
                        "*",
                        "*",
                        1,
                        1,
                        highlighter::Format {
                            color: None,
                            font: Some(Font {
                                style: iced::font::Style::Italic,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "_",
                        "_",
                        1,
                        1,
                        highlighter::Format {
                            color: None,
                            font: Some(Font {
                                style: iced::font::Style::Italic,
                                ..Default::default()
                            }),
                        },
                    ),
                    (
                        "`",
                        "`",
                        1,
                        1,
                        highlighter::Format {
                            color: Some(Color::from_rgb(0.8, 0.6, 0.4)),
                            font: Some(Font::MONOSPACE),
                        },
                    ),
                ];

                let mut best_match: Option<(usize, usize, highlighter::Format<Font>)> = None;

                // Process markers first
                {
                    let mut update_best = |start, end, format| {
                        if best_match.is_none() || start < best_match.unwrap().0 {
                            best_match = Some((start, end, format));
                        }
                    };

                    for &(start_marker, end_marker, start_len, end_len, format) in &markers {
                        if let Some(start_pos) = remaining.find(start_marker)
                            && let Some(end_pos) =
                                remaining[start_pos + start_len..].find(end_marker)
                        {
                            let abs_start = cursor + start_pos;
                            let abs_end = abs_start + start_len + end_pos + end_len;
                            update_best(abs_start, abs_end, format);
                        }
                    }
                }

                let best_match_pos = best_match.as_ref().map(|(pos, _, _)| *pos);

                // Standard Markdown links: [label](url)
                let mut search_idx = 0;
                while let Some(start_pos) = remaining[search_idx..].find('[') {
                    let abs_start = cursor + search_idx + start_pos;

                    // Early termination: if we already have a match that starts before this position, skip
                    if let Some(best_pos) = best_match_pos
                        && best_pos <= abs_start
                    {
                        break;
                    }

                    if remaining[search_idx + start_pos..].starts_with("[[") {
                        search_idx += start_pos + 2;
                        continue;
                    }
                    if let Some(mid_pos) = remaining[search_idx + start_pos..].find("](") {
                        let mid_abs = search_idx + start_pos + mid_pos;
                        let link_text = &remaining[search_idx + start_pos + 1..mid_abs];
                        if !link_text.contains('[')
                            && let Some(end_pos) = remaining[mid_abs..].find(')')
                        {
                            let abs_end = cursor + mid_abs + end_pos + 1;
                            best_match = Some((
                                abs_start,
                                abs_end,
                                highlighter::Format {
                                    color: link_color,
                                    font: Some(Font {
                                        weight: iced::font::Weight::Bold,
                                        ..Default::default()
                                    }),
                                },
                            ));
                            break;
                        }
                    }
                    search_idx += start_pos + 1;
                }

                // Bare URLs (http:// or https://)
                for scheme in &["https://", "http://"] {
                    if let Some(start_pos) = remaining.find(scheme) {
                        let abs_start = cursor + start_pos;

                        // Skip if we already have a better match
                        if let Some(best_pos) = best_match_pos
                            && best_pos <= abs_start
                        {
                            continue;
                        }

                        let mut end_offset = 0;
                        for c in remaining[start_pos..].chars() {
                            if c.is_whitespace() || c == ')' || c == ']' {
                                break;
                            }
                            end_offset += c.len_utf8();
                        }
                        let abs_end = abs_start + end_offset;
                        // Update best_match directly since we dropped the closure
                        if best_match.is_none() || abs_start < best_match.as_ref().unwrap().0 {
                            best_match = Some((
                                abs_start,
                                abs_end,
                                highlighter::Format {
                                    color: link_color,
                                    font: Some(Font {
                                        weight: iced::font::Weight::Bold,
                                        ..Default::default()
                                    }),
                                },
                            ));
                        }
                    }
                }

                if let Some((abs_start, abs_end, format)) = best_match {
                    for byte_format in byte_formats.iter_mut().take(abs_end).skip(abs_start) {
                        if format.color.is_some() {
                            byte_format.color = format.color;
                        }
                        if format.font.is_some() {
                            byte_format.font = format.font;
                        }
                    }
                    cursor = abs_end;
                } else {
                    break;
                }
            }

            // 2. Apply Smart Syntax to rest_of_line
            let tokens = crate::model::parser::tokenize_smart_input(rest_of_line, false);
            let is_dark_theme = self.is_dark;

            for t in tokens {
                if t.kind == crate::model::parser::SyntaxType::Text {
                    continue;
                }
                let text = &rest_of_line[t.start..t.end];
                let format = match t.kind {
                    crate::model::parser::SyntaxType::Priority => {
                        let p = text.trim_start_matches('!').parse::<u8>().unwrap_or(0);
                        let (r, g, b) = crate::color_utils::get_priority_rgb(p, is_dark_theme);
                        highlighter::Format {
                            color: Some(Color::from_rgb(r, g, b)),
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        }
                    }
                    crate::model::parser::SyntaxType::DueDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.6, 1.0)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::StartDate => highlighter::Format {
                        color: Some(Color::from_rgb(0.4, 0.8, 0.4)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Recurrence => highlighter::Format {
                        color: Some(Color::from_rgb(0.8, 0.4, 0.8)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Duration => highlighter::Format {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Tag => {
                        let tag_name = text.trim_start_matches('#');
                        let (r, g, b) = crate::color_utils::generate_color(tag_name);
                        highlighter::Format {
                            color: Some(Color::from_rgb(r, g, b)),
                            font: Some(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        }
                    }
                    crate::model::parser::SyntaxType::Location => highlighter::Format {
                        color: Some(Color::from_rgb(0.8, 0.5, 0.0)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Url => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.2, 0.8)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::WikiLink => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.7, 1.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Dependency => highlighter::Format {
                        color: Some(Color::from_rgb(0.9, 0.6, 0.2)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Relation => highlighter::Format {
                        color: Some(Color::from_rgb(0.4, 0.6, 0.9)), // Soft Blue
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Geo => highlighter::Format {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Description => highlighter::Format {
                        color: Some(Color::from_rgb(0.6, 0.0, 0.6)),
                        font: None,
                    },
                    crate::model::parser::SyntaxType::Reminder => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.4, 0.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Operator => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.0, 1.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Goal => highlighter::Format {
                        color: Some(Color::from_rgb(0.2, 0.8, 0.6)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Calendar => highlighter::Format {
                        color: Some(Color::from_rgb(0.91, 0.11, 0.38)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    crate::model::parser::SyntaxType::Pin => highlighter::Format {
                        color: Some(Color::from_rgb(1.0, 0.4, 0.0)),
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                    _ => base_format,
                };

                for byte_format in byte_formats.iter_mut().take(t.end).skip(t.start) {
                    if format.color.is_some() {
                        byte_format.color = format.color;
                    }
                    if format.font.is_some() {
                        byte_format.font = format.font;
                    }
                }
            }

            // 3. Coalesce
            let mut current_format = byte_formats[0];
            let mut current_start = 0;
            for (i, byte_format) in byte_formats
                .iter()
                .enumerate()
                .skip(1)
                .take(rest_of_line.len() - 1)
            {
                if *byte_format != current_format && rest_of_line.is_char_boundary(i) {
                    spans.push((
                        checkbox_end + current_start..checkbox_end + i,
                        current_format,
                    ));
                    current_format = *byte_format;
                    current_start = i;
                }
            }
            spans.push((
                checkbox_end + current_start..checkbox_end + rest_of_line.len(),
                current_format,
            ));

            return spans.into_iter();
        }

        // Scan for inline elements (Links, UIDs, and Formatting)
        while cursor < line.len() {
            let remaining = &line[cursor..];

            let markers = [
                (
                    "<!-- uid:",
                    "-->",
                    9,
                    3,
                    highlighter::Format {
                        color: dim_color,
                        font: Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "[[",
                    "]]",
                    2,
                    2,
                    highlighter::Format {
                        color: link_color,
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "**",
                    "**",
                    2,
                    2,
                    highlighter::Format {
                        color: None, // Inherit base color
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "__",
                    "__",
                    2,
                    2,
                    highlighter::Format {
                        color: None,
                        font: Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "~~",
                    "~~",
                    2,
                    2,
                    highlighter::Format {
                        color: None,
                        font: None,
                    },
                ),
                (
                    "*",
                    "*",
                    1,
                    1,
                    highlighter::Format {
                        color: None,
                        font: Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "_",
                    "_",
                    1,
                    1,
                    highlighter::Format {
                        color: None,
                        font: Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "`",
                    "`",
                    1,
                    1,
                    highlighter::Format {
                        color: Some(Color::from_rgb(0.8, 0.6, 0.4)),
                        font: Some(Font::MONOSPACE),
                    },
                ),
            ];

            let mut best_match: Option<(usize, usize, highlighter::Format<Font>)> = None;

            // Process markers first
            {
                let mut update_best = |start, end, format| {
                    if best_match.is_none() || start < best_match.unwrap().0 {
                        best_match = Some((start, end, format));
                    }
                };

                for &(start_marker, end_marker, start_len, end_len, format) in &markers {
                    if let Some(start_pos) = remaining.find(start_marker)
                        && let Some(end_pos) = remaining[start_pos + start_len..].find(end_marker)
                    {
                        let abs_start = cursor + start_pos;
                        let abs_end = abs_start + start_len + end_pos + end_len;
                        update_best(abs_start, abs_end, format);
                    }
                }
            }

            let best_match_pos = best_match.as_ref().map(|(pos, _, _)| *pos);

            // Standard Markdown links: [label](url)
            let mut search_idx = 0;
            while let Some(start_pos) = remaining[search_idx..].find('[') {
                let abs_start = cursor + search_idx + start_pos;

                // Early termination: if we already have a match that starts before this position, skip
                if let Some(best_pos) = best_match_pos
                    && best_pos <= abs_start
                {
                    break;
                }

                if remaining[search_idx + start_pos..].starts_with("[[") {
                    search_idx += start_pos + 2;
                    continue;
                }
                if let Some(mid_pos) = remaining[search_idx + start_pos..].find("](") {
                    let mid_abs = search_idx + start_pos + mid_pos;
                    let link_text = &remaining[search_idx + start_pos + 1..mid_abs];
                    if !link_text.contains('[')
                        && let Some(end_pos) = remaining[mid_abs..].find(')')
                    {
                        let abs_end = cursor + mid_abs + end_pos + 1;
                        best_match = Some((
                            abs_start,
                            abs_end,
                            highlighter::Format {
                                color: link_color,
                                font: Some(Font {
                                    weight: iced::font::Weight::Bold,
                                    ..Default::default()
                                }),
                            },
                        ));
                        break;
                    }
                }
                search_idx += start_pos + 1;
            }

            // Bare URLs (http:// or https://)
            for scheme in &["https://", "http://"] {
                if let Some(start_pos) = remaining.find(scheme) {
                    let abs_start = cursor + start_pos;

                    // Skip if we already have a better match
                    if let Some(best_pos) = best_match_pos
                        && best_pos <= abs_start
                    {
                        continue;
                    }

                    let mut end_offset = 0;
                    for c in line[abs_start..].chars() {
                        if c.is_whitespace() || c == ')' || c == ']' {
                            break;
                        }
                        end_offset += c.len_utf8();
                    }
                    let abs_end = abs_start + end_offset;
                    // Update best_match directly since we dropped the closure
                    if best_match.is_none() || abs_start < best_match.as_ref().unwrap().0 {
                        best_match = Some((
                            abs_start,
                            abs_end,
                            highlighter::Format {
                                color: link_color,
                                font: Some(Font {
                                    weight: iced::font::Weight::Bold,
                                    ..Default::default()
                                }),
                            },
                        ));
                    }
                }
            }

            if let Some((abs_start, abs_end, mut format)) = best_match {
                if abs_start > cursor {
                    spans.push((cursor..abs_start, base_format));
                }

                if format.color.is_none() {
                    format.color = base_format.color;
                }
                if format.font.is_none() {
                    format.font = base_format.font;
                }

                spans.push((abs_start..abs_end, format));
                cursor = abs_end;
            } else {
                spans.push((cursor..line.len(), base_format));
                break;
            }
        }

        spans.into_iter()
    }
}
