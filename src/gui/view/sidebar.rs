// File: ./src/gui/view/sidebar.rs
// Renders the sidebar (calendars, tags, locations) for the GUI.

use super::tooltip_style;
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;

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
    let are_all_visible = app
        .calendars
        .iter()
        .filter(|c| !app.disabled_calendars.contains(&c.href))
        .all(|c| !app.hidden_calendars.contains(&c.href));

    let theme = app.theme();
    let toggler_style = |theme: &Theme, status: toggler::Status| -> toggler::Style {
        let mut style = toggler::default(theme, status);
        match status {
            toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled } => {
                if is_toggled {
                    style.background = Color::from_rgb(1.0, 0.6, 0.0).into();
                    style.foreground = theme.extended_palette().background.base.text.into();
                }
            }
            _ => {}
        }
        style
    };

    let toggle_all = toggler(are_all_visible)
        .label("Show all")
        .text_size(12)
        .text_alignment(iced::alignment::Horizontal::Left)
        .spacing(10)
        .width(Length::Fill)
        .on_toggle(Message::ToggleAllCalendars)
        .style(toggler_style);
    let toggle_container = container(toggle_all).padding(5);

    // List generation (mostly unchanged logic, just wrapped in scrollable at end)
    let list = column(
        app.calendars
            .iter()
            .filter(|c| !app.disabled_calendars.contains(&c.href))
            .map(|cal| {
                let is_visible = !app.hidden_calendars.contains(&cal.href);
                let is_target = app.active_cal_href.as_ref() == Some(&cal.href);

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
                    text(if is_visible { "Hide" } else { "Show" }).size(12),
                    tooltip::Position::Right,
                )
                .style(tooltip_style)
                .delay(Duration::from_millis(700));

                let mut label = button(text(&cal.name).size(16))
                    .width(Length::Fill)
                    .padding(10)
                    .on_press(Message::SelectCalendar(cal.href.clone()));
                if is_target {
                    label = label.style(|_theme: &Theme, _status| button::Style {
                        text_color: Color::from_rgb(1.0, 0.6, 0.0),
                        background: Some(Color::from_rgba(1.0, 0.6, 0.0, 0.05).into()),
                        ..button::Style::default()
                    });
                } else if !is_visible {
                    label = label.style(|_theme: &Theme, _status| button::Style {
                        text_color: Color::from_rgb(0.5, 0.5, 0.5),
                        ..button::Style::default()
                    });
                } else {
                    label = label.style(button::text);
                }

                let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                    .style(button::text)
                    .padding(10)
                    .on_press(Message::IsolateCalendar(cal.href.clone()));

                let focus_tooltip = tooltip(
                    focus_btn,
                    text("Focus (hide others)").size(12),
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

    // ADDED: Wrap list in scrollable, keep toggle_container sticky
    column![
        toggle_container,
        scrollable(list)
            .height(Length::Fill)
            .id(app.sidebar_scrollable_id.clone())
    ]
    .spacing(5)
    .into()
}

// ... format_mins and DurationOpt unchanged ...
// (Omitting to save space)
#[derive(Debug, Clone, PartialEq, Eq)]
struct DurationOpt(Option<u32>, String);
impl std::fmt::Display for DurationOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}
fn format_mins(m: u32) -> String {
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
}

// --- CATEGORIES ---
pub fn view_sidebar_categories(app: &GuiApp) -> Element<'_, Message> {
    let all_cats = &app.cached_categories;

    // --- 1. Header Logic ---
    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let has_selection = !app.selected_categories.is_empty();

    let clear_btn = if has_selection {
        if is_filter_empty {
            // Error state: color button icon red
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
            // Normal active state
            button(icon::icon(icon::CLEAR_ALL).size(16))
                .style(button::text)
                .padding(5)
                .on_press(Message::ClearAllTags)
        }
    } else {
        // Disabled state
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
        text("Clear all tags").size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let logic_text = if app.match_all_categories {
        "Match: AND"
    } else {
        "Match: OR"
    };
    let logic_btn = button(text(logic_text).size(12))
        .style(button::secondary)
        .padding(5)
        .on_press(Message::CategoryMatchModeChanged(!app.match_all_categories));

    let logic_tooltip = tooltip(
        logic_btn,
        text("Toggle matching logic").size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    // STICKY HEADER
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

    // --- 2. Duration Filters Logic (Moved Up) ---
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
    let mut opts = vec![DurationOpt(None, "Any".to_string())];
    for d in sorted_durs {
        opts.push(DurationOpt(Some(d), format_mins(d)));
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
        text("Filter duration")
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7)),
        row![
            text("Min:").size(12).width(30),
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
            text("Max:").size(12).width(30),
            iced::widget::pick_list(opts, Some(current_max), |o| Message::SetMaxDuration(o.0))
                .text_size(12)
                .padding(5)
                .width(Length::Fill)
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center),
        iced::widget::checkbox(app.filter_include_unset_duration)
            .label("Include unset")
            .text_size(12)
            .size(16)
            .on_toggle(Message::ToggleIncludeUnsetDuration)
    ]
    .spacing(8)
    .padding(iced::Padding {
        top: 10.0,
        ..Default::default()
    });

    // --- 3. Tag List Construction ---
    let tags_column = if all_cats.is_empty() {
        column![
            container(
                text("No tags found")
                    .size(14)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
            )
            .padding(10)
        ]
    } else {
        column(
            all_cats
                .iter()
                .map(|(cat, count)| {
                    let is_hovered = app.hovered_tag_uid.as_ref() == Some(cat);
                    let is_selected = app.selected_categories.contains(cat.as_str());
                    let cat_clone_toggle = cat.clone();
                    let cat_clone_focus = cat.clone();

                    let (r, g, b) = color_utils::generate_color(cat);
                    let tag_color = Color::from_rgb(r, g, b);

                    // Icon character depends on selection state
                    let icon_char = if is_selected {
                        icon::TAG_CHECK
                    } else {
                        icon::TAG_OUTLINE
                    };

                    // The icon widget itself has no color set; the button style will control it.
                    let icon_content = icon::icon(icon_char).size(16);

                    let icon_btn = button(icon_content)
                        .style(move |_theme: &Theme, status: button::Status| {
                            // On hover, always use the full opaque tag color for feedback.
                            // Otherwise, use the appropriate color based on selection state.
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
                        text(format!("Uncategorized ({})", count))
                            .size(16)
                            .color(color)
                            .into()
                    } else {
                        let text_color = if is_hovered {
                            tag_color
                        } else {
                            app.theme().extended_palette().background.base.text
                        };
                        rich_text![
                            span("#").color(tag_color),
                            span(format!("{} ({})", cat, count)).color(text_color)
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
                    .style(button::text)
                    .padding(0)
                    .on_press(Message::CategoryToggled(cat_clone_toggle));

                    // Use new FocusTag message
                    let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::FocusTag(cat_clone_focus));

                    let focus_tooltip = tooltip(
                        focus_btn,
                        text("Focus (hide others)").size(12),
                        tooltip::Position::Left,
                    )
                    .style(tooltip_style)
                    .delay(Duration::from_millis(700));

                    let item_row = row![
                        icon_btn,
                        label_btn,
                        Space::new().width(Length::Fill),
                        focus_tooltip
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center)
                    // Added Right Padding so scrollbar doesn't overlap arrow
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

    // --- 4. Combine: Tags + Filter Duration ---
    let scroll_content = tags_column.push(Space::new().height(10)).push(dur_filters);

    // --- 5. Final Output: Header + Scrollable Area ---
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
    let has_selection = !app.selected_locations.is_empty();

    // Highlight clear button red if selected locations are likely the cause of an empty result.
    let is_filter_empty = app.tasks.is_empty() && app.store.has_any_tasks();
    let clear_btn = if has_selection {
        // If filters produced an empty result and locations are selected, color the clear button red to attribute.
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
        text("Clear all locations").size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    // STICKY HEADER
    let header = row![
        text("Locations").size(14),
        Space::new().width(Length::Fill),
        clear_tooltip
    ]
    .padding(10)
    .align_y(iced::Alignment::Center);

    let list_content: Element<'_, Message> = if all_locs.is_empty() {
        container(
            text("No locations")
                .size(14)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        )
        .padding(10)
        .into()
    } else {
        let list = column(
            all_locs
                .iter()
                .map(|(loc, count)| {
                    let is_selected = app.selected_locations.contains(loc.as_str());
                    let loc_clone_toggle = loc.clone();
                    let loc_clone_focus = loc.clone();

                    // Icon Logic: Amber for Checked
                    let (icon_char, icon_color) = if is_selected {
                        (icon::CHECK_CIRCLE, Color::from_rgb(1.0, 0.6, 0.0)) // Amber
                    } else {
                        (icon::MAP_PIN, Color::from_rgb(0.5, 0.5, 0.5)) // Gray
                    };

                    let icon_btn = button(icon::icon(icon_char).size(14).color(icon_color))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::LocationToggled(loc_clone_toggle.clone()));

                    let label = rich_text![span(format!("{} ({})", loc, count))]
                        .size(14)
                        .on_link_click(never);

                    let label_btn = button(
                        container(label)
                            .width(Length::Shrink)
                            .align_x(iced::alignment::Horizontal::Left),
                    )
                    .style(button::text)
                    .padding(0)
                    .on_press(Message::LocationToggled(loc_clone_toggle));

                    // Use new FocusLocation message
                    let focus_btn = button(icon::icon(icon::ARROW_RIGHT).size(14))
                        .style(button::text)
                        .padding(2)
                        .on_press(Message::FocusLocation(loc_clone_focus));

                    let focus_tooltip = tooltip(
                        focus_btn,
                        text("Focus (hide others)").size(12),
                        tooltip::Position::Left,
                    )
                    .style(tooltip_style)
                    .delay(Duration::from_millis(700));

                    row![
                        icon_btn,
                        label_btn,
                        Space::new().width(Length::Fill),
                        focus_tooltip
                    ]
                    .spacing(5)
                    .align_y(iced::Alignment::Center)
                    // Added Right Padding so scrollbar doesn't overlap arrow
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
