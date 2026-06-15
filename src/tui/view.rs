// SPDX-License-Identifier: GPL-3.0-or-later
// File: src/tui/view.rs
/* Renders the Terminal User Interface (TUI) layout and widgets.
 *
 * This file implements the main `draw` function used by the TUI. It renders
 * the sidebar, the task list and the details pane. It also handles rendering
 * of the "virtual" expand/collapse rows that the model injects when completed
 * groups are truncated (see `model::VirtualState`). Those virtual rows are
 * rendered as simple cyan arrow rows and are handled by the key handler to
 * toggle expansion.
 */

use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::store::{TaskListItem, UNCATEGORIZED_ID};
use crate::tui::action::SidebarMode;
use crate::tui::state::{AppState, Focus, InputMode};

use rust_i18n::t;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn highlight_markdown_raw(input: &str, is_dark_theme: bool) -> Text<'static> {
    use ratatui::text::Text;
    let mut lines = Vec::new();

    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let mut spans = Vec::new();

        if trimmed.starts_with('#') {
            spans.push(Span::styled(
                line.to_string(),
                Style::default()
                    .fg(if is_dark_theme {
                        Color::Blue
                    } else {
                        Color::Rgb(150, 50, 50)
                    })
                    .add_modifier(Modifier::BOLD),
            ));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            spans.push(Span::styled(
                line.to_string(),
                Style::default().fg(if is_dark_theme {
                    Color::Yellow
                } else {
                    Color::Rgb(150, 100, 0)
                }),
            ));
        } else if trimmed.starts_with("> ") {
            spans.push(Span::styled(
                line.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        } else if trimmed.starts_with("```") {
            spans.push(Span::styled(
                line.to_string(),
                Style::default().fg(if is_dark_theme {
                    Color::Green
                } else {
                    Color::Rgb(0, 120, 0)
                }),
            ));
        } else {
            spans.push(Span::raw(line.to_string()));
        }

        lines.push(Line::from(spans));
    }

    if input.is_empty() {
        lines.push(Line::from(""));
    }

    Text::from(lines)
}

