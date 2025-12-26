// File: src/gui/view/task_row.rs
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::COLOR_LOCATION; // Import the shared color
use crate::model::Task as TodoTask;
use std::collections::HashSet;
use std::time::Duration;

use super::tooltip_style;
use iced::widget::{Space, button, column, container, responsive, row, text, tooltip};
use iced::{Border, Color, Element, Length, Theme};

// Imports needed for NoPointer and general widget logic
use iced::advanced::Renderer as AdvancedRenderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::{Event, Rectangle, Size, Vector};

pub fn view_task_row<'a>(
    app: &'a GuiApp,
    index: usize,
    task: &'a TodoTask,
) -> Element<'a, Message> {
    let is_blocked = app.store.is_blocked(task);
    let is_selected = app.selected_uid.as_ref() == Some(&task.uid);

    let color = if is_blocked {
        Color::from_rgb(0.5, 0.5, 0.5)
    } else {
        let (r, g, b) = color_utils::get_priority_rgb(task.priority);
        Color::from_rgb(r, g, b)
    };

    let show_indent = app.active_cal_href.is_some() && app.search_value.is_empty();
    let indent_size = if show_indent { task.depth * 12 } else { 0 };
    let indent = Space::new().width(Length::Fixed(indent_size as f32));

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
                text_color: Color::WHITE,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(palette.background.strong.color.into()),
                text_color: Color::WHITE,
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
                text_color: Color::WHITE,
                border: Border {
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(palette.danger.strong.color.into()),
                text_color: Color::WHITE,
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

    let date_text: Element<'a, Message> = match &task.due {
        Some(d) => container(
            // Use reference
            text(d.format_smart()) // Use format_smart
                .size(14) // Use format_smart
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        )
        .width(Length::Fixed(80.0))
        .into(),
        None => Space::new().width(Length::Fixed(0.0)).into(),
    };

    let has_desc = !task.description.is_empty();
    let has_valid_parent = task.parent_uid.as_ref().is_some_and(|uid| !uid.is_empty());
    let has_deps = !task.dependencies.is_empty();

    // Determine if we should show the hand cursor (has content details)
    let has_content_to_show = has_desc || has_valid_parent || has_deps;

    let is_expanded = app.expanded_tasks.contains(&task.uid);

    let mut actions = row![].spacing(3).align_y(iced::Alignment::Center);

    if has_desc || has_deps {
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
            // Reduced padding and switched width to Shrink for tighter layout
            .padding(2)
            .width(Length::Shrink)
            .on_press(Message::ToggleDetails(task.uid.clone()));
        // Apply tooltip_style
        actions = actions.push(
            tooltip(
                info_btn,
                text("Show details").size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );
    } else {
        // Reduced spacer size to match visual footprint of smaller button
        actions = actions.push(Space::new().width(Length::Fixed(16.0)));
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
                    text("Block (depends on)").size(12),
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
                    text("Make child").size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );
        } else {
            let unlink_btn = button(icon::icon(icon::UNLINK).size(14))
                .style(button::primary)
                .padding(4)
                .on_press(Message::ClearYank);
            actions = actions.push(
                tooltip(unlink_btn, text("Unlink").size(12), tooltip::Position::Top)
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
                    text("Create subtask").size(12),
                    tooltip::Position::Top,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700)),
            );

            // ELEVATOR UP (Moved here as requested)
            if task.parent_uid.is_some() {
                let lift_btn = button(icon::icon(icon::ELEVATOR_UP).size(14))
                    .style(action_style)
                    .padding(4)
                    .on_press(Message::RemoveParent(task.uid.clone()));
                actions = actions.push(
                    tooltip(
                        lift_btn,
                        text("Promote (remove parent)").size(12),
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
                text("Yank (copy ID)").size(12),
                tooltip::Position::Top,
            )
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
        );
    }

    if task.status != crate::model::TaskStatus::Completed
        && task.status != crate::model::TaskStatus::Cancelled
    {
        // 1. Play / Pause / Resume Button
        let (action_icon, next_action_msg, tooltip_text) =
            if task.status == crate::model::TaskStatus::InProcess {
                // It is running -> Show Pause
                (
                    icon::PAUSE,
                    Message::PauseTask(task.uid.clone()),
                    "Pause task",
                )
            } else if task.is_paused() {
                // It is Paused -> Show Resume
                (
                    icon::PLAY,
                    Message::StartTask(task.uid.clone()),
                    "Resume task",
                )
            } else {
                // It is Stopped/New -> Show Start
                (
                    icon::PLAY,
                    Message::StartTask(task.uid.clone()),
                    "Start task",
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

        // 2. Stop Button (Show if Running OR Paused)
        if task.status == crate::model::TaskStatus::InProcess || task.is_paused() {
            let stop_btn = button(icon::icon(icon::DEBUG_STOP).size(14))
                .style(action_style)
                .padding(4)
                .on_press(Message::StopTask(task.uid.clone()));

            actions = actions.push(
                tooltip(
                    stop_btn,
                    text("Stop (Reset)").size(12),
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
            text("Increase priority").size(12),
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
            text("Decrease priority").size(12),
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
        tooltip(edit_btn, text("Edit").size(12), tooltip::Position::Top)
            .style(tooltip_style)
            .delay(Duration::from_millis(700)),
    );

    let delete_btn = button(icon::icon(icon::TRASH).size(14))
        .style(danger_style)
        .padding(4)
        .on_press(Message::DeleteTask(index));
    actions = actions.push(
        tooltip(delete_btn, text("Delete").size(12), tooltip::Position::Top)
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
            tooltip(cancel_btn, text("Cancel").size(12), tooltip::Position::Top)
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

    // --- Calendar Color Logic ---
    let mut custom_border_color = default_border_color;

    // Find the calendar this task belongs to
    if let Some(cal) = app.calendars.iter().find(|c| c.href == task.calendar_href)
        && let Some(hex) = &cal.color
        && let Some((r, g, b)) = color_utils::parse_hex_to_floats(hex)
    {
        custom_border_color = Color::from_rgb(r, g, b);
    }

    let status_btn = button(
        container(if icon_char != ' ' {
            icon::icon(icon_char).size(12).color(Color::WHITE)
        } else {
            text("").size(12).color(Color::WHITE)
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
        let base_active = button::Style {
            background: Some(bg_color.into()),
            text_color: Color::WHITE,
            border: iced::Border {
                // Apply the custom border color here
                color: custom_border_color,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..button::Style::default()
        };
        match status {
            iced::widget::button::Status::Hovered => button::Style {
                border: iced::Border {
                    // Keep border visible on hover even if task is unchecked
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

    let has_metadata = !task.categories.is_empty()
        || task.rrule.is_some()
        || is_blocked
        || task.estimated_duration.is_some()
        // --- NEW FIELDS ---
        || task.location.is_some()
        || task.url.is_some()
        || task.geo.is_some();

    let main_text_col = responsive(move |size| {
        let available_width = size.width;
        let mut tags_width = 0.0;

        let mut tags_to_hide: HashSet<String> = if show_indent && let Some(p_uid) = &task.parent_uid
        {
            let mut p_cats = HashSet::new();
            if let Some(href) = app.store.index.get(p_uid)
                && let Some(list) = app.store.calendars.get(href)
                && let Some(p) = list.iter().find(|t| t.uid == *p_uid)
            {
                p_cats = p.categories.iter().cloned().collect();
            }
            p_cats
        } else {
            HashSet::new()
        };

        for cat in &task.categories {
            let mut search = cat.as_str();
            loop {
                if let Some(targets) = app.tag_aliases.get(search) {
                    for t in targets {
                        tags_to_hide.insert(t.clone());
                    }
                }
                if let Some(idx) = search.rfind(':') {
                    search = &search[..idx];
                } else {
                    break;
                }
            }
        }

        if has_metadata {
            if is_blocked {
                tags_width += 65.0;
            }
            for cat in &task.categories {
                if !tags_to_hide.contains(cat) {
                    // Approximate width: (chars + 1) * 7px + padding/spacing
                    tags_width += (cat.len() as f32 + 1.0) * 7.0 + 10.0;
                }
            }
            if let Some(l) = &task.location {
                tags_width += (l.len() as f32 * 7.0) + 25.0;
            }
            if task.estimated_duration.is_some() {
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
            let mut tags_row: iced::widget::Row<'_, Message> = row![].spacing(3);

            if is_blocked {
                tags_row = tags_row.push(
                    container(text("[Blocked]").size(12).color(Color::WHITE))
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

            for cat in &task.categories {
                if tags_to_hide.contains(cat) {
                    continue;
                }
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

            // --- COLORED LOCATION PILL (Moved Here) ---
            if let Some(loc) = &task.location {
                let text_color = Color::WHITE; // White text on gray bg

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
            // ----------------------------------------

            if let Some(mins) = task.estimated_duration {
                let label = if mins >= 525600 {
                    format!("~{}y", mins / 525600)
                } else if mins >= 43200 {
                    format!("~{}mo", mins / 43200)
                } else if mins >= 10080 {
                    format!("~{}w", mins / 10080)
                } else if mins >= 1440 {
                    format!("~{}d", mins / 1440)
                } else if mins >= 60 {
                    format!("~{}h", mins / 60)
                } else {
                    format!("~{}m", mins)
                };
                tags_row = tags_row.push(
                    container(text(label).size(10).color(Color::WHITE))
                        .style(|_| container::Style {
                            background: Some(Color::from_rgb(0.5, 0.5, 0.5).into()),
                            border: iced::Border {
                                radius: 4.0.into(),
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
                        text("Open Coordinates").size(12),
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
                    tooltip(url_btn, text("Open URL").size(12), tooltip::Position::Top)
                        .style(tooltip_style),
                );
            }

            tags_row.into()
        };

        // Estimate title width (font size 20, roughly 10-11px per char average)
        let title_width_est = task.summary.len() as f32 * 10.0;

        let required_title_space = title_width_est.min(90.0);
        let padding_safety = 5.0;

        let place_inline = if !has_metadata {
            true
        } else {
            (available_width - tags_width - padding_safety) > required_title_space
        };

        // --- ENFORCE WRAPPING ---
        let summary_text = text(&task.summary)
            .size(20)
            .color(color)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::Word);

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

    let row_main = row![indent, status_btn, main_text_col, date_text, actions]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let task_button = button(row_main)
        .on_press(Message::ToggleDetails(task.uid.clone()))
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

    let row_id = iced::widget::Id::from(task.uid.clone());

    // Prepare container content
    let container_content: Element<'a, Message> = if is_expanded && has_content_to_show {
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
                .unwrap_or_else(|| "Unknown parent".to_string());
            let remove_parent_btn = button(icon::icon(icon::CROSS).size(10))
                .style(button::danger)
                .padding(2)
                .on_press(Message::RemoveParent(task.uid.clone()));
            let row = row![
                text("Parent:")
                    .size(12)
                    .color(Color::from_rgb(0.4, 0.8, 0.4)),
                text(p_name).size(12),
                tooltip(
                    remove_parent_btn,
                    text("Remove parent").size(12),
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
                text("[Blocked By]:")
                    .size(12)
                    .color(Color::from_rgb(0.8, 0.4, 0.4)),
            );
            for dep_uid in &task.dependencies {
                let name = app
                    .store
                    .get_summary(dep_uid)
                    .unwrap_or_else(|| "Unknown task".to_string());
                let is_done = app.store.is_task_done(dep_uid).unwrap_or(false);
                let check = if is_done { "[x]" } else { "[ ]" };
                let remove_dep_btn = button(icon::icon(icon::CROSS).size(10))
                    .style(button::danger)
                    .padding(2)
                    .on_press(Message::RemoveDependency(task.uid.clone(), dep_uid.clone()));
                let dep_row = row![
                    text(format!("{} {}", check, name))
                        .size(12)
                        .color(Color::from_rgb(0.6, 0.6, 0.6)),
                    tooltip(
                        remove_dep_btn,
                        text("Remove dependency").size(12),
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

        let desc_row = row![
            Space::new().width(Length::Fixed(indent_size as f32 + 30.0)),
            details_col
        ];
        column![task_button, desc_row].spacing(5).into()
    } else if !has_content_to_show {
        NoPointer {
            content: task_button.into(),
        }
        .into()
    } else {
        task_button.into()
    };

    container(container_content)
        .padding(if is_expanded && has_content_to_show {
            5
        } else {
            0
        })
        .id(row_id)
        .into()
}

// Wrapper widget to suppress pointer cursor on hover
struct NoPointer<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for NoPointer<'a, Message, Theme, Renderer>
where
    Renderer: AdvancedRenderer,
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
    Renderer: AdvancedRenderer + 'a,
{
    fn from(widget: NoPointer<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}
