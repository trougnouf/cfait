// File: ./src/gui/view/task_row.rs
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::COLOR_LOCATION;
use crate::gui::view::focusable::focusable;
use crate::model::Task as TodoTask;
use crate::model::display::random_related_icon;
use chrono::Utc;
use std::collections::HashSet;
use std::time::Duration;

use super::tooltip_style;
use iced::widget::{
    Space, button, column, container, responsive, rich_text, row, span, text, tooltip,
};
use iced::{Border, Color, Element, Length, Theme};

use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Clipboard, Layout, Shell, mouse};
use iced::{Event, Rectangle, Size, Vector};

pub fn view_task_row<'a>(
    app: &'a GuiApp,
    index: usize,
    task: &'a TodoTask,
    row_id: iced::widget::Id,
) -> Element<'a, Message> {
    use crate::model::VirtualState;

    match &task.virtual_state {
        VirtualState::Expand(key) => {
            let indent_size = if app.active_cal_href.is_some() {
                task.depth * 12
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

            return row![indent, btn].into();
        }
        VirtualState::Collapse(key) => {
            let indent_size = if app.active_cal_href.is_some() {
                task.depth * 12
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

            return row![indent, btn].into();
        }
        _ => {}
    }

    let is_blocked = task.is_blocked;
    let is_selected = app.selected_uid.as_ref() == Some(&task.uid);

    let theme = app.theme();
    let is_dark_theme = theme.extended_palette().is_dark;
    let default_text_color = theme.extended_palette().background.base.text;

    let color = if is_blocked {
        Color::from_rgb(0.5, 0.5, 0.5)
    } else if task.priority == 0 {
        default_text_color
    } else {
        let (r, g, b) = color_utils::get_priority_rgb(task.priority, is_dark_theme);
        Color::from_rgb(r, g, b)
    };

    let show_indent = app.active_cal_href.is_some();
    let indent_size = if show_indent { task.depth * 12 } else { 0 };
    let indent = Space::new().width(Length::Fixed(indent_size as f32));

    let (parent_tags, parent_location) = if show_indent && let Some(p_uid) = &task.parent_uid {
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

    let action_style = |theme: &Theme, status: button::Status| -> button::Style {
        let palette = theme.extended_palette();
        let base = button::Style {
            background: Some(Color::TRANSPARENT.into()),
            text_color: palette.background.weak.text,
            border: Border::default(),
            ..button::Style::default()
        };
        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(palette.background.weak.color.into()),
                text_color: palette.background.weak.text,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(palette.background.strong.color.into()),
                text_color: palette.background.strong.text,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: palette.background.weak.text.scale_alpha(0.3),
                ..base
            },
        }
    };
    let danger_style = |theme: &Theme, status: button::Status| -> button::Style {
        let palette = theme.extended_palette();
        let base = button::Style {
            background: Some(Color::TRANSPARENT.into()),
            text_color: palette.danger.base.color,
            border: Border::default(),
            ..button::Style::default()
        };
        match status {
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(palette.danger.base.color.into()),
                text_color: palette.danger.base.text,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(palette.danger.strong.color.into()),
                text_color: palette.danger.strong.text,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: palette.danger.base.color.scale_alpha(0.3),
                ..base
            },
        }
    };

    let has_active_alarm = task.alarms.iter().any(|a| a.acknowledged.is_none());

    let date_and_alarm_section: Element<'a, Message> = {
        let mut row_content = row![].spacing(3).align_y(iced::Alignment::Center);

        if has_active_alarm {
            let bell_icon = icon::icon(icon::BELL)
                .size(14)
                .color(Color::from_rgb(1.0, 0.4, 0.0));

            row_content = row_content.push(
                tooltip(
                    container(bell_icon).padding(1),
                    text(rust_i18n::t!("active_alarm")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style),
            );
        }

        let now = Utc::now();
        let is_future_start = task
            .dtstart
            .as_ref()
            .map(|start| start.to_start_comparison_time() > now)
            .unwrap_or(false);

        let dim_color = Color::from_rgb(0.7, 0.7, 0.7);

        let is_overdue = if let Some(d) = &task.due {
            !task.status.is_done() && d.to_comparison_time() < now
        } else {
            false
        };

        let due_color = if is_overdue {
            Color::from_rgb(0.8, 0.2, 0.2)
        } else {
            Color::from_rgb(0.5, 0.5, 0.5)
        };

        if task.status.is_done() {
            if let Some(done_dt) = task.completion_date() {
                let local_done = done_dt.with_timezone(&chrono::Local);
                let (done_icon, done_color) = if task.status == crate::model::TaskStatus::Completed
                {
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
                    }
                } else {
                    due.format_smart()
                };

                if start_str == due.format_smart() {
                    row_content = row_content.push(text(start_str).size(14).color(dim_color));
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
            row_content = row_content.push(text(d.format_smart()).size(14).color(due_color));
        }

        container(row_content).width(Length::Shrink).into()
    };

    let has_desc = !task.description.is_empty();
    let has_valid_parent = task.parent_uid.as_ref().is_some_and(|uid| !uid.is_empty());
    let has_deps = !task.dependencies.is_empty();
    let has_related = !task.related_to.is_empty();
    let has_incoming_related = !app.store.get_tasks_related_to(&task.uid).is_empty();

    let has_blocking = !app.store.get_tasks_blocking(&task.uid).is_empty();

    let has_content_to_show = has_desc
        || has_valid_parent
        || has_deps
        || has_related
        || has_incoming_related
        || has_blocking;

    let is_expanded = app.expanded_tasks.contains(&task.uid);

    let mut actions = row![].spacing(3).align_y(iced::Alignment::Center);

    if has_desc || has_deps || has_blocking {
        let info_icon = icon::icon(icon::INFO)
            .size(12)
            .line_height(1.0)
            .align_y(iced::alignment::Vertical::Center);

        let info_btn = button(info_icon)
            .style(if is_expanded {
                button::primary
            } else {
                action_style
            })
            .padding(2)
            .width(Length::Shrink)
            .on_press(Message::ToggleDetails(task.uid.clone()));

        actions = actions.push(
            tooltip(
                info_btn,
                text(rust_i18n::t!("show_details")).size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );
    } else {
        actions = actions.push(Space::new().width(Length::Fixed(16.0)));
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
            let block_btn = button(icon::icon(icon::BLOCKED).size(14))
                .style(action_style)
                .padding(4)
                .on_press(Message::AddDependency(task.uid.clone()));
            actions = actions.push(
                tooltip(
                    block_btn,
                    text(rust_i18n::t!("block_depends_on")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );
            let child_btn = button(icon::icon(icon::CHILD).size(14))
                .style(action_style)
                .padding(4)
                .on_press(Message::MakeChild(task.uid.clone()));
            actions = actions.push(
                tooltip(
                    child_btn,
                    text(rust_i18n::t!("make_child")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );
            let related_btn = button(icon::icon(random_related_icon(&task.uid, yanked)).size(14))
                .style(action_style)
                .padding(4)
                .on_press(Message::AddRelatedTo(task.uid.clone()));
            actions = actions.push(
                tooltip(
                    related_btn,
                    text(rust_i18n::t!("related_to")).size(12),
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
                    text(rust_i18n::t!("unlink")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );
            let create_child_btn = button(icon::icon(icon::CREATE_CHILD).size(14))
                .style(button::primary)
                .padding(4)
                .on_press(Message::StartCreateChild(task.uid.clone()));
            actions = actions.push(
                tooltip(
                    create_child_btn,
                    text(rust_i18n::t!("create_subtask")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );

            if task.parent_uid.is_some() {
                let lift_btn = button(icon::icon(icon::ELEVATOR_UP).size(14))
                    .style(action_style)
                    .padding(4)
                    .on_press(Message::RemoveParent(task.uid.clone()));
                actions = actions.push(
                    tooltip(
                        lift_btn,
                        text(rust_i18n::t!("promote_remove_parent")).size(12),
                        tooltip::Position::Top,
                    )
                    .style(tooltip_style)
                    .delay(Duration::from_millis(700)),
                );
            }
        }
    } else {
        let link_btn = button(icon::icon(icon::LINK).size(14))
            .style(action_style)
            .padding(4)
            .on_press(Message::YankTask(task.uid.clone()));
        actions = actions.push(
            tooltip(
                link_btn,
                text(rust_i18n::t!("yank_copy_id")).size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );
    }

    if task.status != crate::model::TaskStatus::Completed
        && task.status != crate::model::TaskStatus::Cancelled
    {
        let (action_icon, next_action_msg, tooltip_text) =
            if task.status == crate::model::TaskStatus::InProcess {
                (
                    icon::PAUSE,
                    Message::PauseTask(task.uid.clone()),
                    rust_i18n::t!("pause_task").to_string(),
                )
            } else if task.is_paused() {
                (
                    icon::PLAY,
                    Message::StartTask(task.uid.clone()),
                    rust_i18n::t!("resume_task").to_string(),
                )
            } else {
                (
                    icon::PLAY,
                    Message::StartTask(task.uid.clone()),
                    rust_i18n::t!("start_task").to_string(),
                )
            };

        let status_toggle_btn = button(icon::icon(action_icon).size(14))
            .style(action_style)
            .padding(4)
            .on_press(next_action_msg);

        actions = actions.push(
            tooltip(
                status_toggle_btn,
                text(tooltip_text).size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );

        if task.status == crate::model::TaskStatus::InProcess || task.is_paused() {
            let stop_btn = button(icon::icon(icon::DEBUG_STOP).size(14))
                .style(action_style)
                .padding(4)
                .on_press(Message::StopTask(task.uid.clone()));

            actions = actions.push(
                tooltip(
                    stop_btn,
                    text(rust_i18n::t!("stop_reset")).size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );
        }
    }

    let plus_btn = button(icon::icon(icon::PLUS).size(14))
        .style(action_style)
        .padding(4)
        .on_press(Message::ChangePriority(index, 1));
    actions = actions.push(
        tooltip(
            plus_btn,
            text(rust_i18n::t!("increase_priority")).size(12),
            tooltip::Position::Top,
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
    );

    let minus_btn = button(icon::icon(icon::MINUS).size(14))
        .style(action_style)
        .padding(4)
        .on_press(Message::ChangePriority(index, -1));
    actions = actions.push(
        tooltip(
            minus_btn,
            text(rust_i18n::t!("decrease_priority")).size(12),
            tooltip::Position::Top,
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
    );

    let edit_btn = button(icon::icon(icon::EDIT).size(14))
        .style(action_style)
        .padding(4)
        .on_press(Message::EditTaskStart(index));
    actions = actions.push(
        tooltip(
            edit_btn,
            text(rust_i18n::t!("edit")).size(12),
            tooltip::Position::Top,
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
    );

    let delete_btn = button(icon::icon(icon::TRASH).size(14))
        .style(danger_style)
        .padding(4)
        .on_press(Message::DeleteTask(index));
    actions = actions.push(
        tooltip(
            delete_btn,
            text(rust_i18n::t!("delete")).size(12),
            tooltip::Position::Top,
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
    );

    if task.status != crate::model::TaskStatus::Completed
        && task.status != crate::model::TaskStatus::Cancelled
    {
        let cancel_btn = button(icon::icon(icon::CROSS).size(14))
            .style(danger_style)
            .padding(4)
            .on_press(Message::SetTaskStatus(
                index,
                crate::model::TaskStatus::Cancelled,
            ));
        actions = actions.push(
            tooltip(
                cancel_btn,
                text(rust_i18n::t!("cancel")).size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );
    }

    let (icon_char, bg_color, default_border_color) = if task.is_paused() {
        (
            icon::PAUSE,
            Color::from_rgb(0.9, 0.7, 0.2), // Amber
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
            crate::model::TaskStatus::NeedsAction => {
                (' ', Color::TRANSPARENT, Color::from_rgb(0.5, 0.5, 0.5))
            }
        }
    };

    let mut custom_border_color = default_border_color;

    if let Some(cal) = app.calendars.iter().find(|c| c.href == task.calendar_href)
        && let Some(hex) = &cal.color
        && let Some((r, g, b)) = color_utils::parse_hex_to_floats(hex)
    {
        custom_border_color = Color::from_rgb(r, g, b);
    }

    let status_btn = button(
        container(if icon_char != ' ' {
            icon::icon(icon_char)
                .size(12)
                .color(theme.extended_palette().background.base.text)
        } else {
            text("")
                .size(12)
                .color(theme.extended_palette().background.base.text)
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
    .style(move |_theme, status| {
        let palette = _theme.extended_palette();
        let base_active = button::Style {
            background: Some(iced::Background::Color(bg_color)),
            text_color: palette.background.base.text,
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
                        palette.background.base.text
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..base_active
            },
            _ => base_active,
        }
    });

    let has_metadata = !task.categories.is_empty()
        || task.rrule.is_some()
        || is_blocked
        || task.estimated_duration.is_some()
        || task.location.is_some()
        || task.url.is_some()
        || task.geo.is_some()
        || task.time_spent_seconds > 0
        || task.last_started_at.is_some();

    let main_text_col = responsive(move |size| {
        let available_width = size.width;
        let mut tags_width = 0.0;

        if has_metadata {
            if is_blocked {
                tags_width += 65.0;
            }
            for cat in &visible_tags {
                tags_width += (cat.len() as f32 + 1.0) * 7.0 + 10.0;
            }
            if let Some(l) = &visible_location {
                tags_width += (l.len() as f32 * 7.0) + 25.0;
            }
            if task.estimated_duration.is_some()
                || task.time_spent_seconds > 0
                || task.last_started_at.is_some()
            {
                tags_width += 50.0;
            }
            if task.rrule.is_some() {
                tags_width += 30.0;
            }
            if task.url.is_some() {
                tags_width += 20.0;
            }
        }

        let build_tags = || -> Element<'a, Message> {
            let mut tags_row: iced::widget::Row<'_, Message> =
                row![].spacing(3).align_y(iced::Alignment::Center);

            if is_blocked {
                tags_row =
                    tags_row.push(
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

            for cat in &visible_tags {
                let (r, g, b) = color_utils::generate_color(cat);
                let bg_color = Color::from_rgb(r, g, b);

                let text_color = if color_utils::is_dark(r, g, b) {
                    Color::WHITE
                } else {
                    Color::BLACK
                };

                tags_row = tags_row.push(
                    button(text(format!("#{}", cat)).size(12).color(text_color))
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
                        .on_press(Message::JumpToTag(cat.clone())),
                );
            }

            if let Some(loc) = &visible_location {
                let text_color = Color::WHITE;

                let loc_btn = button(text(format!("@@{}", loc)).size(12).color(text_color))
                    .style(move |_theme, status| {
                        let base = button::Style {
                            background: Some(COLOR_LOCATION.into()),
                            text_color,
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..button::Style::default()
                        };
                        match status {
                            button::Status::Hovered | button::Status::Pressed => button::Style {
                                border: iced::Border {
                                    color: Color::BLACK.scale_alpha(0.2),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..base
                            },
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

            if total_mins > 0 || task.estimated_duration.is_some() || task.last_started_at.is_some()
            {
                let fmt_dur = |m: u32| -> String {
                    if m >= 525600 {
                        format!("{}y", m / 525600)
                    } else if m >= 43200 {
                        format!("{}mo", m / 43200)
                    } else if m >= 10080 {
                        format!("{}w", m / 10080)
                    } else if m >= 1440 {
                        format!("{}d", m / 1440)
                    } else if m >= 60 {
                        format!("{}h", m / 60)
                    } else {
                        format!("{}m", m)
                    }
                };

                let est_label = if let Some(min) = task.estimated_duration {
                    if let Some(max) = task.estimated_duration_max {
                        if max > min {
                            format!("~{}-{}", fmt_dur(min), fmt_dur(max))
                        } else {
                            format!("~{}", fmt_dur(min))
                        }
                    } else {
                        format!("~{}", fmt_dur(min))
                    }
                } else {
                    String::new()
                };

                let label = if total_mins > 0 || task.last_started_at.is_some() {
                    if !est_label.is_empty() {
                        format!("{} / {}", fmt_dur(total_mins), est_label)
                    } else {
                        fmt_dur(total_mins)
                    }
                } else {
                    est_label
                };

                let dur_bg = if task.last_started_at.is_some() {
                    Color::from_rgb(0.25, 0.50, 0.25)
                } else {
                    Color::from_rgb(0.50, 0.50, 0.50)
                };
                let dur_border = if task.last_started_at.is_some() {
                    Color::from_rgb(0.25, 0.60, 0.25)
                } else {
                    Color::BLACK.scale_alpha(0.05)
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
                let recurrence_icon = icon::icon(icon::REPEAT)
                    .size(14)
                    .color(Color::from_rgb(0.5, 0.5, 0.5));
                tags_row = tags_row.push(container(recurrence_icon).padding(0));
            }

            if let Some(geo) = &task.geo {
                let geo_target = format!("geo:{}", geo);
                let geo_btn = button(icon::icon(icon::MAP_LOCATION_DOT).size(14))
                    .style(button::text)
                    .padding(0)
                    .on_press(Message::OpenUrl(geo_target));

                tags_row = tags_row.push(
                    tooltip(
                        geo_btn,
                        text(rust_i18n::t!("open_coordinates")).size(12),
                        tooltip::Position::Top,
                    )
                    .style(tooltip_style),
                );
            }

            if let Some(u) = &task.url {
                let url_btn = button(icon::icon(icon::URL_CHECK).size(14))
                    .style(button::text)
                    .padding(0)
                    .on_press(Message::OpenUrl(u.clone()));
                tags_row = tags_row.push(
                    tooltip(
                        url_btn,
                        text(rust_i18n::t!("open_url")).size(12),
                        tooltip::Position::Top,
                    )
                    .style(tooltip_style),
                );
            }

            tags_row.into()
        };

        let title_width_est = task.summary.len() as f32 * 10.0;
        let required_title_space = title_width_est.min(90.0);
        let padding_safety = 5.0;

        let place_inline = if !has_metadata {
            true
        } else {
            (available_width - tags_width - padding_safety) > required_title_space
        };

        let is_done_or_cancelled =
            task.status.is_done() || task.status == crate::model::TaskStatus::Cancelled;
        let title_color = if is_done_or_cancelled {
            Color { a: 0.75, ..color }
        } else {
            color
        };

        let is_trash = task.calendar_href == "local://trash";

        let summary_text: Element<'a, Message> = if (app.strikethrough_completed
            && task.status.is_done())
            || is_trash
        {
            Into::<Element<'a, Message>>::into(
                rich_text![span::<Message, iced::Font>(task.summary.clone()).strikethrough(true)]
                    .size(20)
                    .color(title_color)
                    .width(Length::Fill),
            )
        } else {
            Into::<Element<'a, Message>>::into(
                text(&task.summary)
                    .size(20)
                    .color(title_color)
                    .width(Length::Fill)
                    .wrapping(iced::widget::text::Wrapping::Word),
            )
        };

        if place_inline {
            row![
                summary_text,
                if has_metadata {
                    build_tags()
                } else {
                    Space::new().width(Length::Fixed(0.0)).into()
                }
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            column![
                summary_text,
                if has_metadata {
                    row![Space::new().width(Length::Fill), build_tags()]
                } else {
                    row![]
                }
            ]
            .spacing(2)
            .into()
        }
    })
    .width(Length::Fill)
    .height(Length::Shrink);

    let row_main = row![
        indent,
        status_btn,
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
                    border: Border {
                        color: Color {
                            a: 0.5,
                            ..palette.warning.base.color
                        },
                        width: 1.0,
                        radius: 4.0.into(),
                    },
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
                button::Status::Pressed => button::Style {
                    background: Some(
                        Color {
                            a: 0.05,
                            ..palette.background.base.text
                        }
                        .into(),
                    ),
                    ..button::Style::default()
                },
                _ => button::Style::default(),
            }
        });

    let container_content: Element<'a, Message> = if has_content_to_show {
        if is_expanded {
            let mut details_col = column![].spacing(5);
            if !task.description.is_empty() {
                details_col = details_col.push(
                    text(&task.description)
                        .size(14)
                        .color(Color::from_rgb(0.7, 0.7, 0.7)),
                );
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
                    text(p_name).size(12),
                    tooltip(
                        remove_parent_btn,
                        text(rust_i18n::t!("remove_parent")).size(12),
                        tooltip::Position::Top
                    )
                    .style(tooltip_style)
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
                        .style(tooltip_style)
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
                    let name = app
                        .store
                        .get_summary(related_uid)
                        .unwrap_or_else(|| rust_i18n::t!("unknown_task").to_string());
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
                        .style(tooltip_style)
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
                        .style(tooltip_style)
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
                for (related_uid, related_name) in incoming_related {
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
                        .style(tooltip_style)
                        .delay(Duration::from_millis(700))
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center);
                    details_col = details_col.push(related_row);
                }
            }

            let desc_row = row![
                Space::new().width(Length::Fixed(indent_size as f32 + 30.0)),
                details_col
            ];
            column![task_button, desc_row].spacing(5).into()
        } else {
            column![task_button].into()
        }
    } else {
        NoPointer {
            content: task_button.into(),
        }
        .into()
    };

    focusable(
        container(container_content).padding(if is_expanded && has_content_to_show {
            5
        } else {
            0
        }),
    )
    .id(row_id)
    .into()
}

struct NoPointer<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for NoPointer<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport)
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        )
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }

    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }
}

impl<'a, Message, Theme, Renderer> From<NoPointer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: NoPointer<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}
