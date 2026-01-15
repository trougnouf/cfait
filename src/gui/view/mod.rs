// File: ./src/gui/view/mod.rs
// Main view composition and layout logic for the GUI.
use std::time::Duration;
pub mod help;
pub mod settings;
pub mod sidebar;
pub mod syntax;
pub mod task_row;

use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp, ResizeDirection, SidebarMode};
use crate::gui::view::help::view_help;
use crate::gui::view::settings::view_settings;
use crate::gui::view::sidebar::{view_sidebar_calendars, view_sidebar_categories};
use crate::gui::view::task_row::view_task_row;

use iced::alignment::Horizontal;
// --- Import for resize interaction ---
use iced::mouse;
use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{
    MouseArea, Space, button, column, container, row, scrollable, stack, svg, text, text_editor,
    text_input, tooltip,
};
use iced::{Color, Element, Length, Theme, Vector};

/// Shared semantic color for Locations (Gray)
pub const COLOR_LOCATION: Color = Color::from_rgb(0.4, 0.4, 0.6);

/// Shared style for tooltips with slight transparency
pub fn tooltip_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        // 85% Opacity Background
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

pub fn root_view(app: &GuiApp) -> Element<'_, Message> {
    // 1. Generate the base content
    let base_content: Element<'_, Message> = match app.state {
        AppState::Loading => container(text("Loading...").size(30))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
        AppState::Onboarding | AppState::Settings => view_settings(app),
        AppState::Help => view_help(),
        AppState::Active => {
            let content_height = match app.sidebar_mode {
                SidebarMode::Calendars => {
                    app.calendars
                        .iter()
                        .filter(|c| !app.disabled_calendars.contains(&c.href))
                        .count() as f32
                        * 44.0
                }
                SidebarMode::Categories => app.cached_categories.len() as f32 * 34.0,
                SidebarMode::Locations => app.cached_locations.len() as f32 * 34.0,
            };
            let available_height = app.current_window_size.height - 110.0;
            let show_logo = (available_height - content_height) > 140.0;
            let content_layout = row![
                view_sidebar(app, show_logo),
                iced::widget::rule::vertical(1),
                container(view_main_content(app, !show_logo))
                    .width(Length::Fill)
                    .center_x(Length::Fill)
            ];

            container(content_layout)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    };

    // 2. Modals (Import / Alarm)
    // Wrap base_content in modals if necessary
    let mut content_with_modals = base_content;

    if app.ics_import_dialog_open {
        content_with_modals = view_ics_import_dialog(app, content_with_modals);
    } else if !app.ringing_tasks.is_empty() {
        // --- ALARM MODAL ---
        let (task, alarm) = &app.ringing_tasks[0];

        // --- ALARM MODAL ---
        let icon_header = container(
            icon::icon(icon::BELL)
                .size(30)
                .color(Color::from_rgb(1.0, 0.4, 0.0)),
        )
        .padding(5)
        .center_x(Length::Fill);

        let title = text("Reminder")
            .size(24)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fill)
            .align_x(Horizontal::Center);

        // Task Summary (Title)
        let summary = text(&task.summary)
            .size(18)
            .width(Length::Fill)
            .align_x(Horizontal::Center);

        // Task Description (New)
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

        // --- BUTTONS ---

        // Load config for presets
        let (s1, s2) = if let Ok(cfg) = crate::config::Config::load() {
            (cfg.snooze_short_mins, cfg.snooze_long_mins)
        } else {
            (15, 60)
        };

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
            text_input("Custom (eg 30m)", &app.snooze_custom_input)
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

        let dismiss_btn = button(text("Dismiss").size(14).font(iced::Font {
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
            dismiss_btn
        ]
        .align_x(iced::Alignment::Center);

        // Combine content into a scrollable area to handle dynamic sizes
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
            // Max height constraint to ensure it fits on screen even with huge descriptions
            .max_height(500.0)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(
                        Color {
                            a: 0.95,
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
            });

        content_with_modals = stack![
            content_with_modals,
            container(modal_card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.6).into()),
                    ..Default::default()
                })
        ]
        .into();
    }

    // 3. Resize Grips (if !SSD)
    // Apply resize grips *after* modals so grips are on top of everything (including full-screen modal overlays)
    let final_content = if app.force_ssd {
        content_with_modals
    } else {
        let t = 6.0; // Thickness of edge grips
        let c = 12.0; // Size of corner grips

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
            content_with_modals, // Content is bottom layer
            // Grips are top layers
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

    // 4. Final Window Style Container
    container(final_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                // Apply the actual background color here
                background: Some(iced::Background::Color(palette.background.base.color)),
                // Apply the Border Radius here (remove when SSD)
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
    let active_style =
        |_theme: &Theme, _status: iced::widget::button::Status| -> iced::widget::button::Style {
            iced::widget::button::Style {
                background: Some(Color::from_rgb(1.0, 0.6, 0.0).into()),
                text_color: Color::BLACK,
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..iced::widget::button::Style::default()
            }
        };

    // Icons for tabs
    let btn_cals =
        button(container(icon::icon(icon::CALENDARS_HEADER).size(18)).center_x(Length::Fill))
            .padding(8)
            .width(Length::Fill)
            .style(if app.sidebar_mode == SidebarMode::Calendars {
                active_style
            } else {
                button::text
            })
            .on_press(Message::SidebarModeChanged(SidebarMode::Calendars));

    let btn_tags = button(container(icon::icon(icon::TAGS_HEADER).size(18)).center_x(Length::Fill))
        .padding(8)
        .width(Length::Fill)
        .style(if app.sidebar_mode == SidebarMode::Categories {
            active_style
        } else {
            button::text
        })
        .on_press(Message::SidebarModeChanged(SidebarMode::Categories));

    let btn_locs =
        button(container(icon::icon(app.location_tab_icon).size(18)).center_x(Length::Fill))
            .padding(8)
            .width(Length::Fill)
            .style(if app.sidebar_mode == SidebarMode::Locations {
                active_style
            } else {
                button::text
            })
            .on_press(Message::SidebarModeChanged(SidebarMode::Locations));

    let tabs = row![btn_cals, btn_tags, btn_locs].spacing(2);

    let content = match app.sidebar_mode {
        SidebarMode::Calendars => view_sidebar_calendars(app),
        SidebarMode::Categories => view_sidebar_categories(app),
        SidebarMode::Locations => crate::gui::view::sidebar::view_sidebar_locations(app),
    };

    let settings_btn = iced::widget::button(
        container(icon::icon(icon::SETTINGS_GEAR).size(20))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(0)
    .height(Length::Fixed(40.0))
    .width(Length::Fill)
    .style(iced::widget::button::secondary)
    .on_press(Message::OpenSettings);

    let help_btn = iced::widget::button(
        container(icon::icon(icon::HELP_RHOMBUS).size(20))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(0)
    .height(Length::Fixed(40.0))
    .width(Length::Fixed(50.0))
    .style(iced::widget::button::secondary)
    .on_press(Message::OpenHelp);

    // Apply tooltip_style
    let footer = row![
        tooltip(
            settings_btn,
            text("Settings").size(12),
            tooltip::Position::Top
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
        tooltip(help_btn, text("Help").size(12), tooltip::Position::Top)
            .style(tooltip_style)
            .delay(Duration::from_millis(700))
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

fn view_main_content(app: &GuiApp, show_logo: bool) -> Element<'_, Message> {
    // 1. Identify Active Calendar
    let active_cal = app
        .active_cal_href
        .as_ref()
        .and_then(|href| app.calendars.iter().find(|c| &c.href == href));

    let title_text = if app.loading {
        "Loading...".to_string()
    } else if let Some(cal) = active_cal {
        cal.name.clone()
    } else if app.selected_categories.is_empty() {
        "All tasks".to_string()
    } else {
        "Tasks".to_string()
    };

    // 2. Identify Other Visible Calendars
    // Logic change: Only show other visible calendars if we are NOT in the Calendars sidebar mode.
    // If we are in Calendar mode, the sidebar already shows what is active/visible.
    let other_visible_cals: Vec<&crate::model::CalendarListEntry> =
        if !app.loading && app.sidebar_mode != SidebarMode::Calendars {
            app.calendars
                .iter()
                .filter(|c| {
                    !app.disabled_calendars.contains(&c.href)
                        && !app.hidden_calendars.contains(&c.href)
                        && Some(&c.href) != app.active_cal_href.as_ref()
                })
                .collect()
        } else {
            vec![]
        };

    // Prepare Active Calendar Color (computed outside closure to allow move)
    let active_cal_color_opt = active_cal
        .and_then(|c| c.color.as_ref())
        .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
        .map(|(r, g, b)| Color::from_rgb(r, g, b));

    // Dynamic Title Style Closure
    let title_style = move |theme: &Theme| -> text::Style {
        text::Style {
            color: Some(
                active_cal_color_opt.unwrap_or(theme.extended_palette().background.base.text),
            ),
        }
    };

    let active_count = app.tasks.iter().filter(|t| !t.status.is_done()).count();
    let mut subtitle = format!("{} Tasks", active_count);

    if !app.search_value.is_empty() {
        subtitle.push_str(&format!(" | Search: '{}'", app.search_value));
    } else if !app.selected_categories.is_empty() {
        let tag_count = app.selected_categories.len();
        if tag_count == 1 {
            subtitle.push_str(&format!(
                " | Tag: #{}",
                app.selected_categories.iter().next().unwrap()
            ));
        } else {
            subtitle.push_str(&format!(" | {} Tags", tag_count));
        }
    }

    // Reduced spacing to 0 to remove gaps between +
    let mut title_group = row![].spacing(0).align_y(iced::Alignment::Center);

    if show_logo {
        title_group = title_group.push(
            svg(svg::Handle::from_memory(icon::LOGO))
                .width(24)
                .height(24),
        );
        // Add spacing back manually only after logo
        title_group = title_group.push(Space::new().width(10));
    }

    // Add Active Calendar Name with Color
    title_group = title_group.push(
        text(title_text)
            .size(20)
            .font(iced::Font::DEFAULT)
            .style(title_style), // Apply color style
    );

    // Add "+" for other visible calendars with their respective colors
    for other in other_visible_cals {
        let other_color = other
            .color
            .as_ref()
            .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
            .map(|(r, g, b)| Color::from_rgb(r, g, b))
            .unwrap_or(Color::from_rgb(0.5, 0.5, 0.5)); // Fallback gray

        title_group = title_group.push(text("+").size(18).color(other_color).font(iced::Font {
            ..Default::default()
        }));
    }

    let mut left_section = row![title_group]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    if app.unsynced_changes {
        left_section = left_section.push(
            container(
                text("Unsynced")
                    .size(10)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.base.text),
                    }),
            )
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.8, 0.5, 0.0).into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .padding(3),
        );
    }

    let refresh_btn = iced::widget::button(icon::icon(icon::REFRESH).size(16))
        .style(iced::widget::button::text)
        .padding(4)
        .on_press(Message::Refresh);

    // Apply tooltip_style
    left_section = left_section.push(
        tooltip(
            refresh_btn,
            text("Force sync").size(12),
            tooltip::Position::Bottom,
        )
        .style(tooltip_style)
        .delay(Duration::from_millis(700)),
    );

    let subtitle_text = text(subtitle)
        .size(14)
        .color(Color::from_rgb(0.6, 0.6, 0.6));
    let middle_container = container(subtitle_text)
        .width(Length::Fill)
        .height(Length::Shrink)
        .center_x(Length::Fill)
        .center_y(Length::Shrink);

    let search_input = iced::widget::text_input("Search...", &app.search_value)
        .id("header_search_input") // Stable ID prevents focus loss
        .on_input(Message::SearchChanged)
        .padding(5)
        .size(14)
        .width(Length::Fixed(180.0));

    // --- UPDATED SEARCH BAR LOGIC ---
    let mut search_row = row![].align_y(iced::Alignment::Center).spacing(5);
    let is_search_empty = app.search_value.is_empty();
    let (search_icon_char, icon_color, on_press) = if is_search_empty {
        (icon::SEARCH, Color::from_rgb(0.4, 0.4, 0.4), None) // Gray, no action
    } else {
        (
            icon::SEARCH_STOP,
            app.theme().extended_palette().background.base.text,
            Some(Message::SearchChanged(String::new())),
        ) // theme-aware clear action color
    };

    let mut clear_btn =
        iced::widget::button(icon::icon(search_icon_char).size(14).color(icon_color))
            .style(iced::widget::button::text)
            .padding(4);

    if let Some(msg) = on_press {
        clear_btn = clear_btn.on_press(msg);
    }

    search_row = search_row.push(clear_btn);
    search_row = search_row.push(search_input);
    // --------------------------------

    let window_controls = if app.force_ssd {
        // If SSD, hide custom controls
        row![].spacing(0)
    } else {
        row![
            iced::widget::button(icon::icon(icon::WINDOW_MINIMIZE).size(14))
                .style(iced::widget::button::text)
                .padding(8)
                .on_press(Message::MinimizeWindow),
            iced::widget::button(icon::icon(icon::CROSS).size(14))
                .style(iced::widget::button::danger)
                .padding(8)
                .on_press(Message::CloseWindow)
        ]
        .spacing(0)
    };

    let right_section = row![search_row, window_controls]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let header_row = row![left_section, middle_container, right_section]
        .spacing(10)
        .padding(iced::Padding {
            top: 10.0,
            bottom: 5.0,
            left: 10.0,
            right: 10.0,
        })
        .align_y(iced::Alignment::Center);

    let header_drag_area = if app.force_ssd {
        // If SSD, let the OS handle dragging; don't make a custom drag area
        Element::from(header_row)
    } else {
        MouseArea::new(header_row)
            .on_press(Message::WindowDragged)
            .into()
    };

    // If viewing any local calendar, show export-to-caldav button bar
    let export_ui: Element<'_, Message>;
    if let Some(active_href) = &app.active_cal_href {
        if active_href.starts_with("local://") {
            let targets: Vec<_> = app
                .calendars
                .iter()
                .filter(|c| {
                    !c.href.starts_with("local://") && !app.disabled_calendars.contains(&c.href)
                })
                .collect();
            if !targets.is_empty() {
                let mut row = row![
                    text("Export to:")
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
    let mut main_col = column![header_drag_area, export_ui, input_area];

    // Existing Tag Jump
    if app.search_value.starts_with('#') {
        let tag = app.search_value.trim_start_matches('#').trim().to_string();
        if !tag.is_empty() {
            main_col = main_col.push(
                container(
                    iced::widget::button(
                        row![
                            icon::icon(icon::TAG).size(14),
                            text(format!(" Go to tag: #{}", tag)).size(14)
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center),
                    )
                    .style(iced::widget::button::secondary)
                    .padding(5)
                    .width(Length::Fill)
                    .on_press(Message::JumpToTag(tag)),
                )
                .padding(iced::Padding {
                    left: 10.0,
                    right: 10.0,
                    bottom: 5.0,
                    ..Default::default()
                }),
            );
        }
    }

    // Location Jump Button
    if app.search_value.starts_with("@@") || app.search_value.starts_with("loc:") {
        let raw = if app.search_value.starts_with("@@") {
            app.search_value.trim_start_matches("@@")
        } else {
            app.search_value.trim_start_matches("loc:")
        };
        let loc = raw.trim().to_string();

        if !loc.is_empty() {
            main_col = main_col.push(
                container(
                    iced::widget::button(
                        row![
                            icon::icon(icon::LOCATION).size(14),
                            text(format!(" Go to location: @@{}", loc)).size(14)
                        ]
                        .spacing(5)
                        .align_y(iced::Alignment::Center),
                    )
                    .style(iced::widget::button::secondary)
                    .padding(5)
                    .width(Length::Fill)
                    .on_press(Message::JumpToLocation(loc)),
                )
                .padding(iced::Padding {
                    left: 10.0,
                    right: 10.0,
                    bottom: 5.0,
                    ..Default::default()
                }),
            );
        }
    }

    if let Some(err) = &app.error_msg {
        let error_content = row![
            text(err)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.base.text)
                })
                .size(14)
                .width(Length::Fill),
            iced::widget::button(
                icon::icon(icon::CROSS)
                    .size(14)
                    .color(app.theme().extended_palette().background.base.text)
            )
            .style(iced::widget::button::text)
            .padding(2)
            .on_press(Message::DismissError)
        ]
        .align_y(iced::Alignment::Center);
        main_col = main_col.push(
            container(error_content)
                .width(Length::Fill)
                .padding(5)
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.8, 0.2, 0.2).into()),
                    ..Default::default()
                }),
        );
    }

    let tasks_view = column(
        app.tasks
            .iter()
            .enumerate()
            .map(|(real_index, task)| view_task_row(app, real_index, task))
            .collect::<Vec<_>>(),
    )
    .spacing(1);

    // --- UPDATED SCROLLABLE WITH AUTO-SCROLL ---
    main_col = main_col.push(
        scrollable(tasks_view)
            .height(Length::Fill)
            .id(app.scrollable_id.clone())
            .direction(Direction::Vertical(
                Scrollbar::new().width(10).scroller_width(10).margin(0),
            ))
            .auto_scroll(true),
    );

    container(main_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            right: 8.0,
            ..Default::default()
        })
        .into()
}

