// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashSet;
use std::time::Duration;
pub mod focusable;
pub mod help;
pub mod settings;
pub mod sidebar;
pub mod syntax;
pub mod task_row;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::{AppState, Focus, GuiApp, ResizeDirection, SidebarMode};
use crate::gui::view::help::view_help;
use crate::gui::view::settings::view_settings;
use crate::gui::view::sidebar::{view_sidebar_calendars, view_sidebar_categories};
use crate::gui::view::task_row::view_task_row;
use chrono::Datelike;
use iced::alignment::Horizontal;
use iced::mouse;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::text::IntoFragment;
use iced::widget::{
    MouseArea, Space, button, column, container, rich_text, row, scrollable, span, stack, svg,
    text, text_editor, text_input, tooltip,
};
use iced::{Color, Element, Length, Theme, Vector};

pub const COLOR_LOCATION: Color = Color::from_rgb(0.4, 0.4, 0.6);
pub const CONTEXT_MENU_WIDTH: f32 = 190.0;

pub fn is_action_available(
    action: &crate::config::TaskAction,
    task: &crate::model::Task,
    app: &GuiApp,
) -> bool {
    if (task.is_note || task.is_journal)
        && matches!(
            action,
            crate::config::TaskAction::ToggleTimer
                | crate::config::TaskAction::StopTimer
                | crate::config::TaskAction::AddSession
                | crate::config::TaskAction::CompleteAndShift
        )
    {
        return false;
    }

    let is_done_or_cancelled =
        task.status.is_done() || task.status == crate::model::TaskStatus::Cancelled;
    let is_paused = task.is_paused();

    let has_info = !task.description.is_empty()
        || !task.dependencies.is_empty()
        || !task.related_to.is_empty()
        || task.has_blocking_tasks
        || task.has_related_tasks
        || task.parent_uid.as_ref().is_some_and(|uid| !uid.is_empty());

    let has_time = !task.sessions.is_empty() || task.time_spent_seconds > 0;

    match action {
        crate::config::TaskAction::Move => {
            let enabled_cal_count = app
                .calendars
                .iter()
                .filter(|c| !app.disabled_calendars.contains(&c.href))
                .count();
            enabled_cal_count > 1
        }
        crate::config::TaskAction::OpenUrl => task.url.is_some(),
        crate::config::TaskAction::DeleteTree => task.has_subtasks,
        crate::config::TaskAction::CompleteTree => task.has_subtasks,
        crate::config::TaskAction::OpenCoordinates => task.geo.is_some(),
        crate::config::TaskAction::OpenLocations => task.tree_location_count > 1,
        crate::config::TaskAction::ToggleDetails => {
            has_info
                || has_time
                || task.created_date().is_some()
                || task.last_modified_date().is_some()
        }
        crate::config::TaskAction::CompleteAndShift => {
            task.rrule.is_some() && !is_done_or_cancelled && !task.is_relative_recurrence()
        }
        crate::config::TaskAction::EditTree => true,
        crate::config::TaskAction::TogglePin => true,
        crate::config::TaskAction::Promote => task.parent_uid.is_some(),
        crate::config::TaskAction::Yank => app.yanked_uid.is_none(),
        crate::config::TaskAction::StopTimer => {
            task.status == crate::model::TaskStatus::InProcess || is_paused
        }
        crate::config::TaskAction::ToggleTimer
        | crate::config::TaskAction::AddSession
        | crate::config::TaskAction::Cancel => !is_done_or_cancelled,
        crate::config::TaskAction::BrowseRelations => false, // TUI-only
        _ => true,
    }
}

pub fn tooltip_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(
            Color {
                a: 0.85,
                ..palette.background.weak.color
            }
            .into(),
        ),
        text_color: Some(palette.background.weak.text),
        border: iced::Border {
            radius: 5.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..container::Style::default()
    }
}

fn modal_card_style(bg_alpha: f32, theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(
            Color {
                a: bg_alpha,
                ..palette.background.weak.color
            }
            .into(),
        ),
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.5),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
        ..Default::default()
    }
}

fn modal_backdrop<'a>(
    card: impl Into<Element<'a, Message>>,
    backdrop_alpha: f32,
) -> Element<'a, Message> {
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, backdrop_alpha).into()),
            ..Default::default()
        })
        .into()
}

fn modal_icon_header(icon_char: char, color: Color) -> Element<'static, Message> {
    container(icon::icon(icon_char).size(30).color(color))
        .padding(5)
        .center_x(Length::Fill)
        .into()
}

fn modal_title(title: impl IntoFragment<'static>) -> Element<'static, Message> {
    text(title)
        .size(24)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .into()
}

fn tip<'a>(
    content: impl Into<Element<'a, Message>>,
    label: impl IntoFragment<'a>,
    position: tooltip::Position,
) -> Element<'a, Message> {
    tooltip(content, text(label).size(12), position)
        .style(tooltip_style)
        .delay(Duration::from_millis(700))
        .into()
}

fn sidebar_tab_active_style(
    _theme: &Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: Some(Color::from_rgb(1.0, 0.6, 0.0).into()),
        text_color: Color::BLACK,
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..iced::widget::button::Style::default()
    }
}

fn sidebar_tab_btn(
    sidebar_mode: SidebarMode,
    icon_char: char,
    mode: SidebarMode,
    label: &str,
    shortcut: char,
    error: bool,
) -> Element<'static, Message> {
    let icon_el = if error && sidebar_mode != mode {
        icon::icon(icon_char)
            .size(18)
            .color(Color::from_rgb(0.9, 0.2, 0.2))
    } else {
        icon::icon(icon_char).size(18)
    };
    tip(
        button(container(icon_el).center_x(Length::Fill))
            .padding(8)
            .width(Length::Fill)
            .style(if sidebar_mode == mode {
                sidebar_tab_active_style
            } else {
                button::text
            })
            .on_press(Message::SidebarModeChanged(mode)),
        format!("{} ({})", label, shortcut),
        tooltip::Position::Bottom,
    )
}

fn sidebar_footer_btn(
    icon_char: char,
    width: Length,
    label: impl IntoFragment<'static>,
    msg: Message,
) -> Element<'static, Message> {
    let mut icon_container = container(icon::icon(icon_char).size(20));
    if matches!(width, Length::Fill) {
        icon_container = icon_container.width(Length::Fill);
    }
    icon_container = icon_container.center_x(Length::Fill).center_y(Length::Fill);
    tip(
        iced::widget::button(icon_container)
            .padding(0)
            .height(Length::Fixed(40.0))
            .width(width)
            .style(iced::widget::button::secondary)
            .on_press(msg),
        label,
        tooltip::Position::Top,
    )
}

pub(crate) fn message_banner<'a>(
    text_str: &'a str,
    cross_color: Color,
    background: impl Fn(&Theme) -> iced::Background + 'a,
    dismiss: Message,
) -> Element<'a, Message> {
    let content = row![
        text(text_str)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.base.text)
            })
            .size(14)
            .width(Length::Fill),
        iced::widget::button(icon::icon(icon::CROSS).size(14).color(cross_color))
            .style(iced::widget::button::text)
            .padding(2)
            .on_press(dismiss),
    ]
    .align_y(iced::Alignment::Center);
    container(content)
        .width(Length::Fill)
        .padding(5)
        .style(move |theme: &Theme| container::Style {
            background: Some(background(theme)),
            ..Default::default()
        })
        .into()
}

fn lock_bar(
    icon_char: char,
    label: String,
    summary: Option<String>,
    lock_icon: char,
    lock_active: bool,
    toggle: Message,
) -> Element<'static, Message> {
    let label_el = match &summary {
        Some(_) => text(label).size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        None => text(label)
            .size(14)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fill),
    };
    let mut row = row![
        icon::icon(icon_char)
            .size(16)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().primary.base.color)
            }),
        label_el,
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);
    if let Some(summary) = summary {
        row = row.push(text(summary).size(14).width(Length::Fill));
    }
    row = row
        .push(
            button(icon::icon(lock_icon).size(14))
                .style(move |theme: &Theme, _status| {
                    if lock_active {
                        button::Style {
                            text_color: theme.extended_palette().primary.base.color,
                            ..button::text(theme, _status)
                        }
                    } else {
                        button::Style {
                            text_color: Color::from_rgba(0.5, 0.5, 0.5, 0.7),
                            ..button::text(theme, _status)
                        }
                    }
                })
                .padding(5)
                .on_press(toggle),
        )
        .push(
            button(icon::icon(icon::CROSS).size(14))
                .style(iced::widget::button::text)
                .padding(5)
                .on_press(Message::EscapePressed),
        );
    container(row)
        .padding(10)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.weak.color.into()),
                border: iced::Border {
                    color: palette.primary.base.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

fn jump_button(icon_char: char, label: String, msg: Message) -> Element<'static, Message> {
    container(
        iced::widget::button(
            row![icon::icon(icon_char).size(14), text(label).size(14)]
                .spacing(5)
                .align_y(iced::Alignment::Center),
        )
        .style(iced::widget::button::secondary)
        .padding(5)
        .width(Length::Fill)
        .on_press(msg),
    )
    .padding(iced::Padding {
        left: 10.0,
        right: 10.0,
        bottom: 5.0,
        ..Default::default()
    })
    .into()
}

fn window_controls(force_ssd: bool) -> Element<'static, Message> {
    if force_ssd {
        row![].spacing(0).into()
    } else {
        row![
            iced::widget::button(icon::icon(icon::WINDOW_MINIMIZE).size(14))
                .style(iced::widget::button::text)
                .padding(8)
                .on_press(Message::MinimizeWindow),
            iced::widget::button(icon::icon(icon::CROSS).size(14))
                .style(iced::widget::button::danger)
                .padding(8)
                .on_press(Message::CloseWindow),
        ]
        .spacing(0)
        .into()
    }
}

fn drag_area<'a>(el: impl Into<Element<'a, Message>>, force_ssd: bool) -> Element<'a, Message> {
    if force_ssd {
        el.into()
    } else {
        MouseArea::new(el).on_press(Message::WindowDragged).into()
    }
}

fn journal_activity_row(
    icon_char: char,
    icon_color: Color,
    label: String,
    tasks: &[crate::model::Task],
) -> Option<Element<'static, Message>> {
    if tasks.is_empty() {
        return None;
    }
    let mut spans = Vec::new();
    for (i, t) in tasks.iter().enumerate() {
        if i > 0 {
            spans.push(span(", ").color(Color::from_rgb(0.6, 0.6, 0.6)));
        }
        spans.push(
            span(t.summary.clone())
                .color(Color::from_rgb(0.2, 0.7, 1.0))
                .link(t.uid.clone()),
        );
    }
    let rt = rich_text(spans).size(13).on_link_click(Message::JumpToTask);
    Some(
        column![
            row![
                icon::icon(icon_char).size(10).color(icon_color),
                text(label).size(12).color(Color::from_rgb(0.6, 0.6, 0.6))
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
            rt,
        ]
        .spacing(2)
        .into(),
    )
}

fn journal_panel_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: iced::Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

