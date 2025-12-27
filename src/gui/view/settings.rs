// File: src/gui/view/settings.rs
use crate::config::AppTheme;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};

use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Color, Element, Length};

pub fn view_settings(app: &GuiApp) -> Element<'_, Message> {
    let is_settings = matches!(app.state, AppState::Settings);

    // --- Header with Back Button ---
    let title_text = text(if is_settings {
        "Settings"
    } else {
        "Welcome to Cfait"
    })
    .size(40);

    let title_row: Element<_> = if is_settings {
        row![
            button(icon::icon(icon::ARROW_LEFT).size(24))
                .style(button::text)
                .on_press(Message::CancelSettings), // Functions as Back
            title_text
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![title_text].into()
    };

    let error = if let Some(e) = &app.error_msg {
        text(e).color(Color::from_rgb(1.0, 0.0, 0.0))
    } else {
        text("")
    };

    // --- Components ---

    let cal_names: Vec<String> = app.calendars.iter().map(|c| c.name.clone()).collect();
    let picker: Element<_> = if !cal_names.is_empty() && is_settings {
        column![
            text("Default calendar:"),
            iced::widget::pick_list(
                cal_names,
                app.ob_default_cal.clone(),
                Message::ObDefaultCalChanged
            )
            .width(Length::Fill)
            .padding(10)
        ]
        .spacing(5)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let prefs: Element<'_, Message> = if is_settings {
        std::convert::Into::<Element<'_, Message>>::into(container(
            column![
                row![
                    text("Theme:"),
                    iced::widget::pick_list(
                        &AppTheme::ALL[..],
                        Some(app.current_theme),
                        Message::ThemeChanged
                    )
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                std::convert::Into::<Element<'_, Message>>::into(
                    checkbox(app.hide_completed)
                        .label("Hide completed and canceled tasks") // RENAMED
                        .on_toggle(Message::ToggleHideCompleted),
                ),
                if !app.hide_completed {
                    std::convert::Into::<Element<'_, Message>>::into(
                        checkbox(app.hide_fully_completed_tags)
                            .label("Hide tags containing only completed tasks")
                            .on_toggle(Message::ToggleHideFullyCompletedTags),
                    )
                } else {
                    std::convert::Into::<Element<'_, Message>>::into(Space::new().width(0))
                },
            ]
            .spacing(10),
        ))
    } else {
        std::convert::Into::<Element<'_, Message>>::into(Space::new().width(0))
    };

    let sorting_ui: Element<_> = if is_settings {
        column![
            text("Sorting priority cutoff (months):"),
            text("(Tasks due within this range are shown first. Blank = all timed first)")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            text_input("6", &app.ob_sort_months_input)
                .on_input(Message::ObSortMonthsChanged)
                .padding(10)
                .width(Length::Fixed(100.0)),
            // New Urgency UI
            Space::new().height(10),
            text("Urgency Rules (Shown at top):").size(16),
            row![
                text("Due within (days):").width(Length::Fixed(150.0)),
                text_input("1", &app.ob_urgent_days_input)
                    .on_input(Message::ObUrgentDaysChanged)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            row![
                text("Priority <= (!):").width(Length::Fixed(150.0)),
                text_input("1", &app.ob_urgent_prio_input)
                    .on_input(Message::ObUrgentPrioChanged)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(5)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let notifications_ui: Element<_> = if is_settings {
        column![
            text("Notifications & Reminders").size(20),
            checkbox(app.auto_reminders)
                .label("Auto-remind on Start/Due dates (if no alarms set)")
                .on_toggle(Message::SetAutoReminders),
            row![
                text("Default reminder time (HH:MM):").width(Length::Fixed(200.0)),
                text_input("09:00", &app.default_reminder_time)
                    .on_input(Message::SetDefaultReminderTime)
                    .width(Length::Fixed(80.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            text("Snooze Presets (minutes):").size(14),
            row![
                text("Short:"),
                text_input("15", &app.snooze_short_mins.to_string())
                    .on_input(Message::SetSnoozeShort)
                    .width(Length::Fixed(60.0))
                    .padding(5),
                text("Long:"),
                text_input("60", &app.snooze_long_mins.to_string())
                    .on_input(Message::SetSnoozeLong)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let aliases_ui: Element<_> = if is_settings {
        let mut list_col = column![text("Tag aliases").size(20)].spacing(10);
        for (key, vals) in &app.tag_aliases {
            let val_str = vals.join(", ");
            let row_item = row![
                text(format!("#{}", key)).width(Length::FillPortion(1)),
                text("->").width(Length::Fixed(20.0)),
                text(val_str).width(Length::FillPortion(2)),
                button(icon::icon(icon::CROSS).size(12))
                    .style(button::danger)
                    .padding(5)
                    .on_press(Message::RemoveAlias(key.clone()))
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);
            list_col = list_col.push(row_item);
        }
        let input_row = row![
            text_input("Alias (#cfait)", &app.alias_input_key)
                .on_input(Message::AliasKeyInput)
                .padding(5)
                .width(Length::FillPortion(1)),
            // FIX: Updated placeholder to show new capabilities
            text_input("#tag,@@loc,!3", &app.alias_input_values)
                .on_input(Message::AliasValueInput)
                .padding(5)
                .width(Length::FillPortion(2)),
            button("Add").padding(5).on_press(Message::AddAlias)
        ]
        .spacing(10);
        let area =
            container(column![list_col, iced::widget::rule::horizontal(1), input_row].spacing(15))
                .padding(10)
                .style(|_| container::Style {
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: Color::from_rgb(0.3, 0.3, 0.3),
                    },
                    ..Default::default()
                });
        area.into()
    } else {
        Space::new().width(0).into()
    };

    let cal_mgmt_ui: Element<_> = if is_settings && !app.calendars.is_empty() {
        let mut col = column![text("Manage calendars").size(20)].spacing(10);
        for cal in &app.calendars {
            let is_enabled = !app.disabled_calendars.contains(&cal.href);
            let row_content = row![
                checkbox(is_enabled)
                    .label(&cal.name)
                    .on_toggle(move |v| Message::ToggleCalendarDisabled(cal.href.clone(), !v))
                    .width(Length::Fill)
            ];
            col = col.push(row_content.spacing(10).align_y(iced::Alignment::Center));
        }
        container(col)
            .padding(10)
            .style(|_| container::Style {
                border: iced::Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: Color::from_rgb(0.3, 0.3, 0.3),
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::new().width(0).into()
    };

    // Connection Button (Moved inside form)
    let save_connect_btn = button(if is_settings {
        "Save & Connect"
    } else {
        "Connect"
    })
    .padding(10)
    .width(Length::Fill)
    .on_press(Message::ObSubmit);

    let insecure_check = checkbox(app.ob_insecure)
        .label("Allow insecure SSL (e.g. self-signed)")
        .on_toggle(Message::ObInsecureToggled)
        .size(16)
        .text_size(14);

    // --- FIX IS HERE ---
    let offline_button_or_space: Element<_> = if !is_settings {
        button("Use offline mode")
            .padding(10)
            .style(button::secondary)
            .on_press(Message::ObSubmitOffline)
            .into()
    } else {
        Space::new().height(0).into()
    };

    // --- FORM LAYOUT ---
    let form = column![
        // 1. Connection Section
        container(
            column![
                text("Server Connection").size(20),
                text("CalDAV server URL:"),
                text_input("https://...", &app.ob_url)
                    .on_input(Message::ObUrlChanged)
                    .padding(10),
                text("Username:"),
                text_input("User", &app.ob_user)
                    .on_input(Message::ObUserChanged)
                    .padding(10),
                text("Password:"),
                text_input("Password", &app.ob_pass)
                    .on_input(Message::ObPassChanged)
                    .secure(true)
                    .padding(10),
                insecure_check,
                save_connect_btn // <--- Moved Here
            ]
            .spacing(15)
        )
        .padding(10)
        .style(|_| container::Style {
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: Color::from_rgba(0.5, 0.5, 0.5, 0.2)
            },
            ..Default::default()
        }),
        // 2. Preferences
        picker,
        prefs,
        notifications_ui,
        sorting_ui,
        aliases_ui,
        cal_mgmt_ui,
        // 3. Bottom Actions (Offline Mode for onboarding)
        offline_button_or_space,
    ]
    .spacing(20)
    .max_width(500);

    let content = column![title_row, error, form]
        .spacing(20)
        .align_x(iced::Alignment::Center);

    container(scrollable(
        container(content)
            .width(Length::Fill)
            .padding(20)
            .center_x(Length::Fill),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
