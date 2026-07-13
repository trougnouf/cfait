// File: ./src/gui/view/task_row.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! GUI view component for rendering individual task rows.
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::COLOR_LOCATION;
use crate::gui::view::focusable::focusable;

use crate::model::display::random_related_icon;
use chrono::Utc;
use fastrand;
use rust_i18n::t;
use std::time::Duration;

use super::tooltip_style;
use iced::widget::{
    Space, button, column, container, rich_text, row, span, text, text_editor, tooltip,
};

pub fn parse_inline_markdown(
    text_str: &str,
    base_color: Color,
    is_strikethrough: bool,
) -> Vec<iced::widget::text::Span<'static, String>> {
    // FAST PATH: Skip expensive parsing if no markdown trigger characters are present.
    if !text_str.contains(['[', '*', '_', '~', '`']) && !text_str.contains("http") {
        let mut sp = span(text_str.to_string()).color(base_color);
        if is_strikethrough {
            sp = sp.strikethrough(true);
        }
        return vec![sp];
    }

    let mut spans = Vec::new();
    let mut current_idx = 0;

    while current_idx < text_str.len() {
        let remaining = &text_str[current_idx..];

        let markers = [
            ("[[", "]]", 2, 2),
            ("**", "**", 2, 2),
            ("__", "__", 2, 2),
            ("~~", "~~", 2, 2),
            ("*", "*", 1, 1),
            ("_", "_", 1, 1),
            ("`", "`", 1, 1),
        ];

        let mut best_match: Option<(usize, usize, &str, usize, usize)> = None;

        // Process markers first
        {
            let mut update_best = |start, end, marker, slen, elen| {
                if best_match.is_none() || start < best_match.unwrap().0 {
                    best_match = Some((start, end, marker, slen, elen));
                }
            };

            for &(start_marker, end_marker, start_len, end_len) in &markers {
                if let Some(start_pos) = remaining.find(start_marker)
                    && let Some(end_pos) = remaining[start_pos + start_len..].find(end_marker)
                {
                    let abs_start = current_idx + start_pos;
                    let abs_end = abs_start + start_len + end_pos + end_len;
                    update_best(abs_start, abs_end, start_marker, start_len, end_len);
                }
            }
        }

        let best_match_pos = best_match.as_ref().map(|(pos, _, _, _, _)| *pos);

        // Standard Markdown links: [label](url)
        let mut search_idx = 0;
        while let Some(start_pos) = remaining[search_idx..].find('[') {
            let abs_start = current_idx + search_idx + start_pos;

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
                    let abs_end = current_idx + mid_abs + end_pos + 1;
                    best_match = Some((abs_start, abs_end, "[]()", 0, 0));
                    break;
                }
            }
            search_idx += start_pos + 1;
        }

        // Bare URLs (http:// or https://)
        for scheme in &["https://", "http://"] {
            if let Some(start_pos) = remaining.find(scheme) {
                let abs_start = current_idx + start_pos;

                // Skip if we already have a better match
                if let Some(best_pos) = best_match_pos
                    && best_pos <= abs_start
                {
                    continue;
                }

                let mut end_offset = 0;
                for c in text_str[abs_start..].chars() {
                    if c.is_whitespace() || c == ')' || c == ']' {
                        break;
                    }
                    end_offset += c.len_utf8();
                }
                let abs_end = abs_start + end_offset;
                // Update best_match directly since we dropped the closure
                if best_match.is_none() || abs_start < best_match.as_ref().unwrap().0 {
                    best_match = Some((abs_start, abs_end, "http", 0, 0));
                }
            }
        }

        if let Some((abs_start, abs_end, start_marker, start_len, end_len)) = best_match {
            if abs_start > current_idx {
                let text = text_str[current_idx..abs_start].to_string();
                let mut sp = span(text).color(base_color);
                if is_strikethrough {
                    sp = sp.strikethrough(true);
                }
                spans.push(sp);
            }

            let chunk = text_str[abs_start..abs_end].to_string();
            let inner_chunk = text_str[abs_start + start_len..abs_end - end_len].to_string();

            let mut sp = match start_marker {
                "[]()" => {
                    let mid = chunk.find("](").unwrap();
                    let display = &chunk[1..mid];
                    let url = &chunk[mid + 2..chunk.len() - 1];
                    span(display.to_string())
                        .color(Color::from_rgba(0.2, 0.7, 1.0, base_color.a))
                        .link(url.to_string())
                }
                "http" => span(chunk.clone())
                    .color(Color::from_rgba(0.2, 0.7, 1.0, base_color.a))
                    .link(chunk),
                "[[" => {
                    let inner = &text_str[abs_start + start_len..abs_end - end_len];
                    let (target, display) = if let Some((t, d)) = inner.split_once('|') {
                        (t.to_string(), d.to_string())
                    } else {
                        (inner.to_string(), inner.to_string())
                    };
                    span(display.to_string())
                        .color(Color::from_rgba(0.2, 0.7, 1.0, base_color.a))
                        .link(target)
                }
                "**" | "__" => span(inner_chunk).color(base_color).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
                "*" | "_" => span(inner_chunk).color(base_color).font(iced::Font {
                    style: iced::font::Style::Italic,
                    ..Default::default()
                }),
                "`" => span(inner_chunk)
                    .color(Color::from_rgba(0.8, 0.6, 0.4, base_color.a))
                    .font(iced::Font::MONOSPACE),
                "~~" => span(inner_chunk).color(base_color).strikethrough(true),
                _ => span(inner_chunk).color(base_color),
            };

            if is_strikethrough && start_marker != "~~" {
                sp = sp.strikethrough(true);
            }
            spans.push(sp);

            current_idx = abs_end;
        } else {
            break;
        }
    }

    if current_idx < text_str.len() {
        let text = text_str[current_idx..].to_string();
        let mut sp = span(text).color(base_color);
        if is_strikethrough {
            sp = sp.strikethrough(true);
        }
        spans.push(sp);
    }

    if spans.is_empty() {
        let text = text_str.to_string();
        let mut sp = span(text).color(base_color);
        if is_strikethrough {
            sp = sp.strikethrough(true);
        }
        spans.push(sp);
    }

    spans
}
use iced::{Color, Element, Length, Theme};

// Helper inside the file to provide generic action styles

/// Generate a random example for session logging syntax
fn random_session_example() -> String {
    const DURATIONS: &[&str] = &["30m", "1h", "2h", "6h", "14:00-15:30", "09:00-10:15"];
    DURATIONS[fastrand::usize(..DURATIONS.len())].to_string()
}

pub fn action_style(theme: &Theme, status: button::Status, style_mode: u8) -> button::Style {
    let palette = theme.extended_palette();
    let base = button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: if style_mode == 1 {
            palette.danger.base.color
        } else if style_mode == 3 {
            palette.primary.base.color
        } else {
            palette.background.weak.text
        },
        border: iced::Border::default(),
        ..button::Style::default()
    };
    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(if style_mode == 1 {
                palette.danger.base.color.into()
            } else if style_mode == 3 {
                Color {
                    a: 0.2,
                    ..palette.primary.base.color
                }
                .into()
            } else {
                palette.background.strong.color.into()
            }),
            text_color: if style_mode == 1 {
                palette.danger.base.text
            } else if style_mode == 3 {
                palette.primary.base.text
            } else {
                palette.background.strong.text
            },
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..base
        },
        _ => base,
    }
}

