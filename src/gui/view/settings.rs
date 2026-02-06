// Renders the settings and onboarding screens.
use crate::config::AppTheme;
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::storage::LOCAL_CALENDAR_HREF;

use iced::widget::{
    MouseArea, Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{Color, Element, Length};
#[cfg(feature = "gui")]
use iced_aw::color_picker;
use strum::IntoEnumIterator;

pub fn view_settings(app: &GuiApp) -> Element<'_, Message> {
    let is_settings = matches!(app.state, AppState::Settings);

    // --- Header with Back Button ---
    let title_text = text(if is_settings {
        "Settings"
    } else {
        "Welcome to Cfait"
    })
    .size(40);

    let title_row = if is_settings {
        row![
            button(icon::icon(icon::ARROW_LEFT).size(24))
                .style(button::text)
                .on_press(Message::CancelSettings), // Functions as Back
            title_text,
            Space::new().width(Length::Fill)
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center)
    } else {
        row![title_text, Space::new().width(Length::Fill)]
    };

    let title_drag_area: Element<_> =
        MouseArea::new(container(title_row).width(Length::Fill).padding(20))
            .on_press(Message::WindowDragged)
            .into();

    let error = if let Some(e) = &app.error_msg {
        text(e).color(Color::from_rgb(1.0, 0.0, 0.0))
    } else {
        text("")
    };

    // --- FATAL ERROR GUARD ---
    // If the config file existed but was corrupted (syntax/IO), show a blocking error
    // screen to force the user to fix or remove the file. This prevents accidental
    // overwrites by the onboarding/save flows.
    if app.config_was_corrupted {
        let error_text = app.error_msg.clone().unwrap_or_default();

        return container(
            column![
                icon::icon(icon::TRASH)
                    .size(40)
                    .color(Color::from_rgb(0.8, 0.2, 0.2)),
                text("Configuration File Error").size(24),
                text("The existing configuration file could not be loaded.").size(16),
                container(
                    text(error_text)
                        .size(14)
                        .font(iced::Font::MONOSPACE)
                        .color(Color::from_rgb(0.8, 0.1, 0.1))
                )
                .padding(10)
                .style(container::rounded_box),
                text("To prevent data loss, the application will not start."),
                text("Please fix the syntax error in the file manually or delete it to reset."),
                button("Quit Application")
                    .style(button::danger)
                    .on_press(Message::CloseWindow)
            ]
            .spacing(20)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

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
                        AppTheme::iter().collect::<Vec<_>>(),
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
        container(
            column![
                text("Sorting & Visibility").size(20),
                Space::new().height(10),
                text("Future Task Grace Period:").size(16),
                row![
                    text("Start within (days):").width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_start_grace_input)
                        .on_input(Message::ObStartGraceChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text("Tasks starting within this period won't be pushed to 'Future'")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(15),
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
                Space::new().height(15),
                text("Priority Settings:").size(16),
                row![
                    text("Default Priority (!):").width(Length::Fixed(150.0)),
                    text_input("5", &app.ob_default_priority_input)
                        .on_input(Message::ObDefaultPriorityChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text("(Tasks without priority (0) sort as this value)")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(10),
                row![
                    text("Priority cutoff (months):").width(Length::Fixed(150.0)),
                    text_input("6", &app.ob_sort_months_input)
                        .on_input(Message::ObSortMonthsChanged)
                        .width(Length::Fixed(100.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text("(Tasks due within this range are shown first. Blank = all timed first)")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5),
        )
        .padding(15)
        .style(container::rounded_box)
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
            text("Snooze Presets:").size(14),
            row![
                text("Short:"),
                text_input("1h", &app.ob_snooze_short_input)
                    .on_input(Message::SetSnoozeShort)
                    .width(Length::Fixed(60.0))
                    .padding(5),
                text("Long:"),
                text_input("1d", &app.ob_snooze_long_input)
                    .on_input(Message::SetSnoozeLong)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            // Added Refresh Row
            row![
                text("Auto-refresh interval:").width(Length::Fixed(200.0)),
                text_input("30m", &app.ob_auto_refresh_input)
                    .on_input(Message::SetAutoRefreshInterval)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            text("").size(5),
            text("Calendar Integration").size(20),
            {
                let cb = checkbox(app.create_events_for_tasks)
                    .label("Create calendar events (VEVENT) for tasks with dates");
                if !app.deleting_events {
                    cb.on_toggle(Message::SetCreateEventsForTasks)
                } else {
                    cb
                }
            },
            text("Events will be retroactively created. Use +cal or -cal in task input to override per-task")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            text("").size(5),
            {
                let cb = checkbox(app.delete_events_on_completion)
                    .label("Delete events when tasks are completed");
                if !app.deleting_events {
                    cb.on_toggle(Message::SetDeleteEventsOnCompletion)
                } else {
                    cb
                }
            },
            text("Regardless, events are always deleted when tasks are deleted.")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            {
                let btn = button("Delete all calendar events");
                if !app.deleting_events {
                    btn.on_press(Message::DeleteAllCalendarEvents)
                } else {
                    btn
                }
            },
            if app.deleting_events {
                text("Deleting events...")
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            } else {
                text("")
            },
        ]
        .spacing(10)
        .into()
    } else {
        Space::new().width(0).into()
    };

    // Data Management is now merged into Local Calendar UI below
    let data_management_ui: Element<_> = Space::new().width(0).into();

    let aliases_ui: Element<_> = if is_settings {
        let mut list_col = column![text("Tag aliases").size(20)].spacing(10);
        for (key, vals) in &app.tag_aliases {
            let val_str = vals.join(", ");
            let row_item = row![
                text(if key.starts_with("@@") {
                    key.to_string()
                } else {
                    format!("#{}", key)
                })
                .width(Length::FillPortion(1)),
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
            text_input("Alias (#tag or @@loc)", &app.alias_input_key)
                .on_input(Message::AliasKeyInput)
                .padding(5)
                .width(Length::FillPortion(1)),
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

    // --- Local Calendar Management UI ---
    let local_cal_ui: Element<_> = if is_settings {
        let mut local_cal_col = column![
            text("Local Calendars").size(20),
            text("Manage your offline calendars here.")
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
        ]
        .spacing(10);

        for cal in &app.local_cals_editing {
            let href = cal.href.clone();
            let is_default = href == LOCAL_CALENDAR_HREF;

            // Name input
            let name_input = text_input("Name", &cal.name)
                .on_input(move |s| Message::LocalCalendarNameChanged(href.clone(), s))
                .padding(5)
                .width(Length::FillPortion(3));

            // Export button
            let export_href = cal.href.clone();
            let export_btn = button(
                row![icon::icon(icon::EXPORT).size(14), text("Export").size(10)]
                    .spacing(3)
                    .align_y(iced::Alignment::Center),
            )
            .padding(5)
            .style(button::secondary)
            .on_press(Message::ExportLocalIcs(export_href));

            // Import button
            let import_href = cal.href.clone();
            let import_btn = button(
                row![icon::icon(icon::IMPORT).size(14), text("Import").size(10)]
                    .spacing(3)
                    .align_y(iced::Alignment::Center),
            )
            .padding(5)
            .style(button::secondary)
            .on_press(Message::ImportLocalIcs(import_href));

            // Color Button with palette icon
            let current_color = cal
                .color
                .as_ref()
                .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
                .map(|(r, g, b)| Color::from_rgb(r, g, b))
                .unwrap_or(Color::from_rgb(0.5, 0.5, 0.5));

            let color_btn = button(
                text(icon::PALETTE_COLOR.to_string())
                    .font(icon::FONT)
                    .size(16)
                    .color(current_color),
            )
            .padding(5)
            .style(button::text)
            .on_press(Message::OpenColorPicker(cal.href.clone(), current_color));

            // Wrap in ColorPicker if active
            let color_widget: Element<_> =
                if app.color_picker_active_href.as_ref() == Some(&cal.href) {
                    color_picker::ColorPicker::new(
                        true,
                        current_color,
                        color_btn,
                        Message::CancelColorPicker,
                        Message::SubmitColorPicker,
                    )
                    .into()
                } else {
                    color_btn.into()
                };

            // Trash button (or spacer for default calendar)
            let delete_btn: Element<_> = if !is_default {
                let h = cal.href.clone();
                button(icon::icon(icon::TRASH).size(14))
                    .style(button::danger)
                    .padding(5)
                    .on_press(Message::DeleteLocalCalendar(h))
                    .into()
            } else {
                Space::new().width(Length::Fixed(22.0)).into()
            };

            local_cal_col = local_cal_col.push(
                row![name_input, export_btn, import_btn, color_widget, delete_btn]
                    .spacing(5)
                    .align_y(iced::Alignment::Center),
            );
        }

        local_cal_col = local_cal_col.push(
            button("Add Local Calendar")
                .style(button::secondary)
                .on_press(Message::AddLocalCalendar),
        );

        container(local_cal_col)
            .padding(10)
            .style(|_| container::Style {
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(0.5, 0.5, 0.5, 0.2),
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
        cal_mgmt_ui,
        local_cal_ui,
        data_management_ui,
        prefs,
        notifications_ui,
        sorting_ui,
        aliases_ui,
        // 3. Bottom Actions (Offline Mode for onboarding)
        offline_button_or_space,
    ]
    .spacing(20)
    .max_width(500);

    let scrollable_content = column![error, form]
        .spacing(20)
        .align_x(iced::Alignment::Center);

    let main_col = column![
        title_drag_area,
        scrollable(
            container(scrollable_content)
                .width(Length::Fill)
                .padding(20)
                .center_x(Length::Fill),
        )
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    container(main_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
