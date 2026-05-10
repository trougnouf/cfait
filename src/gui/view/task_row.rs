// File: ./src/gui/view/task_row.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! GUI view component for rendering individual task rows.
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::focusable::focusable;
use iced::widget::{MouseArea, Space, button, column, container, responsive, row, text, tooltip};
use iced::{Color, Element, Length, Theme};
use std::collections::HashSet;

// Helper inside the file to provide generic action styles
pub fn action_style(theme: &Theme, status: button::Status, style_mode: u8) -> button::Style {
    let palette = theme.extended_palette();
    let base = button::Style {
        background: Some(Color::TRANSPARENT.into()),
        text_color: if style_mode == 1 {
            palette.danger.base.color
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
            } else {
                palette.background.strong.color.into()
            }),
            text_color: if style_mode == 1 {
                palette.danger.base.text
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
                icon::icon(icon::ARROW_EXPAND_DOWN)
                    .size(16)
                    .color(Color::from_rgb(0.5, 0.5, 0.8)),
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
                icon::icon(icon::ARROW_EXPAND_UP)
                    .size(16)
                    .color(Color::from_rgb(0.5, 0.5, 0.8)),
            )
            .style(iced::widget::button::text)
            .width(Length::Fill)
            .on_press(Message::ToggleDoneGroup(key.clone()));
            focusable(row![indent, btn]).id(row_id).into()
        }
        crate::store::TaskListItem::Task(task) => {
            let theme = app.theme();
            let is_dark_theme = theme.extended_palette().is_dark;
            let is_selected = app.selected_uid.as_ref() == Some(&task.uid);
            let show_indent = app.active_cal_href.is_some();
            let indent_size = if show_indent { task.depth * 12 } else { 0 };
            let is_tree_collapsed = app.session.collapsed_trees.contains(&task.uid);

            let (parent_tags, parent_location) =
                if show_indent && let Some(p_uid) = &task.parent_uid {
                    if let Some(cached) = app.parent_attributes_cache.get(p_uid) {
                        (cached.0.clone(), cached.1.clone())
                    } else {
                        (HashSet::new(), None)
                    }
                } else {
                    (HashSet::new(), None)
                };

            let (visible_tags, visible_location) =
                task.resolve_visual_attributes(&parent_tags, &parent_location, &app.tag_aliases);

            let is_done = task.status.is_done();
            let is_trash = task.calendar_href == "local://trash";
            let is_blocked = task.is_blocked;

            let title_color = if is_done || is_trash {
                if is_dark_theme {
                    Color::from_rgb(0.53, 0.53, 0.53)
                } else {
                    Color::from_rgb(0.63, 0.63, 0.63)
                }
            } else if is_blocked {
                if is_dark_theme {
                    Color::from_rgb(0.47, 0.47, 0.47)
                } else {
                    Color::from_rgb(0.5, 0.5, 0.5)
                }
            } else if task.priority > 0 {
                let (r, g, b) = crate::color_utils::get_priority_rgb(task.priority, is_dark_theme);
                Color::from_rgb(r, g, b)
            } else {
                theme.extended_palette().background.base.text
            };

            let is_paused = task.status == crate::model::TaskStatus::NeedsAction
                && ((task.percent_complete.unwrap_or(0) > 0
                    && task.percent_complete.unwrap_or(0) < 100)
                    || task.time_spent_seconds > 0
                    || !task.sessions.is_empty());

            let now = chrono::Utc::now();
            let mut date_badge = None;
            let mut date_color = if is_dark_theme {
                Color::from_rgb(0.67, 0.67, 0.67)
            } else {
                Color::from_rgb(0.4, 0.4, 0.4)
            };
            let mut date_icon = icon::CALENDAR;

            if is_done {
                if let Some(done_dt) = task.completion_date() {
                    let local = done_dt.with_timezone(&chrono::Local);
                    date_badge = Some(local.format("%Y-%m-%d %H:%M").to_string());
                    date_icon = if task.status == crate::model::TaskStatus::Completed {
                        icon::CALENDAR_CHECK
                    } else {
                        icon::CALENDAR_XMARK
                    };
                    date_color = if task.status == crate::model::TaskStatus::Completed {
                        Color::from_rgb(0.4, 0.73, 0.42) // #66BB6A
                    } else {
                        Color::from_rgb(0.94, 0.33, 0.31) // #EF5350
                    };
                }
            } else if let Some(start) = &task
                .dtstart
                .as_ref()
                .filter(|s| s.to_start_comparison_time() > now)
            {
                let start_str = start.format_smart();
                date_icon = icon::HOURGLASS_START;
                if let Some(due) = &task.due {
                    if start_str == due.format_smart() {
                        date_badge = Some(start_str);
                    } else {
                        date_badge = Some(format!("{} - {}", start_str, due.format_smart()));
                    }
                } else {
                    date_badge = Some(start_str);
                }
            } else if let Some(d) = &task.due {
                let is_overdue = d.to_comparison_time() < now;
                date_badge = Some(d.format_smart());
                date_icon = icon::HOURGLASS_END;
                if is_overdue {
                    date_color = Color::from_rgb(0.94, 0.33, 0.31); // #EF5350
                }
            }

            let has_active_alarm = task.alarms.iter().any(|a| a.acknowledged.is_none());

            let now_ts = now.timestamp();
            let current_session = task
                .last_started_at
                .map(|s| (now_ts - s).max(0) as u64)
                .unwrap_or(0);
            let total_mins = (task.time_spent_seconds + current_session) / 60;

            let est_str = if let Some(min) = task.estimated_duration {
                if let Some(max) = task.estimated_duration_max.filter(|m| *m > min) {
                    format!(
                        "~{}-{}",
                        crate::model::parser::format_duration_compact(min),
                        crate::model::parser::format_duration_compact(max)
                    )
                } else {
                    format!("~{}", crate::model::parser::format_duration_compact(min))
                }
            } else {
                "".to_string()
            };

            let time_str = if total_mins > 0 || task.last_started_at.is_some() {
                if !est_str.is_empty() {
                    format!(
                        "{} / {}",
                        crate::model::parser::format_duration_compact(total_mins as u32),
                        est_str
                    )
                } else {
                    crate::model::parser::format_duration_compact(total_mins as u32).to_string()
                }
            } else {
                est_str
            };

            let pc_str = if !is_done && task.percent_complete.unwrap_or(0) > 0 {
                format!("{}%", task.percent_complete.unwrap())
            } else {
                "".to_string()
            };

            let duration_badge = if !pc_str.is_empty() && !time_str.is_empty() {
                Some(format!("{} | {}", pc_str, time_str))
            } else if !pc_str.is_empty() {
                Some(pc_str)
            } else if !time_str.is_empty() {
                Some(time_str)
            } else {
                None
            };

            let dur_color = if task.last_started_at.is_some() {
                Color::from_rgb(0.4, 0.73, 0.42) // #66BB6A
            } else {
                if is_dark_theme {
                    Color::from_rgb(0.67, 0.67, 0.67)
                } else {
                    Color::from_rgb(0.4, 0.4, 0.4)
                }
            };

            let closure_theme = theme.clone();
            let summary_text = task.summary.clone();
            
            let main_text_col = responsive(move |_size| {
                let mut badges = row![].spacing(6).align_y(iced::Alignment::Center);
                let mut has_badges = false;

                if is_blocked {
                    has_badges = true;
                    badges = badges.push(
                        container(
                            text(rust_i18n::t!("blocked"))
                                .size(12)
                                .color(closure_theme.extended_palette().background.base.text),
                        )
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
                if has_active_alarm {
                    has_badges = true;
                    badges = badges.push(
                        icon::icon(icon::BELL)
                            .size(12)
                            .color(Color::from_rgb(1.0, 0.4, 0.0)),
                    );
                }
                if let Some(date_text) = &date_badge {
                    has_badges = true;
                    badges = badges.push(
                        row![
                            icon::icon(date_icon).size(12).color(date_color),
                            text(date_text.clone()).size(12).color(date_color)
                        ]
                        .spacing(3)
                        .align_y(iced::Alignment::Center),
                    );
                }
                if let Some(dur_text) = &duration_badge {
                    has_badges = true;
                    badges = badges.push(
                        container(
                            text(dur_text.clone())
                                .size(10)
                                .color(closure_theme.extended_palette().background.base.text),
                        )
                        .style(move |_| container::Style {
                            background: Some(dur_color.into()),
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .padding(3),
                    );
                }
                if let Some(loc) = &visible_location {
                    has_badges = true;
                    badges = badges.push(
                        button(text(format!("@{}", loc)).size(12).color(Color::WHITE))
                            .style(move |_, _| button::Style {
                                background: Some(crate::gui::view::COLOR_LOCATION.into()),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..button::Style::default()
                            })
                            .padding(3)
                            .on_press(Message::JumpToLocation(loc.clone())),
                    );
                }
                for tag in &visible_tags {
                    has_badges = true;
                    let (r, g, b) = crate::color_utils::generate_color(tag);
                    let bg = Color::from_rgb(r, g, b);
                    let tc = if crate::color_utils::is_dark(r, g, b) { Color::WHITE } else { Color::BLACK };
                    badges = badges.push(
                        button(text(format!("#{}", tag)).size(12).color(tc))
                            .style(move |_, _| button::Style {
                                background: Some(bg.into()),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..button::Style::default()
                            })
                            .padding(3)
                            .on_press(Message::JumpToTag(tag.clone())),
                    );
                }

                let summary = text(summary_text.clone())
                    .size(20)
                    .color(title_color)
                    .width(Length::Fill)
                    .wrapping(iced::widget::text::Wrapping::Word);
                
                if has_badges {
                    column![summary, badges].spacing(2).into()
                } else {
                    column![summary].into()
                }
            })
            .width(Length::Fill)
            .height(Length::Shrink);

            let mut actions = row![].spacing(3).align_y(iced::Alignment::Center);

            let has_subtasks = task.has_visible_subtasks;
            let has_notes_or_deps = !task.description.is_empty()
                || !task.dependencies.is_empty()
                || !task.related_to.is_empty();

            if has_subtasks || is_tree_collapsed {
                let (icon_char, tooltip_text) = if is_tree_collapsed {
                    (
                        icon::FAMILY_TREE,
                        rust_i18n::t!("expand_tree_with_key").to_string(),
                    )
                } else {
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
                    (
                        trees[(hash % 5) as usize],
                        rust_i18n::t!("collapse_tree_with_key").to_string(),
                    )
                };

                let collapse_btn = button(icon::icon(icon_char).size(14))
                    .style(|theme, status| action_style(theme, status, 0))
                    .padding(4)
                    .on_press(Message::ToggleTreeCollapse(task.uid.clone()));
                actions = actions.push(
                    tooltip(
                        collapse_btn,
                        text(tooltip_text).size(12),
                        tooltip::Position::Top,
                    )
                    .style(crate::gui::view::tooltip_style)
                    .delay(std::time::Duration::from_millis(700)),
                );
            }

            use crate::config::TaskAction;
            for action in TaskAction::ALL {
                if !app.pinned_actions.contains(action) {
                    continue;
                }
                let is_done_or_cancelled =
                    is_done || task.status == crate::model::TaskStatus::Cancelled;

                if *action == TaskAction::OpenUrl && task.url.is_none() {
                    continue;
                }
                if *action == TaskAction::ToggleDetails
                    && !(has_notes_or_deps
                        || task.time_spent_seconds > 0
                        || !task.sessions.is_empty())
                {
                    continue;
                }
                if *action == TaskAction::DeleteTree && !has_subtasks {
                    continue;
                }
                if *action == TaskAction::OpenCoordinates && task.geo.is_none() {
                    continue;
                }
                if *action == TaskAction::OpenLocations
                    && app.store.count_tree_locations(&task.uid) <= 1
                {
                    continue;
                }
                if *action == TaskAction::Promote && task.parent_uid.is_none() {
                    continue;
                }
                if *action == TaskAction::Yank && app.yanked_uid.is_some() {
                    continue;
                }
                if *action == TaskAction::StopTimer
                    && !(task.status == crate::model::TaskStatus::InProcess || is_paused)
                {
                    continue;
                }
                if (*action == TaskAction::ToggleTimer
                    || *action == TaskAction::AddSession
                    || *action == TaskAction::Cancel)
                    && is_done_or_cancelled
                {
                    continue;
                }

                let mut label = action.label();
                if *action == TaskAction::DuplicateTree && !has_subtasks {
                    label = rust_i18n::t!("duplicate_single_task").to_string();
                }

                let (icon_element, msg, style_mode, tooltip_str): (
                    Element<'_, Message>,
                    Message,
                    u8,
                    String,
                ) = match action {
                    TaskAction::ToggleDetails => (
                        icon::icon(icon::INFO).size(14).into(),
                        Message::ToggleDetails(task.uid.clone()),
                        0,
                        rust_i18n::t!("show_details").to_string(),
                    ),
                    TaskAction::ToggleTimer => {
                        if task.status == crate::model::TaskStatus::InProcess {
                            (
                                icon::icon(icon::PAUSE).size(14).into(),
                                Message::PauseTask(task.uid.clone()),
                                0,
                                rust_i18n::t!("pause_task").to_string(),
                            )
                        } else if is_paused {
                            (
                                icon::icon(icon::PLAY).size(14).into(),
                                Message::StartTask(task.uid.clone()),
                                0,
                                rust_i18n::t!("resume_task").to_string(),
                            )
                        } else {
                            (
                                icon::icon(icon::PLAY).size(14).into(),
                                Message::StartTask(task.uid.clone()),
                                0,
                                rust_i18n::t!("start_task").to_string(),
                            )
                        }
                    }
                    TaskAction::StopTimer => (
                        icon::icon(icon::DEBUG_STOP).size(14).into(),
                        Message::StopTask(task.uid.clone()),
                        0,
                        rust_i18n::t!("stop_reset").to_string(),
                    ),
                    TaskAction::AddSession => (
                        icon::icon(icon::TIMER_PLUS).size(14).into(),
                        Message::StartAddSession(task.uid.clone()),
                        0,
                        rust_i18n::t!("help_metadata_log_time").to_string(),
                    ),
                    TaskAction::IncreasePriority => (
                        icon::icon(icon::PLUS).size(14).into(),
                        Message::ChangePriority(index, 1),
                        0,
                        rust_i18n::t!("increase_priority").to_string(),
                    ),
                    TaskAction::DecreasePriority => (
                        icon::icon(icon::MINUS).size(14).into(),
                        Message::ChangePriority(index, -1),
                        0,
                        rust_i18n::t!("menu_decrease_prio").to_string(),
                    ),
                    TaskAction::Edit => (
                        icon::icon(icon::EDIT).size(14).into(),
                        Message::EditTaskStart(index),
                        0,
                        rust_i18n::t!("edit").to_string(),
                    ),
                    TaskAction::Yank => (
                        icon::icon(icon::LINK).size(14).into(),
                        Message::YankTask(task.uid.clone()),
                        0,
                        rust_i18n::t!("yank_copy_id").to_string(),
                    ),
                    TaskAction::CreateSubtask => (
                        icon::icon(icon::CREATE_CHILD).size(14).into(),
                        Message::StartCreateChild(task.uid.clone()),
                        0,
                        rust_i18n::t!("create_subtask").to_string(),
                    ),
                    TaskAction::DuplicateTree => (
                        icon::icon(icon::CLONE).size(14).into(),
                        Message::DuplicateTask(task.uid.clone()),
                        0,
                        label.clone(),
                    ),
                    TaskAction::Promote => (
                        icon::icon(icon::ELEVATOR_UP).size(14).into(),
                        Message::RemoveParent(task.uid.clone()),
                        0,
                        rust_i18n::t!("promote_remove_parent").to_string(),
                    ),
                    TaskAction::Move => (
                        icon::icon(icon::MOVE).size(14).into(),
                        Message::StartMoveTask(task.uid.clone()),
                        0,
                        rust_i18n::t!("menu_move").to_string(),
                    ),
                    TaskAction::Cancel => (
                        icon::icon(icon::CROSS).size(14).into(),
                        Message::SetTaskStatus(index, crate::model::TaskStatus::Cancelled),
                        1,
                        rust_i18n::t!("cancel").to_string(),
                    ),
                    TaskAction::Delete => (
                        icon::icon(icon::TRASH).size(14).into(),
                        Message::DeleteTask(index),
                        1,
                        rust_i18n::t!("delete").to_string(),
                    ),
                    TaskAction::DeleteTree => (
                        icon::icon(icon::TRASH).size(14).into(),
                        Message::DeleteTaskTree(task.uid.clone()),
                        2,
                        rust_i18n::t!("delete_task_tree").to_string(),
                    ),
                    TaskAction::OpenCoordinates => (
                        icon::icon(icon::MAP_LOCATION_DOT).size(14).into(),
                        Message::OpenCoordinates(task.uid.clone()),
                        0,
                        rust_i18n::t!("open_coordinates").to_string(),
                    ),
                    TaskAction::OpenLocations => (
                        icon::icon(icon::MAP_MARKER_MULTIPLE).size(14).into(),
                        Message::OpenLocations(task.uid.clone()),
                        0,
                        rust_i18n::t!("action_open_locations").to_string(),
                    ),
                    TaskAction::OpenUrl => (
                        icon::icon(icon::URL_CHECK).size(14).into(),
                        Message::OpenUrl(task.url.clone().unwrap()),
                        0,
                        rust_i18n::t!("open_url").to_string(),
                    ),
                };

                let btn = button(icon_element)
                    .style(move |theme, status| action_style(theme, status, style_mode))
                    .padding(4)
                    .on_press(msg);
                actions = actions.push(
                    tooltip(btn, text(tooltip_str).size(12), tooltip::Position::Top)
                        .style(crate::gui::view::tooltip_style)
                        .delay(std::time::Duration::from_millis(700)),
                );
            }

            let ellipsis_btn = button(icon::icon(icon::ELLIPSIS).size(14))
                .padding(4)
                .style(|theme, status| action_style(theme, status, 0))
                .on_press(Message::OpenContextMenu(task.uid.clone(), false));
            actions = actions.push(ellipsis_btn);

            // Restore the Native Checkbox
            let (icon_char, bg_color, default_border_color) = if is_paused {
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

            let mut custom_border_color = default_border_color;
            if let Some(cal) = app.calendars.iter().find(|c| c.href == task.calendar_href)
                && let Some(hex) = &cal.color
                && let Some((r, g, b)) = crate::color_utils::parse_hex_to_floats(hex)
            {
                custom_border_color = Color::from_rgb(r, g, b);
            }

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

            let row_main = row![
                Space::new().width(Length::Fixed(indent_size as f32)),
                status_btn,
                main_text_col,
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

            let container_content: Element<'a, Message> = MouseArea::new(task_button)
                .on_right_press(Message::OpenContextMenu(task.uid.clone(), true))
                .into();

            focusable(container(container_content)).id(row_id).into()
        }
    }
}
