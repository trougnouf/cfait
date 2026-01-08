// Renders the sidebar (calendars, tags, locations) for the GUI.
use super::tooltip_style;
use crate::color_utils;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::COLOR_LOCATION;
use crate::store::UNCATEGORIZED_ID;
use iced::never;
use iced::widget::{Space, button, checkbox, column, container, row, text, toggler, tooltip};
// Import rich_text and span for multi-colored text
use iced::widget::{rich_text, span};
use iced::{Color, Element, Length, Theme};
use std::time::Duration;

// Define the semantic color for Locations (Amber/Gold)

pub fn view_sidebar_calendars(app: &GuiApp) -> Element<'_, Message> {
    // ... [setup] ...
    let are_all_visible = app
        .calendars
        .iter()
        .filter(|c| !app.disabled_calendars.contains(&c.href))
        .all(|c| !app.hidden_calendars.contains(&c.href));
    // Capture the current theme so we can pick theme-aware defaults for icons/foregrounds
    let theme = app.theme();
    let toggler_style = |theme: &Theme, status: toggler::Status| -> toggler::Style {
        let mut style = toggler::default(theme, status);
        match status {
            toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled } => {
                if is_toggled {
                    style.background = Color::from_rgb(1.0, 0.6, 0.0).into();
                    // Use the theme's text color instead of a hardcoded white so the toggler adapts
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

    let list = column(
        app.calendars
            .iter()
            .filter(|c| !app.disabled_calendars.contains(&c.href))
            .map(|cal| {
                let is_visible = !app.hidden_calendars.contains(&cal.href);
                let is_target = app.active_cal_href.as_ref() == Some(&cal.href);

                // --- Color Resolution ---
                let cal_color = cal
                    .color
                    .as_ref()
                    .and_then(|c| color_utils::parse_hex_to_floats(c))
                    .map(|(r, g, b)| Color::from_rgb(r, g, b));

                let (icon_char, icon_color) = if is_target {
                    (
                        icon::CONTENT_SAVE_EDIT,
                        // Use cal color if present, else theme foreground (adapts to light/dark)
                        cal_color.unwrap_or(theme.extended_palette().background.base.text),
                    )
                } else if is_visible {
                    (
                        icon::EYE,
                        // Use cal color if present, else a weaker theme text color for non-target items
                        cal_color.unwrap_or(theme.extended_palette().background.weak.text),
                    )
                } else {
                    // Hidden calendars use the theme's weak text color by default
                    (icon::EYE_CLOSED, theme.extended_palette().secondary.base.color)
                };
                // ---------------------------------

                let vis_btn = button(icon::icon(icon_char).size(16).style(move |_| text::Style {
                    color: Some(icon_color),
                }))
                .style(button::text)
                .padding(8)
                .on_press(Message::ToggleCalendarVisibility(
                    cal.href.clone(),
                    !is_visible,
                ));

                // Apply tooltip_style
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

                // Apply tooltip_style
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

    column![toggle_container, list].spacing(5).into()
}

// ... DurationOpt (unchanged) ...
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

pub fn view_sidebar_categories(app: &GuiApp) -> Element<'_, Message> {
    // ... [setup] ...
    let all_cats = &app.cached_categories;
    let has_selection = !app.selected_categories.is_empty();

    let clear_btn = if has_selection {
        button(icon::icon(icon::CLEAR_ALL).size(16))
            .style(button::text)
            .padding(5)
            .on_press(Message::ClearAllTags)
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

    // Apply tooltip_style
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

    // Apply tooltip_style
    let logic_tooltip = tooltip(
        logic_btn,
        text("Toggle matching logic").size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let header = row![clear_tooltip, Space::new(), logic_tooltip]
        .spacing(5)
        .align_y(iced::Alignment::Center)
        .padding(iced::Padding {
            right: 14.0,
            ..Default::default()
        });

    // ... [List] ...
    let tags_list: Element<'_, Message> = if all_cats.is_empty() {
        column![
            header,
            text("No tags found")
                .size(14)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
        ]
        .spacing(10)
        .into()
    } else {
        let list = column(
            all_cats
                .iter()
                .map(|(cat, count)| {
                    let is_selected = app.selected_categories.contains(cat.as_str());
                    let cat_clone_check = cat.clone();
                    let cat_clone_text = cat.clone();
                    let check = checkbox(is_selected)
                        .size(18)
                        .on_toggle(move |_| Message::CategoryToggled(cat_clone_check.clone()));
                    let label_content: Element<'_, Message> = if cat == UNCATEGORIZED_ID {
                        text(format!("Uncategorized ({})", count)).size(16).into()
                    } else {
                        let (r, g, b) = color_utils::generate_color(cat);
                        let tag_color = Color::from_rgb(r, g, b);
                        rich_text![
                            span("#").color(tag_color),
                            span(format!("{} ({})", cat, count))
                        ]
                        .size(16)
                        .on_link_click(never)
                        .into()
                    };
                    let label_btn = button(label_content)
                        .style(button::text)
                        .padding(0)
                        .on_press(Message::CategoryToggled(cat_clone_text));
                    row![check, label_btn]
                        .spacing(5)
                        .align_y(iced::Alignment::Center)
                        .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(5);
        column![header, list].spacing(10).into()
    };

    // ... [Filters] ...
    let mut dur_set = std::collections::HashSet::new();
    for tasks in app.store.calendars.values() {
        for t in tasks {
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
        checkbox(app.filter_include_unset_duration)
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

    column![tags_list, dur_filters].spacing(10).into()
}

pub fn view_sidebar_locations(app: &GuiApp) -> Element<'_, Message> {
    let all_locs = &app.cached_locations;

    let has_selection = !app.selected_locations.is_empty();

    let clear_btn = if has_selection {
        button(icon::icon(icon::CLEAR_ALL).size(16))
            .style(button::text)
            .padding(5)
            .on_press(Message::ClearAllLocations)
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

    // Apply tooltip
    let clear_tooltip = tooltip(
        clear_btn,
        text("Clear all locations").size(12),
        tooltip::Position::Top,
    )
    .style(tooltip_style)
    .delay(Duration::from_millis(700));

    let header = row![
        text("Locations").size(14),
        Space::new().width(Length::Fill),
        clear_tooltip
    ]
    .padding(10)
    .align_y(iced::Alignment::Center);

    if all_locs.is_empty() {
        return column![
            header,
            text("No locations")
                .size(14)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
        ]
        .into();
    }

    let list = column(
        all_locs
            .iter()
            .map(|(loc, count)| {
                let is_selected = app.selected_locations.contains(loc.as_str());
                let loc_clone = loc.clone();

                let check = checkbox(is_selected)
                    .on_toggle(move |_| Message::LocationToggled(loc_clone.clone()));

                // Highlighted @@ text for Locations
                let label = rich_text![
                    span(icon::MAP_PIN.to_string())
                        .font(icon::FONT)
                        .color(COLOR_LOCATION),
                    span(format!(" {} ({})", loc, count))
                ]
                .size(14)
                .on_link_click(never); // Prevent interactivity on text

                // Make the text clickable to toggle
                let label_btn = button(label)
                    .style(button::text)
                    .padding(0)
                    .on_press(Message::LocationToggled(loc.clone()));

                row![check, label_btn]
                    .spacing(5)
                    .align_y(iced::Alignment::Center)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(5);

    column![header, list].spacing(10).into()
}
