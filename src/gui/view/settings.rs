// Renders the settings and onboarding screens.
// File: ./src/gui/view/settings.rs
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct LangOption {
    code: String,
    label: String,
}

impl std::fmt::Display for LangOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

// Dynamically maps ISO codes to their native names using `isolang`
fn get_native_language_name(code: &str) -> String {
    // Extract base code (e.g., "pt" from "pt-BR") for the lookup
    let base_code = code.split(&['-', '_'][..]).next().unwrap_or(code);

    if let Some(lang) = isolang::Language::from_639_1(base_code) {
        // Prefer the autonym (native name), fallback to English name
        let raw_name = lang.to_autonym().unwrap_or_else(|| lang.to_name());

        // Capitalize the first letter nicely for UI presentation
        let mut chars = raw_name.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    } else {
        code.to_string() // Fallback to raw code if completely unknown
    }
}

pub fn view_settings(app: &GuiApp) -> Element<'_, Message> {
    let is_settings = matches!(app.state, AppState::Settings);

    // --- Header with Back Button ---
    let title_text = text(if is_settings {
        rust_i18n::t!("settings")
    } else {
        rust_i18n::t!("welcome_title")
    })
    .size(40);

    let title_row = if is_settings {
        row![
            button(icon::icon(icon::ARROW_LEFT).size(24))
                .style(button::text)
                .on_press(Message::CancelSettings),
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

    if app.config_was_corrupted {
        let error_text = app.error_msg.clone().unwrap_or_default();

        return container(
            column![
                icon::icon(icon::TRASH)
                    .size(40)
                    .color(Color::from_rgb(0.8, 0.2, 0.2)),
                text(rust_i18n::t!("config_error_title")).size(24),
                text(rust_i18n::t!("config_error_corrupted")).size(16),
                container(
                    text(error_text)
                        .size(14)
                        .font(iced::Font::MONOSPACE)
                        .color(Color::from_rgb(0.8, 0.1, 0.1))
                )
                .padding(10)
                .style(container::rounded_box),
                text(rust_i18n::t!("config_error_fix_remove")),
                button(text(rust_i18n::t!("quit_application")))
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

    let cal_names: Vec<String> = app.calendars.iter().map(|c| c.name.clone()).collect();
    let picker: Element<_> = if !cal_names.is_empty() && is_settings {
        column![
            text(rust_i18n::t!("default_collection")),
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

    // Language selector: Auto-detects available locales from rust_i18n
    let lang_picker: Element<_> = if is_settings {
        let mut lang_options = vec![LangOption {
            code: "auto".to_string(),
            label: rust_i18n::t!("language_system").to_string(),
        }];

        // Dynamically populate from the locales/ folder
        let mut available = rust_i18n::available_locales!().to_vec();
        available.sort(); // Keep the dropdown alphabetical by code

        for loc in available {
            lang_options.push(LangOption {
                code: loc.to_string(),
                label: get_native_language_name(loc),
            });
        }

        let current_lang_code = app.language.clone().unwrap_or_else(|| "auto".to_string());
        let current_lang_opt = lang_options
            .iter()
            .find(|o| o.code == current_lang_code)
            .cloned()
            .unwrap_or_else(|| lang_options[0].clone());

        let lang_picker_row = row![
            text(rust_i18n::t!("language_select")),
            iced::widget::pick_list(lang_options, Some(current_lang_opt), |opt| {
                Message::SetLanguage(opt.code)
            })
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        lang_picker_row.into()
    } else {
        Space::new().width(0).into()
    };

    let notifications_ui: Element<_> = if is_settings {
        column![
            text(rust_i18n::t!("notifications_and_reminders")).size(20),
            checkbox::<Message, iced::Theme, iced::Renderer>(app.auto_reminders)
                .label(rust_i18n::t!("auto_remind_on_due_start_label"))
                .on_toggle(Message::SetAutoReminders),
            row![
                text(rust_i18n::t!("default_time_label")).width(Length::Fixed(200.0)),
                text_input("09:00", &app.default_reminder_time)
                    .on_input(Message::SetDefaultReminderTime)
                    .width(Length::Fixed(80.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            text(rust_i18n::t!("snooze_presets")).size(14),
            row![
                text(rust_i18n::t!("short_label")),
                text_input("1h", &app.ob_snooze_short_input)
                    .on_input(Message::SetSnoozeShort)
                    .width(Length::Fixed(60.0))
                    .padding(5),
                text(rust_i18n::t!("long_label")),
                text_input("1d", &app.ob_snooze_long_input)
                    .on_input(Message::SetSnoozeLong)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            row![
                text(rust_i18n::t!("sync_interval_label")).width(Length::Fixed(200.0)),
                text_input("30m", &app.ob_auto_refresh_input)
                    .on_input(Message::SetAutoRefreshInterval)
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            text("").size(5),
            text(rust_i18n::t!("calendar_integration")).size(20),
            {
                let cb =
                    checkbox::<Message, iced::Theme, iced::Renderer>(app.create_events_for_tasks)
                        .label(rust_i18n::t!("create_calendar_events_for_tasks_with_dates"));
                if !app.deleting_events {
                    cb.on_toggle(Message::SetCreateEventsForTasks)
                } else {
                    cb
                }
            },
            text(rust_i18n::t!("create_calendar_events_note"))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            text("").size(5),
            {
                let cb = checkbox::<Message, iced::Theme, iced::Renderer>(
                    app.delete_events_on_completion,
                )
                .label(rust_i18n::t!("delete_calendar_events_on_completion_label"));
                if !app.deleting_events {
                    cb.on_toggle(Message::SetDeleteEventsOnCompletion)
                } else {
                    cb
                }
            },
            text(rust_i18n::t!("events_deleted_on_task_delete"))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            {
                let btn = button(text(rust_i18n::t!("delete_all_calendar_events")));
                if !app.deleting_events {
                    btn.on_press(Message::DeleteAllCalendarEvents)
                } else {
                    btn
                }
            },
            if app.deleting_events {
                text(rust_i18n::t!("export_debug_status_exporting"))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            } else {
                text(rust_i18n::t!("calendar_events_reversible_note"))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            },
        ]
        .spacing(10)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let advanced_ui: Element<_> = if is_settings {
        let content = if app.show_advanced_settings {
            let hide_fully_ui: Element<_> = if !app.hide_completed {
                checkbox::<Message, iced::Theme, iced::Renderer>(app.hide_fully_completed_tags)
                    .label(rust_i18n::t!("hide_fully_completed_tags"))
                    .on_toggle(Message::ToggleHideFullyCompletedTags)
                    .into()
            } else {
                Space::new().width(0).into()
            };

            column![
                text(rust_i18n::t!("sorting_and_visibility")).size(20),
                checkbox::<Message, iced::Theme, iced::Renderer>(app.hide_completed)
                    .label(rust_i18n::t!("hide_completed_and_canceled_tasks"))
                    .on_toggle(Message::ToggleHideCompleted),
                hide_fully_ui,
                checkbox::<Message, iced::Theme, iced::Renderer>(app.strikethrough_completed)
                    .label(rust_i18n::t!("strikethrough_completed"))
                    .on_toggle(Message::SetStrikethroughCompleted),
                Space::new().height(10),
                text(rust_i18n::t!("priority_rules")).size(16),
                row![
                    text(rust_i18n::t!("due_within_days")).width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_urgent_days_input)
                        .on_input(Message::ObUrgentDaysChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                row![
                    text(rust_i18n::t!("priority_le")).width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_urgent_prio_input)
                        .on_input(Message::ObUrgentPrioChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                row![
                    text(rust_i18n::t!("default_priority_label")).width(Length::Fixed(150.0)),
                    text_input("5", &app.ob_default_priority_input)
                        .on_input(Message::ObDefaultPriorityChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                Space::new().height(10),
                text(rust_i18n::t!("sorting_timeframes")).size(16),
                row![
                    text(rust_i18n::t!("start_grace_days")).width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_start_grace_input)
                        .on_input(Message::ObStartGraceChanged)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                row![
                    text(rust_i18n::t!("priority_cutoff_months")).width(Length::Fixed(150.0)),
                    text_input("6", &app.ob_sort_months_input)
                        .on_input(Message::ObSortMonthsChanged)
                        .width(Length::Fixed(100.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                Space::new().height(10),
                text(rust_i18n::t!("display_limits")).size(16),
                row![
                    text(rust_i18n::t!("max_completed_tasks_root")).width(Length::Fixed(200.0)),
                    text_input("20", &app.ob_max_done_roots_input)
                        .on_input(Message::SetMaxDoneRoots)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text(rust_i18n::t!("max_completed_tasks_root_explain"))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(10),
                row![
                    text(rust_i18n::t!("max_completed_subtasks")).width(Length::Fixed(200.0)),
                    text_input("5", &app.ob_max_done_subtasks_input)
                        .on_input(Message::SetMaxDoneSubtasks)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text(rust_i18n::t!("max_completed_subtasks_explain"))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(10),
                text(rust_i18n::t!("data_management")).size(16),
                row![
                    text(rust_i18n::t!("trash_retention_days_label")).width(Length::Fixed(200.0)),
                    text_input("14", &app.ob_trash_retention_input)
                        .on_input(Message::SetTrashRetention)
                        .width(Length::Fixed(60.0))
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                text(rust_i18n::t!("trash_retention_explain"))
                    .size(12)
                    .color(Color::from_rgb(0.6, 0.6, 0.6)),
            ]
            .spacing(5)
            .padding(10)
        } else {
            column![]
        };

        column![
            button(
                row![
                    text(rust_i18n::t!("advanced_settings_button")).size(16),
                    Space::new().width(Length::Fill),
                    icon::icon(if app.show_advanced_settings {
                        icon::ARROW_EXPAND_UP
                    } else {
                        icon::ARROW_EXPAND_DOWN
                    })
                    .size(14)
                ]
                .align_y(iced::Alignment::Center)
            )
            .width(Length::Fill)
            .style(iced::widget::button::text)
            .on_press(Message::ToggleAdvancedSettings(!app.show_advanced_settings)),
            content
        ]
        .spacing(5)
        .into()
    } else {
        Space::new().width(0).into()
    };

    let aliases_ui: Element<_> = if is_settings {
        let mut list_col = column![text(rust_i18n::t!("tag_aliases")).size(20)].spacing(10);
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
            text_input(&rust_i18n::t!("alias_key_label"), &app.alias_input_key)
                .on_input(Message::AliasKeyInput)
                .padding(5)
                .width(Length::FillPortion(1)),
            text_input(&rust_i18n::t!("alias_value_label"), &app.alias_input_values)
                .on_input(Message::AliasValueInput)
                .padding(5)
                .width(Length::FillPortion(2)),
            button(text(rust_i18n::t!("add")))
                .padding(5)
                .on_press(Message::AddAlias)
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
        let mut col = column![text(rust_i18n::t!("manage_collections")).size(20)].spacing(10);
        for cal in &app.calendars {
            let is_enabled = !app.disabled_calendars.contains(&cal.href);
            let row_content = row![
                checkbox::<Message, iced::Theme, iced::Renderer>(is_enabled)
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

    let local_cal_ui: Element<_> = if is_settings {
        let mut local_cal_col = column![
            text(rust_i18n::t!("local_collections")).size(20),
            text(rust_i18n::t!("local_collections_explain"))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
        ]
        .spacing(10);

        for cal in &app.local_cals_editing {
            let href = cal.href.clone();
            let is_default = href == LOCAL_CALENDAR_HREF;

            let name_input = text_input(&rust_i18n::t!("name_label"), &cal.name)
                .on_input(move |s| Message::LocalCalendarNameChanged(href.clone(), s))
                .padding(5)
                .width(Length::FillPortion(3));

            let export_href = cal.href.clone();
            let export_btn = button(
                row![
                    icon::icon(icon::EXPORT).size(14),
                    text(rust_i18n::t!("export")).size(10)
                ]
                .spacing(3)
                .align_y(iced::Alignment::Center),
            )
            .padding(5)
            .style(button::secondary)
            .on_press(Message::ExportLocalIcs(export_href));

            let import_href = cal.href.clone();
            let import_btn = button(
                row![
                    icon::icon(icon::IMPORT).size(14),
                    text(rust_i18n::t!("import")).size(10)
                ]
                .spacing(3)
                .align_y(iced::Alignment::Center),
            )
            .padding(5)
            .style(button::secondary)
            .on_press(Message::ImportLocalIcs(import_href));

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
            button(text(rust_i18n::t!("create_new_local_calendar")))
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

    let save_connect_btn = button(text(if is_settings {
        rust_i18n::t!("save_and_connect")
    } else {
        rust_i18n::t!("connect")
    }))
    .padding(10)
    .width(Length::Fill)
    .on_press(Message::ObSubmit);

    let insecure_check = checkbox::<Message, iced::Theme, iced::Renderer>(app.ob_insecure)
        .label(rust_i18n::t!("allow_insecure_ssl"))
        .on_toggle(Message::ObInsecureToggled)
        .size(16)
        .text_size(14);

    let offline_button_or_space: Element<_> = if !is_settings {
        button(text(rust_i18n::t!("local_label")))
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
                text(rust_i18n::t!("server_connection")).size(20),
                text(rust_i18n::t!("caldav_url")),
                text_input("https://...", &app.ob_url)
                    .on_input(Message::ObUrlChanged)
                    .padding(10),
                text(rust_i18n::t!("username")),
                text_input("User", &app.ob_user)
                    .on_input(Message::ObUserChanged)
                    .padding(10),
                text(rust_i18n::t!("password")),
                text_input("Password", &app.ob_pass)
                    .on_input(Message::ObPassChanged)
                    .secure(true)
                    .padding(10),
                insecure_check,
                save_connect_btn
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
        lang_picker, // <-- language picker added to the form layout
        picker,
        cal_mgmt_ui,
        local_cal_ui,
        notifications_ui,
        aliases_ui,
        advanced_ui,
        // 3. Bottom Actions
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
