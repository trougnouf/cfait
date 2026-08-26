// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/gui/view/sidebar.rs
// Renders the sidebar (calendars, tags, locations) for the GUI.

use super::tooltip_style;
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::storage::LOCAL_TRASH_HREF;

use crate::store::UNCATEGORIZED_ID;
use iced::never;
use iced::widget::{
    MouseArea, Space, button, column, container, rich_text, row, scrollable, span, text, toggler,
    tooltip,
};
use iced::{Color, Element, Length, Theme};
use std::time::Duration;

// --- CALENDARS ---
pub fn view_sidebar_calendars(app: &GuiApp) -> Element<'_, Message> {
    let visible_calendars = app.get_filtered_calendars();

    let are_all_visible = visible_calendars
        .iter()
        .filter(|c| c.href != LOCAL_TRASH_HREF && c.href != "local://recovery")
        .all(|c| !app.hidden_calendars.contains(&c.href));

    let theme = app.theme();
    let toggler_style = |theme: &Theme, status: toggler::Status| -> toggler::Style {
        let mut style = toggler::default(theme, status);
        match status {
            toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled }
                if is_toggled =>
            {
                style.background = Color::from_rgb(1.0, 0.6, 0.0).into();
                style.foreground = theme.extended_palette().background.base.text.into();
            }
            _ => {}
        }
        style
    };

    let toggle_all = toggler(are_all_visible)
        .label(rust_i18n::t!("show_all").to_string())
        .text_size(12)
        .text_alignment(iced::alignment::Horizontal::Left)
        .spacing(10)
        .width(Length::Fill)
        .on_toggle(Message::ToggleAllCalendars)
        .style(toggler_style);

    let toggle_container = tooltip(
        container(toggle_all).padding(5),
        text(format!("{} (*)", rust_i18n::t!("show_all"))).size(12),
        tooltip::Position::Bottom,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let list = column(
        visible_calendars
            .into_iter()
            .enumerate()
            .map(|(i, cal)| {
                let is_visible = !app.hidden_calendars.contains(&cal.href);
                let is_target = app.active_cal_href.as_ref() == Some(&cal.href);
                let is_kb_selected = app.active_focus == crate::gui::state::Focus::Sidebar
                    && app.sidebar_selection_idx == i;

                let cal_color = cal
                    .color
                    .as_ref()
                    .and_then(|c| color_utils::parse_hex_to_floats(c))
                    .map(|(r, g, b)| Color::from_rgb(r, g, b));

                let (icon_char, icon_color) = if is_target {
                    (
                        icon::CONTENT_SAVE_EDIT,
                        cal_color.unwrap_or(theme.extended_palette().background.base.text),
                    )
                } else if is_visible {
                    (
                        icon::EYE,
                        cal_color.unwrap_or(theme.extended_palette().background.weak.text),
                    )
                } else {
                    (
                        icon::EYE_CLOSED,
                        theme.extended_palette().secondary.base.color,
                    )
                };

                let vis_btn = button(icon::icon(icon_char).size(16).style(move |_| text::Style {
                    color: Some(icon_color),
                }))
                .style(button::text)
                .padding(8)
                .on_press(Message::ToggleCalendarVisibility(
                    cal.href.clone(),
                    !is_visible,
                ));

                let vis_tooltip = tooltip(
                    vis_btn,
                    text(if is_visible {
                        rust_i18n::t!("hide")
                    } else {
                        rust_i18n::t!("show")
                    })
                    .size(12),
                    tooltip::Position::Right,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700));

                let mut label = button(text(&cal.name).size(16))
                    .width(Length::Fill)
                    .padding(10)
                    .on_press(Message::SelectCalendar(cal.href.clone()));
                if is_target {
                    label = label.style(move |_theme: &Theme, _status| button::Style {
                        text_color: Color::from_rgb(1.0, 0.6, 0.0),
                        background: Some(Color::from_rgba(1.0, 0.6, 0.0, 0.05).into()),
                        border: if is_kb_selected {
                            iced::Border {
                                width: 1.0,
                                color: Color::from_rgb(1.0, 0.6, 0.0),
                                radius: 4.0.into(),
                            }
                        } else {
                            iced::Border::default()
                        },
                        ..button::Style::default()
                    });
                } else if !is_visible {
                    label = label.style(move |_theme: &Theme, _status| button::Style {
                        text_color: Color::from_rgb(0.5, 0.5, 0.5),
                        border: if is_kb_selected {
                            iced::Border {
                                width: 1.0,
                                color: Color::from_rgb(0.5, 0.5, 0.5),
                                radius: 4.0.into(),
                            }
                        } else {
                            iced::Border::default()
                        },
                        ..button::Style::default()
                    });
                } else {
                    label = label.style(move |theme: &Theme, _status| button::Style {
                        border: if is_kb_selected {
                            iced::Border {
                                width: 1.0,
                                color: theme.extended_palette().primary.base.color,
                                radius: 4.0.into(),
                            }
                        } else {
                            iced::Border::default()
                        },
                        ..iced::widget::button::text(theme, _status)
                    });
                }

                let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                    .style(button::text)
                    .padding(10)
                    .on_press(Message::IsolateCalendar(cal.href.clone()));

                let focus_tooltip = tooltip(
                    focus_btn,
                    text(rust_i18n::t!("focus_hide_others")).size(12),
                    tooltip::Position::Left,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700));

                row![vis_tooltip, label, focus_tooltip]
                    .spacing(0)
                    .align_y(iced::Alignment::Center)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(2)
    .width(Length::Fill);

    column![
        toggle_container,
        scrollable(list)
            .height(Length::Fill)
            .id(app.sidebar_scrollable_id.clone())
    ]
    .spacing(5)
    .into()
}

// --- CATEGORIES ---
pub fn view_sidebar_categories(app: &GuiApp) -> Element<'_, Message> {
    let all_cats = &app.cached_categories;

    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let has_selection = !app.session.selected_categories.is_empty();

    let clear_btn = if has_selection {
        if is_filter_empty {
            button(
                icon::icon(icon::CLEAR_ALL)
                    .size(16)
                    .style(move |_| text::Style {
                        color: Some(Color::from_rgb(0.9, 0.2, 0.2)),
                    }),
            )
            .style(button::text)
            .padding(5)
            .on_press(Message::ClearAllTags)
        } else {
            button(icon::icon(icon::CLEAR_ALL).size(16))
                .style(button::text)
                .padding(5)
                .on_press(Message::ClearAllTags)
        }
    } else {
        button(
            icon::icon(icon::CLEAR_ALL)
                .size(16)
                .style(move |_| text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                }),
        )
        .style(button::text)
        .padding(5)
    };

    let clear_tooltip = tooltip(
        clear_btn,
        text(format!("{} (*)", rust_i18n::t!("clear_all_tags"))).size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let logic_text = if app.session.match_all_categories {
        rust_i18n::t!("match_and")
    } else {
        rust_i18n::t!("match_or")
    };
    let logic_btn = button(text(logic_text).size(12))
        .style(button::secondary)
        .padding(5)
        .on_press(Message::CategoryMatchModeChanged(
            !app.session.match_all_categories,
        ));

    let logic_tooltip = tooltip(
        logic_btn,
        text(format!("{} (m)", rust_i18n::t!("toggle_matching_logic"))).size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let header = row![
        clear_tooltip,
        Space::new().width(Length::Fill),
        logic_tooltip
    ]
    .spacing(5)
    .align_y(iced::Alignment::Center)
    .padding(iced::Padding {
        right: 14.0,
        bottom: 5.0,
        ..Default::default()
    });

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DurationOpt(Option<u32>, String);
    impl std::fmt::Display for DurationOpt {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.1)
        }
    }
    let mut dur_set = std::collections::HashSet::new();
    for map in app.store.calendars.values() {
        for t in map.values() {
            if let Some(d) = t.estimated_duration {
                dur_set.insert(d);
            }
        }
    }
    let mut sorted_durs: Vec<u32> = dur_set.into_iter().collect();
    sorted_durs.sort();
    let mut opts = vec![DurationOpt(None, rust_i18n::t!("any").to_string())];
    for d in sorted_durs {
        opts.push(DurationOpt(
            Some(d),
            crate::model::parser::format_duration_compact(d),
        ));
    }
    let current_min = opts
        .iter()
        .find(|o| o.0 == app.filter_min_duration)
        .cloned()
        .unwrap_or_else(|| opts[0].clone());
    let current_max = opts
        .iter()
        .find(|o| o.0 == app.filter_max_duration)
        .cloned()
        .unwrap_or_else(|| opts[0].clone());

    let dur_filters = column![
        iced::widget::rule::horizontal(1),
        text(rust_i18n::t!("filter_duration"))
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7)),
        row![
            text(rust_i18n::t!("min")).size(12).width(30),
            iced::widget::pick_list(opts.clone(), Some(current_min), |o| {
                Message::SetMinDuration(o.0)
            })
            .text_size(12)
            .padding(5)
            .width(Length::Fill)
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center),
        row![
            text(rust_i18n::t!("max")).size(12).width(30),
            iced::widget::pick_list(opts, Some(current_max), |o| Message::SetMaxDuration(o.0))
                .text_size(12)
                .padding(5)
                .width(Length::Fill)
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center),
        iced::widget::checkbox(app.filter_include_unset_duration)
            .label(rust_i18n::t!("include_unset"))
            .text_size(12)
            .size(16)
            .on_toggle(Message::ToggleIncludeUnsetDuration)
    ]
    .spacing(8)
    .padding(iced::Padding {
        top: 10.0,
        ..Default::default()
    });

    let tags_column = if all_cats.is_empty() {
        column![
            container(
                column![
                    text(rust_i18n::t!("no_tags_found"))
                        .size(13)
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                ]
                .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(10)
        ]
    } else {
        column(
            all_cats
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let cat = &item.full_key;
                    let is_kb_selected = app.active_focus == crate::gui::state::Focus::Sidebar
                        && app.sidebar_selection_idx == i;
                    let count = item.count;
                    let is_hovered = app.hovered_tag_uid.as_ref() == Some(cat);
                    let is_selected = app.session.selected_categories.contains(cat);
                    let cat_clone_toggle = cat.clone();
                    let cat_clone_focus = cat.clone();

                    let (r, g, b) = color_utils::generate_color(cat);
                    let tag_color = Color::from_rgb(r, g, b);

                    let icon_char = if is_selected {
                        icon::TAG_CHECK
                    } else {
                        icon::TAG_OUTLINE
                    };

                    let icon_content = icon::icon(icon_char).size(16);

                    let icon_btn = button(icon_content)
                        .style(move |_theme: &Theme, status: button::Status| {
                            let color =
                                if status == button::Status::Hovered || is_selected || is_hovered {
                                    tag_color
                                } else {
                                    Color {
                                        a: 0.5,
                                        ..tag_color
                                    }
                                };

                            button::Style {
                                text_color: color,
                                background: None,
                                ..button::Style::default()
                            }
                        })
                        .padding(2)
                        .on_press(Message::CategoryToggled(cat_clone_toggle.clone()));

                    let label_content: Element<'_, Message> = if cat == UNCATEGORIZED_ID {
                        let color = if is_hovered {
                            app.theme().extended_palette().primary.base.color
                        } else {
                            app.theme().extended_palette().background.base.text
                        };
                        text(format!("{} ({})", item.display_name, count))
                            .size(16)
                            .color(color)
                            .into()
                    } else {
                        let text_color = if is_hovered {
                            tag_color
                        } else {
                            app.theme().extended_palette().background.base.text
                        };
                        let prefix = if item.display_name.contains('=') {
                            ""
                        } else {
                            "#"
                        };
                        rich_text![
                            span(prefix).color(tag_color),
                            span(format!("{} ({})", item.display_name, count)).color(text_color)
                        ]
                        .size(16)
                        .on_link_click(never)
                        .into()
                    };

                    let label_btn = button(
                        container(label_content)
                            .width(Length::Shrink)
                            .align_x(iced::alignment::Horizontal::Left),
                    )
                    .style(move |theme: &Theme, status| {
                        let mut st = iced::widget::button::text(theme, status);
                        if is_kb_selected {
                            st.border = iced::Border {
                                width: 1.0,
                                color: theme.extended_palette().primary.base.color,
                                radius: 4.0.into(),
                            };
                        }
                        st
                    })
                    .padding(2)
                    .on_press(Message::CategoryToggled(cat_clone_toggle));

                    let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::FocusTag(cat_clone_focus));

                    let focus_tooltip = tooltip(
                        focus_btn,
                        text(rust_i18n::t!("focus_hide_others")).size(12),
                        tooltip::Position::Left,
                    )
                    .style(tooltip_style)
                    .delay(Duration::from_millis(700));

                    let expand_btn: Element<'_, Message> = if item.has_children {
                        let trees = [
                            icon::TREE_FA,
                            icon::TREE_FAE,
                            icon::TREE_MD,
                            icon::PALM_TREE,
                            icon::PINE_TREE,
                        ];
                        let hash = cat.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));

                        let r = ((hash >> 16) % 20) as f32 / 100.0;
                        let g = 0.6 + ((hash >> 8) % 30) as f32 / 100.0;
                        let b = (hash % 20) as f32 / 100.0;

                        let (icon_char, tree_color) = if item.is_expanded {
                            (trees[(hash % 5) as usize], Color::from_rgb(r, g, b))
                        } else {
                            (icon::FAMILY_TREE, Color::from_rgb(0.7, 0.42, 0.0))
                        };

                        button(icon::icon(icon_char).size(14).color(tree_color))
                            .style(button::text)
                            .padding(2)
                            .on_press(Message::ToggleTagCollapse(cat.clone()))
                            .into()
                    } else {
                        Space::new().width(Length::Fixed(0.0)).into()
                    };

                    let indent = Space::new().width(Length::Fixed(item.depth as f32 * 15.0));

                    let item_row = row![
                        indent,
                        icon_btn,
                        label_btn,
                        Space::new().width(Length::Fill),
                        expand_btn,
                        focus_tooltip
                    ]
                    .spacing(3)
                    .align_y(iced::Alignment::Center)
                    .padding(iced::Padding {
                        right: 15.0,
                        ..Default::default()
                    });

                    MouseArea::new(item_row)
                        .on_enter(Message::TagHovered(Some(cat.clone())))
                        .on_exit(Message::TagHovered(None))
                        .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(2)
    };

    let scroll_content = tags_column.push(Space::new().height(10)).push(dur_filters);

    column![
        header,
        scrollable(scroll_content)
            .height(Length::Fill)
            .id(app.sidebar_scrollable_id.clone())
    ]
    .spacing(5)
    .into()
}