pub fn root_view(app: &GuiApp) -> Element<'_, Message> {
    let is_expanded =
        app.editing_uid.is_some() || app.editing_tree_uid.is_some() || app.creating_with_desc;

    let base_content: Element<'_, Message> = match app.state {
        AppState::Loading => container(
            column![
                text(rust_i18n::t!("loading")).size(30),
                text(rust_i18n::t!("waiting_for_keyring"))
                    .size(14)
                    .color(Color::from_rgb(0.5, 0.5, 0.5))
            ]
            .align_x(iced::Alignment::Center)
            .spacing(10),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into(),
        AppState::Onboarding | AppState::Settings => view_settings(app),
        AppState::Help(tab, _) => view_help(tab, app),
        AppState::Active if app.sidebar_mode == SidebarMode::Journal => {
            let content_layout = if app.sidebar_is_hidden {
                row![container(view_journal_main_pane(app)).width(Length::Fill)]
            } else {
                row![
                    view_sidebar(app, false),
                    iced::widget::rule::vertical(1),
                    container(view_journal_main_pane(app)).width(Length::Fill)
                ]
            };
            container(content_layout)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        AppState::Active => {
            let content_height = match app.sidebar_mode {
                SidebarMode::Calendars => app.get_filtered_calendars().len() as f32 * 44.0,
                SidebarMode::Categories => app.cached_categories.len() as f32 * 34.0,
                SidebarMode::Locations => app.cached_locations.len() as f32 * 34.0,
                SidebarMode::Journal => 300.0, // Mini-calendar height
                SidebarMode::Goals => app.core_config.goals.len() as f32 * 60.0,
            };
            let available_height = app.current_window_size.height - 110.0;
            let show_logo = (available_height - content_height) > 140.0;

            let content_layout = if app.sidebar_is_hidden {
                row![
                    container(view_main_content(app, !show_logo, is_expanded))
                        .width(Length::Fill)
                        .center_x(Length::Fill)
                ]
            } else {
                row![
                    view_sidebar(app, show_logo),
                    iced::widget::rule::vertical(1),
                    container(view_main_content(app, !show_logo, is_expanded))
                        .width(Length::Fill)
                        .center_x(Length::Fill)
                ]
            };

            container(content_layout)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    };

    let mut stack_children: Vec<Element<'_, Message>> = vec![base_content];

    if app.ics_import_dialog_open {
        stack_children.push(view_ics_import_overlay(app));
    } else if app.confirm_delete_all_open {
        let uids: Vec<_> = app
            .tasks
            .iter()
            .filter_map(|t| {
                if let crate::store::TaskListItem::Task(task) = t {
                    Some(task.as_ref())
                } else {
                    None
                }
            })
            .collect();

        let icon_header = modal_icon_header(icon::TRASH, Color::from_rgb(0.9, 0.2, 0.2));
        let title = modal_title(rust_i18n::t!("delete_all_title"));

        let count = uids.len();
        let prompt = if count == 1 {
            rust_i18n::t!("delete_all_confirm.one").to_string()
        } else {
            rust_i18n::t!("delete_all_confirm.other", count = count).to_string()
        };

        let mut list_col = column![text(prompt).size(16)].spacing(10);

        let mut tasks_col = column![].spacing(5);
        for t in uids.iter().take(10) {
            tasks_col = tasks_col.push(
                text(format!("- {}", t.summary))
                    .size(14)
                    .wrapping(iced::widget::text::Wrapping::Glyph)
                    .style(|t: &Theme| text::Style {
                        color: Some(t.extended_palette().background.base.text),
                    }),
            );
        }
        if uids.len() > 10 {
            tasks_col = tasks_col.push(
                text(format!("...and {} more", uids.len() - 10))
                    .size(14)
                    .wrapping(iced::widget::text::Wrapping::Glyph)
                    .style(|_t: &Theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
            );
        }

        list_col = list_col.push(
            scrollable(tasks_col)
                .width(Length::Fill)
                .height(Length::Fixed(200.0)),
        );

        let buttons = row![
            button(text(rust_i18n::t!("cancel")).size(14))
                .style(iced::widget::button::secondary)
                .padding([8, 16])
                .on_press(Message::CancelDeleteAll),
            button(text(rust_i18n::t!("delete")).size(14))
                .style(iced::widget::button::danger)
                .padding([8, 16])
                .on_press(Message::ExecuteDeleteAll)
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let modal_content = column![
            icon_header,
            title,
            Space::new().height(Length::Fixed(10.0)),
            list_col,
            Space::new().height(Length::Fixed(20.0)),
            buttons
        ]
        .spacing(5)
        .align_x(iced::Alignment::Center);

        let modal_card = container(modal_content)
            .padding(20)
            .width(Length::Fixed(400.0))
            .max_height(600.0)
            .style(|theme: &Theme| modal_card_style(0.98, theme));

        stack_children.push(modal_backdrop(modal_card, 0.7));
    } else if !app.ringing_tasks.is_empty() {
        let (task, alarm) = &app.ringing_tasks[0];

        let icon_header = modal_icon_header(icon::BELL, Color::from_rgb(1.0, 0.4, 0.0));
        let title = modal_title(rust_i18n::t!("reminder_title"));

        let summary = text(&task.summary)
            .size(18)
            .width(Length::Fill)
            .align_x(Horizontal::Center);

        let task_desc_content = if !task.description.is_empty() {
            column![
                text(&task.description)
                    .size(14)
                    .color(Color::from_rgb(0.9, 0.9, 0.9)),
                Space::new().height(Length::Fixed(10.0))
            ]
        } else {
            column![]
        };

        let s1 = app.snooze_short_mins;
        let s2 = app.snooze_long_mins;

        let snooze_btn = |mins: u32| {
            let label = if mins >= 60 {
                format!("{}h", mins / 60)
            } else {
                format!("{}m", mins)
            };
            button(text(label).size(12))
                .style(iced::widget::button::secondary)
                .padding([6, 12])
                .on_press(Message::SnoozeAlarm(
                    task.uid.clone(),
                    alarm.uid.clone(),
                    mins,
                ))
        };

        let custom_snooze_row = row![
            text_input(
                &format!(
                    "{} ({} 30m)",
                    rust_i18n::t!("snooze_custom_title"),
                    rust_i18n::t!("eg")
                ),
                &app.snooze_custom_input
            )
            .on_input(Message::SnoozeCustomInput)
            .on_submit(Message::SnoozeCustomSubmit(
                task.uid.clone(),
                alarm.uid.clone()
            ))
            .padding(5)
            .size(12)
            .width(Length::Fixed(100.0)),
            button(icon::icon(icon::CHECK).size(12))
                .style(iced::widget::button::secondary)
                .padding(6)
                .on_press(Message::SnoozeCustomSubmit(
                    task.uid.clone(),
                    alarm.uid.clone()
                ))
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center);

        let done_btn = button(text(rust_i18n::t!("done")).size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .style(iced::widget::button::success)
        .padding([8, 16])
        .on_press(Message::CompleteTaskFromAlarm(
            task.uid.clone(),
            alarm.uid.clone(),
        ));

        let cancel_btn = button(text(rust_i18n::t!("cancel_task")).size(14))
            .style(iced::widget::button::danger)
            .padding([8, 16])
            .on_press(Message::CancelTaskFromAlarm(
                task.uid.clone(),
                alarm.uid.clone(),
            ));

        let dismiss_btn = button(text(rust_i18n::t!("dismiss")).size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .style(iced::widget::button::primary)
        .padding([8, 16])
        .on_press(Message::DismissAlarm(task.uid.clone(), alarm.uid.clone()));

        let buttons = column![
            row![snooze_btn(s1), snooze_btn(s2), custom_snooze_row]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            Space::new().height(10),
            row![done_btn, cancel_btn, dismiss_btn].spacing(10)
        ]
        .align_x(iced::Alignment::Center);

        let modal_content = scrollable(
            column![
                icon_header,
                title,
                summary,
                Space::new().height(Length::Fixed(10.0)),
                task_desc_content,
                Space::new().height(Length::Fixed(20.0)),
                buttons
            ]
            .spacing(5)
            .align_x(iced::Alignment::Center),
        )
        .height(Length::Shrink);

        let modal_card = container(modal_content)
            .padding(20)
            .width(Length::Fixed(380.0))
            .max_height(500.0)
            .style(|theme: &Theme| modal_card_style(0.95, theme));

        stack_children.push(modal_backdrop(modal_card, 0.6));
    }

    // --- CONTEXT MENU OVERLAY ---
    if let Some((uid, is_full, pt)) = &app.active_context_menu
        && let Some(idx) = app.find_task_index_by_uid(uid)
        && let Some(task) = app.get_task_at_index(idx)
    {
        use crate::config::TaskAction;

        let mut menu_actions = column![].spacing(2);

        let menu_btn_style = |theme: &Theme, status: button::Status| -> button::Style {
            let palette = theme.extended_palette();
            match status {
                button::Status::Hovered | button::Status::Pressed => button::Style {
                    background: Some(palette.background.strong.color.into()),
                    text_color: palette.background.strong.text,
                    ..button::Style::default()
                },
                _ => button::Style {
                    background: Some(Color::TRANSPARENT.into()),
                    text_color: palette.background.base.text,
                    ..button::Style::default()
                },
            }
        };

        let danger_menu_style = |theme: &Theme, status: button::Status| -> button::Style {
            let palette = theme.extended_palette();
            match status {
                button::Status::Hovered | button::Status::Pressed => button::Style {
                    background: Some(palette.danger.base.color.into()),
                    text_color: palette.danger.base.text,
                    ..button::Style::default()
                },
                _ => button::Style {
                    background: Some(Color::TRANSPARENT.into()),
                    text_color: palette.danger.base.color,
                    ..button::Style::default()
                },
            }
        };

        let build_btn = |action: &TaskAction| -> Option<Element<'_, Message>> {
            if !crate::gui::view::is_action_available(action, task, app) {
                return None;
            }

            let has_info = !task.description.is_empty()
                || !task.dependencies.is_empty()
                || !task.related_to.is_empty()
                || task.has_blocking_tasks
                || task.has_related_tasks
                || task.parent_uid.as_ref().is_some_and(|uid| !uid.is_empty());
            let has_time = !task.sessions.is_empty() || task.time_spent_seconds > 0;

            let mut label = action.label();
            if *action == TaskAction::DuplicateTree && !task.has_subtasks {
                label = rust_i18n::t!("duplicate_single_task").to_string();
            }

            let shortcut = match *action {
                TaskAction::CompleteAndShift => " (Shift+Space)",
                TaskAction::ToggleDetails => " (L)",
                TaskAction::ToggleTimer => " (s)",
                TaskAction::StopTimer => " (S)",
                TaskAction::AddSession => " (t)",
                TaskAction::IncreasePriority => " (+)",
                TaskAction::DecreasePriority => " (-)",
                TaskAction::Focus => " (f)",
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
                TaskAction::OpenCoordinates | TaskAction::OpenLocations => " (g)",
                TaskAction::OpenUrl => " (o)",
                _ => "",
            };
            label.push_str(shortcut);

            let (icon_element, msg, is_danger): (Element<'_, Message>, Message, bool) = match action
            {
                TaskAction::ToggleDetails => {
                    let mut icon_row = row![].spacing(2).align_y(iced::Alignment::Center);
                    if has_info {
                        icon_row = icon_row.push(icon::icon(icon::INFO).size(14).line_height(1.0));
                    }
                    if has_time {
                        icon_row = icon_row
                            .push(icon::icon(icon::TIMER_SETTINGS).size(14).line_height(1.0));
                    }
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
                    (
                        icon_row.into(),
                        Message::ToggleDetails(task.uid.clone()),
                        false,
                    )
                }
                TaskAction::ToggleTimer => {
                    if task.status == crate::model::TaskStatus::InProcess {
                        label = rust_i18n::t!("pause_task").to_string();
                        (
                            icon::icon(icon::PAUSE).size(14).into(),
                            Message::PauseTask(task.uid.clone()),
                            false,
                        )
                    } else if task.is_paused() {
                        label = rust_i18n::t!("resume_task").to_string();
                        (
                            icon::icon(icon::PLAY).size(14).into(),
                            Message::StartTask(task.uid.clone()),
                            false,
                        )
                    } else {
                        label = rust_i18n::t!("start_task").to_string();
                        (
                            icon::icon(icon::PLAY).size(14).into(),
                            Message::StartTask(task.uid.clone()),
                            false,
                        )
                    }
                }
                TaskAction::StopTimer => (
                    icon::icon(icon::DEBUG_STOP).size(14).into(),
                    Message::StopTask(task.uid.clone()),
                    false,
                ),
                TaskAction::AddSession => (
                    icon::icon(icon::TIMER_PLUS).size(14).into(),
                    Message::StartAddSession(task.uid.clone()),
                    false,
                ),
                TaskAction::IncreasePriority => (
                    icon::icon(icon::PLUS).size(14).into(),
                    Message::ChangePriority(idx, 1),
                    false,
                ),
                TaskAction::DecreasePriority => (
                    icon::icon(icon::MINUS).size(14).into(),
                    Message::ChangePriority(idx, -1),
                    false,
                ),
                TaskAction::Focus => (
                    icon::icon(app.focus_icon).size(14).into(),
                    Message::FocusSelected,
                    false,
                ),
                TaskAction::Edit => (
                    icon::icon(icon::EDIT).size(14).into(),
                    Message::EditTaskStart(idx),
                    false,
                ),
                TaskAction::EditTree => (
                    icon::icon(icon::EDIT_TREE).size(14).into(),
                    Message::EditTaskTree(uid.clone()),
                    false,
                ),
                TaskAction::Yank => (
                    icon::icon(icon::LINK).size(14).into(),
                    Message::YankTask(uid.clone()),
                    false,
                ),
                TaskAction::CreateSubtask => (
                    icon::icon(icon::CREATE_CHILD).size(14).into(),
                    Message::StartCreateChild(uid.clone()),
                    false,
                ),
                TaskAction::DuplicateTree => (
                    icon::icon(icon::CLONE).size(14).into(),
                    Message::DuplicateTask(uid.clone()),
                    false,
                ),
                TaskAction::CompleteTree => (
                    icon::icon(icon::LIST_CHECK).size(14).into(),
                    Message::CompleteTree(uid.clone()),
                    false,
                ),
                TaskAction::Promote => (
                    icon::icon(icon::ELEVATOR_UP).size(14).into(),
                    Message::RemoveParent(uid.clone()),
                    false,
                ),
                TaskAction::Move => (
                    icon::icon(icon::MOVE).size(14).into(),
                    Message::StartMoveTask(uid.clone()),
                    false,
                ),
                TaskAction::Cancel => (
                    icon::icon(icon::CROSS).size(14).into(),
                    Message::CancelTask(idx),
                    true,
                ),
                TaskAction::Delete => (
                    icon::icon(icon::TRASH).size(14).into(),
                    Message::DeleteTask(idx),
                    true,
                ),
                TaskAction::DeleteTree => (
                    icon::icon(icon::TRASH).size(14).into(),
                    Message::DeleteTaskTree(uid.clone()),
                    true,
                ),
                TaskAction::OpenCoordinates => (
                    icon::icon(icon::MAP_LOCATION_DOT).size(14).into(),
                    Message::OpenCoordinates(uid.clone()),
                    false,
                ),
                TaskAction::OpenLocations => (
                    icon::icon(icon::MAP_MARKER_MULTIPLE).size(14).into(),
                    Message::OpenLocations(uid.clone()),
                    false,
                ),
                TaskAction::OpenUrl => (
                    icon::icon(icon::URL_CHECK).size(14).into(),
                    Message::OpenUrl(task.url.clone().unwrap()),
                    false,
                ),
                TaskAction::CompleteAndShift => (
                    icon::icon(icon::REPEAT).size(14).into(),
                    Message::ToggleTaskShift(uid.clone()),
                    false,
                ),
                TaskAction::TogglePin => (
                    icon::icon(icon::THUMB_TACK).size(14).into(),
                    Message::TogglePin(uid.clone()),
                    false,
                ),
                TaskAction::BrowseRelations => {
                    // TUI-only; filtered out by is_action_available above
                    unreachable!()
                }
            };

            let btn = button(
                row![icon_element, text(label).size(14)]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(8)
            .style(if is_danger {
                danger_menu_style
            } else {
                menu_btn_style
            })
            .on_press(msg);

            Some(btn.into())
        };

        // Custom ordering for context menu - put location actions at top
        let context_menu_order = if *is_full {
            vec![
                TaskAction::OpenUrl,         // Add this at the top
                TaskAction::OpenCoordinates, // Single coordinates first
                TaskAction::OpenLocations,   // GPX export second
                TaskAction::ToggleDetails,
                TaskAction::CompleteAndShift,
                TaskAction::ToggleTimer,
                TaskAction::StopTimer,
                TaskAction::AddSession,
                TaskAction::IncreasePriority,
                TaskAction::DecreasePriority,
                TaskAction::Edit,
                TaskAction::EditTree,
                TaskAction::Yank,
                TaskAction::CreateSubtask,
                TaskAction::DuplicateTree,
                TaskAction::CompleteTree,
                TaskAction::Promote,
                TaskAction::Move,
                TaskAction::Cancel,
                TaskAction::Delete,
                TaskAction::DeleteTree,
                TaskAction::Focus,
            ]
        } else {
            // For non-full menu, put OpenLocations at top of unpinned actions
            let mut unpinned_actions: Vec<TaskAction> = TaskAction::ALL
                .iter()
                .filter(|a| !app.pinned_actions.contains(a))
                .cloned()
                .collect();

            // Move preferred actions to the front, in this order
            let preferred_order = [
                TaskAction::OpenUrl,
                TaskAction::OpenCoordinates,
                TaskAction::Focus,
                TaskAction::CompleteAndShift,
                TaskAction::OpenLocations,
            ];
            let mut insert_idx = 0;
            for preferred in preferred_order {
                if let Some(pos) = unpinned_actions.iter().position(|&a| a == preferred) {
                    unpinned_actions.remove(pos);
                    unpinned_actions.insert(insert_idx, preferred);
                    insert_idx += 1;
                }
            }
            unpinned_actions
        };

        let mut added_unpinned = false;
        let mut num_items = 0;
        for action in context_menu_order {
            if let Some(btn) = build_btn(&action) {
                menu_actions = menu_actions.push(btn);
                num_items += 1;
                if !*is_full && !app.pinned_actions.contains(&action) {
                    added_unpinned = true;
                }
            }
        }

        if !*is_full && !added_unpinned {
            menu_actions = menu_actions.push(
                container(
                    text(rust_i18n::t!("all_actions_pinned"))
                        .size(12)
                        .color(Color::from_rgb(0.5, 0.5, 0.5)),
                )
                .padding(8),
            );
            num_items += 1;
        }

        let max_available_height = (app.current_window_size.height - 20.0).max(100.0);

        let menu_scrollable = scrollable(menu_actions).direction(Direction::Vertical(
            Scrollbar::new().width(6).scroller_width(6).margin(0),
        ));

        let menu_container = container(menu_scrollable)
            .width(Length::Fixed(CONTEXT_MENU_WIDTH))
            .max_height(max_available_height)
            .padding(4)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: Color::BLACK.scale_alpha(0.5),
                        offset: Vector::new(0.0, 4.0),
                        blur_radius: 10.0,
                    },
                    ..Default::default()
                }
            });

        // Position the menu exactly by the mouse
        let menu_width = CONTEXT_MENU_WIDTH;
        let estimated_menu_height = (num_items as f32 * 34.0 + 8.0).min(max_available_height);

        let mut top_padding = pt.y;
        let mut left_padding = pt.x;

        if left_padding + menu_width > app.current_window_size.width {
            left_padding = (app.current_window_size.width - menu_width - 10.0).max(0.0);
        }
        if top_padding + estimated_menu_height > app.current_window_size.height {
            top_padding = (app.current_window_size.height - estimated_menu_height - 10.0).max(0.0);
        }

        let positioned_menu = container(menu_container)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: top_padding,
                left: left_padding,
                ..Default::default()
            });

        // Backdrop captures clicks anywhere else to close the menu completely
        let backdrop = iced::widget::opaque(
            MouseArea::new(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(Message::CloseContextMenu)
                .on_right_press(Message::CloseContextMenu),
        );

        stack_children.push(backdrop);
        stack_children.push(positioned_menu.into());
    }

    // --- MOVE TASK MODAL OVERLAY ---
    if let Some(uid) = &app.moving_task_uid
        && let Some(task) = app.store.get_task_ref(uid).cloned()
    {
        let targets = app.get_move_targets(&task.calendar_href, app.moving_task_is_tree);

        let icon_header = modal_icon_header(icon::MOVE, Color::from_rgb(0.3, 0.7, 1.0));
        let title = modal_title(if app.moving_task_is_tree {
            rust_i18n::t!("move_task_tree")
        } else {
            rust_i18n::t!("move_task_title")
        });

        let mut cal_list = column![].spacing(5);
        for (i, cal) in targets.iter().enumerate() {
            let is_selected = i == app.move_target_idx;

            let mut cal_button = button(
                row![
                    icon::icon(icon::CALENDAR).size(14).color(if is_selected {
                        Color::from_rgb(1.0, 1.0, 1.0)
                    } else {
                        Color::from_rgb(0.6, 0.6, 0.6)
                    }),
                    text(&cal.name).size(14)
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(10)
            .on_press(Message::MoveTask(task.uid.clone(), cal.href.clone()));

            if is_selected {
                cal_button = cal_button.style(iced::widget::button::primary);
            } else {
                cal_button = cal_button.style(iced::widget::button::secondary);
            }

            cal_list = cal_list.push(cal_button);
        }

        let calendar_scroll = scrollable(cal_list)
            .id(iced::widget::Id::new("move_modal_scrollable"))
            .height(Length::Fixed(250.0))
            .direction(Direction::Vertical(
                Scrollbar::new().width(8).scroller_width(8),
            ));

        let mut bottom_controls = row![].spacing(10).align_y(iced::Alignment::Center);
        if app.store.children_index.contains_key(&task.uid) {
            bottom_controls = bottom_controls.push(
                iced::widget::checkbox::<Message, iced::Theme, iced::Renderer>(
                    app.moving_task_is_tree,
                )
                .label(rust_i18n::t!("move_task_tree"))
                .on_toggle(Message::ToggleMoveTree),
            );
        }

        let cancel_btn = button(text(rust_i18n::t!("cancel")).size(14))
            .style(iced::widget::button::secondary)
            .padding([8, 16])
            .on_press(Message::CancelMoveTask);

        bottom_controls = bottom_controls
            .push(Space::new().width(Length::Fill))
            .push(cancel_btn);

        let modal_content = column![
            icon_header,
            title,
            Space::new().height(Length::Fixed(10.0)),
            text(rust_i18n::t!("move_to"))
                .size(14)
                .color(Color::from_rgb(0.7, 0.7, 0.7)),
            Space::new().height(Length::Fixed(5.0)),
            calendar_scroll,
            Space::new().height(Length::Fixed(20.0)),
            container(bottom_controls).width(Length::Fill)
        ]
        .spacing(5)
        .align_x(iced::Alignment::Center);

        let modal_card = container(modal_content)
            .padding(20)
            .width(Length::Fixed(350.0))
            .max_height(500.0)
            .style(|theme: &Theme| modal_card_style(0.98, theme));

        stack_children.push(modal_backdrop(modal_card, 0.6));
    }

    if app.blur_when_unfocused && !app.is_window_focused {
        let overlay = container(
            column![
                icon::icon(icon::LOCK)
                    .size(48)
                    .color(app.theme().extended_palette().background.weak.text),
                text(rust_i18n::t!("privacy_mode"))
                    .size(24)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .color(app.theme().extended_palette().background.weak.text),
            ]
            .spacing(20)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.base.color.into()),
                ..Default::default()
            }
        });

        stack_children.push(iced::widget::opaque(overlay));
    }

    let content_with_modals: Element<'_, Message> = iced::widget::stack(stack_children).into();

    let final_content = if app.force_ssd {
        content_with_modals
    } else {
        let t = 6.0;
        let c = 12.0;

        let n_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(t)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::North))
        .interaction(mouse::Interaction::ResizingVertically);

        let s_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(t)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::South))
        .interaction(mouse::Interaction::ResizingVertically);

        let e_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(t))
                .height(Length::Fill),
        )
        .on_press(Message::ResizeStart(ResizeDirection::East))
        .interaction(mouse::Interaction::ResizingHorizontally);

        let w_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(t))
                .height(Length::Fill),
        )
        .on_press(Message::ResizeStart(ResizeDirection::West))
        .interaction(mouse::Interaction::ResizingHorizontally);

        let nw_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(c))
                .height(Length::Fixed(c)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::NorthWest))
        .interaction(mouse::Interaction::ResizingDiagonallyDown);

        let ne_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(c))
                .height(Length::Fixed(c)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::NorthEast))
        .interaction(mouse::Interaction::ResizingDiagonallyUp);

        let sw_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(c))
                .height(Length::Fixed(c)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::SouthWest))
        .interaction(mouse::Interaction::ResizingDiagonallyUp);

        let se_grip = MouseArea::new(
            container(text(""))
                .width(Length::Fixed(c))
                .height(Length::Fixed(c)),
        )
        .on_press(Message::ResizeStart(ResizeDirection::SouthEast))
        .interaction(mouse::Interaction::ResizingDiagonallyDown);

        stack![
            content_with_modals,
            container(n_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Top),
            container(s_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Bottom),
            container(e_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right),
            container(w_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left),
            container(nw_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Top),
            container(ne_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Top),
            container(sw_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Bottom),
            container(se_grip)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Bottom),
        ]
        .into()
    };

    container(final_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.base.color)),
                border: iced::Border {
                    color: palette.background.strong.color,
                    width: if app.force_ssd { 0.0 } else { 1.0 },
                    radius: if app.force_ssd {
                        0.0.into()
                    } else {
                        12.0.into()
                    },
                },
                ..Default::default()
            }
        })
        .into()
}