fn format_description_for_markdown(raw: &str) -> String {
    let text = raw.replace("\r\n", "\n");
    let paragraphs: Vec<String> = text
        .split("\n\n")
        .map(|p| {
            let has_md_structure = p.lines().any(|l| {
                let t = l.trim();
                t.starts_with('#')
                    || t.starts_with("- ")
                    || t.starts_with("* ")
                    || t.starts_with("> ")
                    || t.starts_with("```")
            });

            if has_md_structure {
                p.to_string()
            } else {
                p.replace('\n', "  \n")
            }
        })
        .collect();

    paragraphs.join("\n\n")
}

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let is_dark_theme = state.theme.is_dark();
    let footer_height = if state.mode == InputMode::EditingDescription {
        Constraint::Length(10)
    } else {
        Constraint::Length(3)
    };

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), footer_height])
        .split(f.area());

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(v_chunks[0]);

    // Placeholder for details text (built after task list rendering)
    let mut details_md = String::new();
    let mut selected_task_was_truncated = false;

    // initial main chunks (we'll recalc details height after building content)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(h_chunks[1]);

    // Sidebar title/items
    // Determine if filters produced empty result while store has tasks
    let is_filter_empty = state.tasks.is_empty() && state.store.has_any_tasks();
    // Default border style for sidebar (may be overridden per-tab below)
    let mut sidebar_border_style = if state.active_focus == Focus::Sidebar {
        Style::default().fg(if is_dark_theme {
            Color::Yellow
        } else {
            Color::Rgb(200, 100, 0)
        })
    } else {
        Style::default()
    };

    let (sidebar_title, sidebar_items) = match state.sidebar_mode {
        SidebarMode::Calendars => {
            let items: Vec<ListItem> = state
                .get_filtered_calendars()
                .into_iter()
                .map(|c| {
                    let is_target = Some(&c.href) == state.active_cal_href.as_ref();
                    let is_visible = !state.hidden_calendars.contains(&c.href);

                    let cal_color_style = if is_visible {
                        if let Some(hex) = &c.color
                            && let Some((r, g, b)) = color_utils::parse_hex_to_u8(hex)
                        {
                            Style::default().fg(Color::Rgb(r, g, b))
                        } else {
                            Style::default()
                        }
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let prefix = if is_target { ">" } else { " " };
                    let check_mark = if is_visible { "x" } else { " " };

                    let mut spans = vec![
                        Span::raw(format!("{} ", prefix)),
                        Span::styled("[", cal_color_style),
                        Span::raw(check_mark),
                        Span::styled("]", cal_color_style),
                    ];

                    let text_style = if is_target {
                        Style::default()
                            .fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(200, 100, 0)
                            })
                            .add_modifier(Modifier::BOLD)
                    } else if !is_visible {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    spans.push(Span::styled(format!(" {}", c.name), text_style));
                    ListItem::new(Line::from(spans))
                })
                .collect();
            (
                format!("  {}", rust_i18n::t!("calendars")).to_string(),
                items,
            )
        }
        SidebarMode::Categories => {
            // Use cached categories derived from the last filter() call instead of
            // performing a global store scan here.
            let all_cats = &state.cached_categories;
            let items: Vec<ListItem> = all_cats
                .iter()
                .map(|item| {
                    let selected = if state.selected_categories.contains(&item.full_key) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let indent = "  ".repeat(item.depth as usize);
                    let tree_icon_span = if item.has_children && !item.is_expanded {
                        Span::styled(
                            "[+]",
                            Style::default().fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(200, 100, 0)
                            }),
                        )
                    } else {
                        Span::raw("")
                    };

                    if item.full_key == UNCATEGORIZED_ID {
                        let spans = vec![
                            Span::raw(indent),
                            Span::raw(selected),
                            tree_icon_span,
                            Span::raw(format!(" {} ({})", item.display_name, item.count)),
                        ];
                        ListItem::new(Line::from(spans))
                    } else {
                        let (r, g, b) =
                            color_utils::generate_tui_color(&item.full_key, is_dark_theme);
                        let color =
                            Color::Rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                        let prefix = if item.display_name.contains('=') {
                            " "
                        } else {
                            " #"
                        };
                        let spans = vec![
                            Span::raw(indent),
                            Span::raw(selected),
                            tree_icon_span,
                            Span::styled(prefix, Style::default().fg(color)),
                            Span::raw(format!("{} ({})", item.display_name, item.count)),
                        ];
                        ListItem::new(Line::from(spans))
                    }
                })
                .collect();
            // ATTRIBUTION: If empty AND tags are selected -> Red Border
            if is_filter_empty && !state.selected_categories.is_empty() {
                sidebar_border_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
            }
            let logic = if state.match_all_categories {
                rust_i18n::t!("match_and")
            } else {
                rust_i18n::t!("match_or")
            };
            (
                format!("  {} ({}) ", rust_i18n::t!("tags"), logic).to_string(),
                items,
            )
        }
        SidebarMode::Locations => {
            // Use cached locations derived from the last filter() call instead of
            // scanning the store here.
            let all_locs = &state.cached_locations;
            let items: Vec<ListItem> = all_locs
                .iter()
                .map(|item| {
                    let selected = if state.selected_locations.contains(&item.full_key) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let indent = "  ".repeat(item.depth as usize);
                    let tree_icon_span = if item.has_children && !item.is_expanded {
                        Span::styled(
                            "[+]",
                            Style::default().fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(200, 100, 0)
                            }),
                        )
                    } else {
                        Span::raw("")
                    };
                    let spans = vec![
                        Span::raw(indent),
                        Span::raw(selected),
                        tree_icon_span,
                        Span::styled(
                            " @@",
                            Style::default().fg(if is_dark_theme {
                                Color::LightCyan
                            } else {
                                Color::Magenta
                            }),
                        ),
                        Span::raw(format!("{} ({})", item.display_name, item.count)),
                    ];
                    ListItem::new(Line::from(spans))
                })
                .collect();
            // ATTRIBUTION: If empty AND locations are selected -> Red Border
            if is_filter_empty && !state.selected_locations.is_empty() {
                sidebar_border_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
            }
            (
                format!("  {}", rust_i18n::t!("locations")).to_string(),
                items,
            )
        }
    };

    let sidebar = List::new(sidebar_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(sidebar_title)
                .border_style(sidebar_border_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Blue),
        );
    f.render_stateful_widget(sidebar, h_chunks[0], &mut state.cal_state);

    // Build Task list items
    let list_inner_width = main_chunks[0].width.saturating_sub(2) as usize;

    let task_items: Vec<ListItem> = state
        .tasks
        .iter()
        .enumerate()
        .map(|(idx, task_item)| {
            // Handle expand/collapse control items
            match task_item {
                TaskListItem::ExpandGroup(_, depth) => {
                    let indent = "  ".repeat(*depth);
                    let content = format!("{}  \u{f0796}", indent);
                    ListItem::new(Line::from(Span::styled(
                        content,
                        Style::default().fg(Color::Cyan),
                    )))
                }
                TaskListItem::CollapseGroup(_, depth) => {
                    let indent = "  ".repeat(*depth);
                    let content = format!("{}  \u{f0799}", indent);
                    ListItem::new(Line::from(Span::styled(
                        content,
                        Style::default().fg(Color::Cyan),
                    )))
                }
                TaskListItem::Task(t) => {
                    // Parent attributes (for resolving visible tags/location)
                    let visible_tags = &t.visible_categories;
                    let visible_location = &t.visible_location;

                    // Styling
                    let is_blocked = t.is_blocked;

                    // Compute a base color (as Color) based on priority / blocked state.
                    // We'll build the style from this color so we can dim it for done/cancelled tasks.
                    let mut base_color = if is_blocked {
                        Color::DarkGray
                    } else if t.priority == 0 || t.priority > 9 {
                        Color::Reset
                    } else {
                        let (r, g, b) = color_utils::get_priority_rgb(t.priority, is_dark_theme);
                        Color::Rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
                    };

                    // If task is done or cancelled, dim the color by blending toward the background (black).
                    // A 25% transparency effect (i.e. 75% opacity) is approximated by scaling RGB by 0.75.
                    let is_done_or_cancelled =
                        t.status.is_done() || t.status == crate::model::TaskStatus::Cancelled;
                    if is_done_or_cancelled {
                        base_color = match base_color {
                            Color::Rgb(r, g, b) => Color::Rgb(
                                ((r as f32) * 0.75) as u8,
                                ((g as f32) * 0.75) as u8,
                                ((b as f32) * 0.75) as u8,
                            ),
                            // For named/constant colors, approximate by scaling their RGB equivalents.
                            Color::Reset => Color::DarkGray,
                            Color::Red => Color::Rgb((255.0 * 0.75) as u8, 0, 0),
                            Color::Yellow => {
                                Color::Rgb((255.0 * 0.75) as u8, (255.0 * 0.75) as u8, 0)
                            }
                            Color::DarkGray => Color::Rgb(
                                (105.0 * 0.75) as u8,
                                (105.0 * 0.75) as u8,
                                (105.0 * 0.75) as u8,
                            ),
                            Color::White => Color::Rgb(
                                (255.0 * 0.75) as u8,
                                (255.0 * 0.75) as u8,
                                (255.0 * 0.75) as u8,
                            ),
                            // Fallback: leave as-is if we don't recognize the variant.
                            other => other,
                        };
                    }

                    let mut base_style = Style::default().fg(base_color);

                    let is_trash = t.calendar_href == "local://trash";

                    if (t.status.is_done() && state.strikethrough_completed) || is_trash {
                        base_style = base_style.add_modifier(Modifier::CROSSED_OUT);
                    }

                    let bracket_style = Style::default();
                    let full_symbol = t.checkbox_symbol();
                    let inner_char = full_symbol.trim_start_matches('[').trim_end_matches(']');

                    // Date / duration / recurrence
                    let is_future_start = t.is_future_start;

                    let (date_display_str, date_style) = if t.status.is_done() {
                        // NEW TUI LOGIC for completion date
                        if let Some(done_dt) = t.completion_date() {
                            let local_done = done_dt.with_timezone(&chrono::Local);
                            let color = Color::DarkGray;
                            (
                                format!(" 🗓️ {}", local_done.format("%Y-%m-%d %H:%M")),
                                Style::default().fg(color),
                            )
                        } else {
                            (String::new(), Style::default())
                        }
                    } else if is_future_start {
                        let start_ref = t.dtstart.as_ref().unwrap();
                        let start_str = start_ref.format_smart();

                        if let Some(due) = &t.due {
                            let is_same_day = start_ref.to_date_naive() == due.to_date_naive();
                            let due_str = if is_same_day {
                                match due {
                                    crate::model::DateType::Specific(dt) => {
                                        dt.with_timezone(&chrono::Local).format("%H:%M").to_string()
                                    }
                                    crate::model::DateType::AllDay(_) => due.format_smart(),
                                    crate::model::DateType::Month(_, _) => due.format_smart(),
                                    crate::model::DateType::Year(_) => due.format_smart(),
                                }
                            } else {
                                due.format_smart()
                            };

                            if start_str == due.format_smart() {
                                (
                                    format!(" ►{}⌛", start_str),
                                    Style::default().fg(Color::DarkGray),
                                )
                            } else {
                                (
                                    format!(" ►{}-{}⌛", start_str, due_str),
                                    Style::default().fg(Color::DarkGray),
                                )
                            }
                        } else {
                            (
                                format!(" ►{}", start_str),
                                Style::default().fg(Color::DarkGray),
                            )
                        }
                    } else if let Some(d) = &t.due {
                        let style = if t.is_overdue {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(if is_dark_theme {
                                Color::LightBlue
                            } else {
                                Color::Magenta
                            })
                        };

                        (format!(" @{}⌛", d.format_smart()), style)
                    } else {
                        (String::new(), Style::default())
                    };

                    let dur_str = t.format_duration_short();
                    let recur_str = if t.rrule.is_some() { " (R)" } else { "" };

                    // Prefix indentation + checkbox
                    let prefix_indent = Span::raw(if state.active_cal_href.is_some() {
                        "  ".repeat(t.depth)
                    } else {
                        "".to_string()
                    });

                    let tree_indicator = if t.has_visible_subtasks && t.collapsed {
                        Span::styled(
                            "[+]",
                            Style::default().fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(200, 100, 0)
                            }),
                        )
                    } else {
                        Span::raw("")
                    };

                    let prefix_bracket_l = Span::styled("[", bracket_style);
                    let prefix_inner = Span::styled(inner_char, base_style);
                    let prefix_bracket_r = Span::styled("]", bracket_style);
                    let prefix_blocked = Span::raw(if is_blocked { " [B] " } else { " " });

                    let prefix_width = (if state.active_cal_href.is_some() {
                        t.depth * 2 + 6
                    } else {
                        6
                    }) + if t.has_visible_subtasks && t.collapsed {
                        3
                    } else {
                        0
                    };

                    // Build metadata spans
                    let mut metadata_spans = Vec::new();
                    if !dur_str.is_empty() {
                        metadata_spans.push(Span::styled(
                            format!(" {}", dur_str),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    if !recur_str.is_empty() {
                        let r_color = if t.is_relative_recurrence() {
                            Color::Magenta
                        } else {
                            Color::DarkGray
                        };
                        metadata_spans.push(Span::styled(
                            recur_str.to_string(),
                            Style::default().fg(r_color),
                        ));
                    }

                    if t.pinned {
                        metadata_spans.push(Span::raw(" "));
                        metadata_spans
                            .push(Span::styled("📌", Style::default().fg(Color::LightRed)));
                    }

                    if state.show_priority_numbers && t.priority > 0 {
                        metadata_spans.push(Span::raw(" "));
                        metadata_spans.push(Span::styled(
                            format!("!{}", t.priority),
                            base_style.add_modifier(Modifier::BOLD),
                        ));
                    }

                    if t.alarms
                        .iter()
                        .any(|a| a.acknowledged.is_none() && !a.is_snooze())
                    {
                        metadata_spans.push(Span::raw(" "));
                        metadata_spans.push(Span::styled(
                            "🔔",
                            Style::default()
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    if !date_display_str.is_empty() {
                        if !metadata_spans
                            .last()
                            .map(|s| s.content.ends_with(' '))
                            .unwrap_or(true)
                        {
                            metadata_spans.push(Span::raw(" "));
                        }
                        metadata_spans.push(Span::styled(date_display_str, date_style));
                    }

                    if t.geo.is_some() {
                        metadata_spans.push(Span::raw(" "));
                        metadata_spans.push(Span::styled(
                            "\u{ee69}",
                            Style::default().fg(if is_dark_theme {
                                Color::LightBlue
                            } else {
                                Color::Magenta
                            }),
                        ));
                    }
                    if t.url.is_some() {
                        metadata_spans.push(Span::raw(" "));
                        metadata_spans.push(Span::styled(
                            "\u{f0789}",
                            Style::default().fg(if is_dark_theme {
                                Color::LightBlue
                            } else {
                                Color::Magenta
                            }),
                        ));
                    }

                    // Right side (location + visible tags)
                    let mut right_spans = Vec::new();
                    if let Some(loc) = &visible_location {
                        right_spans.push(Span::styled(
                            "@@",
                            Style::default().fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(180, 100, 0)
                            }),
                        ));
                        right_spans.push(Span::styled(
                            loc.clone(),
                            Style::default().fg(if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(180, 100, 0)
                            }),
                        ));
                    }

                    for cat in visible_tags {
                        let (r, g, b) = color_utils::generate_tui_color(cat, is_dark_theme);
                        if !right_spans.is_empty() {
                            right_spans.push(Span::raw(" "));
                        }

                        let display_cat = if cat.contains('=') {
                            cat.rsplit(':').next().unwrap_or(cat)
                        } else {
                            cat.as_str()
                        };
                        let label = if cat.contains('=') {
                            display_cat.to_string()
                        } else {
                            format!("#{}", display_cat)
                        };

                        right_spans.push(Span::styled(
                            label,
                            Style::default().fg(Color::Rgb(
                                (r * 255.0) as u8,
                                (g * 255.0) as u8,
                                (b * 255.0) as u8,
                            )),
                        ));
                    }

                    let metadata_width: usize =
                        metadata_spans.iter().map(|s| s.content.width()).sum();
                    let right_width: usize = right_spans.iter().map(|s| s.content.width()).sum();

                    let reserved_width = prefix_width + metadata_width + right_width;
                    let available_for_title = if reserved_width + 10 < list_inner_width {
                        list_inner_width
                            .saturating_sub(reserved_width)
                            .saturating_sub(1)
                    } else {
                        30
                    };

                    let (display_title, is_truncated) = {
                        let title_width = t.summary.width();
                        if title_width > available_for_title {
                            let mut truncated = String::new();
                            let mut acc = 0usize;
                            let trunc_target = available_for_title.saturating_sub(3);
                            for c in t.summary.chars() {
                                let cw = UnicodeWidthChar::width(c).unwrap_or(0);
                                if acc + cw > trunc_target {
                                    break;
                                }
                                truncated.push(c);
                                acc += cw;
                            }
                            truncated.push_str("...");
                            (truncated, true)
                        } else {
                            (t.summary.clone(), false)
                        }
                    };

                    if is_truncated && Some(idx) == state.list_state.selected() {
                        selected_task_was_truncated = true;
                    }

                    let mut spans = vec![
                        prefix_indent,
                        prefix_bracket_l,
                        prefix_inner,
                        prefix_bracket_r,
                        tree_indicator,
                        prefix_blocked,
                        Span::styled(display_title, base_style),
                    ];
                    spans.extend(metadata_spans);

                    if !right_spans.is_empty() {
                        let left_width: usize = spans.iter().map(|s| s.content.width()).sum();
                        let total_content = left_width + right_width;
                        if total_content < list_inner_width {
                            let padding = list_inner_width - total_content;
                            spans.push(Span::raw(" ".repeat(padding)));
                        } else {
                            spans.push(Span::raw(" "));
                        }
                        spans.extend(right_spans);
                    }

                    ListItem::new(Line::from(spans))
                }
            }
        })
        .collect();

    // Build details content for selected task
    if let Some(task) = state.get_selected_task() {
        if selected_task_was_truncated && !task.summary.is_empty() {
            details_md.push_str(&task.summary);
            details_md.push_str("\n\n");
        }

        if !task.description.is_empty() {
            let formatted_desc = format_description_for_markdown(&task.description);
            details_md.push_str(&formatted_desc);
            details_md.push_str("\n\n");
        }

        let mut meta = Vec::new();
        if let Some(url) = &task.url {
            meta.push(format!("- **URL:** {}", url));
        }
        if let Some(geo) = &task.geo {
            meta.push(format!("- **Geo:** {}", geo));
        }
        if let Some(loc) = &task.location {
            meta.push(format!("- **Location:** {}", loc));
        }
        let mut date_infos = Vec::new();
        let created_opt = task.created_date();
        let modified_opt = task.last_modified_date();

        if let Some(created) = created_opt {
            let local = created.with_timezone(&chrono::Local);
            date_infos.push(format!(
                "**{}**: {}",
                rust_i18n::t!("created_label"),
                local.format("%Y-%m-%d %H:%M")
            ));
        }
        if let Some(modified) = modified_opt
            && created_opt != Some(modified)
        {
            let local = modified.with_timezone(&chrono::Local);
            date_infos.push(format!(
                "**{}**: {}",
                rust_i18n::t!("last_modified_label"),
                local.format("%Y-%m-%d %H:%M")
            ));
        }
        if !date_infos.is_empty() {
            meta.push(format!("- {}", date_infos.join("  |  ")));
        }

        if !meta.is_empty() {
            details_md.push_str("---\n");
            details_md.push_str(&meta.join("\n"));
            details_md.push_str("\n\n");
        }

        if !task.dependencies.is_empty() {
            details_md.push_str("### Blocked By (Predecessors)\n");
            for dep_uid in &task.dependencies {
                let name = state
                    .store
                    .get_summary(dep_uid)
                    .unwrap_or_else(|| "Unknown".to_string());
                // FIXED: Use is_task_done instead of get_task_status
                let is_done = state.store.is_task_done(dep_uid).unwrap_or(false);
                let check = if is_done { "[x]" } else { "[ ]" };
                details_md.push_str(&format!("- {} {}\n", check, name));
            }
            details_md.push('\n');
        }

        // Blocking Section (Successors) - tasks that are blocked BY this task
        let blocking_tasks = state.store.get_tasks_blocking(&task.uid);
        if !blocking_tasks.is_empty() {
            details_md.push_str("### Blocking (Successors)\n");
            for (_uid, name) in blocking_tasks {
                details_md.push_str(&format!("- ⬇ {}\n", name));
            }
            details_md.push('\n');
        }

        if !task.related_to.is_empty() {
            details_md.push_str("### Related To\n");
            for related_uid in &task.related_to {
                let mut name = "Unknown".to_string();
                if let Some(rel_task) = state.store.get_task_ref(related_uid) {
                    name = rel_task.summary.clone();
                    if rel_task.status.is_done() {
                        if let Some(comp_date) = rel_task.completion_date() {
                            let local = comp_date.with_timezone(&chrono::Local);
                            name = format!("{} (✓ {})", name, local.format("%Y-%m-%d %H:%M"));
                        } else {
                            name = format!("{} (✓)", name);
                        }
                    }
                }
                details_md.push_str(&format!("- {}\n", name));
            }
            details_md.push('\n');
        }

        let incoming_related = state.store.get_tasks_related_to(&task.uid);
        if !incoming_related.is_empty() {
            details_md.push_str("### Related From\n");
            for (related_uid, mut related_name) in incoming_related {
                if let Some(rel_task) = state.store.get_task_ref(&related_uid)
                    && rel_task.status.is_done()
                {
                    if let Some(comp_date) = rel_task.completion_date() {
                        let local = comp_date.with_timezone(&chrono::Local);
                        related_name =
                            format!("{} (✓ {})", related_name, local.format("%Y-%m-%d %H:%M"));
                    } else {
                        related_name = format!("{} (✓)", related_name);
                    }
                }
                details_md.push_str(&format!("- {}\n", related_name));
            }
            details_md.push('\n');
        }

        // --- History (for recurring tasks) ---
        if task.rrule.is_some() {
            let (c7, c30) = state.store.get_completion_history_stats(&task.uid);
            if c30 > 0 {
                details_md.push_str(&format!("### {}\n", t!("habit_history")));
                if c7 == 1 {
                    details_md.push_str(&format!("- {}\n", t!("habit_completed_7_days.one")));
                } else {
                    details_md.push_str(&format!(
                        "- {}\n",
                        t!("habit_completed_7_days.other", count = c7)
                    ));
                }
                if c30 == 1 {
                    details_md.push_str(&format!("- {}\n\n", t!("habit_completed_30_days.one")));
                } else {
                    details_md.push_str(&format!(
                        "- {}\n\n",
                        t!("habit_completed_30_days.other", count = c30)
                    ));
                }
            }
        }

        // --- Work Sessions (recent) ---
        if !task.sessions.is_empty() {
            let mut total_mins: i64 = 0;
            let mut session_lines: Vec<String> = Vec::new();
            // Show most recent sessions first
            for session in task.sessions.iter().rev() {
                total_mins += (session.end - session.start) / 60;
                let s_dt = chrono::DateTime::from_timestamp(session.start, 0)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                    .with_timezone(&chrono::Local);
                let e_dt = chrono::DateTime::from_timestamp(session.end, 0)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                    .with_timezone(&chrono::Local);
                let dur = (session.end - session.start) / 60;
                session_lines.push(format!(
                    "- {} {}-{} *({}m)*",
                    s_dt.format("%Y-%m-%d"),
                    s_dt.format("%H:%M"),
                    e_dt.format("%H:%M"),
                    dur
                ));
            }
            details_md.push_str(&format!(
                "### {}\n",
                t!(
                    "time_tracked_duration",
                    h = total_mins / 60,
                    m = total_mins % 60
                )
            ));
            for line in session_lines.into_iter().take(3) {
                details_md.push_str(&line);
                details_md.push('\n');
            }
            if task.sessions.len() > 3 {
                let count = task.sessions.len() - 3;
                let text = if count == 1 {
                    t!("tui_older_sessions_hidden.one").to_string()
                } else {
                    t!("tui_older_sessions_hidden.other", count = count).to_string()
                };
                details_md.push_str(&format!("*{text}*\n"));
            }
            details_md.push('\n');
        }
    }

    if details_md.is_empty() {
        details_md = "_No details_".to_string();
    }

    let active_count = state
        .tasks
        .iter()
        .filter_map(|item| {
            if let TaskListItem::Task(task) = item {
                Some(task.as_ref())
            } else {
                None
            }
        })
        .filter(|t| !t.status.is_done())
        .count();

    // Calculate details height dynamically
    let details_width = h_chunks[1].width.saturating_sub(2);
    let mut required_lines: u16 = 0;
    if details_width > 0 {
        for line in details_md.lines() {
            let line_len = line.width() as u16;
            if line_len == 0 {
                required_lines += 1;
            } else {
                required_lines += line_len.div_ceil(details_width);
            }
        }
    }

    let calculated_height = required_lines + 2;
    let available_height = v_chunks[0].height;
    let max_details_height = available_height / 2;
    let final_details_height = calculated_height.clamp(3, max_details_height);

    // Recalculate layout with final details height
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(final_details_height)])
        .split(h_chunks[1]);

    let mut title = if state.loading {
        format!(
            " {} ({}) ",
            rust_i18n::t!("tasks"),
            rust_i18n::t!("loading")
        )
    } else {
        let tasks_str = match active_count {
            0 => rust_i18n::t!("tasks_count.zero"),
            1 => rust_i18n::t!("tasks_count.one"),
            _ => rust_i18n::t!("tasks_count.other", count = active_count),
        };
        format!(" {} ", tasks_str)
    };
    if state.unsynced_changes {
        title.push_str(&format!(" [{}] ", rust_i18n::t!("unsynced")));
    }
    if !state.active_search_query.is_empty() {
        title.push_str(&format!(
            "[{}: '{}']",
            rust_i18n::t!("search"),
            state.active_search_query
        ));
    }

    let main_style = if state.active_focus == Focus::Main {
        Style::default().fg(if is_dark_theme {
            Color::Yellow
        } else {
            Color::Rgb(200, 100, 0)
        })
    } else if state.unsynced_changes {
        Style::default().fg(Color::LightRed)
    } else {
        Style::default()
    };

    let task_list = List::new(task_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(main_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(if is_dark_theme {
                    Color::Green
                } else {
                    Color::Rgb(255, 200, 100)
                })
                .fg(Color::Black),
        );
    f.render_stateful_widget(task_list, main_chunks[0], &mut state.list_state);

    // Details rendering (markdown)
    if state.mode != InputMode::EditingDescription {
        let md_text = tui_markdown::from_str(&details_md);
        let details_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", rust_i18n::t!("details")))
            .border_style(Style::default().fg(Color::Blue));
        let p = Paragraph::new(md_text)
            .block(details_block)
            .wrap(Wrap { trim: true });
        f.render_widget(p, main_chunks[1]);
    } else {
        let p = Paragraph::new(rust_i18n::t!("editing")).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", rust_i18n::t!("details")))
                .style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(p, main_chunks[1]);
    }

    // Footer area
    let footer_area = v_chunks[1];
    f.render_widget(Clear, footer_area);

    match state.mode {
        InputMode::Creating
        | InputMode::Editing
        | InputMode::Searching
        | InputMode::EditingDescription
        | InputMode::AddingSession => {
            // Determine input title and color. If filters are the culprit, make search show red.
            let (mut title_str, prefix, color) = match state.mode {
                InputMode::Searching => {
                    let is_search_culprit = is_filter_empty && !state.input_buffer.is_empty();
                    if is_search_culprit {
                        (
                            format!(" {} (0) ", rust_i18n::t!("search")),
                            "/ ",
                            Color::Red,
                        )
                    } else {
                        (format!(" {} ", rust_i18n::t!("search")), "/ ", Color::Green)
                    }
                }
                InputMode::Editing => (
                    format!(" {} ", rust_i18n::t!("edit_task_title")),
                    "> ",
                    Color::Magenta,
                ),
                InputMode::EditingDescription => (
                    format!(" {} ", rust_i18n::t!("edit_task_title")),
                    "📝 ",
                    Color::Blue,
                ),
                InputMode::Creating => {
                    if state.creating_child_of.is_some() {
                        (
                            format!(" {} ", rust_i18n::t!("mode_create")),
                            "> ",
                            if is_dark_theme {
                                Color::LightYellow
                            } else {
                                Color::Rgb(200, 150, 0)
                            },
                        )
                    } else {
                        (
                            format!(" {} ", rust_i18n::t!("mode_create")),
                            "> ",
                            if is_dark_theme {
                                Color::Yellow
                            } else {
                                Color::Rgb(180, 100, 0)
                            },
                        )
                    }
                }
                InputMode::AddingSession => (" Log Time ".to_string(), "> ", Color::Green),
                _ => (
                    format!(" {} ", rust_i18n::t!("mode_create")),
                    "> ",
                    if is_dark_theme {
                        Color::Yellow
                    } else {
                        Color::Rgb(180, 100, 0)
                    },
                ),
            };

            if (state.mode == InputMode::Searching && state.input_buffer.starts_with('#'))
                || (state.mode == InputMode::Creating
                    && state.input_buffer.starts_with('#')
                    && state.creating_child_of.is_none())
            {
                title_str.push_str(" [Enter to jump to tag] ");
            }

            let prefix_span = Span::styled(prefix, Style::default().fg(color));
            let inner_width = footer_area.width.saturating_sub(2) as usize;
            let prefix_width = prefix.width();
            let input_area_width = inner_width.saturating_sub(prefix_width).saturating_sub(1);

            let (visible_text, visible_cursor_x) = if state.mode == InputMode::EditingDescription {
                (String::new(), 0)
            } else {
                let text_up_to_cursor: String = state
                    .input_buffer
                    .chars()
                    .take(state.cursor_position)
                    .collect();
                let cursor_visual_x = text_up_to_cursor.width();

                if cursor_visual_x >= input_area_width {
                    let target_scroll_w = cursor_visual_x - input_area_width + 1;
                    let mut skipped_chars = 0;
                    let mut skipped_w = 0;
                    for c in state.input_buffer.chars() {
                        if skipped_w >= target_scroll_w {
                            break;
                        }
                        skipped_w += c.width().unwrap_or(0);
                        skipped_chars += 1;
                    }

                    let slice: String = state.input_buffer.chars().skip(skipped_chars).collect();
                    let vis_cursor_x = cursor_visual_x - skipped_w;
                    (slice, vis_cursor_x as u16)
                } else {
                    (state.input_buffer.clone(), cursor_visual_x as u16)
                }
            };

            let mut input_spans = vec![prefix_span];

            if state.mode == InputMode::EditingDescription {
                input_spans.push(Span::raw(rust_i18n::t!("desc_editor_help").to_string()));
            } else {
                let tokens =
                    tokenize_smart_input(&visible_text, state.mode == InputMode::Searching);
                for token in tokens {
                    let text = &visible_text[token.start..token.end];
                    let style = match token.kind {
                        SyntaxType::Priority => {
                            let p = text.trim_start_matches('!').parse::<u8>().unwrap_or(0);
                            let (r, g, b) = color_utils::get_priority_rgb(p, is_dark_theme);
                            Style::default()
                                .fg(Color::Rgb(
                                    (r * 255.0) as u8,
                                    (g * 255.0) as u8,
                                    (b * 255.0) as u8,
                                ))
                                .add_modifier(Modifier::BOLD)
                        }
                        SyntaxType::DueDate => Style::default().fg(if is_dark_theme {
                            Color::LightBlue
                        } else {
                            Color::Magenta
                        }),
                        SyntaxType::StartDate => Style::default().fg(Color::Green),
                        SyntaxType::Recurrence => Style::default().fg(Color::Magenta),
                        SyntaxType::Duration => Style::default().fg(Color::DarkGray),
                        SyntaxType::Tag => {
                            let tag_name = text.trim_start_matches('#');
                            let (r, g, b) =
                                color_utils::generate_tui_color(tag_name, is_dark_theme);
                            Style::default().fg(Color::Rgb(
                                (r * 255.0) as u8,
                                (g * 255.0) as u8,
                                (b * 255.0) as u8,
                            ))
                        }
                        SyntaxType::Text => Style::default().fg(color),
                        SyntaxType::Location => Style::default().fg(if is_dark_theme {
                            Color::LightCyan
                        } else {
                            Color::Magenta
                        }),
                        SyntaxType::Url => Style::default().fg(if is_dark_theme {
                            Color::LightBlue
                        } else {
                            Color::Magenta
                        }),
                        SyntaxType::Geo => Style::default().fg(Color::DarkGray),
                        SyntaxType::Description => Style::default().fg(Color::Gray),
                        SyntaxType::Reminder => Style::default().fg(Color::LightRed),
                        SyntaxType::Operator => Style::default().fg(Color::Magenta), // Highlight boolean/operator tokens
                        SyntaxType::Calendar => Style::default().fg(Color::Magenta), // Added for +cal/-cal
                        SyntaxType::Pin => Style::default().fg(Color::LightRed), // Added for +pin/-pin
                        SyntaxType::Filter => Style::default().fg(Color::Cyan), // Added for search operators / filters
                    };
                    input_spans.push(Span::styled(text, style));
                }
            }

            let input_text = Line::from(input_spans);
            let input = Paragraph::new(input_text)
                .style(Style::default())
                .block(Block::default().borders(Borders::ALL).title(title_str))
                .wrap(Wrap { trim: false });
            f.render_widget(input, footer_area);

            if state.mode == InputMode::EditingDescription {
                let cursor_x = footer_area.x + 2;
                let cursor_y = footer_area.y + 1;
                f.set_cursor_position((cursor_x, cursor_y));
            } else {
                let cursor_x = footer_area.x + 1 + prefix_width as u16 + visible_cursor_x;
                f.set_cursor_position((
                    cursor_x.min(footer_area.x + footer_area.width - 2),
                    footer_area.y + 1,
                ));
            }
        }
        InputMode::Help(_) => {
            let help = Paragraph::new(rust_i18n::t!("tui_help_actions"))
                .alignment(Alignment::Right)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} ", rust_i18n::t!("actions"))),
                );
            f.render_widget(help, footer_area);
        }
        _ => {
            let status = Paragraph::new(state.message.clone())
                .style(Style::default().fg(if is_dark_theme {
                    Color::Cyan
                } else {
                    Color::Rgb(200, 100, 0)
                }))
                .block(
                    Block::default()
                        .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
                        .title(format!(" {} ", rust_i18n::t!("status"))),
                );
            let help_text = match state.active_focus {
                Focus::Sidebar => rust_i18n::t!("tui_sidebar_help").to_string(),
                Focus::Main => {
                    if let Some(uid) = &state.yanked_uid {
                        let mut summary = state
                            .store
                            .get_summary(uid)
                            .unwrap_or_else(|| "Unknown".to_string());
                        if state.yank_lock_active {
                            summary.push_str(" \u{f10ba}");
                        }
                        rust_i18n::t!(
                            "tui_yanked_help",
                            yanked_label = rust_i18n::t!("yanked_label"),
                            summary = summary
                        )
                        .to_string()
                    } else {
                        rust_i18n::t!("tui_main_help").to_string()
                    }
                }
            };
            let help = Paragraph::new(help_text).alignment(Alignment::Right).block(
                Block::default()
                    .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                    .title(format!(" {} ", rust_i18n::t!("actions"))),
            );
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(footer_area);
            f.render_widget(status, chunks[0]);
            f.render_widget(help, chunks[1]);
        }
    }

    // Remaining popups (moving, exporting, relationship browsing, alarms, description editor)
    // ... these are intentionally left identical to previous behavior and kept minimal here.
    // For brevity we will render them similarly to earlier code paths if their modes are active.

    if state.mode == InputMode::Moving {
        let area = centered_rect(60, 50, f.area());
        let items: Vec<ListItem> = state
            .move_targets
            .iter()
            .map(|c| ListItem::new(c.name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", rust_i18n::t!("move_task_title"))),
            )
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.move_selection_state);
    }

    // Relationship browsing
    if state.mode == InputMode::RelationshipBrowsing {
        let area = centered_rect(70, 60, f.area());
        let items: Vec<ListItem> = state
            .relationship_items
            .iter()
            .map(|(_, display_name, _)| ListItem::new(display_name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                " {} (Del/x: Remove) ",
                rust_i18n::t!("jump_to_related_task")
            )))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.relationship_selection_state);
    }

    // Session management popup (opened when managing sessions)
    if state.mode == InputMode::ManagingSessions {
        let area = centered_rect(60, 50, f.area());
        let items: Vec<ListItem> = state
            .session_items
            .iter()
            .map(|(_, display_name)| ListItem::new(display_name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", t!("tui_manage_sessions_title"))),
            )
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.session_selection_state);
    }

    // Alarm popup (simplified rendering if active)
    if let Some((task, _alarm_uid)) = &state.active_alarm {
        let area = centered_rect(60, 40, f.area());
        let block = Block::default()
            .title(format!(" {} ", rust_i18n::t!("reminder_title"))) // TODO upper?
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightRed));
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Task: "),
                Span::styled(
                    task.summary.clone(),
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::White),
                ),
            ]),
            Line::from(""),
        ];
        if !task.description.is_empty() {
            lines.push(Line::from(Span::styled(
                task.description.clone(),
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(""));
        }
        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        f.render_widget(Clear, area);
        f.render_widget(p, area);
    }

    // Action menu popup
    if state.mode == InputMode::ActionMenu {
        let area = centered_rect(50, 60, f.area());
        f.render_widget(Clear, area);

        let items: Vec<ListItem> = state
            .action_menu_items
            .iter()
            .map(|a| {
                let mut label = a.label();
                if *a == crate::config::TaskAction::ToggleDetails {
                    label = rust_i18n::t!("help_metadata_jump_related").to_string();
                } else if *a == crate::config::TaskAction::DuplicateTree
                    && let Some(task) = state.get_selected_task()
                    && !task.has_subtasks
                {
                    label = rust_i18n::t!("duplicate_single_task").to_string();
                }
                ListItem::new(label)
            })
            .collect();

        let title = if state.action_filter.is_empty() {
            format!(" {} ", rust_i18n::t!("actions"))
        } else {
            format!(
                " {} (Filter: {}) ",
                rust_i18n::t!("actions"),
                state.action_filter
            )
        };

        let popup = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(if is_dark_theme {
                        Color::Yellow
                    } else {
                        Color::Rgb(200, 100, 0)
                    })),
            )
            .highlight_style(
                Style::default()
                    .bg(if is_dark_theme {
                        Color::Blue
                    } else {
                        Color::Rgb(255, 200, 100)
                    })
                    .fg(Color::Black),
            );

        f.render_stateful_widget(popup, area, &mut state.action_selection_state);
    }

    // Editing description popup
    if state.mode == InputMode::EditingDescription {
        let area = centered_rect(80, 70, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(format!(" {} ", rust_i18n::t!("edit_description_title")))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if is_dark_theme {
                Color::Yellow
            } else {
                Color::Rgb(200, 100, 0)
            }));
        let inner_area = block.inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner_area);

        let viewport_height = chunks[0].height;
        let viewport_width = chunks[0].width.saturating_sub(1);

        let byte_idx_at_cursor = state
            .input_buffer
            .char_indices()
            .map(|(i, _)| i)
            .nth(state.cursor_position)
            .unwrap_or(state.input_buffer.len());
        let text_up_to_cursor = &state.input_buffer[..byte_idx_at_cursor];
        let cursor_row = text_up_to_cursor.chars().filter(|&c| c == '\n').count() as u16;
        let last_newline_byte_idx = text_up_to_cursor.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let current_line_slice = &text_up_to_cursor[last_newline_byte_idx..];
        let cursor_col = current_line_slice.width() as u16;

        if cursor_row < state.edit_scroll_offset {
            state.edit_scroll_offset = cursor_row;
        }
        if cursor_row >= state.edit_scroll_offset + viewport_height {
            state.edit_scroll_offset = cursor_row - viewport_height + 1;
        }
        if cursor_col < state.edit_scroll_x {
            state.edit_scroll_x = cursor_col;
        }
        if cursor_col >= state.edit_scroll_x + viewport_width {
            state.edit_scroll_x = cursor_col - viewport_width + 1;
        }

        let styled_content = highlight_markdown_raw(&state.input_buffer, is_dark_theme);
        let p = Paragraph::new(styled_content)
            .block(Block::default())
            .scroll((state.edit_scroll_offset, state.edit_scroll_x));
        f.render_widget(block, area);
        f.render_widget(p, chunks[0]);

        let instructions = Line::from(rust_i18n::t!("tui_desc_editor_help").to_string());
        f.render_widget(
            Paragraph::new(instructions).alignment(Alignment::Center),
            chunks[1],
        );

        let visual_row = cursor_row.saturating_sub(state.edit_scroll_offset);
        let visual_col = cursor_col.saturating_sub(state.edit_scroll_x);
        if visual_row < viewport_height && visual_col < chunks[0].width {
            f.set_cursor_position((chunks[0].x + visual_col, chunks[0].y + visual_row));
        }
    }

    // Render Help modal when in Help mode (popover)
    if let InputMode::Help(tab) = state.mode {
        let area = centered_rect(90, 90, f.area());
        f.render_widget(Clear, area);

        // 1. Build the Top Tabs
        let mut tab_spans = Vec::new();
        // Manually iterate over the 3 tabs for the TUI
        let tabs = [
            crate::help::HelpTab::Syntax,
            crate::help::HelpTab::Shortcuts,
            crate::help::HelpTab::About,
        ];

        for &c in &tabs {
            let label = match c {
                crate::help::HelpTab::Syntax => rust_i18n::t!("help_syntax_tab").to_string(),
                crate::help::HelpTab::Shortcuts => rust_i18n::t!("help_shortcuts_tab").to_string(),
                crate::help::HelpTab::About => rust_i18n::t!("help_about_tab").to_string(),
            };
            if c == tab {
                tab_spans.push(Span::styled(
                    label,
                    Style::default()
                        .fg(if is_dark_theme {
                            Color::Black
                        } else {
                            Color::White
                        })
                        .bg(if is_dark_theme {
                            Color::Yellow
                        } else {
                            Color::Rgb(200, 100, 0)
                        })
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                tab_spans.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
            }
            tab_spans.push(Span::raw(" "));
        }

        let mut lines = vec![Line::from(tab_spans), Line::from("")];

        // 2. Render Content
        if tab == crate::help::HelpTab::About {
            lines.push(Line::from(Span::styled(
                rust_i18n::t!("about_title").to_string(),
                Style::default()
                    .fg(if is_dark_theme {
                        Color::Yellow
                    } else {
                        Color::Rgb(200, 100, 0)
                    })
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(
                rust_i18n::t!("about_version", version = env!("CARGO_PKG_VERSION")).to_string(),
            ));
            lines.push(Line::from(rust_i18n::t!("about_license").to_string()));
            lines.push(Line::from(""));
            lines.push(Line::from(
                rust_i18n::t!(
                    "about_repository",
                    url = "https://codeberg.org/trougnouf/cfait"
                )
                .to_string(),
            ));
            lines.push(Line::from(
                rust_i18n::t!("about_chat", url = "#Cfait:matrix.org").to_string(),
            ));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                rust_i18n::t!("support_card_title").to_string(),
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("Liberapay: https://liberapay.com/trougnouf"));
            lines.push(Line::from("Ko-fi:     https://ko-fi.com/trougnouf"));
            lines.push(Line::from("Bank (SEPA): BE77 9731 6116 6342"));
            lines.push(Line::from(
                "Bitcoin:   bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979",
            ));
            lines.push(Line::from(
                "Litecoin:  ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg",
            ));
            lines.push(Line::from(
                "Ethereum:  0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB",
            ));
        } else {
            let data = if tab == crate::help::HelpTab::Syntax {
                crate::help::get_syntax_help()
            } else {
                crate::help::get_shortcuts_help()
            };

            for sec in data {
                lines.push(Line::from(Span::styled(
                    format!("--- {} ---", sec.title),
                    Style::default()
                        .fg(if is_dark_theme {
                            Color::LightCyan
                        } else {
                            Color::Rgb(200, 100, 0)
                        })
                        .add_modifier(Modifier::BOLD),
                )));
                for item in sec.items {
                    let keys_span = Span::styled(
                        format!("{:width$}", item.keys, width = 22),
                        Style::default().fg(Color::Green),
                    );
                    let desc_span = Span::raw(item.desc);
                    if item.example.is_empty() {
                        lines.push(Line::from(vec![Span::raw("  "), keys_span, desc_span]));
                    } else {
                        let example_span = Span::styled(
                            format!(" (e.g. {})", item.example),
                            Style::default().fg(Color::DarkGray),
                        );
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            keys_span,
                            desc_span,
                            example_span,
                        ]));
                    }
                }
                lines.push(Line::from(""));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " {}{}",
                rust_i18n::t!("help"),
                rust_i18n::t!("help_tab_to_switch")
            ))
            .border_style(Style::default().fg(if is_dark_theme {
                Color::Yellow
            } else {
                Color::Rgb(200, 100, 0)
            }));

        let p = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((state.edit_scroll_offset, 0));

        f.render_widget(p, area);
    }
}

/// Helper to center rects
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