fn view_input_area(app: &GuiApp) -> Element<'_, Message> {
    let is_dark_mode = app.theme().extended_palette().is_dark;
    let input_title = text_editor(&app.input_value)
        .id("main_input")
        .placeholder(&app.current_placeholder)
        .on_action(Message::InputChanged)
        .highlight_with::<self::syntax::SmartInputHighlighter>(is_dark_mode, |highlight, _theme| {
            *highlight
        })
        .padding(10)
        .height(Length::Fixed(45.0))
        .font(iced::Font::DEFAULT);

    let inner_content: Element<'_, Message> = if app.editing_uid.is_some() {
        let input_desc = text_editor(&app.description_value)
            .id("description_input") // ID Added here
            .placeholder("Notes...")
            .on_action(Message::DescriptionChanged)
            .padding(10)
            .height(Length::Fixed(100.0));
        let cancel_btn = iced::widget::button(text("Cancel").size(16))
            .style(iced::widget::button::secondary)
            .on_press(Message::CancelEdit);
        let save_btn = iced::widget::button(text("Save").size(16))
            .style(iced::widget::button::primary)
            .on_press(Message::SubmitTask);
        let top_bar = row![
            text("Editing")
                .size(14)
                .color(Color::from_rgb(0.7, 0.7, 1.0)),
            Space::new().width(Length::Fill),
            cancel_btn,
            save_btn
        ]
        .align_y(iced::Alignment::Center)
        .spacing(10);
        let mut move_element: Element<'_, Message> = row![].into();

        if let Some(edit_uid) = &app.editing_uid
            && let Some(task) = app.tasks.iter().find(|t| t.uid == *edit_uid)
        {
            let targets: Vec<_> = app
                .calendars
                .iter()
                .filter(|c| {
                    c.href != task.calendar_href && !app.disabled_calendars.contains(&c.href)
                })
                .collect();
            if !targets.is_empty() {
                let label = text("Move to:")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6));
                let mut btn_row = row![].spacing(5);
                for cal in targets {
                    btn_row = btn_row.push(
                        iced::widget::button(text(&cal.name).size(12))
                            .style(iced::widget::button::secondary)
                            .padding(5)
                            .on_press(Message::MoveTask(task.uid.clone(), cal.href.clone())),
                    );
                }
                move_element = row![label, scrollable(btn_row).height(30)]
                    .spacing(10)
                    .align_y(iced::Alignment::Center)
                    .into();
            }
        }
        column![top_bar, input_title, input_desc, move_element]
            .spacing(10)
            .into()
    } else {
        column![input_title].spacing(5).into()
    };

    container(inner_content)
        .padding(iced::Padding {
            top: 5.0,
            bottom: 8.0,
            left: 10.0,
            right: 10.0,
        })
        .into()
}