fn view_sidebar(app: &GuiApp, show_logo: bool) -> Element<'_, Message> {
    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let is_tag_error = is_filter_empty && !app.session.selected_categories.is_empty();
    let is_loc_error = is_filter_empty && !app.session.selected_locations.is_empty();

    let btn_cals = sidebar_tab_btn(
        app.sidebar_mode,
        icon::CALENDARS_HEADER,
        SidebarMode::Calendars,
        &rust_i18n::t!("calendars"),
        '1',
        false,
    );
    let btn_tags = sidebar_tab_btn(
        app.sidebar_mode,
        icon::TAGS_HEADER,
        SidebarMode::Categories,
        &rust_i18n::t!("tags"),
        '2',
        is_tag_error,
    );
    let btn_locs = sidebar_tab_btn(
        app.sidebar_mode,
        app.location_tab_icon,
        SidebarMode::Locations,
        &rust_i18n::t!("locations"),
        '3',
        is_loc_error,
    );

    let mut tabs = row![].spacing(2);

    if app.show_calendars_tab {
        tabs = tabs.push(btn_cals);
    }

    if app.show_tags_tab {
        tabs = tabs.push(btn_tags);
    }

    if app.show_locations_tab {
        tabs = tabs.push(btn_locs);
    }

    if app.show_goals_tab {
        tabs = tabs.push(sidebar_tab_btn(
            app.sidebar_mode,
            app.goal_icon,
            SidebarMode::Goals,
            &rust_i18n::t!("goals"),
            '4',
            false,
        ));
    }

    if app.show_journal_tab {
        tabs = tabs.push(sidebar_tab_btn(
            app.sidebar_mode,
            app.journal_icon,
            SidebarMode::Journal,
            &rust_i18n::t!("journal"),
            '5',
            false,
        ));
    }

    let content = match app.sidebar_mode {
        SidebarMode::Calendars => view_sidebar_calendars(app),
        SidebarMode::Categories => view_sidebar_categories(app),
        SidebarMode::Locations => crate::gui::view::sidebar::view_sidebar_locations(app),
        SidebarMode::Journal => crate::gui::view::sidebar::view_sidebar_journal(app),
        SidebarMode::Goals => crate::gui::view::sidebar::view_sidebar_goals(app),
    };

    let footer = row![
        sidebar_footer_btn(
            icon::SETTINGS_GEAR,
            Length::Fill,
            format!("{} (Ctrl+,)", rust_i18n::t!("settings")),
            Message::OpenSettings,
        ),
        sidebar_footer_btn(
            icon::KEYBOARD,
            Length::Fixed(40.0),
            format!("{} (?)", rust_i18n::t!("keyboard_shortcuts")),
            Message::OpenHelp(crate::help::HelpTab::Shortcuts),
        ),
        sidebar_footer_btn(
            icon::HELP_RHOMBUS,
            Length::Fixed(40.0),
            format!("{} (?)", rust_i18n::t!("syntax_help")),
            Message::OpenHelp(crate::help::HelpTab::Syntax),
        ),
        sidebar_footer_btn(
            icon::BARS,
            Length::Fixed(40.0),
            rust_i18n::t!("toggle_sidebar"),
            Message::ToggleSidebar,
        ),
    ]
    .spacing(5);

    let mut sidebar_col = column![tabs, content];

    if show_logo {
        sidebar_col = sidebar_col.push(
            container(
                svg(svg::Handle::from_memory(icon::LOGO))
                    .width(100)
                    .height(100)
                    .content_fit(iced::ContentFit::Contain),
            )
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding(iced::Padding {
                top: 20.0,
                bottom: 20.0,
                ..Default::default()
            }),
        );
    }

    sidebar_col = sidebar_col.push(footer);

    container(sidebar_col.spacing(10).padding(10))
        .width(220)
        .height(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(iced::Background::Color(palette.background.weak.color)),
                ..Default::default()
            }
        })
        .into()
}