// --- LOCATIONS ---
pub fn view_sidebar_locations(app: &GuiApp) -> Element<'_, Message> {
    let all_locs = &app.cached_locations;
    let has_selection = !app.session.selected_locations.is_empty();

    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let clear_btn = if has_selection {
        if is_filter_empty {
            button(
                icon::icon(icon::CLEAR_ALL)
                    .size(16)
                    .style(move |_| text::Style {
                        color: Some(Color::from_rgb(0.9, 0.2, 0.2)),
                    }),
            )
            .style(button::text)
            .padding(5)
            .on_press(Message::ClearAllLocations)
        } else {
            button(icon::icon(icon::CLEAR_ALL).size(16))
                .style(button::text)
                .padding(5)
                .on_press(Message::ClearAllLocations)
        }
    } else {
        button(
            icon::icon(icon::CLEAR_ALL)
                .size(16)
                .style(move |_| text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                }),
        )
        .style(button::text)
        .padding(5)
    };

    let clear_tooltip = tooltip(
        clear_btn,
        text(format!("{} (*)", rust_i18n::t!("clear_all_locations"))).size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let header = row![
        text(rust_i18n::t!("locations")).size(14),
        Space::new().width(Length::Fill),
        clear_tooltip
    ]
    .padding(10)
    .align_y(iced::Alignment::Center);

    let list_content: Element<'_, Message> = if all_locs.is_empty() {
        container(
            column![
                text(rust_i18n::t!("no_locations"))
                    .size(13)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            ]
            .align_x(iced::alignment::Horizontal::Center),
        )
        .width(Length::Fill)
        .padding(10)
        .into()
    } else {
        let list = column(
            all_locs
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let loc = &item.full_key;
                    let is_kb_selected = app.active_focus == crate::gui::state::Focus::Sidebar
                        && app.sidebar_selection_idx == i;
                    let count = item.count;
                    let is_selected = app.session.selected_locations.contains(loc);
                    let loc_clone_toggle = loc.clone();
                    let loc_clone_focus = loc.clone();

                    let (icon_char, icon_color) = if is_selected {
                        (icon::CHECK_CIRCLE, Color::from_rgb(1.0, 0.6, 0.0))
                    } else {
                        (icon::MAP_PIN, Color::from_rgb(0.5, 0.5, 0.5))
                    };

                    let icon_btn = button(icon::icon(icon_char).size(14).color(icon_color))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::LocationToggled(loc_clone_toggle.clone()));

                    let label = rich_text![span(format!("{} ({})", item.display_name, count))]
                        .size(14)
                        .on_link_click(never);

                    let label_btn = button(
                        container(label)
                            .width(Length::Shrink)
                            .align_x(iced::alignment::Horizontal::Left),
                    )
                    .style(move |theme: &Theme, status| {
                        let mut st = iced::widget::button::text(theme, status);
                        if is_kb_selected {
                            st.border = iced::Border {
                                width: 1.0,
                                color: theme.extended_palette().primary.base.color,
                                radius: 4.0.into(),
                            };
                        }
                        st
                    })
                    .padding(2)
                    .on_press(Message::LocationToggled(loc_clone_toggle));

                    let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::FocusLocation(loc_clone_focus));

                    let focus_tooltip = tooltip(
                        focus_btn,
                        text(rust_i18n::t!("focus_hide_others")).size(12),
                        tooltip::Position::Left,
                    )
                    .style(tooltip_style)
                    .delay(Duration::from_millis(700));

                    let expand_btn: Element<'_, Message> = if item.has_children {
                        let trees = [
                            icon::TREE_FA,
                            icon::TREE_FAE,
                            icon::TREE_MD,
                            icon::PALM_TREE,
                            icon::PINE_TREE,
                        ];
                        let hash = loc.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));

                        let r = ((hash >> 16) % 20) as f32 / 100.0;
                        let g = 0.6 + ((hash >> 8) % 30) as f32 / 100.0;
                        let b = (hash % 20) as f32 / 100.0;

                        let (icon_char, tree_color) = if item.is_expanded {
                            (trees[(hash % 5) as usize], Color::from_rgb(r, g, b))
                        } else {
                            (icon::FAMILY_TREE, Color::from_rgb(0.5, 0.5, 0.8))
                        };

                        button(icon::icon(icon_char).size(14).color(tree_color))
                            .style(button::text)
                            .padding(2)
                            .on_press(Message::ToggleLocationCollapse(loc.clone()))
                            .into()
                    } else {
                        Space::new().width(Length::Fixed(0.0)).into()
                    };

                    let indent = Space::new().width(Length::Fixed(item.depth as f32 * 15.0));

                    row![
                        indent,
                        icon_btn,
                        label_btn,
                        Space::new().width(Length::Fill),
                        expand_btn,
                        focus_tooltip
                    ]
                    .spacing(3)
                    .align_y(iced::Alignment::Center)
                    .padding(iced::Padding {
                        right: 15.0,
                        ..Default::default()
                    })
                    .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(2);

        scrollable(list)
            .height(Length::Fill)
            .id(app.sidebar_scrollable_id.clone())
            .into()
    };

    column![header, list_content].spacing(0).into()
}