pub fn view_task_row<'a>(
    app: &'a GuiApp,
    index: usize,
    item: &'a crate::store::TaskListItem,
    row_id: iced::widget::Id,
) -> Element<'a, Message> {
    match item {
        crate::store::TaskListItem::ExpandGroup(key, depth) => {
            let indent_size = if app.active_cal_href.is_some() {
                *depth * 12
            } else {
                0
            };
            let indent = Space::new().width(Length::Fixed(indent_size as f32));
            let btn = button(
                row![
                    icon::icon(icon::ARROW_EXPAND_DOWN)
                        .size(16)
                        .color(Color::from_rgb(0.5, 0.5, 0.8)),
                    text("Expand completed tasks")
                        .size(12)
                        .color(Color::from_rgb(0.5, 0.5, 0.8))
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .style(iced::widget::button::text)
            .width(Length::Fill)
            .on_press(Message::ToggleDoneGroup(key.clone()));
            focusable(row![indent, btn]).id(row_id).into()
        }
        crate::store::TaskListItem::CollapseGroup(key, depth) => {
            let indent_size = if app.active_cal_href.is_some() {
                *depth * 12
            } else {
                0
            };
            let indent = Space::new().width(Length::Fixed(indent_size as f32));
            let btn = button(
                row![
                    icon::icon(icon::ARROW_EXPAND_UP)
                        .size(16)
                        .color(Color::from_rgb(0.5, 0.5, 0.8)),
                    text("Collapse completed tasks")
                        .size(12)
                        .color(Color::from_rgb(0.5, 0.5, 0.8))
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .style(iced::widget::button::text)
            .width(Length::Fill)
            .on_press(Message::ToggleDoneGroup(key.clone()));
            focusable(row![indent, btn]).id(row_id).into()
        }
        crate::store::TaskListItem::Task(task) => {
            let is_blocked = task.is_blocked;
            let is_selected = app.selected_uid.as_ref() == Some(&task.uid);

            let theme = app.theme();
            let is_dark_theme = theme.extended_palette().is_dark;
            let default_text_color = theme.extended_palette().background.base.text;

            let dim_factor = if task.is_search_context { 0.35 } else { 1.0 };

            let mut color = if is_blocked {
                Color::from_rgb(0.5, 0.5, 0.5)
            } else if task.priority == 0 {
                default_text_color
            } else {
                let (r, g, b) = color_utils::get_priority_rgb(task.priority, is_dark_theme);
                Color::from_rgb(r, g, b)
            };
            color.a *= dim_factor;

            let show_indent = app.active_cal_href.is_some();
            let indent_size = if show_indent { task.depth * 12 } else { 0 };
            let indent = Space::new().width(Length::Fixed(indent_size as f32));
            let is_tree_collapsed = task.collapsed;
            let is_expanded = app.expanded_tasks.contains(&task.uid);

            let visible_tags = &task.visible_categories;
            let visible_location = &task.visible_location;

            let mut font_size = 20;
            if task.is_note {
                font_size = if task.parent_uid.is_none() { 22 } else { 20 };
            }

            let is_paused = task.is_paused();

            let has_active_alarm = task.alarms.iter().any(|a| a.acknowledged.is_none());

            let date_and_alarm_section: Element<'a, Message> = {
                let mut row_content = row![].spacing(3).align_y(iced::Alignment::Center);

                if has_active_alarm {
                    let bell_icon = icon::icon(icon::BELL)
                        .size(14)
                        .color(Color::from_rgb(1.0, 0.4, 0.0));

                    let content = container(bell_icon).padding(1);
                    row_content = row_content.push(
                        tooltip(
                            content,
                            text(rust_i18n::t!("active_alarm")).size(12),
                            tooltip::Position::Top,
                        )
                        .style(crate::gui::view::tooltip_style),
                    );
                }

                let is_future_start = task.is_future_start;
                let is_overdue = task.is_overdue;
                let dim_color = Color::from_rgba(0.7, 0.7, 0.7, dim_factor);

                let due_color = if is_overdue {
                    Color::from_rgba(0.8, 0.2, 0.2, dim_factor)
                } else {
                    Color::from_rgba(0.5, 0.5, 0.5, dim_factor)
                };

                if task.status.is_done() {
                    if let Some(done_dt) = task.completion_date() {
                        let local_done = done_dt.with_timezone(&chrono::Local);
                        let (done_icon, done_color) =
                            if task.status == crate::model::TaskStatus::Completed {
                                (icon::CALENDAR_CHECK, Color::from_rgb(0.4, 0.6, 0.4))
                            } else {
                                (icon::CALENDAR_XMARK, Color::from_rgb(0.8, 0.2, 0.2))
                            };

                        row_content = row_content.push(
                            container(icon::icon(done_icon).size(12).color(done_color))
                                .align_y(iced::Alignment::Center),
                        );
                        row_content = row_content.push(
                            text(local_done.format("%Y-%m-%d %H:%M").to_string())
                                .size(14)
                                .color(done_color),
                        );
                    }
                } else if is_future_start {
                    row_content = row_content.push(
                        container(icon::icon(icon::HOURGLASS_START).size(12).color(dim_color))
                            .align_y(iced::Alignment::Center),
                    );

                    let start_ref = task.dtstart.as_ref().unwrap();
                    let start_str = start_ref.format_smart();

                    if let Some(due) = &task.due {
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
                            row_content =
                                row_content.push(text(start_str).size(14).color(dim_color));
                        } else {
                            row_content = row_content.push(
                                text(format!("{} - {}", start_str, due_str))
                                    .size(14)
                                    .color(dim_color),
                            );
                        }
                        row_content = row_content.push(
                            container(icon::icon(icon::HOURGLASS_END).size(12).color(dim_color))
                                .align_y(iced::Alignment::Center),
                        );
                    } else {
                        row_content = row_content.push(text(start_str).size(14).color(dim_color));
                    }
                } else if let Some(d) = &task.due {
                    row_content = row_content.push(
                        container(icon::icon(icon::CALENDAR).size(12).color(due_color))
                            .align_y(iced::Alignment::Center),
                    );
                    row_content =
                        row_content.push(text(d.format_smart()).size(14).color(due_color));
                }

                container(row_content).width(Length::Shrink).into()
            };

            let has_desc = !task.description.is_empty();
            let has_valid_parent = task.parent_uid.as_ref().is_some_and(|uid| !uid.is_empty());
            let has_deps = !task.dependencies.is_empty();
            let has_related = !task.related_to.is_empty();
            let has_incoming_related = task.has_related_tasks;
            let has_blocking = task.has_blocking_tasks;

            let has_info = has_desc
                || has_deps
                || has_blocking
                || has_related
                || has_incoming_related
                || has_valid_parent;
            let has_time = !task.sessions.is_empty() || task.time_spent_seconds > 0;

            let has_content_to_show = has_info
                || has_time
                || app.adding_session_uid.as_ref() == Some(&task.uid)
                || task.created_date().is_some()
                || task.last_modified_date().is_some();

            let has_metadata = !task.categories.is_empty()
                || task.rrule.is_some()
                || is_blocked
                || task.estimated_duration.is_some()
                || task.location.is_some()
                || task.url.is_some()
                || task.geo.is_some()
                || task.time_spent_seconds > 0
                || task.last_started_at.is_some()
                || (app.show_priority_numbers && task.priority > 0);

            // Accurate overhead estimate:
            // ~130px (dates) + ~120px (actions) + 24px (checkbox) + 40px (spacing) + 22px (padding) + 10px (scrollbar)
            let overhead_allowance = 450.0;
            let available_width = app.current_window_size.width
                - if app.sidebar_is_hidden { 0.0 } else { 220.0 }
                - indent_size as f32
                - overhead_allowance;

            let mut tags_width = 0.0;

            if has_metadata {
                let show_pc = !task.status.is_done() && task.percent_complete.unwrap_or(0) > 0;

                if is_blocked {
                    tags_width += 65.0;
                }
                if app.show_priority_numbers && task.priority > 0 {
                    tags_width += 25.0;
                }
                for cat in visible_tags {
                    tags_width += (cat.len() as f32 + 1.0) * 7.0 + 10.0;
                }
                if let Some(l) = &visible_location {
                    tags_width += (l.len() as f32 * 7.0) + 25.0;
                }
                if task.estimated_duration.is_some()
                    || task.time_spent_seconds > 0
                    || task.last_started_at.is_some()
                    || show_pc
                {
                    tags_width += 50.0;
                    if show_pc {
                        tags_width += 30.0; // Extra room for "100% | "
                    }
                }
                if task.rrule.is_some() {
                    tags_width += 30.0;
                }
                if task.url.is_some() {
                    tags_width += 20.0;
                }
            }

            let tags_element: Element<'a, Message> = if has_metadata {
                let mut tags_row = row![].spacing(3).align_y(iced::Alignment::Center);

                if task.pinned {
                    tags_row = tags_row.push(
                        container(
                            icon::icon(icon::THUMB_TACK)
                                .size(12)
                                .color(Color::from_rgb(1.0, 0.4, 0.0)),
                        )
                        .padding(3),
                    );
                }

                if is_blocked {
                    tags_row = tags_row.push(
                        container(text(rust_i18n::t!("blocked")).size(12).style(
                            |theme: &Theme| text::Style {
                                color: Some(theme.extended_palette().background.base.text),
                            },
                        ))
                        .style(|_| container::Style {
                            background: Some(Color::from_rgb(0.8, 0.2, 0.2).into()),
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .padding(3),
                    );
                }

                if app.show_priority_numbers && task.priority > 0 {
                    let priority_text = text(format!("!{}", task.priority)).size(11).color(color);
                    tags_row =
                        tags_row.push(container(priority_text).padding([2, 4]).style(move |_| {
                            container::Style {
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    color: color.scale_alpha(0.5),
                                    width: 1.0,
                                },
                                ..Default::default()
                            }
                        }));
                }

                for cat in visible_tags {
                    let (r, g, b) = color_utils::generate_color(cat);
                    let bg_color = Color::from_rgba(r, g, b, dim_factor);

                    let mut text_color = if color_utils::is_dark(r, g, b) {
                        Color::WHITE
                    } else {
                        Color::BLACK
                    };
                    text_color.a *= dim_factor;

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

                    tags_row = tags_row.push(
                        button(text(label).size(12).color(text_color))
                            .style(move |_theme, status| {
                                let base = button::Style {
                                    background: Some(bg_color.into()),
                                    text_color,
                                    border: iced::Border {
                                        radius: 4.0.into(),
                                        ..Default::default()
                                    },
                                    ..button::Style::default()
                                };
                                match status {
                                    button::Status::Hovered | button::Status::Pressed => {
                                        button::Style {
                                            border: iced::Border {
                                                color: Color::BLACK.scale_alpha(0.2),
                                                width: 1.0,
                                                radius: 4.0.into(),
                                            },
                                            ..base
                                        }
                                    }
                                    _ => base,
                                }
                            })
                            .padding(3)
                            .on_press(Message::JumpToTag(cat.to_string())),
                    );
                }

                if let Some(loc) = &visible_location {
                    let mut text_color = Color::WHITE;
                    text_color.a *= dim_factor;

                    let loc_bg = {
                        let mut bg = COLOR_LOCATION;
                        bg.a *= dim_factor;
                        bg
                    };

                    let loc_btn = button(text(format!("@@{}", loc)).size(12).color(text_color))
                        .style(move |_theme, status| {
                            let base = button::Style {
                                background: Some(loc_bg.into()),
                                text_color,
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..button::Style::default()
                            };
                            match status {
                                button::Status::Hovered | button::Status::Pressed => {
                                    button::Style {
                                        border: iced::Border {
                                            color: Color::BLACK.scale_alpha(0.2),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..base
                                    }
                                }
                                _ => base,
                            }
                        })
                        .padding(3)
                        .on_press(Message::JumpToLocation(loc.clone()));

                    tags_row = tags_row.push(loc_btn);
                }

                let now_ts = Utc::now().timestamp();
                let current_session = task
                    .last_started_at
                    .map(|start| (now_ts - start).max(0) as u64)
                    .unwrap_or(0);
                let total_seconds = task.time_spent_seconds + current_session;
                let total_mins = (total_seconds / 60) as u32;

                let show_pc = !task.status.is_done() && task.percent_complete.unwrap_or(0) > 0;

                if total_mins > 0
                    || task.estimated_duration.is_some()
                    || task.last_started_at.is_some()
                    || show_pc
                {
                    let est_label = if let Some(min) = task.estimated_duration {
                        if let Some(max) = task.estimated_duration_max {
                            if max > min {
                                format!(
                                    "~{}-{}",
                                    crate::model::parser::format_duration_compact(min),
                                    crate::model::parser::format_duration_compact(max)
                                )
                            } else {
                                format!("~{}", crate::model::parser::format_duration_compact(min))
                            }
                        } else {
                            format!("~{}", crate::model::parser::format_duration_compact(min))
                        }
                    } else {
                        String::new()
                    };

                    let time_label = if total_mins > 0 || task.last_started_at.is_some() {
                        if !est_label.is_empty() {
                            format!(
                                "{} / {}",
                                crate::model::parser::format_duration_compact(total_mins),
                                est_label
                            )
                        } else {
                            crate::model::parser::format_duration_compact(total_mins)
                        }
                    } else {
                        est_label
                    };

                    let pc_str = if show_pc {
                        task.percent_complete
                            .map(|pc| format!("{}%", pc))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    let label = if !pc_str.is_empty() && !time_label.is_empty() {
                        format!("{} | {}", pc_str, time_label)
                    } else if !pc_str.is_empty() {
                        pc_str
                    } else {
                        time_label
                    };

                    let dur_bg = if task.last_started_at.is_some() {
                        Color::from_rgba(0.25, 0.50, 0.25, dim_factor)
                    } else {
                        Color::from_rgba(0.50, 0.50, 0.50, dim_factor)
                    };
                    let dur_border = if task.last_started_at.is_some() {
                        Color::from_rgba(0.25, 0.60, 0.25, dim_factor)
                    } else {
                        Color::BLACK.scale_alpha(0.05 * dim_factor)
                    };

                    tags_row = tags_row.push(
                        container(text(label).size(10).style(|theme: &Theme| text::Style {
                            color: Some(theme.extended_palette().background.base.text),
                        }))
                        .style(move |_| container::Style {
                            background: Some(dur_bg.into()),
                            border: iced::Border {
                                radius: 4.0.into(),
                                color: dur_border,
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .padding(3),
                    );
                }
                if task.rrule.is_some() {
                    let r_color = if task.is_relative_recurrence() {
                        Color::from_rgba(0.67, 0.28, 0.74, dim_factor) // #ab47bc
                    } else {
                        Color::from_rgba(0.5, 0.5, 0.5, dim_factor)
                    };
                    let recurrence_icon = icon::icon(icon::REPEAT).size(14).color(r_color);
                    tags_row = tags_row.push(container(recurrence_icon).padding(0));
                }

                tags_row.into()
            } else {
                Space::new().width(Length::Fixed(0.0)).into()
            };

            let title_width_est = task.summary.len() as f32 * 10.0;
            let required_title_space = title_width_est.min(90.0);
            let padding_safety = 5.0;

            let place_inline = if !has_metadata {
                true
            } else {
                (available_width - tags_width - padding_safety) > required_title_space
            };

            let title_color = if task.status.is_done() {
                Color { a: 0.75, ..color }
            } else {
                color
            };

            let is_strikethrough = (app.strikethrough_completed && task.status.is_done())
                || task.calendar_href == "local://trash";

            let summary_spans = parse_inline_markdown(&task.summary, title_color, is_strikethrough);

            let summary_text: Element<'a, Message> = rich_text(summary_spans)
                .size(font_size)
                .width(Length::Fill)
                .on_link_click(|target: String| {
                    if target.starts_with("http://") || target.starts_with("https://") {
                        Message::OpenUrl(target)
                    } else {
                        Message::OpenWikiLink(target)
                    }
                })
                .into();

            let main_text_col: Element<'a, Message> = if place_inline {
                row![summary_text, tags_element]
                    .spacing(6)
                    .align_y(iced::Alignment::Center)
                    .into()
            } else {
                column![
                    summary_text,
                    if has_metadata {
                        row![Space::new().width(Length::Fill), tags_element]
                    } else {
                        row![]
                    }
                ]
                .spacing(2)
                .into()
            };

            let main_text_col = container(main_text_col)
                .width(Length::Fill)
                .height(Length::Shrink);

            let mut actions = row![].spacing(3).align_y(iced::Alignment::Center);

            let has_subtasks = task.has_visible_subtasks;
            let _has_notes_or_deps = !task.description.is_empty()
                || !task.dependencies.is_empty()
                || !task.related_to.is_empty();

            if has_subtasks || is_tree_collapsed {
                let trees = [
                    icon::TREE_FA,
                    icon::TREE_FAE,
                    icon::TREE_MD,
                    icon::PALM_TREE,
                    icon::PINE_TREE,
                ];
                let hash = task
                    .uid
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_add(b as u32));

                // Generate random green shade from hash
                // G is dominant (0.6-0.9), R and B add variety (0.0-0.2)
                let r = ((hash >> 16) % 20) as f32 / 100.0; // 0.0-0.19
                let g = 0.6 + ((hash >> 8) % 30) as f32 / 100.0; // 0.6-0.89
                let b = (hash % 20) as f32 / 100.0; // 0.0-0.19

                let (icon_char, tooltip_text) = if is_tree_collapsed {
                    (
                        icon::FAMILY_TREE,
                        rust_i18n::t!("expand_tree_with_key").to_string(),
                    )
                } else {
                    (
                        trees[(hash % 5) as usize],
                        rust_i18n::t!("collapse_tree_with_key").to_string(),
                    )
                };

                let tree_color = if is_tree_collapsed {
                    Color::from_rgb(1.0, 0.6, 0.0)
                } else {
                    Color::from_rgb(r, g, b)
                };

                let collapse_btn = button(icon::icon(icon_char).size(14).color(tree_color))
                    .style(|theme, status| action_style(theme, status, 0))
                    .padding(4)
                    .on_press(Message::SetTreeCollapse(
                        task.uid.clone(),
                        !is_tree_collapsed,
                    ));
                actions = actions.push(
                    tooltip(
                        collapse_btn,
                        text(tooltip_text).size(12),
                        tooltip::Position::Top,
                    )
                    .style(crate::gui::view::tooltip_style)
                    .delay(Duration::from_millis(700)),
                );
            }

            if has_related || has_incoming_related {
                let related_icon_name = if !task.related_to.is_empty() {
                    random_related_icon(&task.uid, &task.related_to[0])
                } else if has_incoming_related {
                    let incoming = app.store.get_tasks_related_to(&task.uid);
                    if !incoming.is_empty() {
                        random_related_icon(&task.uid, &incoming[0].0)
                    } else {
                        icon::LINK
                    }
                } else {
                    icon::LINK
                };

                actions = actions.push(
                    icon::icon(related_icon_name)
                        .size(12)
                        .color(Color::from_rgb(0.5, 0.5, 0.5)),
                );
            }

            if let Some(yanked) = &app.yanked_uid {
                if *yanked != task.uid {
                    let yanked_summary = app.store.get_summary(yanked).unwrap_or_default();
                    let t_sum = if task.summary.chars().count() > 25 {
                        format!("{}...", task.summary.chars().take(22).collect::<String>())
                    } else {
                        task.summary.clone()
                    };
                    let y_sum = if yanked_summary.chars().count() > 25 {
                        format!("{}...", yanked_summary.chars().take(22).collect::<String>())
                    } else {
                        yanked_summary.clone()
                    };

                    let block_btn = button(icon::icon(icon::BLOCKED).size(14))
                        .style(|theme, status| action_style(theme, status, 0))
                        .padding(4)
                        .on_press(Message::AddDependency(task.uid.clone()));
                    actions = actions.push(
                        tooltip(
                            block_btn,
                            text(rust_i18n::t!(
                                "yank_tooltip_block",
                                target = t_sum.clone(),
                                yanked = y_sum.clone()
                            ))
                            .size(12),
                            tooltip::Position::Top,
                        )
                        .style(tooltip_style)
                        .delay(Duration::from_millis(700)),
                    );

                    let child_btn = button(icon::icon(icon::CHILD).size(14))
                        .style(|theme, status| action_style(theme, status, 0))
                        .padding(4)
                        .on_press(Message::MakeChild(task.uid.clone()));
                    actions = actions.push(
                        tooltip(
                            child_btn,
                            text(rust_i18n::t!(
                                "yank_tooltip_child",
                                target = t_sum.clone(),
                                yanked = y_sum.clone()
                            ))
                            .size(12),
                            tooltip::Position::Top,
                        )
                        .style(tooltip_style)
                        .delay(Duration::from_millis(700)),
                    );

                    let related_btn =
                        button(icon::icon(random_related_icon(&task.uid, yanked)).size(14))
                            .style(|theme, status| action_style(theme, status, 0))
                            .padding(4)
                            .on_press(Message::AddRelatedTo(task.uid.clone()));
                    actions = actions.push(
                        tooltip(
                            related_btn,
                            text(rust_i18n::t!(
                                "related_to_tooltip",
                                target = t_sum.clone(),
                                yanked = y_sum.clone()
                            ))
                            .size(12),
                            tooltip::Position::Top,
                        )
                        .style(tooltip_style)
                        .delay(Duration::from_millis(700)),
                    );
                } else {
                    let unlink_btn = button(icon::icon(icon::UNLINK).size(14))
                        .style(button::primary)
                        .padding(4)
                        .on_press(Message::EscapePressed);
                    actions = actions.push(
                        tooltip(
                            unlink_btn,
                            text(format!("{} (Esc)", rust_i18n::t!("unlink"))).size(12),
                            tooltip::Position::Top,
                        )
                        .style(tooltip_style)
                        .delay(Duration::from_millis(700)),
                    );
                }
            }

            use crate::config::TaskAction;
            for action in TaskAction::ALL {
                if !app.pinned_actions.contains(action) {
                    continue;
                }
                if !crate::gui::view::is_action_available(action, task, app) {
                    continue;
                }

                let (icon_element, msg, style_mode): (Element<'a, Message>, Message, u8) =
                    match action {
                        TaskAction::CompleteAndShift => (
                            icon::icon(icon::REPEAT).size(14).into(),
                            Message::ToggleTaskShift(task.uid.clone()),
                            0,
                        ),
                        TaskAction::ToggleDetails => {
                            let mut icon_row = row![].spacing(2).align_y(iced::Alignment::Center);
                            if has_info {
                                icon_row =
                                    icon_row.push(icon::icon(icon::INFO).size(14).line_height(1.0));
                            }
                            if has_time {
                                icon_row = icon_row.push(
                                    icon::icon(icon::TIMER_SETTINGS).size(14).line_height(1.0),
                                );
                            }
                            let style = if is_expanded { 3 } else { 0 };
                            (
                                icon_row.into(),
                                Message::ToggleDetails(task.uid.clone()),
                                style,
                            )
                        }
                        TaskAction::ToggleTimer => {
                            if task.status == crate::model::TaskStatus::InProcess {
                                (
                                    icon::icon(icon::PAUSE).size(14).into(),
                                    Message::PauseTask(task.uid.clone()),
                                    0,
                                )
                            } else {
                                (
                                    icon::icon(icon::PLAY).size(14).into(),
                                    Message::StartTask(task.uid.clone()),
                                    0,
                                )
                            }
                        }
                        TaskAction::StopTimer => (
                            icon::icon(icon::DEBUG_STOP).size(14).into(),
                            Message::StopTask(task.uid.clone()),
                            0,
                        ),
                        TaskAction::AddSession => {
                            let style =
                                if app.adding_session_uid.as_deref() == Some(task.uid.as_str()) {
                                    3
                                } else {
                                    0
                                };
                            (
                                icon::icon(icon::TIMER_PLUS).size(14).into(),
                                Message::StartAddSession(task.uid.clone()),
                                style,
                            )
                        }
                        TaskAction::IncreasePriority => (
                            icon::icon(icon::PLUS).size(14).into(),
                            Message::ChangePriority(index, 1),
                            0,
                        ),
                        TaskAction::DecreasePriority => (
                            icon::icon(icon::MINUS).size(14).into(),
                            Message::ChangePriority(index, -1),
                            0,
                        ),
                        TaskAction::Focus => (
                            icon::icon(app.focus_icon).size(14).into(),
                            Message::FocusSelected,
                            0,
                        ),
                        TaskAction::Edit => (
                            icon::icon(icon::EDIT).size(14).into(),
                            Message::EditTaskStart(index),
                            0,
                        ),
                        TaskAction::Yank => (
                            icon::icon(icon::LINK).size(14).into(),
                            Message::YankTask(task.uid.clone()),
                            0,
                        ),
                        TaskAction::TogglePin => (
                            icon::icon(icon::THUMB_TACK).size(14).into(),
                            Message::TogglePin(task.uid.clone()),
                            0,
                        ),
                        TaskAction::CreateSubtask => (
                            icon::icon(icon::CREATE_CHILD).size(14).into(),
                            Message::StartCreateChild(task.uid.clone()),
                            0,
                        ),
                        TaskAction::DuplicateTree => (
                            icon::icon(icon::CLONE).size(14).into(),
                            Message::DuplicateTask(task.uid.clone()),
                            0,
                        ),
                        TaskAction::CompleteTree => (
                            icon::icon(icon::LIST_CHECK).size(14).into(),
                            Message::CompleteTree(task.uid.clone()),
                            0,
                        ),
                        TaskAction::Promote => (
                            icon::icon(icon::ELEVATOR_UP).size(14).into(),
                            Message::RemoveParent(task.uid.clone()),
                            0,
                        ),
                        TaskAction::Move => (
                            icon::icon(icon::MOVE).size(14).into(),
                            Message::StartMoveTask(task.uid.clone()),
                            0,
                        ),
                        TaskAction::Cancel => (
                            icon::icon(icon::CROSS).size(14).into(),
                            Message::SetTaskStatus(index, crate::model::TaskStatus::Cancelled),
                            1,
                        ),
                        TaskAction::Delete => (
                            icon::icon(icon::TRASH).size(14).into(),
                            Message::DeleteTask(index),
                            1,
                        ),
                        TaskAction::DeleteTree => (
                            icon::icon(icon::TRASH).size(14).into(),
                            Message::DeleteTaskTree(task.uid.clone()),
                            2,
                        ),
                        TaskAction::OpenCoordinates => (
                            icon::icon(icon::MAP_LOCATION_DOT).size(14).into(),
                            Message::OpenCoordinates(task.uid.clone()),
                            0,
                        ),
                        TaskAction::OpenLocations => (
                            icon::icon(icon::MAP_MARKER_MULTIPLE).size(14).into(),
                            Message::OpenLocations(task.uid.clone()),
                            0,
                        ),
                        TaskAction::OpenUrl => (
                            icon::icon(icon::URL_CHECK).size(14).into(),
                            Message::OpenUrl(task.url.clone().unwrap()),
                            0,
                        ),
                        TaskAction::EditTree => (
                            icon::icon(icon::EDIT_TREE).size(14).into(),
                            Message::EditTaskTree(task.uid.clone()),
                            0,
                        ),
                    };

                let style_mode_mapped = if style_mode == 2 { 1 } else { style_mode };
                let btn = button(icon_element)
                    .style(move |theme, status| action_style(theme, status, style_mode_mapped))
                    .padding(4)
                    .on_press(msg);

                let mut label = action.label();
                if *action == TaskAction::DuplicateTree && !has_subtasks {
                    label = rust_i18n::t!("duplicate_single_task").to_string();
                }
                if *action == TaskAction::ToggleDetails {
                    label = if has_info && has_time {
                        format!(
                            "{} / {}",
                            rust_i18n::t!("show_details"),
                            rust_i18n::t!("help_metadata_manage_sessions")
                        )
                    } else if has_time {
                        rust_i18n::t!("help_metadata_manage_sessions").to_string()
                    } else {
                        rust_i18n::t!("show_details").to_string()
                    };
                } else if *action == TaskAction::ToggleTimer {
                    label = if task.status == crate::model::TaskStatus::InProcess {
                        rust_i18n::t!("pause_task").to_string()
                    } else if is_paused {
                        rust_i18n::t!("resume_task").to_string()
                    } else {
                        rust_i18n::t!("start_task").to_string()
                    };
                }

                let shortcut = match *action {
                    TaskAction::CompleteAndShift => " (Shift+Space)",
                    TaskAction::Focus => " (f)",
                    TaskAction::ToggleDetails => " (L)",
                    TaskAction::ToggleTimer => " (s)",
                    TaskAction::StopTimer => " (S)",
                    TaskAction::AddSession => " (t)",
                    TaskAction::IncreasePriority => " (+)",
                    TaskAction::DecreasePriority => " (-)",
                    TaskAction::Edit => " (e)",
                    TaskAction::EditTree => " (Ctrl+E)",
                    TaskAction::Yank => " (y)",
                    TaskAction::CreateSubtask => " (C)",
                    TaskAction::DuplicateTree => " (Ctrl+D)",
                    TaskAction::CompleteTree => " (Shift+Space)",
                    TaskAction::Promote => " (<)",
                    TaskAction::Move => " (M)",
                    TaskAction::Cancel => " (x)",
                    TaskAction::Delete => " (Del)",
                    TaskAction::DeleteTree => " (Ctrl+Del)",
                    TaskAction::OpenCoordinates => " (g)",
                    TaskAction::OpenUrl => " (o)",
                    _ => "",
                };
                label.push_str(shortcut);

                actions = actions.push(
                    tooltip(btn, text(label).size(12), tooltip::Position::Top)
                        .style(crate::gui::view::tooltip_style)
                        .delay(std::time::Duration::from_millis(700)),
                );
            }

            let is_context_menu_active =
                app.active_context_menu.as_ref().map(|(u, _, _)| u) == Some(&task.uid);
            let ellipsis_btn = button(icon::icon(icon::ELLIPSIS).size(14))
                .padding(4)
                .style(move |theme, status| {
                    if is_context_menu_active {
                        action_style(theme, status, 3)
                    } else {
                        action_style(theme, status, 0)
                    }
                })
                .on_press(Message::OpenContextMenu(task.uid.clone(), false));
            actions = actions.push(ellipsis_btn);

            // Restore the Native Checkbox
            let (icon_char, mut bg_color, mut default_border_color) = if is_paused {
                (
                    icon::PAUSE,
                    Color::from_rgb(0.9, 0.7, 0.2),
                    Color::from_rgb(0.6, 0.5, 0.2),
                )
            } else {
                match task.status {
                    crate::model::TaskStatus::InProcess => (
                        icon::PLAY_FA,
                        Color::from_rgb(0.6, 0.8, 0.6),
                        Color::from_rgb(0.4, 0.5, 0.4),
                    ),
                    crate::model::TaskStatus::Cancelled => (
                        icon::CROSS,
                        Color::from_rgb(0.3, 0.2, 0.2),
                        Color::from_rgb(0.5, 0.4, 0.4),
                    ),
                    crate::model::TaskStatus::Completed => (
                        icon::CHECK,
                        Color::from_rgb(0.0, 0.6, 0.0),
                        Color::from_rgb(0.0, 0.8, 0.0),
                    ),
                    _ => (' ', Color::TRANSPARENT, Color::from_rgb(0.5, 0.5, 0.5)),
                }
            };

            bg_color.a *= dim_factor;
            default_border_color.a *= dim_factor;

            let mut custom_border_color = default_border_color;
            if let Some(cal) = app.calendars.iter().find(|c| c.href == task.calendar_href)
                && let Some(hex) = &cal.color
                && let Some((r, g, b)) = crate::color_utils::parse_hex_to_floats(hex)
            {
                custom_border_color = Color::from_rgba(r, g, b, dim_factor);
            }

            let status_btn_element: Element<'a, Message> = if task.is_note {
                let note_bg = Color {
                    a: dim_factor * 0.5,
                    ..custom_border_color
                };
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::Fixed(24.0))
                    .height(Length::Fixed(24.0))
                    .style(move |_| container::Style {
                        background: Some(note_bg.into()),
                        border: iced::Border {
                            color: custom_border_color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            } else {
                let status_btn = button(
                    container(if icon_char != ' ' {
                        icon::icon(icon_char)
                            .size(12)
                            .color(theme.extended_palette().background.base.text)
                    } else {
                        text("").size(12)
                    })
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
                )
                .width(Length::Fixed(24.0))
                .height(Length::Fixed(24.0))
                .padding(0)
                .on_press(Message::ToggleTask(index, true))
                .style(move |_, status| {
                    let base_active = button::Style {
                        background: Some(iced::Background::Color(bg_color)),
                        text_color: Color::WHITE,
                        border: iced::Border {
                            color: custom_border_color,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..button::Style::default()
                    };
                    match status {
                        iced::widget::button::Status::Hovered => button::Style {
                            border: iced::Border {
                                color: if icon_char == ' ' {
                                    custom_border_color
                                } else {
                                    Color::WHITE
                                },
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..base_active
                        },
                        _ => base_active,
                    }
                });

                tooltip(
                    status_btn,
                    text(rust_i18n::t!("tooltip_toggle_space")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700))
                .into()
            };

            let row_main = row![
                indent,
                status_btn_element,
                main_text_col,
                date_and_alarm_section,
                actions
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            let task_button = button(row_main)
                .on_press(Message::TaskClick(index, task.uid.clone()))
                .padding(iced::Padding {
                    top: 2.0,
                    right: 16.0,
                    bottom: 2.0,
                    left: 6.0,
                })
                .style(move |theme: &Theme, status: button::Status| {
                    let palette = theme.extended_palette();
                    if is_selected {
                        return button::Style {
                            background: Some(
                                Color {
                                    a: 0.05,
                                    ..palette.warning.base.color
                                }
                                .into(),
                            ),
                            ..button::Style::default()
                        };
                    }
                    match status {
                        button::Status::Hovered => button::Style {
                            background: Some(
                                Color {
                                    a: 0.03,
                                    ..palette.background.base.text
                                }
                                .into(),
                            ),
                            ..button::Style::default()
                        },
                        _ => button::Style::default(),
                    }
                });

            let mut details_col = column![].spacing(5);

            if has_content_to_show && is_expanded {
                if !task.description.is_empty() {
                    let mut desc_col = column![].spacing(8);
                    for paragraph in task.description.split("\n\n") {
                        if paragraph.trim().is_empty() {
                            continue;
                        }
                        desc_col = desc_col.push(
                            rich_text(parse_inline_markdown(
                                paragraph,
                                Color::from_rgb(0.7, 0.7, 0.7),
                                false,
                            ))
                            .size(14)
                            .on_link_click(|target: String| {
                                if target.starts_with("http://") || target.starts_with("https://") {
                                    Message::OpenUrl(target)
                                } else {
                                    Message::OpenWikiLink(target)
                                }
                            }),
                        );
                    }
                    details_col = details_col.push(desc_col);
                }

                if has_valid_parent {
                    let p_uid = task.parent_uid.as_ref().unwrap();
                    let p_name = app
                        .store
                        .get_summary(p_uid)
                        .unwrap_or_else(|| rust_i18n::t!("unknown_parent").to_string());
                    let remove_parent_btn = button(icon::icon(icon::CROSS).size(10))
                        .style(button::danger)
                        .padding(2)
                        .on_press(Message::RemoveParent(task.uid.clone()));
                    let row = row![
                        text(rust_i18n::t!("parent"))
                            .size(12)
                            .color(Color::from_rgb(0.4, 0.8, 0.4)),
                        button(text(p_name).size(12).color(Color::from_rgb(0.7, 0.7, 0.7)))
                            .style(button::text)
                            .padding(0)
                            .on_press(Message::JumpToTask(p_uid.clone())),
                        tooltip(
                            remove_parent_btn,
                            text(rust_i18n::t!("remove_parent")).size(12),
                            tooltip::Position::Top
                        )
                        .style(crate::gui::view::tooltip_style)
                        .delay(Duration::from_millis(700))
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center);
                    details_col = details_col.push(row);
                }

                if !task.dependencies.is_empty() {
                    details_col = details_col.push(
                        text(rust_i18n::t!("blocked_by"))
                            .size(12)
                            .color(Color::from_rgb(0.8, 0.4, 0.4)),
                    );
                    for dep_uid in &task.dependencies {
                        let name = app
                            .store
                            .get_summary(dep_uid)
                            .unwrap_or_else(|| rust_i18n::t!("unknown_task").to_string());
                        let is_done = app.store.is_task_done(dep_uid).unwrap_or(false);
                        let check = if is_done { "[x]" } else { "[ ]" };
                        let remove_dep_btn = button(icon::icon(icon::CROSS).size(10))
                            .style(button::danger)
                            .padding(2)
                            .on_press(Message::RemoveDependency(task.uid.clone(), dep_uid.clone()));
                        let name_btn = button(
                            text(format!("{} {}", check, name))
                                .size(12)
                                .color(Color::from_rgb(0.6, 0.6, 0.6)),
                        )
                        .style(button::text)
                        .padding(0)
                        .on_press(Message::JumpToTask(dep_uid.clone()));

                        let dep_row = row![
                            name_btn,
                            tooltip(
                                remove_dep_btn,
                                text(rust_i18n::t!("remove_dependency")).size(12),
                                tooltip::Position::Top
                            )
                            .style(crate::gui::view::tooltip_style)
                            .delay(Duration::from_millis(700))
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center);
                        details_col = details_col.push(dep_row);
                    }
                }

                if !task.related_to.is_empty() {
                    details_col = details_col.push(
                        text(rust_i18n::t!("related_to_label"))
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.6, 0.8)),
                    );
                    for related_uid in &task.related_to {
                        let mut name = rust_i18n::t!("unknown_task").to_string();
                        if let Some(rel_task) = app.store.get_task_ref(related_uid) {
                            name = rel_task.summary.clone();
                            if rel_task.status.is_done() {
                                if let Some(comp_date) = rel_task.completion_date() {
                                    let local = comp_date.with_timezone(&chrono::Local);
                                    name =
                                        format!("{} (✓ {})", name, local.format("%Y-%m-%d %H:%M"));
                                } else {
                                    name = format!("{} (✓)", name);
                                }
                            }
                        }
                        let remove_related_btn = button(icon::icon(icon::CROSS).size(10))
                            .style(button::danger)
                            .padding(2)
                            .on_press(Message::RemoveRelatedTo(
                                task.uid.clone(),
                                related_uid.clone(),
                            ));
                        let name_btn =
                            button(text(name).size(12).color(Color::from_rgb(0.7, 0.7, 0.7)))
                                .style(button::text)
                                .padding(0)
                                .on_press(Message::JumpToTask(related_uid.clone()));

                        let related_row = row![
                            icon::icon(random_related_icon(&task.uid, related_uid)).size(12),
                            name_btn,
                            tooltip(
                                remove_related_btn,
                                text(rust_i18n::t!("remove_relation")).size(12),
                                tooltip::Position::Top
                            )
                            .style(crate::gui::view::tooltip_style)
                            .delay(Duration::from_millis(700))
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center);
                        details_col = details_col.push(related_row);
                    }
                }

                let blocking_tasks = app.store.get_tasks_blocking(&task.uid);
                if !blocking_tasks.is_empty() {
                    details_col = details_col.push(
                        text(rust_i18n::t!("blocking_label"))
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.4, 0.8)),
                    );
                    for (blocked_uid, blocked_name) in blocking_tasks {
                        let remove_block_btn = button(icon::icon(icon::UNLINK).size(10))
                            .style(button::danger)
                            .padding(2)
                            .on_press(Message::RemoveDependency(
                                blocked_uid.clone(),
                                task.uid.clone(),
                            ));

                        let name_btn = button(
                            text(blocked_name)
                                .size(12)
                                .color(Color::from_rgb(0.7, 0.7, 0.7)),
                        )
                        .style(button::text)
                        .padding(0)
                        .on_press(Message::JumpToTask(blocked_uid.clone()));

                        let blocking_row = row![
                            icon::icon(icon::HAND_STOP)
                                .size(12)
                                .color(Color::from_rgb(0.5, 0.5, 0.5)),
                            name_btn,
                            tooltip(
                                remove_block_btn,
                                text(rust_i18n::t!("unblock_remove_dependency")).size(12),
                                tooltip::Position::Top
                            )
                            .style(crate::gui::view::tooltip_style)
                            .delay(Duration::from_millis(700))
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center);

                        details_col = details_col.push(blocking_row);
                    }
                }

                let incoming_related = app.store.get_tasks_related_to(&task.uid);
                if !incoming_related.is_empty() {
                    details_col = details_col.push(
                        text(rust_i18n::t!("related_from_label"))
                            .size(12)
                            .color(Color::from_rgb(0.8, 0.6, 0.8)),
                    );
                    for (related_uid, mut related_name) in incoming_related {
                        if let Some(rel_task) = app.store.get_task_ref(&related_uid)
                            && rel_task.status.is_done()
                        {
                            if let Some(comp_date) = rel_task.completion_date() {
                                let local = comp_date.with_timezone(&chrono::Local);
                                related_name = format!(
                                    "{} (✓ {})",
                                    related_name,
                                    local.format("%Y-%m-%d %H:%M")
                                );
                            } else {
                                related_name = format!("{} (✓)", related_name);
                            }
                        }
                        let remove_related_btn = button(icon::icon(icon::CROSS).size(10))
                            .style(button::danger)
                            .padding(2)
                            .on_press(Message::RemoveRelatedTo(
                                related_uid.clone(),
                                task.uid.clone(),
                            ));
                        let name_btn = button(
                            text(related_name)
                                .size(12)
                                .color(Color::from_rgb(0.7, 0.7, 0.7)),
                        )
                        .style(button::text)
                        .padding(0)
                        .on_press(Message::JumpToTask(related_uid.clone()));

                        let related_row = row![
                            icon::icon(random_related_icon(&task.uid, &related_uid)).size(12),
                            name_btn,
                            tooltip(
                                remove_related_btn,
                                text(rust_i18n::t!("remove_relation")).size(12),
                                tooltip::Position::Top
                            )
                            .style(crate::gui::view::tooltip_style)
                            .delay(Duration::from_millis(700))
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center);
                        details_col = details_col.push(related_row);
                    }
                }

                // --- Work Sessions Rendering ---
                if !task.sessions.is_empty()
                    || app.adding_session_uid.as_ref() == Some(&task.uid)
                    || task.time_spent_seconds > 0
                {
                    details_col = details_col.push(Space::new().height(5.0));

                    let now_ts = Utc::now().timestamp();
                    let current_session = task
                        .last_started_at
                        .map(|start| (now_ts - start).max(0) as u64)
                        .unwrap_or(0);
                    let total_seconds = task.time_spent_seconds + current_session;
                    let total_mins = total_seconds / 60;

                    let mut session_header = row![
                        text(t!(
                            "time_tracked_duration",
                            h = total_mins / 60,
                            m = total_mins % 60
                        ))
                        .size(12)
                        .color(Color::from_rgb(0.6, 0.8, 0.6))
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center);

                    // Inline add button inside the expanded view for rapid logging
                    if app.adding_session_uid.as_ref() != Some(&task.uid) {
                        session_header = session_header.push(
                            button(icon::icon(icon::TIMER_PLUS).size(12))
                                .style(|theme, status| action_style(theme, status, 0))
                                .padding(2)
                                .on_press(Message::StartAddSession(task.uid.clone())),
                        );
                    }

                    details_col = details_col.push(session_header);

                    if app.adding_session_uid.as_ref() == Some(&task.uid) {
                        let is_dark_theme = app.theme().extended_palette().is_dark;
                        let input = text_editor(&app.session_input)
                            .id(iced::widget::Id::from(format!(
                                "session_input_{}",
                                task.uid
                            )))
                            .placeholder(format!(
                                "{} {}, 14:00-15:30",
                                t!("eg"),
                                random_session_example()
                            ))
                            .on_action(Message::SessionInputChanged)
                            .height(Length::Fixed(32.0))
                            .highlight_with::<crate::gui::view::syntax::SessionHighlighter>(
                                is_dark_theme,
                                |highlight, _theme| *highlight,
                            )
                            .padding(6);

                        let input_row = row![
                            input,
                            button(icon::icon(icon::CHECK).size(12))
                                .style(button::primary)
                                .padding(5)
                                .on_press(Message::SubmitSession),
                            button(icon::icon(icon::CROSS).size(12))
                                .style(button::danger)
                                .padding(5)
                                .on_press(Message::CancelAddSession)
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center);
                        details_col = details_col.push(input_row);
                    }

                    let show_all = app.show_all_sessions.contains(&task.uid);
                    let visible_count = if show_all {
                        task.sessions.len()
                    } else {
                        std::cmp::min(3, task.sessions.len())
                    };

                    for (idx, session) in task.sessions.iter().enumerate().rev().take(visible_count)
                    {
                        let s_dt = chrono::DateTime::from_timestamp(session.start, 0)
                            .unwrap_or_default()
                            .with_timezone(&chrono::Local);
                        let e_dt = chrono::DateTime::from_timestamp(session.end, 0)
                            .unwrap_or_default()
                            .with_timezone(&chrono::Local);
                        let dur = (session.end - session.start) / 60;

                        let date_str = s_dt.format("%Y-%m-%d").to_string();
                        let time_str = format!("{}-{}", s_dt.format("%H:%M"), e_dt.format("%H:%M"));

                        let edit_btn = button(
                            icon::icon(icon::EDIT)
                                .size(10)
                                .color(Color::from_rgb(0.5, 0.5, 0.8)),
                        )
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::StartEditSession(task.uid.clone(), idx));

                        let del_btn = button(
                            icon::icon(icon::CROSS)
                                .size(10)
                                .color(Color::from_rgb(0.8, 0.2, 0.2)),
                        )
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::DeleteSession(task.uid.clone(), idx));

                        details_col = details_col.push(
                            row![
                                text(format!("{} {}", date_str, time_str))
                                    .size(12)
                                    .color(Color::from_rgb(0.7, 0.7, 0.7)),
                                Space::new().width(Length::Fixed(6.0)),
                                text(format!(
                                    "({})",
                                    crate::model::parser::format_duration_human(dur as u32)
                                ))
                                .size(12)
                                .color(Color::from_rgb(0.5, 0.5, 0.5)),
                                Space::new().width(Length::Fixed(10.0)),
                                edit_btn,
                                Space::new().width(Length::Fixed(8.0)),
                                del_btn
                            ]
                            .spacing(0)
                            .align_y(iced::Alignment::Center),
                        );
                    }

                    if task.sessions.len() > 3 {
                        let toggle_text = if show_all {
                            t!("show_less").to_string()
                        } else {
                            let count = task.sessions.len() - 3;
                            if count == 1 {
                                t!("show_older_sessions.one").to_string()
                            } else {
                                t!("show_older_sessions.other", count = count).to_string()
                            }
                        };
                        details_col = details_col.push(
                            button(
                                text(toggle_text)
                                    .size(10)
                                    .color(Color::from_rgb(0.5, 0.5, 0.8)),
                            )
                            .style(button::text)
                            .padding(0)
                            .on_press(Message::ToggleShowAllSessions(task.uid.clone())),
                        );
                    }
                }

                let effective_goal = task.get_effective_goal();
                let has_rrule = task.rrule.is_some();
                let has_goal = task.goal.is_some();

                if has_rrule || has_goal {
                    details_col = details_col.push(Space::new().height(5.0));
                    details_col = details_col.push(
                        text(rust_i18n::t!("habit_history"))
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.8, 0.8)),
                    );

                    if let Some(rrule) = &task.rrule {
                        let (count, _, key) =
                            app.store.get_completion_history_stats(&task.uid, rrule);
                        if count > 0 {
                            let window_str = rust_i18n::t!(key).to_string();
                            let text_stat = if count == 1 {
                                rust_i18n::t!("habit_completed_in_past.one", window = window_str)
                                    .to_string()
                            } else {
                                rust_i18n::t!(
                                    "habit_completed_in_past.other",
                                    count = count,
                                    window = window_str
                                )
                                .to_string()
                            };
                            details_col = details_col.push(
                                text(format!("• {}", text_stat))
                                    .size(12)
                                    .color(Color::from_rgb(0.7, 0.7, 0.7)),
                            );
                        }
                    }

                    if let Some(goal) = &task.goal {
                        let progress = app
                            .store
                            .calculate_goal_progress(&format!("task:{}", task.uid), goal);
                        let (cur_str, tar_str) =
                            if goal.goal_type == crate::config::GoalType::Duration {
                                crate::model::parser::format_goal_duration(progress, goal.target)
                            } else {
                                (progress.to_string(), goal.target.to_string())
                            };

                        details_col = details_col.push(
                            text(format!(
                                "- Target: {}",
                                goal.format_target_display(&tar_str)
                            ))
                            .size(12)
                            .color(Color::from_rgb(0.7, 0.7, 0.7)),
                        );
                        details_col = details_col.push(
                            text(format!("- Progress: {}", cur_str))
                                .size(12)
                                .color(Color::from_rgb(0.7, 0.7, 0.7)),
                        );
                    }

                    if let Some(goal) = &effective_goal {
                        let history = app.store.calculate_goal_history(
                            &format!("task:{}", task.uid),
                            goal,
                            7,
                        );
                        let mut heatmap_str = String::new();
                        for pct in history {
                            if pct >= 1.0 {
                                heatmap_str.push('🟩');
                            } else if pct > 0.0 {
                                heatmap_str.push('🟨');
                            } else {
                                heatmap_str.push('⬛');
                            }
                        }

                        details_col = details_col.push(
                            text(format!("- Past: {}", heatmap_str))
                                .size(12)
                                .color(Color::from_rgb(0.7, 0.7, 0.7)),
                        );
                    }
                }

                let mut date_infos = Vec::new();
                let created_opt = task.created_date();
                let modified_opt = task.last_modified_date();

                if let Some(created) = created_opt {
                    let local = created.with_timezone(&chrono::Local);
                    date_infos.push(format!(
                        "{}: {}",
                        rust_i18n::t!("created_label"),
                        local.format("%Y-%m-%d %H:%M")
                    ));
                }
                if let Some(modified) = modified_opt
                    && created_opt != Some(modified)
                {
                    let local = modified.with_timezone(&chrono::Local);
                    date_infos.push(format!(
                        "{}: {}",
                        rust_i18n::t!("last_modified_label"),
                        local.format("%Y-%m-%d %H:%M")
                    ));
                }
                if !date_infos.is_empty() {
                    details_col = details_col.push(
                        text(date_infos.join("  |  "))
                            .size(12)
                            .color(Color::from_rgb(0.5, 0.5, 0.5)),
                    );
                }
            }

            let col_content = if is_expanded && has_content_to_show {
                let desc_row = container(row![
                    Space::new().width(Length::Fixed(indent_size as f32 + 30.0)),
                    details_col
                ]).padding(iced::Padding {
                    bottom: 5.0,
                    ..Default::default()
                });

                column![task_button, desc_row].spacing(5)
            } else {
                let mut base_col = column![task_button];
                if app.show_inline_descriptions && !task.description.is_empty() && !is_expanded {
                    let mut desc_lines = Vec::new();
                    let mut line_count = 0;
                    for line in task.description.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        desc_lines.push(line.to_string());
                        line_count += 1;
                        if line_count >= 3 {
                            break;
                        }
                    }
                    if !desc_lines.is_empty() {
                        let inline_txt = desc_lines.join("\n");
                        let inline_desc = row![
                            Space::new().width(Length::Fixed(indent_size as f32 + 34.0)),
                            rich_text(parse_inline_markdown(
                                &inline_txt,
                                Color::from_rgb(0.6, 0.6, 0.6),
                                false,
                            ))
                            .size(14)
                            .on_link_click(|target: String| {
                                if target.starts_with("http://") || target.starts_with("https://") {
                                    Message::OpenUrl(target)
                                } else {
                                    Message::OpenWikiLink(target)
                                }
                            })
                        ];
                        base_col = base_col.push(inline_desc).spacing(2);
                    }
                }
                base_col
            };

            let container_content: Element<'a, Message> = iced::widget::MouseArea::new(col_content)
                .on_right_press(Message::OpenContextMenu(task.uid.clone(), true))
                .into();

            focusable(container_content)
                .id(row_id)
                .into()
        }
    }
}
