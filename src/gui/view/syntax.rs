// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/view/syntax.rs
// Implements syntax highlighting for the smart input editor.
use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use iced::advanced::text::highlighter::{self, Highlighter};
use iced::{Color, Font};
use std::ops::Range;

pub fn get_syntax_style(kind: SyntaxType, text: &str, is_dark: bool) -> (Option<Color>, bool) {
    match kind {
        SyntaxType::Priority => {
            let p = text.trim_start_matches('!').parse::<u8>().unwrap_or(0);
            let (r, g, b) = color_utils::get_priority_rgb(p, is_dark);
            (Some(Color::from_rgb(r, g, b)), true)
        }
        SyntaxType::DueDate => (Some(Color::from_rgb(0.2, 0.6, 1.0)), false),
        SyntaxType::StartDate => (Some(Color::from_rgb(0.4, 0.8, 0.4)), false),
        SyntaxType::Recurrence => (Some(Color::from_rgb(0.8, 0.4, 0.8)), false),
        SyntaxType::Duration => (Some(Color::from_rgb(0.6, 0.6, 0.6)), false),
        SyntaxType::Tag => {
            let tag_name = text.trim_start_matches('#');
            let (r, g, b) = color_utils::generate_color(tag_name);
            (Some(Color::from_rgb(r, g, b)), true)
        }
        SyntaxType::Text => (None, false),
        SyntaxType::Location => (Some(Color::from_rgb(0.8, 0.5, 0.0)), false),
        SyntaxType::Url => (Some(Color::from_rgb(0.2, 0.2, 0.8)), false),
        SyntaxType::WikiLink => (Some(Color::from_rgb(0.2, 0.7, 1.0)), true),
        SyntaxType::Dependency => (Some(Color::from_rgb(0.9, 0.6, 0.2)), true),
        SyntaxType::Relation => (Some(Color::from_rgb(0.4, 0.6, 0.9)), true),
        SyntaxType::Geo => (Some(Color::from_rgb(0.5, 0.5, 0.5)), false),
        SyntaxType::Description => (Some(Color::from_rgb(0.6, 0.0, 0.6)), false),
        SyntaxType::Reminder => (Some(Color::from_rgb(1.0, 0.4, 0.0)), true),
        SyntaxType::Filter => (Some(Color::from_rgb(0.0, 0.8, 0.8)), false),
        SyntaxType::Operator => (Some(Color::from_rgb(1.0, 0.0, 1.0)), true),
        SyntaxType::Goal => (Some(Color::from_rgb(0.2, 0.8, 0.6)), true),
        SyntaxType::Collection => (Some(Color::from_rgb(0.9, 0.4, 0.4)), true),
        SyntaxType::Calendar => (Some(Color::from_rgb(0.91, 0.11, 0.38)), true),
        SyntaxType::Pin => (Some(Color::from_rgb(1.0, 0.4, 0.0)), true),
        SyntaxType::Note => (Some(Color::from_rgb(0.5, 0.5, 0.5)), true),
    }
}

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
                let text = &line[t.start..t.end];
                let (opt_color, is_bold) =
                    crate::gui::view::syntax::get_syntax_style(t.kind, text, self.is_dark);
                let format = highlighter::Format {
                    color: opt_color,
                    font: if is_bold {
                        Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        })
                    } else {
                        None
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

        // 4. Apply Markdown inline formatting to rest_of_line using central parser
        let inline_elements = crate::model::parser::parse_inline_markdown(rest_of_line);
        let mut elem_cursor = rest_start;
        for el in inline_elements {
            use crate::model::parser::InlineElement;
            let (raw, is_plain) = match &el {
                InlineElement::Text(r) => (*r, true),
                InlineElement::Bold { raw: r, .. } => (*r, false),
                InlineElement::Italic { raw: r, .. } => (*r, false),
                InlineElement::Strikethrough { raw: r, .. } => (*r, false),
                InlineElement::Code { raw: r, .. } => (*r, false),
                InlineElement::Link { raw: r, .. } => (*r, false),
            };

            if is_plain {
                let tokens = crate::model::parser::tokenize_smart_input(raw, false);
                for t in tokens {
                    let text = &raw[t.start..t.end];
                    let (opt_color, is_bold) = get_syntax_style(t.kind, text, self.is_dark);
                    let mut format = base_format;
                    if opt_color.is_some() {
                        format.color = opt_color;
                    }
                    if is_bold {
                        format.font = Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        });
                    }
                    spans.push((elem_cursor + t.start..elem_cursor + t.end, format));
                }
            } else {
                let mut format = base_format;
                match el {
                    InlineElement::Bold { .. } => {
                        format.font = Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        })
                    }
                    InlineElement::Italic { .. } => {
                        format.font = Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        })
                    }
                    InlineElement::Strikethrough { .. } => {
                        format.color = dim_color;
                        format.font = Some(Font {
                            style: iced::font::Style::Italic,
                            ..Default::default()
                        });
                    }
                    InlineElement::Code { .. } => {
                        format.color = code_color;
                        format.font = Some(Font::MONOSPACE);
                    }
                    InlineElement::Link { .. } => {
                        format.color = link_color;
                        format.font = Some(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        });
                    }
                    _ => {}
                }
                spans.push((elem_cursor..elem_cursor + raw.len(), format));
            }
            elem_cursor += raw.len();
        }
        spans.into_iter()
    }
}