fn build_heatmap_row<'a>(history: &[f32], theme: &Theme) -> Element<'a, Message> {
    let mut heatmap_row = row![].spacing(2);
    for &pct in history {
        let color = if pct >= 1.0 {
            Color::from_rgb(0.2, 0.75, 0.2) // Success Green
        } else if pct > 0.0 {
            let mut p = theme.extended_palette().primary.base.color;
            p.a = 0.6; // Dimmed primary color
            p
        } else {
            // Dark gray base
            Color::from_rgb(0.25, 0.25, 0.25)
        };

        let block = container(Space::new().width(6).height(6)).style(move |_| container::Style {
            background: Some(color.into()),
            border: iced::Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
        heatmap_row = heatmap_row.push(block);
    }

    tooltip(
        heatmap_row,
        text("Consistency (Past 7 periods)").size(12),
        tooltip::Position::Bottom,
    )
    .style(crate::gui::view::tooltip_style)
    .delay(std::time::Duration::from_millis(500))
    .into()
}

// --- GOALS ---
pub fn view_sidebar_goals(app: &GuiApp) -> Element<'_, Message> {
    let mut col = column![].spacing(10);

    if app.core_config.goals.is_empty() && app.cached_task_goals.is_empty() {
        col = col.push(
            container(
                column![
                    text(rust_i18n::t!("goals_empty"))
                        .size(13)
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                ]
                .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(10),
        );
    } else {
        let mut keys: Vec<&String> = app.core_config.goals.keys().collect();
        keys.sort();

        for key in keys {
            let goal = &app.core_config.goals[key];
            let (progress, history) = app
                .cached_goals_progress
                .get(key)
                .cloned()
                .unwrap_or((0, Vec::new()));
            let target = goal.target;
            let pct = if target > 0 {
                (progress as f32 / target as f32).min(1.0)
            } else {
                0.0
            };

            let (cur_str, tar_str) = if goal.goal_type == crate::config::GoalType::Duration {
                crate::model::parser::format_goal_duration(progress, target)
            } else {
                (progress.to_string(), target.to_string())
            };

            let target_display = goal.format_target_display(&tar_str);
            let title = text(format!("{} ({})", key, target_display)).size(14);

            let prog_text = text(rust_i18n::t!(
                "goal_progress",
                current = cur_str,
                target = tar_str
            ))
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6));

            let prog_row = row![
                prog_text,
                Space::new().width(Length::Fill),
                build_heatmap_row(&history, &app.theme())
            ]
            .align_y(iced::Alignment::Center);

            let bar_bg = container(Space::new().width(Length::Fill).height(6.0)).style(|_| {
                container::Style {
                    background: Some(Color::from_rgb(0.2, 0.2, 0.2).into()),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

            let fg_portion = (pct * 1000.0).clamp(0.0, 1000.0) as u16;
            let bg_portion = 1000 - fg_portion;

            let bar_fg =
                container(Space::new().height(6.0)).style(move |theme: &Theme| container::Style {
                    background: Some(if pct >= 1.0 {
                        Color::from_rgb(0.2, 0.8, 0.2).into()
                    } else {
                        theme.extended_palette().primary.base.color.into()
                    }),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });

            let bar_row = if fg_portion == 0 {
                row![Space::new().width(Length::Fill)]
            } else if bg_portion == 0 {
                row![bar_fg.width(Length::Fill)]
            } else {
                row![
                    bar_fg.width(Length::FillPortion(fg_portion)),
                    Space::new().width(Length::FillPortion(bg_portion))
                ]
            };

            let bar_container = iced::widget::stack![bar_bg, bar_row];

            let content_btn = button(column![title, bar_container, prog_row].spacing(4))
                .style(button::text)
                .width(Length::Fill)
                .padding(8);

            let content_btn = if key.starts_with('#') {
                content_btn.on_press(Message::JumpToTag(key.trim_start_matches('#').to_string()))
            } else if key.starts_with("@@") {
                content_btn.on_press(Message::JumpToLocation(
                    key.trim_start_matches("@@").to_string(),
                ))
            } else {
                content_btn
            };

            let delete_btn = button(
                icon::icon(icon::CROSS)
                    .size(14)
                    .color(Color::from_rgb(0.8, 0.4, 0.4)),
            )
            .style(button::text)
            .padding(8)
            .on_press(Message::RemoveGoal(key.clone()));

            let goal_row = row![content_btn, delete_btn].align_y(iced::Alignment::Center);

            col = col.push(goal_row);
        }

        for (uid, summary, goal, progress, history) in &app.cached_task_goals {
            let target = goal.target;
            let pct = if target > 0 {
                (*progress as f32 / target as f32).min(1.0)
            } else {
                0.0
            };

            let (cur_str, tar_str) = if goal.goal_type == crate::config::GoalType::Duration {
                crate::model::parser::format_goal_duration(*progress, target)
            } else {
                (progress.to_string(), target.to_string())
            };

            let target_display = goal.format_target_display(&tar_str);
            let title = text(format!("{} ({})", summary, target_display)).size(14);

            let prog_text = text(rust_i18n::t!(
                "goal_progress",
                current = cur_str,
                target = tar_str
            ))
            .size(12)
            .color(Color::from_rgb(0.6, 0.6, 0.6));

            let prog_row = row![
                prog_text,
                Space::new().width(Length::Fill),
                build_heatmap_row(history, &app.theme())
            ]
            .align_y(iced::Alignment::Center);

            let bar_bg = container(Space::new().width(Length::Fill).height(6.0)).style(|_| {
                container::Style {
                    background: Some(Color::from_rgb(0.2, 0.2, 0.2).into()),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

            let fg_portion = (pct * 1000.0).clamp(0.0, 1000.0) as u16;
            let bg_portion = 1000 - fg_portion;

            let bar_fg =
                container(Space::new().height(6.0)).style(move |theme: &Theme| container::Style {
                    background: Some(if pct >= 1.0 {
                        Color::from_rgb(0.2, 0.8, 0.2).into()
                    } else {
                        theme.extended_palette().primary.base.color.into()
                    }),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });

            let bar_row = if fg_portion == 0 {
                row![Space::new().width(Length::Fill)]
            } else if bg_portion == 0 {
                row![bar_fg.width(Length::Fill)]
            } else {
                row![
                    bar_fg.width(Length::FillPortion(fg_portion)),
                    Space::new().width(Length::FillPortion(bg_portion))
                ]
            };

            let bar_container = iced::widget::stack![bar_bg, bar_row];

            let content_btn = button(column![title, bar_container, prog_row].spacing(4))
                .style(button::text)
                .width(Length::Fill)
                .padding(8)
                .on_press(Message::JumpToTask(uid.clone()));

            // Task goals do not have a delete button in the sidebar (must be edited on the task)
            col = col.push(row![content_btn].align_y(iced::Alignment::Center));
        }
    }

    scrollable(col)
        .height(Length::Fill)
        .id(app.sidebar_scrollable_id.clone())
        .into()
}

// --- JOURNAL SIDEBAR ---
pub fn view_sidebar_journal(app: &GuiApp) -> Element<'_, Message> {
    use chrono::{Datelike, Duration, Local, NaiveDate};

    let today = Local::now().date_naive();
    let sel_date = app.journal_date;
    let is_monday_first = app.first_day_of_week == crate::config::FirstDayOfWeek::Monday;

    let quick_jumps = row![
        button(icon::icon(icon::ARROW_LEFT).size(14))
            .style(button::text)
            .padding([4, 12])
            .on_press(Message::SelectJournalDate(sel_date - Duration::days(1))),
        Space::new().width(Length::Fill),
        button(text(rust_i18n::t!("journal_today")).size(12))
            .style(button::primary)
            .padding([4, 16])
            .on_press(Message::SelectJournalDate(today)),
        Space::new().width(Length::Fill),
        button(icon::icon(icon::ARROW_RIGHT).size(14))
            .style(button::text)
            .padding([4, 12])
            .on_press(Message::SelectJournalDate(sel_date + Duration::days(1))),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let date_input_row = row![
        iced::widget::text_input(
            &rust_i18n::t!("journal_date_placeholder"),
            &app.journal_date_input
        )
        .on_input(Message::JournalDateInputChanged)
        .on_submit(Message::JournalDateInputSubmit)
        .padding(4)
        .size(12)
        .width(Length::Fill),
        button(icon::icon(icon::CHECK).size(12))
            .style(button::secondary)
            .padding(4)
            .on_press(Message::JournalDateInputSubmit)
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let year = sel_date.year();
    let month = sel_date.month();
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();

    let start_offset = if is_monday_first {
        first_day.weekday().num_days_from_monday() as usize
    } else {
        first_day.weekday().num_days_from_sunday() as usize
    };

    let month_str = first_day.format("%B %Y").to_string();

    let prev_month_date = if month == 1 {
        NaiveDate::from_ymd_opt(year - 1, 12, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month - 1, 1).unwrap()
    };
    let next_month_date = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };

    let cal_header = row![
        button(icon::icon(icon::ARROW_LEFT).size(12))
            .style(button::text)
            .padding(4)
            .on_press(Message::SelectJournalDate(prev_month_date)),
        container(text(month_str).size(13).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center),
        button(icon::icon(icon::ARROW_RIGHT).size(12))
            .style(button::text)
            .padding(4)
            .on_press(Message::SelectJournalDate(next_month_date)),
    ]
    .align_y(iced::Alignment::Center);

    let weekdays_row = if is_monday_first {
        row![
            container(text("Mo").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Tu").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("We").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Th").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Fr").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Sa").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Su").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        ]
    } else {
        row![
            container(text("Su").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Mo").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Tu").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("We").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Th").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Fr").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
            container(text("Sa").size(10))
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        ]
    };

    let mut days_grid = column![].spacing(2);
    let mut current_row = row![].spacing(2);

    for _ in 0..start_offset {
        current_row = current_row.push(Space::new().width(Length::Fill));
    }

    let days_in_month = if month == 12 {
        31
    } else {
        (NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1)).day()
    };

    for d in 1..=days_in_month {
        let date = NaiveDate::from_ymd_opt(year, month, d).unwrap();
        let is_selected = date == sel_date;
        let is_today = date == today;

        let has_journal = app
            .calendars
            .iter()
            .filter(|c| {
                if c.href.starts_with("local://") {
                    true
                } else {
                    c.supports_vjournal.unwrap_or(false)
                }
            })
            .filter(|c| !app.hidden_calendars.contains(&c.href))
            .filter(|c| !app.disabled_calendars.contains(&c.href))
            .filter(|c| c.href != crate::storage::LOCAL_TRASH_HREF)
            .filter(|c| c.href != "local://recovery")
            .any(|c| app.store.get_journal_entry(&c.href, date).is_some());

        let day_text = if has_journal && !is_selected && !is_today {
            text(d.to_string()).size(12).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
        } else {
            text(d.to_string()).size(11)
        };

        let btn_style = move |theme: &Theme, status: button::Status| {
            let palette = theme.extended_palette();
            if is_selected {
                button::Style {
                    background: Some(palette.primary.base.color.into()),
                    text_color: palette.primary.base.text,
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..button::Style::default()
                }
            } else if is_today {
                button::Style {
                    background: Some(palette.background.strong.color.into()),
                    text_color: palette.primary.base.color,
                    border: iced::Border {
                        width: 1.0,
                        color: palette.primary.base.color,
                        radius: 4.0.into(),
                    },
                    ..button::Style::default()
                }
            } else {
                let text_col = if has_journal {
                    Color::from_rgb(0.2, 0.8, 0.2)
                } else {
                    palette.background.weak.text
                };
                button::Style {
                    text_color: text_col,
                    ..button::text(theme, status)
                }
            }
        };

        let day_btn = button(
            container(day_text)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .style(btn_style)
        .padding(2)
        .width(Length::Fill)
        .on_press(Message::SelectJournalDate(date));

        current_row = current_row.push(day_btn);

        let pos = start_offset + d as usize;
        if pos.is_multiple_of(7) || d == days_in_month {
            // Pad the last row with empty spaces to fill the week
            let remaining = if pos.is_multiple_of(7) {
                0
            } else {
                7 - (pos % 7)
            };
            for _ in 0..remaining {
                current_row = current_row.push(Space::new().width(Length::Fill));
            }
            days_grid = days_grid.push(current_row);
            current_row = row![].spacing(2);
        }
    }

    let mut index_col = column![
        row![
            text(rust_i18n::t!("wiki_index", default = "Wiki index"))
                .size(14)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
            Space::new().width(Length::Fill),
            tooltip(
                button(icon::icon(app.create_journal_icon).size(12))
                    .style(button::secondary)
                    .padding([2, 6])
                    .on_press(Message::CreateJournalPage),
                text("New Page").size(12),
                tooltip::Position::Left
            )
            .style(crate::gui::view::tooltip_style)
        ]
        .align_y(iced::Alignment::Center)
    ]
    .spacing(4);

    let mut pages = Vec::new();
    for (href, map) in app.store.calendars.iter() {
        if app.hidden_calendars.contains(href)
            || app.disabled_calendars.contains(href)
            || href == crate::storage::LOCAL_TRASH_HREF
            || href == "local://recovery"
        {
            continue;
        }
        for t in map.values() {
            if t.is_journal {
                // A Daily Note is strictly named YYYY-MM-DD. Anything else is a Wiki Page.
                if chrono::NaiveDate::parse_from_str(&t.summary, "%Y-%m-%d").is_err() {
                    pages.push(t);
                }
            }
        }
    }
    pages.sort_by(|a, b| a.summary.cmp(&b.summary));

    let mut children_map: std::collections::HashMap<String, Vec<&crate::model::Task>> =
        std::collections::HashMap::new();
    let mut roots = Vec::new();
    let page_uids: std::collections::HashSet<String> =
        pages.iter().map(|p| p.uid.clone()).collect();

    for p in &pages {
        if let Some(parent) = &p.parent_uid
            && page_uids.contains(parent)
        {
            children_map.entry(parent.clone()).or_default().push(p);
            continue;
        }
        roots.push(p);
    }

    fn render_page_tree<'a>(
        node: &crate::model::Task,
        children_map: &std::collections::HashMap<String, Vec<&crate::model::Task>>,
        depth: usize,
    ) -> Element<'a, Message> {
        let display_name = if node.summary.is_empty() {
            rust_i18n::t!("untitled_page", default = "Untitled page").to_string()
        } else {
            node.summary.clone()
        };

        let indent = Space::new().width(Length::Fixed(depth as f32 * 12.0));
        let btn = button(
            row![
                indent,
                icon::icon(icon::JOURNAL)
                    .size(12)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
                text(display_name).size(13)
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .style(button::text)
        .padding([4, 4])
        .width(Length::Fill)
        .on_press(Message::OpenJournalPage(node.uid.clone()));

        if let Some(children) = children_map.get(&node.uid) {
            let mut col = column![btn];
            for child in children {
                col = col.push(render_page_tree(child, children_map, depth + 1));
            }
            col.into()
        } else {
            btn.into()
        }
    }

    for root in roots {
        index_col = index_col.push(render_page_tree(root, &children_map, 0));
    }

    let index_scroll = scrollable(index_col).height(Length::Fill).direction(
        iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(6),
        ),
    );

    column![
        quick_jumps,
        date_input_row,
        Space::new().height(6),
        cal_header,
        weekdays_row,
        days_grid,
        Space::new().height(10),
        index_scroll,
    ]
    .spacing(6)
    .padding(6)
    .into()
}