fn view_main_content(app: &GuiApp, show_logo: bool, is_expanded: bool) -> Element<'_, Message> {
    let active_cal = app
        .active_cal_href
        .as_ref()
        .and_then(|href| app.calendars.iter().find(|c| &c.href == href));

    let (title_text, title_msg) = if let Some(focus_uid) = &app.session.focused_task_uid {
        if let Some(t) = app.store.get_task_ref(focus_uid) {
            // Dynamically calculate allowed title length to protect the search bar
            // Total Width - Sidebar (~220) - Right Controls (~150) - Search Min (~200) = ~570px used.
            let available_width = app.current_window_size.width - 570.0;
            let max_chars = (available_width / 10.0).max(20.0) as usize;

            let mut trunc = t.summary.clone();
            if trunc.chars().count() > max_chars {
                trunc = format!(
                    "{}...",
                    trunc
                        .chars()
                        .take(max_chars.saturating_sub(3))
                        .collect::<String>()
                );
            }
            (
                format!("{} {} (Esc)", app.focus_icon, trunc),
                Message::ClearFocus,
            )
        } else {
            (
                format!("{} Focused (Esc)", app.focus_icon),
                Message::ClearFocus,
            )
        }
    } else if app.loading {
        (
            rust_i18n::t!("loading").to_string(),
            Message::ToggleAllCalendars(true),
        ) // Will be overridden
    } else if let Some(cal) = active_cal {
        (cal.name.clone(), Message::ToggleAllCalendars(true))
    } else if app.session.selected_categories.is_empty() {
        (
            rust_i18n::t!("all_tasks").to_string(),
            Message::ToggleAllCalendars(true),
        )
    } else {
        (
            rust_i18n::t!("tasks").to_string(),
            Message::ToggleAllCalendars(true),
        )
    };

    let other_visible_cals: Vec<&crate::model::CalendarListEntry> =
        if !app.loading && app.sidebar_mode != SidebarMode::Calendars {
            app.get_filtered_calendars()
                .into_iter()
                .filter(|c| {
                    !app.hidden_calendars.contains(&c.href)
                        && Some(&c.href) != app.active_cal_href.as_ref()
                })
                .collect()
        } else {
            vec![]
        };

    let active_cal_color_opt = active_cal
        .and_then(|c| c.color.as_ref())
        .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
        .map(|(r, g, b)| Color::from_rgb(r, g, b));

    let title_style = move |theme: &Theme| -> text::Style {
        text::Style {
            color: Some(
                active_cal_color_opt.unwrap_or(theme.extended_palette().background.base.text),
            ),
        }
    };

    let active_count = app
        .tasks
        .iter()
        .filter(|item| {
            if let crate::store::TaskListItem::Task(t) = item {
                !t.status.is_done()
            } else {
                false
            }
        })
        .count();
    let mut subtitle = match active_count {
        0 => rust_i18n::t!("tasks_count.zero").to_string(),
        1 => rust_i18n::t!("tasks_count.one").to_string(),
        _ => rust_i18n::t!("tasks_count.other", count = active_count).to_string(),
    };

    let search_text = app.search_value.text();
    if !search_text.is_empty() {
        subtitle.push_str(&format!(" | Search: '{}'", search_text));
    } else if !app.session.selected_categories.is_empty() {
        let tag_count = app.session.selected_categories.len();
        if tag_count == 1 {
            subtitle.push_str(&format!(
                " | Tag: #{}",
                app.session.selected_categories.first().unwrap()
            ));
        } else {
            subtitle.push_str(&format!(" | {} Tags", tag_count));
        }
    }

    let mut title_group = row![].spacing(0).align_y(iced::Alignment::Center);

    if app.sidebar_is_hidden {
        title_group = title_group
            .push(tip(
                iced::widget::button(icon::icon(icon::BARS).size(18))
                    .style(iced::widget::button::text)
                    .padding(4)
                    .on_press(Message::ToggleSidebar),
                rust_i18n::t!("toggle_sidebar"),
                tooltip::Position::Bottom,
            ))
            .push(Space::new().width(8));
    }

    if show_logo {
        title_group = title_group.push(
            svg(svg::Handle::from_memory(icon::LOGO))
                .width(24)
                .height(24),
        );
    }

    let are_all_visible = app
        .get_filtered_calendars()
        .iter()
        .filter(|c| !crate::storage::is_system_calendar(&c.href))
        .all(|c| !app.hidden_calendars.contains(&c.href));

    // Resolve the actual dynamic message (handles Focus clearing vs Calendar toggling)
    let dynamic_msg = if let Message::ToggleAllCalendars(_) = title_msg {
        Message::ToggleAllCalendars(!are_all_visible)
    } else {
        title_msg.clone()
    };

    let title_btn = iced::widget::button(
        text(title_text)
            .size(20)
            .font(iced::Font::DEFAULT)
            .style(title_style),
    )
    .style(iced::widget::button::text)
    .padding(0)
    .on_press(dynamic_msg);

    title_group = title_group.push(tip(
        title_btn,
        format!("{} (*)", rust_i18n::t!("support_clear_filters")),
        tooltip::Position::Bottom,
    ));

    for other in other_visible_cals {
        let other_color = other
            .color
            .as_ref()
            .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.5, 0.5, 0.5));

        title_group = title_group.push(text("+").size(18).color(other_color).font(iced::Font {
            ..Default::default()
        }));
    }

    let mut left_section = row![title_group]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let (sync_icon_char, sync_icon_color, sync_tooltip) = if app.unsynced_changes {
        (
            icon::SYNC_ALERT,
            Color::from_rgb(0.92, 0.0, 0.0), // Red (#EB0000)
            if app.unsynced_tooltip.is_empty() {
                rust_i18n::t!("unsynced").to_string()
            } else {
                app.unsynced_tooltip.clone()
            },
        )
    } else if app.last_sync_failed {
        (
            icon::SYNC_OFF,
            Color::from_rgb(1.0, 0.702, 0.0), // Amber (#FFB300)
            rust_i18n::t!("sync_failed_retry").to_string(),
        )
    } else {
        (
            icon::REFRESH,
            app.theme().extended_palette().background.base.text,
            rust_i18n::t!("force_sync").to_string(),
        )
    };

    let refresh_btn =
        iced::widget::button(icon::icon(sync_icon_char).size(16).color(sync_icon_color))
            .style(iced::widget::button::text)
            .padding(4)
            .on_press(Message::Refresh);

    left_section = left_section.push(tip(refresh_btn, sync_tooltip, tooltip::Position::Bottom));

    let subtitle_text = text(subtitle)
        .size(14)
        .color(Color::from_rgb(0.6, 0.6, 0.6));
    let middle_container = container(subtitle_text)
        .width(Length::Fill)
        .height(Length::Shrink)
        .center_x(Length::Fill)
        .center_y(Length::Shrink);

    let is_dark_mode = app.theme().extended_palette().is_dark;

    let search_input = text_editor(&app.search_value)
        .id("header_search_input")
        .placeholder(&app.search_placeholder)
        .on_action(Message::SearchChanged)
        .highlight_with::<self::syntax::SmartInputHighlighter>(
            (is_dark_mode, true),
            |highlight, _theme| *highlight,
        )
        .padding(5)
        .height(Length::Fixed(32.0))
        .font(iced::Font::DEFAULT);

    let mut search_row = row![].align_y(iced::Alignment::Center).spacing(5);

    let random_btn = iced::widget::button(icon::icon(app.random_icon).size(16))
        .style(iced::widget::button::text)
        .padding(6)
        .on_press(Message::JumpToRandomTask);

    search_row = search_row.push(tip(
        random_btn,
        format!("{} (Shift+R)", rust_i18n::t!("jump_to_random_task")),
        tooltip::Position::Bottom,
    ));

    if app.show_quick_filter {
        let is_active = search_text.contains(&app.quick_filter_term);
        let qf_icon_char = crate::gui::icon::parse_icon(&app.quick_filter_icon);
        let qf_color = if is_active {
            app.theme().extended_palette().primary.base.color
        } else {
            app.theme().extended_palette().background.base.text
        };

        let qf_btn = iced::widget::button(icon::icon(qf_icon_char).size(16).color(qf_color))
            .style(iced::widget::button::text)
            .padding(6)
            .on_press(Message::ToggleQuickFilter);

        search_row = search_row.push(tip(
            qf_btn,
            rust_i18n::t!(
                "tooltip_toggle_quick_filter",
                term = app.quick_filter_term.clone()
            ),
            tooltip::Position::Bottom,
        ));
    }

    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let is_search_empty = search_text.is_empty();
    let is_search_error = is_filter_empty && !is_search_empty;

    let (search_icon_char, icon_color, on_press) = if is_search_empty {
        (icon::SEARCH, Color::from_rgb(0.4, 0.4, 0.4), None)
    } else {
        let icon_col = if is_search_error {
            Color::from_rgb(0.9, 0.2, 0.2)
        } else {
            app.theme().extended_palette().background.base.text
        };
        (icon::SEARCH_STOP, icon_col, Some(Message::ClearSearch))
    };

    let mut clear_btn =
        iced::widget::button(icon::icon(search_icon_char).size(14).color(icon_color))
            .style(iced::widget::button::text)
            .padding(4);

    if let Some(msg) = on_press {
        clear_btn = clear_btn.on_press(msg);
    }

    let search_input_container = container(search_input).padding(0);
    let final_search_widget = if is_search_error {
        search_input_container.style(|_| container::Style {
            border: iced::Border {
                color: Color::from_rgb(0.9, 0.2, 0.2),
                width: 1.5,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
    } else {
        search_input_container
    };

    search_row = search_row.push(if is_search_empty {
        Element::from(clear_btn)
    } else {
        tip(
            clear_btn,
            rust_i18n::t!("tooltip_clear_esc"),
            tooltip::Position::Bottom,
        )
    });
    search_row = search_row.push(final_search_widget);

    let window_controls = window_controls(app.force_ssd);

    let mut right_section = row![search_row]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    if app.sidebar_is_hidden {
        right_section = right_section
            .push(tip(
                iced::widget::button(icon::icon(icon::SETTINGS_GEAR).size(16))
                    .style(iced::widget::button::text)
                    .padding(4)
                    .on_press(Message::OpenSettings),
                rust_i18n::t!("settings"),
                tooltip::Position::Bottom,
            ))
            .push(tip(
                iced::widget::button(icon::icon(icon::HELP_RHOMBUS).size(16))
                    .style(iced::widget::button::text)
                    .padding(4)
                    .on_press(Message::OpenHelp(crate::help::HelpTab::Syntax)),
                rust_i18n::t!("help"),
                tooltip::Position::Bottom,
            ));
    }

    right_section = right_section.push(window_controls);

    let header_row = row![left_section, middle_container, right_section]
        .spacing(10)
        .padding(iced::Padding {
            top: 10.0,
            bottom: 5.0,
            left: 10.0,
            right: 10.0,
        })
        .align_y(iced::Alignment::Center);

    let header_drag_area = drag_area(header_row, app.force_ssd);

    let export_ui: Element<'_, Message>;
    if is_expanded {
        export_ui = Space::new().height(0).into();
    } else if let Some(active_href) = &app.active_cal_href {
        if active_href.starts_with("local://") && active_href != crate::storage::LOCAL_TRASH_HREF {
            let targets: Vec<_> = app
                .calendars
                .iter()
                .filter(|c| {
                    !c.href.starts_with("local://") && !app.disabled_calendars.contains(&c.href)
                })
                .collect();
            if !targets.is_empty() {
                let mut row = row![
                    text(rust_i18n::t!("export_to"))
                        .size(14)
                        .color(Color::from_rgb(0.5, 0.5, 0.5))
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center);
                for cal in targets {
                    let source_href = active_href.clone();
                    row = row.push(
                        iced::widget::button(text(&cal.name).size(12))
                            .style(iced::widget::button::secondary)
                            .padding(5)
                            .on_press(Message::MigrateLocalTo(source_href, cal.href.clone())),
                    );
                }
                export_ui = container(row)
                    .padding(iced::Padding {
                        left: 10.0,
                        bottom: 5.0,
                        ..Default::default()
                    })
                    .into();
            } else {
                export_ui = Space::new().height(0).into();
            }
        } else {
            export_ui = Space::new().height(0).into();
        }
    } else {
        export_ui = Space::new().height(0).into();
    }

    let input_area = view_input_area(app);
    let mut main_col = column![header_drag_area, export_ui];

    if let Some(err) = &app.error_msg {
        main_col = main_col.push(message_banner(
            err,
            app.theme().extended_palette().background.base.text,
            |_| Color::from_rgb(0.8, 0.2, 0.2).into(),
            Message::DismissError,
        ));
    }

    if let Some(info) = &app.info_msg {
        main_col = main_col.push(message_banner(
            info,
            app.theme().extended_palette().background.base.text,
            |theme: &Theme| theme.extended_palette().success.base.color.into(),
            Message::DismissInfo(app.info_msg_version),
        ));
    }

    main_col = main_col.push(input_area);

    if !is_expanded
        && let Some(uid) = &app.yanked_uid
        && let Some(summary) = app.store.get_summary(uid)
    {
        main_col = main_col.push(lock_bar(
            icon::LINK,
            rust_i18n::t!("yanked_label").to_string(),
            Some(summary),
            icon::LINK_LOCK,
            app.yank_lock_active,
            Message::ToggleYankLock,
        ));
    }

    if !is_expanded
        && let Some(uid) = &app.creating_child_of
        && let Some(summary) = app.store.get_summary(uid)
    {
        let child_label = rust_i18n::t!("new_child_of", name = summary.clone());
        main_col = main_col.push(lock_bar(
            icon::CHILD,
            child_label.to_string(),
            None,
            icon::PLUS_LOCK,
            app.child_lock_active,
            Message::ToggleChildLock,
        ));
    }

    if !is_expanded && search_text.starts_with('#') {
        let tag = search_text.trim_start_matches('#').trim().to_string();
        if !tag.is_empty() {
            main_col = main_col.push(jump_button(
                icon::TAG,
                rust_i18n::t!("go_to_tag", tag = tag.clone()).to_string(),
                Message::JumpToTag(tag),
            ));
        }
    }

    if !is_expanded && (search_text.starts_with("@@") || search_text.starts_with("loc:")) {
        let raw = if search_text.starts_with("@@") {
            search_text.trim_start_matches("@@")
        } else {
            search_text.trim_start_matches("loc:")
        };
        let loc = raw.trim().to_string();

        if !loc.is_empty() {
            main_col = main_col.push(jump_button(
                icon::LOCATION,
                rust_i18n::t!("go_to_location", loc = loc.clone()).to_string(),
                Message::JumpToLocation(loc),
            ));
        }
    }

    // We use a hasher to create a stable, `Copy`-able u64 key for the keyed_column
    use std::hash::{Hash, Hasher};

    let highlight_color = if app.theme().extended_palette().is_dark {
        Color::from_rgb(1.0, 0.9, 0.1) // Bright yellow
    } else {
        Color::from_rgb(0.8, 0.2, 0.0) // Reddish for light theme
    };
    let highlight_regex = app.search_highlight_regex.clone();

    let tasks_view =
        iced::widget::keyed_column(app.tasks.iter().enumerate().map(|(real_index, item)| {
            let row_id = match item {
                crate::store::TaskListItem::Task(t) => app
                    .task_ids
                    .get(&t.uid)
                    .cloned()
                    .unwrap_or_else(iced::widget::Id::unique),
                _ => iced::widget::Id::unique(),
            };

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            match item {
                crate::store::TaskListItem::Task(t) => {
                    // STABLE KEY: Use only the UID.
                    // This allows the task to move without losing focus/state.
                    0u8.hash(&mut hasher);
                    t.uid.hash(&mut hasher);
                }
                crate::store::TaskListItem::ExpandGroup(k, _) => {
                    // POSITION KEY: Use index.
                    // Virtual rows don't have unique UIDs, so we pin them to position.
                    1u8.hash(&mut hasher);
                    k.hash(&mut hasher);
                    real_index.hash(&mut hasher);
                }
                crate::store::TaskListItem::CollapseGroup(k, _) => {
                    2u8.hash(&mut hasher);
                    k.hash(&mut hasher);
                    real_index.hash(&mut hasher);
                }
            };
            let key = hasher.finish();

            (
                key,
                view_task_row(
                    app,
                    real_index,
                    item,
                    row_id,
                    highlight_regex.clone(),
                    highlight_color,
                ),
            )
        }))
        .spacing(1);

    if app.editor_maximized && is_expanded {
        // Skip rendering tasks_view to maximize the editor space
    } else {
        main_col = main_col.push(
            scrollable(tasks_view)
                .height(Length::Fill)
                .id(app.scrollable_id.clone())
                .direction(Direction::Vertical(
                    Scrollbar::new().width(10).scroller_width(10).margin(0),
                )),
        );
    }

    container(main_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            right: 8.0,
            ..Default::default()
        })
        .into()
}

/// Word-level deletion (Ctrl+Backspace / Ctrl+Delete, Option+ on macOS) on
/// top of iced's default key bindings. The custom closure replaces the
/// defaults entirely, so every other key press must fall back to
/// `Binding::from_key_press`, which also handles the unfocused case.
fn word_delete_key_binding(
    key_press: text_editor::KeyPress,
) -> Option<text_editor::Binding<Message>> {
    use iced::keyboard::key::Named;
    use text_editor::{Binding, Motion, Status};

    let is_word_delete_key = matches!(
        key_press.key,
        iced::keyboard::Key::Named(Named::Backspace | Named::Delete)
    );
    let word_mod =
        key_press.modifiers.control() || (cfg!(target_os = "macos") && key_press.modifiers.alt());

    if matches!(key_press.status, Status::Focused { .. }) && is_word_delete_key && word_mod {
        let is_backspace = matches!(key_press.key, iced::keyboard::Key::Named(Named::Backspace));
        let motion = if is_backspace {
            Motion::WordLeft
        } else {
            Motion::WordRight
        };
        return Some(Binding::Sequence(vec![
            Binding::Select(motion),
            Binding::Delete,
        ]));
    }
    Binding::from_key_press(key_press)
}

fn view_input_area(app: &GuiApp) -> Element<'_, Message> {
    let is_dark_mode = app.theme().extended_palette().is_dark;

    let is_expanded =
        app.editing_uid.is_some() || app.editing_tree_uid.is_some() || app.creating_with_desc;

    let input_title = text_editor(&app.input_value)
        .id("main_input")
        .placeholder(&app.current_placeholder)
        .on_action(Message::InputChanged)
        .key_binding(word_delete_key_binding)
        .highlight_with::<self::syntax::SmartInputHighlighter>(
            (is_dark_mode, false),
            |highlight, _theme| *highlight,
        )
        .padding(10)
        .height(Length::Fixed(45.0))
        .font(iced::Font::DEFAULT);

    let expand_btn = iced::widget::button(icon::icon(icon::DETAILED_TRIANGLE).size(16))
        .style(iced::widget::button::text)
        .padding(12)
        .on_press(Message::StartCreateWithDescription);

    let expand_tooltip = tooltip(
        expand_btn,
        text(rust_i18n::t!("add_description_tooltip")).size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style);

    let title_row = if app.editing_tree_uid.is_some() {
        row![]
    } else if is_expanded {
        row![container(input_title).width(Length::Fill)].align_y(iced::Alignment::Center)
    } else {
        row![container(input_title).width(Length::Fill), expand_tooltip]
            .align_y(iced::Alignment::Center)
    };

    let is_desc_focused = app.last_edited_field == 1 || app.editing_tree_uid.is_some();
    let active_content = if is_desc_focused {
        &app.description_value
    } else {
        &app.input_value
    };
    let context_banner = build_context_banner(app, active_content);

    let inner_content: Element<'_, Message> = if is_expanded {
        let banner_height = if context_banner.is_some() { 65.0 } else { 0.0 };
        let max_desc_height = (app.current_window_size.height - 190.0 - banner_height).max(160.0);

        let placeholder = if app.creating_with_desc {
            rust_i18n::t!("notes_create_subtasks_placeholder").into_owned()
        } else {
            app.notes_placeholder.clone()
        };

        // Estimate wrapped line height more accurately
        let text_content = app.description_value.text();
        let mut visual_lines = 0.0;
        let approx_char_width = 8.0;
        let available_width_px = if app.sidebar_is_hidden {
            app.current_window_size.width - 60.0
        } else {
            app.current_window_size.width - 280.0
        }
        .max(100.0);
        let chars_per_line = (available_width_px / approx_char_width).max(20.0);

        for line in text_content.lines() {
            let len = line.chars().count() as f32;
            if len == 0.0 {
                visual_lines += 1.0;
            } else {
                visual_lines += (len / chars_per_line).ceil();
            }
        }
        if text_content.ends_with('\n') || text_content.is_empty() {
            visual_lines += 1.0;
        }

        let calculated_height = visual_lines * 21.0 + 30.0;
        let estimated_height = if app.editor_maximized {
            calculated_height // Unbounded because length is Fill
        } else {
            calculated_height.clamp(160.0, max_desc_height)
        };

        let is_focused = app.active_focus == Focus::AddTaskInput;

        let input_desc = text_editor(&app.description_value)
            .id("description_input")
            .placeholder(placeholder)
            .on_action(Message::DescriptionChanged)
            .key_binding(word_delete_key_binding)
            .highlight_with::<self::syntax::MarkdownHighlighter>(
                is_dark_mode,
                |highlight, _theme| *highlight,
            )
            .padding(10)
            .height(if app.editor_maximized {
                Length::Fill
            } else {
                Length::Shrink
            })
            .min_height(estimated_height)
            .style(move |theme: &Theme, status| {
                let class = <Theme as iced::widget::text_editor::Catalog>::default();
                let mut style =
                    <Theme as iced::widget::text_editor::Catalog>::style(theme, &class, status);
                style.background = iced::Background::Color(Color::TRANSPARENT);
                style.border.width = 0.0;
                style
            });

        let scrollable_desc = scrollable(input_desc)
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(Direction::Vertical(
                Scrollbar::new().width(10).scroller_width(10),
            ));

        let desc_container = container(scrollable_desc)
            .width(Length::Fill)
            .height(if app.editor_maximized {
                Length::Fill
            } else {
                Length::Fixed(estimated_height)
            })
            .style(move |theme: &Theme| {
                let status = if is_focused {
                    iced::widget::text_editor::Status::Focused { is_hovered: false }
                } else {
                    iced::widget::text_editor::Status::Active
                };
                let class = <Theme as iced::widget::text_editor::Catalog>::default();
                let te_style =
                    <Theme as iced::widget::text_editor::Catalog>::style(theme, &class, status);
                container::Style {
                    background: Some(te_style.background),
                    border: te_style.border,
                    ..Default::default()
                }
            });

        let cancel_btn = tip(
            iced::widget::button(text(rust_i18n::t!("cancel")).size(16))
                .style(iced::widget::button::secondary)
                .on_press(Message::CancelEdit),
            rust_i18n::t!("tooltip_cancel_esc"),
            tooltip::Position::Top,
        );

        let save_btn = tip(
            iced::widget::button(text(rust_i18n::t!("save")).size(16))
                .style(iced::widget::button::primary)
                .on_press(Message::SubmitTask),
            rust_i18n::t!("tooltip_save_ctrl_s"),
            tooltip::Position::Top,
        );

        let apply_btn = tip(
            iced::widget::button(icon::icon(icon::CONTENT_SAVE_EDIT).size(16))
                .style(iced::widget::button::secondary)
                .on_press(Message::SaveTaskKeepEditing),
            rust_i18n::t!("tooltip_save_keep_editing"),
            tooltip::Position::Top,
        );

        let maximize_btn = tip(
            iced::widget::button(icon::icon(icon::MAXIMIZE).size(16))
                .style(iced::widget::button::secondary)
                .on_press(Message::ToggleEditorMaximize),
            if app.editor_maximized {
                rust_i18n::t!("tooltip_restore_ctrl_m")
            } else {
                rust_i18n::t!("tooltip_maximize_ctrl_m")
            },
            tooltip::Position::Top,
        );

        let header_label = if app.creating_with_desc {
            rust_i18n::t!("mode_create").into_owned()
        } else if app.editing_tree_uid.is_some() {
            rust_i18n::t!("edit_tree_title").into_owned()
        } else {
            rust_i18n::t!("editing").into_owned()
        };

        let switch_btn = if !app.creating_with_desc {
            let (icon_char, label) = if app.editing_tree_uid.is_some() {
                (
                    icon::EDIT,
                    format!(
                        "{} & {}",
                        rust_i18n::t!("save"),
                        rust_i18n::t!("description_label")
                    ),
                )
            } else {
                (
                    icon::EDIT_TREE,
                    format!(
                        "{} & {}",
                        rust_i18n::t!("save"),
                        rust_i18n::t!("edit_tree_title")
                    ),
                )
            };

            if icon_char != '\0' {
                Some(tip(
                    iced::widget::button(icon::icon(icon_char).size(16))
                        .style(iced::widget::button::secondary)
                        .on_press(Message::SaveAndSwitchEditor),
                    label,
                    tooltip::Position::Top,
                ))
            } else {
                None
            }
        } else {
            None
        };

        let mut top_bar = row![
            text(header_label)
                .size(14)
                .color(Color::from_rgb(0.7, 0.7, 1.0)),
            Space::new().width(Length::Fill),
        ];

        top_bar = top_bar.push(maximize_btn);

        if let Some(btn) = switch_btn {
            top_bar = top_bar.push(btn);
        }

        let top_bar = top_bar
            .push(apply_btn)
            .push(cancel_btn)
            .push(save_btn)
            .align_y(iced::Alignment::Center)
            .spacing(10);

        let banner_element = if let Some(banner) = context_banner {
            column![Space::new().height(4), banner]
        } else {
            column![]
        };

        let col = if app.editing_tree_uid.is_some() {
            column![top_bar, desc_container, banner_element]
        } else {
            column![top_bar, title_row, desc_container, banner_element]
        };

        col.spacing(10)
            .height(if app.editor_maximized {
                Length::Fill
            } else {
                Length::Shrink
            })
            .into()
    } else {
        let mut col = column![title_row].spacing(5);
        if let Some(banner) = context_banner {
            col = col.push(banner);
        }
        col.into()
    };

    container(inner_content)
        .padding(iced::Padding {
            top: 5.0,
            bottom: 8.0,
            left: 10.0,
            right: 10.0,
        })
        .height(if app.editor_maximized && is_expanded {
            Length::Fill
        } else {
            Length::Shrink
        })
        .into()
}