fn view_ics_import_dialog<'a>(
    app: &'a GuiApp,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let file_name = app
        .ics_import_file_path
        .as_ref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("file.ics");

    let task_count = app.ics_import_task_count.unwrap_or(0);

    // Header
    let icon_header = container(
        icon::icon(icon::IMPORT)
            .size(30)
            .color(Color::from_rgb(0.3, 0.7, 1.0)),
    )
    .padding(5)
    .center_x(Length::Fill);

    let title = text("Import Tasks")
        .size(24)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let file_info = column![
        text(format!("File: {}", file_name))
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7)),
        text(format!("Tasks found: {}", task_count))
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7)),
    ]
    .spacing(5)
    .align_x(iced::Alignment::Center);

    let select_label = text("Select target calendar:").size(16).font(iced::Font {
        weight: iced::font::Weight::Medium,
        ..Default::default()
    });

    // Calendar selection list
    let mut calendar_list = column![].spacing(5);
    for cal in &app.calendars {
        if app.disabled_calendars.contains(&cal.href) {
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
                    text(" (Local)")
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
        .height(Length::Fixed(250.0))
        .direction(Direction::Vertical(
            Scrollbar::new().width(8).scroller_width(8),
        ));

    // Action buttons
    let cancel_btn = button(text("Cancel").size(14))
        .style(iced::widget::button::secondary)
        .padding([8, 16])
        .on_press(Message::IcsImportDialogCancel);

    let import_btn = button(text("Import").size(14).font(iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    }))
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

    // Combine all elements
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
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(
                    Color {
                        a: 0.98,
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
        });

    // Overlay on top of existing content
    stack![
        content,
        container(modal_card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.7).into()),
                ..Default::default()
            })
    ]
    .into()
}
