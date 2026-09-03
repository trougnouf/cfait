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
                    SyntaxType::Note => highlighter::Format {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
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

        let is_dark = self.is_dark;
        let header_color = Some(if is_dark {
            Color::from_rgb(0.3, 0.7, 1.0)
        } else {
            Color::from_rgb(0.1, 0.4, 0.8)
        });
        let link_color = Some(if is_dark {
            Color::from_rgb(0.2, 0.7, 1.0)
        } else {
            Color::from_rgb(0.1, 0.5, 0.9)
        });
        let dim_color = Some(Color::from_rgba(0.5, 0.5, 0.5, 0.6));
        let checkbox_color = Some(Color::from_rgb(0.4, 0.8, 0.4));
        let list_marker_color = Some(if is_dark {
            Color::from_rgb(0.8, 0.6, 0.0)
        } else {
            Color::from_rgb(0.7, 0.5, 0.0)
        });
        let quote_color = Some(if is_dark {
            Color::from_rgb(0.5, 0.5, 0.5)
        } else {
            Color::from_rgb(0.4, 0.4, 0.4)
        });
        let table_color = Some(Color::from_rgb(0.3, 0.7, 0.5));
        let code_color = Some(Color::from_rgb(0.8, 0.6, 0.4));

        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        let mut marker_end = indent;
        let mut is_header = false;
        let mut is_quote = false;
        let mut is_table = false;
        let mut is_code_fence = false;

        if trimmed.starts_with("```") {
            is_code_fence = true;
            marker_end = line.len();
        } else if trimmed.starts_with("# ") {
            marker_end = indent + 2;
            is_header = true;
        } else if trimmed.starts_with("## ") {
            marker_end = indent + 3;
            is_header = true;
        } else if trimmed.starts_with("### ") {
            marker_end = indent + 4;
            is_header = true;
        } else if trimmed.starts_with("#### ") {
            marker_end = indent + 5;
            is_header = true;
        } else if trimmed.starts_with("##### ") {
            marker_end = indent + 6;
            is_header = true;
        } else if trimmed.starts_with("###### ") {
            marker_end = indent + 7;
            is_header = true;
        } else if trimmed.starts_with("> ") {
            marker_end = indent + 2;
            is_quote = true;
        } else if trimmed.starts_with('|') && trimmed[1..].contains('|') {
            is_table = true;
        } else if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
        {
            marker_end = indent + 2;
        } else {
            let mut digit_bytes = 0;
            for c in trimmed.chars() {
                if c.is_ascii_digit() {
                    digit_bytes += c.len_utf8();
                } else {
                    break;
                }
            }
            if digit_bytes > 0 && trimmed[digit_bytes..].starts_with(". ") {
                marker_end = indent + digit_bytes + 2;
            }
        }

        // Check for checkbox right after marker
        let mut checkbox_end = marker_end;
        if marker_end > indent && marker_end < line.len() {
            let remainder = &line[marker_end..];
            if remainder.starts_with("[ ] ")
                || remainder.starts_with("[x] ")
                || remainder.starts_with("[X] ")
                || remainder.starts_with("[/] ")
                || remainder.starts_with("[-] ")
                || remainder.starts_with("[<] ")
                || remainder.starts_with("[>] ")
                || remainder.starts_with("[*] ")
                || remainder.starts_with("[~] ")
            {
                checkbox_end = marker_end + 4;
            }
        }

        let base_format = if is_header {
            highlighter::Format {
                color: header_color,
                font: Some(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            }
        } else if is_quote {
            highlighter::Format {
                color: quote_color,
                font: Some(Font {
                    style: iced::font::Style::Italic,
                    ..Default::default()
                }),
            }
        } else if is_table {
            highlighter::Format {
                color: table_color,
                font: Some(Font::MONOSPACE),
            }
        } else if is_code_fence {
            highlighter::Format {
                color: code_color,
                font: Some(Font::MONOSPACE),
            }
        } else {
            highlighter::Format {
                color: None,
                font: None,
            }
        };

        // 1. Indent
        if indent > 0 {
            spans.push((
                0..indent,
                highlighter::Format {
                    color: None,
                    font: None,
                },
            ));
        }

        // 2. Marker (Header #, Quote >, List bullet/number)
        if marker_end > indent {
            let fmt = if is_header || is_quote || is_code_fence {
                base_format
            } else {
                highlighter::Format {
                    color: list_marker_color,
                    font: Some(Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                }
            };
            spans.push((indent..marker_end, fmt));
        }

        // 3. Checkbox
        if checkbox_end > marker_end {
            spans.push((
                marker_end..checkbox_end,
                highlighter::Format {
                    color: checkbox_color,
                    font: None,
                },
            ));
        }

        let rest_start = checkbox_end;
        if rest_start >= line.len() {
            return spans.into_iter();
        }

        let rest_of_line = &line[rest_start..];
        let mut byte_formats = vec![base_format; rest_of_line.len()];

        // 4. Apply Markdown inline formatting to rest_of_line
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
                        color: dim_color,
                        font: Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        }),
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
                        color: code_color,
                        font: Some(Font::MONOSPACE),
                    },
                ),
            ];

            let mut best_match: Option<(usize, usize, highlighter::Format<Font>)> = None;

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

            if let Some(pos) = remaining.find("://") {
                let mut scheme_start = pos;
                for (i, c) in remaining[..pos].char_indices().rev() {
                    if c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.' {
                        scheme_start = i;
                    } else {
                        break;
                    }
                }
                if scheme_start < pos {
                    let abs_start = cursor + scheme_start;
                    if best_match.is_none() || abs_start < best_match.as_ref().unwrap().0 {
                        let mut end_offset = pos + 3;
                        for c in remaining[pos + 3..].chars() {
                            if c.is_whitespace() || c == ')' || c == ']' {
                                break;
                            }
                            end_offset += c.len_utf8();
                        }
                        if end_offset > pos + 3 {
                            best_match = Some((
                                abs_start,
                                cursor + end_offset,
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
            }

            if let Some(pos) = remaining.find("mailto:") {
                let abs_start = cursor + pos;
                if best_match.is_none() || abs_start < best_match.as_ref().unwrap().0 {
                    let mut end_offset = 7;
                    for c in remaining[pos + 7..].chars() {
                        if c.is_whitespace() || c == ')' || c == ']' {
                            break;
                        }
                        end_offset += c.len_utf8();
                    }
                    if end_offset > 7 {
                        best_match = Some((
                            abs_start,
                            abs_start + end_offset,
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

        // 5. Apply Smart Syntax to rest_of_line
        let tokens = crate::model::parser::tokenize_smart_input(rest_of_line, false);
        let is_dark_theme = is_dark;

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
                    color: Some(Color::from_rgb(0.4, 0.6, 0.9)),
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
                crate::model::parser::SyntaxType::Note => highlighter::Format {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
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

        // 6. Coalesce adjacent identical formats into spans
        let mut current_format = byte_formats[0];
        let mut current_start = 0;
        for (i, byte_format) in byte_formats
            .iter()
            .enumerate()
            .skip(1)
            .take(rest_of_line.len() - 1)
        {
            if *byte_format != current_format && rest_of_line.is_char_boundary(i) {
                spans.push((rest_start + current_start..rest_start + i, current_format));
                current_format = *byte_format;
                current_start = i;
            }
        }
        spans.push((
            rest_start + current_start..rest_start + rest_of_line.len(),
            current_format,
        ));
        spans.into_iter()
    }
}