fn view_ics_import_overlay<'a>(app: &'a GuiApp) -> Element<'a, Message> {
    let file_name = app
        .ics_import_file_path
        .as_ref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("file.ics");

    let task_count = app.ics_import_task_count.unwrap_or(0);

    let icon_header = modal_icon_header(icon::IMPORT, Color::from_rgb(0.3, 0.7, 1.0));
    let title = modal_title(rust_i18n::t!("ics_import_title"));

    let file_info = column![
        text(rust_i18n::t!("import_file_name", file = file_name))
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7)),
        text(if task_count == 1 {
            rust_i18n::t!("found_tasks_to_import.one").to_string()
        } else {
            rust_i18n::t!("found_tasks_to_import.other", count = task_count).to_string()
        })
        .size(14)
        .color(Color::from_rgb(0.7, 0.7, 0.7)),
    ]
    .spacing(5)
    .align_x(iced::Alignment::Center);

    let select_label = text(rust_i18n::t!("select_target_collection"))
        .size(16)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..Default::default()
        });

    let mut calendar_list = column![].spacing(5);
    for cal in &app.calendars {
        if app.disabled_calendars.contains(&cal.href)
            || crate::storage::is_system_calendar(&cal.href)
        {
            continue;
        }

        let is_selected = app.ics_import_selected_calendar.as_ref() == Some(&cal.href);

        let cal_button = button(
            row![
                if is_selected {
                    icon::icon(icon::CHECK)
                        .size(14)
                        .color(Color::from_rgb(0.3, 0.7, 1.0))
                } else {
                    text(" ").size(14)
                },
                text(&cal.name).size(14),
                if cal.href.starts_with("local://") {
                    text(rust_i18n::t!("local_collection_suffix"))
                        .size(12)
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                } else {
                    text("").size(12)
                }
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .padding(10)
        .style(if is_selected {
            |theme: &Theme, _status| iced::widget::button::Style {
                background: Some(Color::from_rgb(0.2, 0.4, 0.6).into()),
                text_color: theme.extended_palette().background.base.text,
                border: iced::Border {
                    radius: 4.0.into(),
                    width: 2.0,
                    color: Color::from_rgb(0.3, 0.7, 1.0),
                },
                ..iced::widget::button::Style::default()
            }
        } else {
            button::secondary
        })
        .on_press(Message::IcsImportDialogCalendarSelected(cal.href.clone()));

        calendar_list = calendar_list.push(cal_button);
    }

    let calendar_scroll = scrollable(calendar_list)
        .id(iced::widget::Id::new("ics_import_scrollable"))
        .height(Length::Fixed(250.0))
        .direction(Direction::Vertical(
            Scrollbar::new().width(8).scroller_width(8),
        ));

    let cancel_btn = button(text(rust_i18n::t!("cancel")).size(14))
        .style(iced::widget::button::secondary)
        .padding([8, 16])
        .on_press(Message::IcsImportDialogCancel);

    let import_btn = button(
        text(rust_i18n::t!("import_action"))
            .size(14)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
    )
    .style(iced::widget::button::primary)
    .padding([8, 16]);

    let import_btn = if app.ics_import_selected_calendar.is_some() && task_count > 0 {
        import_btn.on_press(Message::IcsImportDialogConfirm)
    } else {
        import_btn
    };

    let buttons = row![cancel_btn, import_btn]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let modal_content = column![
        icon_header,
        title,
        Space::new().height(Length::Fixed(10.0)),
        file_info,
        Space::new().height(Length::Fixed(20.0)),
        select_label,
        Space::new().height(Length::Fixed(10.0)),
        calendar_scroll,
        Space::new().height(Length::Fixed(20.0)),
        buttons
    ]
    .spacing(5)
    .align_x(iced::Alignment::Center);

    let modal_card = container(modal_content)
        .padding(20)
        .width(Length::Fixed(450.0))
        .max_height(600.0)
        .style(|theme: &Theme| modal_card_style(0.98, theme));

    modal_backdrop(modal_card, 0.7)
}

fn view_journal_main_pane<'a>(app: &'a GuiApp) -> Element<'a, Message> {
    let date = app.journal_date;
    let date_str = rust_i18n::t!(
        "journal_date_full",
        weekday = crate::model::display::local_weekday_name(date.weekday()),
        month = crate::model::display::local_month_name(date.month()),
        day = date.day(),
        year = date.year()
    )
    .to_string();

    let mut visible_cals_set = HashSet::new();
    let mut visible_cals = Vec::new();

    for c in &app.calendars {
        let supports = if c.href.starts_with("local://") {
            true
        } else {
            c.supports_vjournal.unwrap_or(false)
        };

        if !app.hidden_calendars.contains(&c.href)
            && !app.disabled_calendars.contains(&c.href)
            && !crate::storage::is_system_calendar(&c.href)
            && supports
        {
            visible_cals_set.insert(c.href.clone());
            visible_cals.push(c.clone());
        }
    }

    // Calendars that already have a VJOURNAL entry for this date, computed
    // once so the sort and the buttons below don't re-scan the store.
    let entry_cals: HashSet<String> = visible_cals
        .iter()
        .filter(|c| app.store.get_journal_entry(&c.href, date).is_some())
        .map(|c| c.href.clone())
        .collect();

    visible_cals.sort_by_key(|c| if entry_cals.contains(&c.href) { 0 } else { 1 });

    let active_href = app
        .journal_editing_href
        .as_ref()
        .or(app.active_cal_href.as_ref())
        .cloned()
        .unwrap_or_else(|| {
            visible_cals
                .first()
                .map(|c| c.href.clone())
                .unwrap_or_else(|| crate::storage::LOCAL_CALENDAR_HREF.to_string())
        });

    let active_name = app
        .calendars
        .iter()
        .find(|c| c.href == active_href)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| active_href.clone());

    let mut cal_buttons = row![].spacing(6).align_y(iced::Alignment::Center);
    for cal in &visible_cals {
        let is_selected = cal.href == active_href;
        let has_entry = entry_cals.contains(&cal.href);
        let cal_name = cal.name.clone();
        let cal_href = cal.href.clone();
        let mut cal_color = cal
            .color
            .as_ref()
            .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.7, 0.7, 0.7));

        if !is_selected && !has_entry {
            cal_color.a *= 0.4;
        } else if !is_selected {
            cal_color.a *= 0.8;
        }

        let btn_style = move |_theme: &Theme, status: button::Status| {
            if is_selected {
                button::Style {
                    background: Some(
                        Color {
                            a: 0.25,
                            ..cal_color
                        }
                        .into(),
                    ),
                    text_color: cal_color,
                    border: iced::Border {
                        width: 1.5,
                        color: cal_color,
                        radius: 6.0.into(),
                    },
                    ..button::Style::default()
                }
            } else {
                let bg_alpha = match status {
                    button::Status::Hovered => 0.1,
                    _ => 0.0,
                };
                button::Style {
                    background: Some(
                        Color {
                            a: bg_alpha,
                            ..cal_color
                        }
                        .into(),
                    ),
                    text_color: cal_color,
                    border: iced::Border {
                        width: 1.0,
                        color: Color {
                            a: cal_color.a * 0.5,
                            ..cal_color
                        },
                        radius: 6.0.into(),
                    },
                    ..button::Style::default()
                }
            }
        };

        let btn = button(
            row![
                icon::icon(if has_entry {
                    app.journal_icon
                } else {
                    icon::EDIT
                })
                .size(12),
                text(cal_name).size(13),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 10])
        .style(btn_style)
        .on_press(Message::SelectJournalCollection(cal_href));

        cal_buttons = cal_buttons.push(btn);
    }

    let has_remote_cal = app
        .calendars
        .iter()
        .any(|c| !c.href.starts_with("local://"));
    let has_remote_journal_cal = visible_cals.iter().any(|c| !c.href.starts_with("local://"));
    if has_remote_cal && !has_remote_journal_cal {
        cal_buttons = cal_buttons.push(
            text(rust_i18n::t!("journal_no_remote_vjournal").to_string())
                .size(12)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        );
    }

    let cal_selector = scrollable(cal_buttons)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(4)
                .scroller_width(4)
                .margin(0),
        ))
        .width(Length::Fill);

    let window_controls = window_controls(app.force_ssd);

    let (header_title, show_activity) = if let Some(uid) = &app.journal_editing_uid {
        let title = app
            .store
            .get_summary(uid)
            .unwrap_or_else(|| "Untitled Page".to_string());
        (title, false)
    } else {
        (date_str, true)
    };

    let current_day_uid = app.journal_editing_uid.clone().or_else(|| {
        app.store
            .get_journal_entry(&active_href, date)
            .map(|t| t.uid.clone())
    });

    let header_row = if let Some(uid) = current_day_uid.clone() {
        let delete_btn = tooltip(
            iced::widget::button(icon::icon(icon::TRASH).size(14))
                .style(iced::widget::button::danger)
                .padding(8)
                .on_press(Message::DeleteTaskTree(uid.clone())),
            text(rust_i18n::t!("delete_task_tree")).size(12),
            tooltip::Position::Bottom,
        )
        .style(crate::gui::view::tooltip_style);

        let move_btn = tooltip(
            iced::widget::button(icon::icon(icon::MOVE).size(14))
                .style(iced::widget::button::secondary)
                .padding(8)
                .on_press(Message::StartMoveTask(uid.clone())),
            text(rust_i18n::t!("menu_move")).size(12),
            tooltip::Position::Bottom,
        )
        .style(crate::gui::view::tooltip_style);

        let create_subpage_btn = tooltip(
            iced::widget::button(icon::icon(icon::CREATE_CHILD).size(14))
                .style(iced::widget::button::secondary)
                .padding(8)
                .on_press(Message::CreateJournalSubPage(uid.clone())),
            text(rust_i18n::t!("create_subtask")).size(12),
            tooltip::Position::Bottom,
        )
        .style(crate::gui::view::tooltip_style);

        if app.journal_editing_uid.is_some() {
            row![
                iced::widget::text_input("Page title...", &app.journal_title_input)
                    .on_input(Message::JournalTitleInputChanged)
                    .size(22)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .padding(5)
                    .width(Length::FillPortion(2)),
                Space::new().width(Length::FillPortion(1)),
                create_subpage_btn,
                move_btn,
                delete_btn,
                Space::new().width(15),
                window_controls
            ]
            .align_y(iced::Alignment::Center)
        } else {
            row![
                text(header_title).size(22).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
                Space::new().width(Length::Fill),
                create_subpage_btn,
                move_btn,
                delete_btn,
                Space::new().width(15),
                window_controls
            ]
            .align_y(iced::Alignment::Center)
        }
    } else {
        row![
            text(header_title).size(22).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
            Space::new().width(Length::Fill),
            window_controls
        ]
        .align_y(iced::Alignment::Center)
    };

    let header_container = container(header_row).padding(iced::Padding {
        top: 10.0,
        bottom: 5.0,
        left: 10.0,
        right: 10.0,
    });

    let header_drag_area = drag_area(header_container, app.force_ssd);

    let top_header = if app.journal_editing_uid.is_some() {
        column![header_drag_area].spacing(0)
    } else {
        column![
            header_drag_area,
            container(cal_selector).padding(iced::Padding {
                left: 10.0,
                right: 10.0,
                top: 5.0,
                bottom: 5.0
            })
        ]
        .spacing(0)
    };

    let is_dark_mode = app.theme().extended_palette().is_dark;

    let editor = text_editor(&app.journal_editor_content)
        .id("journal_editor")
        .placeholder(if app.journal_editing_uid.is_some() {
            rust_i18n::t!("notes_placeholder").to_string()
        } else {
            rust_i18n::t!("journal_no_notes", name = active_name).to_string()
        })
        .on_action(Message::JournalContentChanged)
        .key_binding(word_delete_key_binding)
        .highlight_with::<self::syntax::MarkdownHighlighter>(is_dark_mode, |highlight, _theme| {
            *highlight
        })
        .padding(12)
        .height(Length::Fill)
        .style(move |theme: &Theme, status| {
            let class = <Theme as iced::widget::text_editor::Catalog>::default();
            let mut style =
                <Theme as iced::widget::text_editor::Catalog>::style(theme, &class, status);
            style.background = iced::Background::Color(Color::TRANSPARENT);
            style.border.width = 0.0;
            style
        });

    let context_banner = build_context_banner(app, &app.journal_editor_content);

    let banner_element = if let Some(banner) = context_banner {
        column![Space::new().height(4), banner]
    } else {
        column![]
    };

    let editor_col = column![editor, banner_element];

    let editor_container = container(editor_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(journal_panel_style);

    let day_ctx = app.store.get_day_context(date, &visible_cals_set);
    let mut activity_col = column![].spacing(6);

    let act_title = row![
        icon::icon(icon::REFRESH)
            .size(14)
            .color(Color::from_rgb(1.0, 0.6, 0.0)),
        text(rust_i18n::t!("journal_activity"))
            .size(14)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    activity_col = activity_col.push(act_title);

    let mut has_any_activity = false;

    if day_ctx.total_tracked_mins > 0 {
        has_any_activity = true;
        activity_col = activity_col.push(
            row![
                icon::icon(icon::TIMER_SETTINGS)
                    .size(12)
                    .color(Color::from_rgb(0.4, 0.8, 0.4)),
                text(format!(
                    "{}: {} ({})",
                    rust_i18n::t!("journal_time_tracked"),
                    crate::model::parser::format_duration_human(day_ctx.total_tracked_mins),
                    day_ctx.session_tasks.len()
                ))
                .size(13)
                .color(Color::from_rgb(0.4, 0.8, 0.4))
            ]
            .spacing(6),
        );
    }

    let mut worked_on_uids = std::collections::HashSet::new();
    let mut worked_on_tasks = Vec::new();
    for t in &day_ctx.ongoing_tasks {
        if worked_on_uids.insert(t.uid.clone()) {
            worked_on_tasks.push(t.clone());
        }
    }
    for (t, _) in &day_ctx.session_tasks {
        if worked_on_uids.insert(t.uid.clone()) {
            worked_on_tasks.push(t.clone());
        }
    }

    if let Some(el) = journal_activity_row(
        icon::PLAY_FA,
        Color::from_rgb(0.4, 0.8, 0.4),
        rust_i18n::t!("journal_started_today").to_string(),
        &day_ctx.started_tasks,
    ) {
        has_any_activity = true;
        activity_col = activity_col.push(el);
    }

    if let Some(el) = journal_activity_row(
        icon::TIMER_SETTINGS,
        Color::from_rgb(0.4, 0.8, 0.4),
        rust_i18n::t!("journal_worked_on_today").to_string(),
        &worked_on_tasks,
    ) {
        has_any_activity = true;
        activity_col = activity_col.push(el);
    }

    if let Some(el) = journal_activity_row(
        icon::CHECK,
        Color::from_rgb(0.2, 0.8, 0.2),
        rust_i18n::t!("journal_completed_today").to_string(),
        &day_ctx.completed_tasks,
    ) {
        has_any_activity = true;
        activity_col = activity_col.push(el);
    }

    if let Some(el) = journal_activity_row(
        icon::CALENDAR,
        Color::from_rgb(1.0, 0.6, 0.2),
        rust_i18n::t!("journal_due_today").to_string(),
        &day_ctx.due_tasks,
    ) {
        has_any_activity = true;
        activity_col = activity_col.push(el);
    }

    if !has_any_activity {
        activity_col = activity_col.push(
            text(rust_i18n::t!("journal_no_activity"))
                .size(12)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        );
    }

    let activity_scroll = scrollable(activity_col).height(Length::Shrink).direction(
        iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(6),
        ),
    );

    let activity_container = container(activity_scroll)
        .width(Length::Fill)
        .max_height(250.0)
        .padding(12)
        .style(journal_panel_style);

    let mut main_col = column![
        top_header,
        container(editor_container)
            .padding(iced::Padding {
                left: 10.0,
                right: 10.0,
                top: 0.0,
                bottom: 0.0
            })
            .height(Length::Fill)
    ];

    if show_activity {
        main_col =
            main_col
                .push(Space::new().height(4))
                .push(container(activity_container).padding(iced::Padding {
                    left: 10.0,
                    right: 10.0,
                    top: 0.0,
                    bottom: 10.0,
                }));
    }

    main_col
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn build_context_banner<'a>(
    app: &'a GuiApp,
    content: &text_editor::Content,
) -> Option<Element<'a, Message>> {
    use crate::model::parser::{LEXICON, SyntaxType};
    let target_text = content.text();
    let cursor_pos = 'offset: {
        let cursor = content.cursor().position;
        let mut byte_offset = 0;
        for (current_line, line_str) in target_text.split('\n').enumerate() {
            if current_line == cursor.line {
                let col_bytes: usize = line_str
                    .chars()
                    .take(cursor.column)
                    .map(|c| c.len_utf8())
                    .sum();
                break 'offset byte_offset + col_bytes;
            }
            byte_offset += line_str.len() + 1; // +1 for '\n'
        }
        byte_offset
    };

    if let Some((range, suggs)) = crate::model::autocomplete::suggest(
        &target_text,
        cursor_pos,
        &app.store,
        &app.tag_aliases,
        &app.calendars,
    ) {
        let mut sugg_row = row![].spacing(8).padding(iced::Padding {
            bottom: 8.0,
            ..Default::default()
        });
        for s in suggs {
            let color = if s.display.starts_with('#') {
                let (r, g, b) =
                    crate::color_utils::generate_color(s.display.trim_start_matches('#'));
                Color::from_rgb(r, g, b)
            } else if s.display.starts_with("@@") {
                Color::from_rgb(0.8, 0.5, 0.0)
            } else if s.display.starts_with(':') {
                Color::from_rgb(0.9, 0.2, 0.9)
            } else {
                Color::from_rgb(0.2, 0.7, 1.0)
            };

            let btn = button(
                row![
                    text(s.display).size(14).color(color).font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                    text(s.description)
                        .size(12)
                        .style(move |theme: &Theme| text::Style {
                            color: Some(theme.extended_palette().background.weak.text),
                        })
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .style(
                move |_theme: &Theme, status: iced::widget::button::Status| {
                    let bg_alpha = match status {
                        iced::widget::button::Status::Hovered
                        | iced::widget::button::Status::Pressed => 0.25,
                        _ => 0.15,
                    };
                    iced::widget::button::Style {
                        background: Some(
                            Color {
                                a: bg_alpha,
                                ..color
                            }
                            .into(),
                        ),
                        text_color: color,
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: Color { a: 0.5, ..color },
                        },
                        ..iced::widget::button::Style::default()
                    }
                },
            )
            .padding([6, 12])
            .on_press(Message::ApplySuggestion(range.clone(), s.replacement));

            sugg_row = sugg_row.push(btn);
        }
        return Some(
            container(
                scrollable(sugg_row).direction(iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new()
                        .width(4)
                        .scroller_width(4)
                        .margin(0),
                )),
            )
            .width(Length::Fill)
            .padding([8, 12])
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(
                        Color {
                            a: 0.5,
                            ..palette.background.weak.color
                        }
                        .into(),
                    ),
                    border: iced::Border {
                        color: Color {
                            a: 0.5,
                            ..palette.background.strong.color
                        },
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into(),
        );
    } else {
        let mut active_context: Option<(SyntaxType, String)> = None;
        let line_start = target_text[..cursor_pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = target_text[cursor_pos..]
            .find('\n')
            .map(|i| cursor_pos + i)
            .unwrap_or(target_text.len());
        let current_line = &target_text[line_start..line_end];
        let local_cursor = cursor_pos - line_start;

        let tokens = crate::model::parser::tokenize_smart_input(current_line, false);
        for t in tokens {
            if local_cursor >= t.start && local_cursor <= t.end {
                if matches!(
                    t.kind,
                    SyntaxType::Dependency
                        | SyntaxType::Relation
                        | SyntaxType::WikiLink
                        | SyntaxType::Url
                ) {
                    active_context = Some((t.kind, current_line[t.start..t.end].to_string()));
                }
                break;
            }
        }

        if let Some((kind, raw_word)) = active_context {
            let clean_uid = if kind == SyntaxType::WikiLink {
                crate::model::parser::strip_quotes(
                    raw_word.trim_start_matches("[[").trim_end_matches("]]"),
                )
            } else if kind == SyntaxType::Url {
                crate::model::parser::strip_quotes(raw_word.trim_start_matches("url:"))
            } else {
                let lex_guard = LEXICON.read().unwrap();
                let lower = raw_word.to_lowercase();
                match lex_guard.extract_prefix(&raw_word, &lower) {
                    Some((_, _, rem_original)) => crate::model::parser::strip_quotes(rem_original),
                    None => crate::model::parser::strip_quotes(&raw_word),
                }
            };

            if !clean_uid.is_empty() {
                let context_uid = app
                    .editing_tree_uid
                    .as_ref()
                    .or(app.editing_uid.as_ref())
                    .or(app.creating_child_of.as_ref())
                    .or(app.journal_editing_uid.as_ref())
                    .map(|s| s.as_str());

                let is_url = clean_uid.contains("://") || clean_uid.starts_with("mailto:");

                let (icon_char, color, text_str, msg) = if is_url {
                    (
                        icon::URL_CHECK,
                        Color::from_rgb(0.2, 0.7, 1.0),
                        rust_i18n::t!("menu_open_link").to_string(),
                        Some(Message::OpenUrl(clean_uid.clone())),
                    )
                } else {
                    match app.store.resolve_dependency_ref(&clean_uid, context_uid) {
                        Ok(resolved_uid) => {
                            if let Some(summary) = app.store.get_summary(&resolved_uid) {
                                let icon = if kind == SyntaxType::Dependency {
                                    icon::BLOCKED
                                } else {
                                    icon::LINK
                                };
                                let color = if kind == SyntaxType::Dependency {
                                    Color::from_rgb(0.9, 0.6, 0.2)
                                } else {
                                    Color::from_rgb(0.4, 0.6, 0.9)
                                };
                                (
                                    icon,
                                    color,
                                    summary,
                                    Some(Message::JumpToTask(resolved_uid)),
                                )
                            } else {
                                (
                                    icon::INFO,
                                    Color::from_rgb(0.5, 0.5, 0.5),
                                    rust_i18n::t!("resolving").to_string(),
                                    None,
                                )
                            }
                        }
                        Err(_) => {
                            if kind == SyntaxType::WikiLink {
                                (
                                    icon::NEW_FILE,
                                    Color::from_rgb(0.2, 0.7, 1.0),
                                    rust_i18n::t!("create_named", name = clean_uid.clone())
                                        .to_string(),
                                    Some(Message::OpenWikiLink(
                                        clean_uid.clone(),
                                        context_uid.map(|s| s.to_string()),
                                    )),
                                )
                            } else {
                                (
                                    icon::SYNC_ALERT,
                                    Color::from_rgb(0.9, 0.2, 0.2),
                                    rust_i18n::t!("unknown_uid", name = clean_uid.clone())
                                        .to_string(),
                                    None,
                                )
                            }
                        }
                    }
                };

                let mut btn = button(
                    row![
                        icon::icon(icon_char).size(14).color(color),
                        text(format!("{} ➔ {}", raw_word, text_str))
                            .size(14)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .color(color)
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .style(iced::widget::button::text)
                .padding(0);

                if let Some(m) = msg {
                    btn = btn.on_press(m);
                }

                return Some(
                    container(btn)
                        .padding([6, 12])
                        .width(Length::Fill)
                        .style(move |theme: &Theme| {
                            let palette = theme.extended_palette();
                            container::Style {
                                background: Some(
                                    Color {
                                        a: 0.5,
                                        ..palette.background.weak.color
                                    }
                                    .into(),
                                ),
                                border: iced::Border {
                                    color: Color { a: 0.5, ..color },
                                    width: 1.0,
                                    radius: 8.0.into(),
                                },
                                ..Default::default()
                            }
                        })
                        .into(),
                );
            }
        }
    }
    None
}
