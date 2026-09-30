// SPDX-License-Identifier: GPL-3.0-or-later
// Renders the settings and onboarding screens.
// File: ./src/gui/view/settings.rs
use crate::config::{AppTheme, LogLevel};
use crate::gui::icon;
use crate::gui::message::{Message, NumericField};
use crate::gui::state::{AppState, GuiApp};
use crate::storage::LOCAL_CALENDAR_HREF;

use iced::widget::{
    MouseArea, Space, button, checkbox, column, container, row, scrollable, text, text_input,
    tooltip,
};
use iced::{Color, Element, Length, Theme};
use iced_aw::color_picker;
use strum::IntoEnumIterator;

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

/// Muted 12px caption used for explanatory notes in the settings form.
fn muted_note<'a>(
    content: impl iced::widget::text::IntoFragment<'a>,
) -> iced::widget::Text<'a, iced::Theme, iced::Renderer> {
    text(content).size(12).color(Color::from_rgb(0.6, 0.6, 0.6))
}

/// A checkbox pre-typed for the settings form's message/theme/renderer.
fn cb(
    is_on: bool,
) -> iced::widget::checkbox::Checkbox<'static, Message, iced::Theme, iced::Renderer> {
    checkbox(is_on)
}

/// Text style that highlights a row currently being edited in the goals or
/// aliases lists.
fn editing_row_style(is_editing: bool) -> impl Fn(&Theme) -> text::Style {
    move |theme: &Theme| {
        if is_editing {
            text::Style {
                color: Some(theme.extended_palette().primary.base.color),
            }
        } else {
            text::Style::default()
        }
    }
}

/// A bordered box around a settings section.
fn bordered(radius: f32, border_color: Color) -> container::Style {
    container::Style {
        border: iced::Border {
            radius: radius.into(),
            width: 1.0,
            color: border_color,
        },
        ..Default::default()
    }
}

/// The local/remote badge in the collection manager list.
fn cal_tag<'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    background: Color,
) -> Element<'a, Message> {
    container(text(label).size(10))
        .padding(3)
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// A label + numeric text-input row for the settings form.
fn numeric_row<'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    label_width: f32,
    placeholder: &'a str,
    value: &'a str,
    field: NumericField,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(label_width)),
        text_input(placeholder, value)
            .on_input(move |v| Message::SetNumericField(field, v))
            .width(Length::Fixed(60.0))
            .padding(5),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center)
    .into()
}

/// A label + pick-list row for the settings form.
fn pick_row<'a, T>(
    label: impl iced::widget::text::IntoFragment<'a>,
    options: Vec<T>,
    selected: Option<T>,
    on_select: impl Fn(T) -> Message + 'a,
    width: Length,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
{
    row![
        text(label).width(Length::Fixed(200.0)),
        iced::widget::pick_list(options, selected, on_select)
            .width(width)
            .padding(5),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The add button, or the check/cancel pair shown while editing a goals or
/// aliases list row.
fn add_cancel_buttons(editing: bool, add: Message, cancel: Message) -> Element<'static, Message> {
    if editing {
        row![
            button(icon::icon(icon::CHECK).size(14))
                .style(button::success)
                .padding(6)
                .on_press(add),
            button(icon::icon(icon::CROSS).size(14))
                .style(button::danger)
                .padding(6)
                .on_press(cancel),
        ]
        .spacing(5)
        .into()
    } else {
        row![button(text(rust_i18n::t!("add"))).padding(5).on_press(add)].into()
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

    let mut banners = column![].spacing(5);
    if let Some(e) = &app.error_msg {
        banners = banners.push(crate::gui::view::message_banner(
            e,
            app.theme().extended_palette().background.base.text,
            |_| Color::from_rgb(0.8, 0.2, 0.2).into(),
            Message::DismissError,
        ));
    }
    if let Some(info) = &app.info_msg {
        banners = banners.push(crate::gui::view::message_banner(
            info,
            app.theme().extended_palette().background.base.text,
            |theme: &Theme| theme.extended_palette().success.base.color.into(),
            Message::DismissInfo(app.info_msg_version),
        ));
    }
    if let Some(Some(warn)) = crate::system::KEYRING_WARNING.get() {
        banners = banners.push(text(warn).color(Color::from_rgb(1.0, 0.6, 0.0)));
    }

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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CalOption {
        name: String,
        href: String,
    }
    impl std::fmt::Display for CalOption {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.name)
        }
    }

    let cal_options: Vec<CalOption> = app
        .calendars
        .iter()
        .filter(|c| !crate::storage::is_system_calendar(&c.href))
        .map(|c| CalOption {
            name: c.name.clone(),
            href: c.href.clone(),
        })
        .collect();

    let current_cal_opt = cal_options
        .iter()
        .find(|o| {
            Some(&o.href) == app.ob_default_cal.as_ref()
                || Some(&o.name) == app.ob_default_cal.as_ref()
        })
        .cloned();

    let picker: Element<_> = if !cal_options.is_empty() && is_settings {
        column![
            text(rust_i18n::t!("default_collection")).size(20),
            iced::widget::pick_list(cal_options, current_cal_opt, |opt| {
                Message::ObDefaultCalChanged(opt.href)
            })
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
                label: get_native_language_name(&loc),
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

        column![
            lang_picker_row,
            button(
                text(rust_i18n::t!(
                    "translation_help",
                    url = "translate.codeberg.org"
                ))
                .size(12)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().primary.base.color)
                })
            )
            .style(button::text)
            .padding(0)
            .on_press(Message::OpenUrl(
                "https://translate.codeberg.org/projects/cfait/".to_string()
            ))
        ]
        .spacing(5)
        .into()
    } else {
        Space::new().width(0).into()
    };

    // Theme selection UI
    let theme_picker: Element<_> = if is_settings {
        container(
            column![
                row![
                    text(rust_i18n::t!("app_theme")),
                    iced::widget::pick_list(
                        AppTheme::iter().collect::<Vec<_>>(),
                        Some(app.current_theme),
                        Message::ThemeChanged
                    )
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(10),
        )
        .into()
    } else {
        Space::new().width(0).into()
    };

    let notifications_ui: Element<_> = if is_settings {
        column![
            text(rust_i18n::t!("notifications_and_reminders")).size(20),
            cb(app.auto_reminders)
                .label(rust_i18n::t!("auto_remind_on_due_start_label"))
                .on_toggle(|v| Message::ToggleField(
                    crate::gui::message::BoolField::AutoReminders,
                    v
                )),
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
                    .on_input(|v| Message::SetStringField(
                        crate::gui::message::StringField::SnoozeShort,
                        v
                    ))
                    .width(Length::Fixed(60.0))
                    .padding(5),
                text(rust_i18n::t!("long_label")),
                text_input("1d", &app.ob_snooze_long_input)
                    .on_input(|v| Message::SetStringField(
                        crate::gui::message::StringField::SnoozeLong,
                        v
                    ))
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            row![
                text(rust_i18n::t!("sync_interval_label")).width(Length::Fixed(200.0)),
                text_input("30m", &app.ob_auto_refresh_input)
                    .on_input(|v| Message::SetStringField(
                        crate::gui::message::StringField::AutoRefresh,
                        v
                    ))
                    .width(Length::Fixed(60.0))
                    .padding(5)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            Space::new().height(5),
            text(rust_i18n::t!("calendar_integration")).size(20),
            {
                let events_checkbox = cb(app.create_events_for_tasks)
                    .label(rust_i18n::t!("create_calendar_events_for_tasks_with_dates"));
                if !app.deleting_events {
                    events_checkbox.on_toggle(|v| {
                        Message::ToggleField(
                            crate::gui::message::BoolField::CreateEventsForTasks,
                            v,
                        )
                    })
                } else {
                    events_checkbox
                }
            },
            muted_note(rust_i18n::t!("create_calendar_events_note")),
            Space::new().height(5),
            {
                let delete_events_checkbox = cb(app.delete_events_on_completion)
                    .label(rust_i18n::t!("delete_calendar_events_on_completion_label"));
                if !app.deleting_events {
                    delete_events_checkbox.on_toggle(|v| {
                        Message::ToggleField(
                            crate::gui::message::BoolField::DeleteEventsOnCompletion,
                            v,
                        )
                    })
                } else {
                    delete_events_checkbox
                }
            },
            muted_note(rust_i18n::t!("events_deleted_on_task_delete")),
            {
                let btn = button(text(rust_i18n::t!("delete_all_calendar_events")));
                if !app.deleting_events {
                    btn.on_press(Message::DeleteAllCalendarEvents)
                } else {
                    btn
                }
            },
            if app.deleting_events {
                muted_note(rust_i18n::t!("export_debug_status_exporting"))
            } else {
                muted_note(rust_i18n::t!("calendar_events_reversible_note"))
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
                cb(app.hide_fully_completed_tags)
                    .label(rust_i18n::t!("hide_fully_completed_tags"))
                    .on_toggle(|v| {
                        Message::ToggleField(
                            crate::gui::message::BoolField::HideFullyCompletedTags,
                            v,
                        )
                    })
                    .into()
            } else {
                Space::new().width(0).into()
            };

            column![
                text(rust_i18n::t!("sorting_and_visibility"))
                    .size(20)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    }),
                Space::new().height(5),
                text(rust_i18n::t!("settings_visibility"))
                    .size(16)
                    .style(|t: &Theme| text::Style {
                        color: Some(t.extended_palette().primary.base.color)
                    }),
                cb(app.hide_completed)
                    .label(rust_i18n::t!("hide_completed_and_canceled_tasks"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::HideCompleted,
                        v
                    )),
                hide_fully_ui,
                tooltip(
                    cb(app.hide_aliases_in_sidebar)
                        .label(rust_i18n::t!("hide_aliases_in_sidebar"))
                        .on_toggle(|v| Message::ToggleField(
                            crate::gui::message::BoolField::HideAliasesInSidebar,
                            v
                        )),
                    text(rust_i18n::t!("hide_aliases_in_sidebar_tooltip")).size(12),
                    tooltip::Position::Top
                )
                .style(crate::gui::view::tooltip_style)
                .delay(std::time::Duration::from_millis(700)),
                cb(app.blur_when_unfocused)
                    .label(rust_i18n::t!("blur_when_unfocused"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::BlurWhenUnfocused,
                        v
                    )),
                cb(app.show_inline_descriptions)
                    .label(rust_i18n::t!("show_inline_descriptions"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowInlineDescriptions,
                        v
                    )),
                cb(app.show_priority_numbers)
                    .label(rust_i18n::t!("show_priority_numbers"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowPriorityNumbers,
                        v
                    )),
                cb(app.show_calendars_tab)
                    .label(rust_i18n::t!("show_calendars_tab"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowCalendarsTab,
                        v
                    )),
                cb(app.show_tags_tab)
                    .label(rust_i18n::t!("show_tags_tab"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowTagsTab,
                        v
                    )),
                cb(app.show_locations_tab)
                    .label(rust_i18n::t!("show_locations_tab"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowLocationsTab,
                        v
                    )),
                cb(app.show_journal_tab)
                    .label(rust_i18n::t!("show_journal_tab"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowJournalTab,
                        v
                    )),
                cb(app.strikethrough_completed)
                    .label(rust_i18n::t!("strikethrough_completed"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::StrikethroughCompleted,
                        v
                    )),
                Space::new().height(10),
                text(rust_i18n::t!("settings_sorting"))
                    .size(16)
                    .style(|t: &Theme| text::Style {
                        color: Some(t.extended_palette().primary.base.color)
                    }),
                cb(app.sort_standard_by_priority)
                    .label(rust_i18n::t!("sort_standard_by_priority_label"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::SortStandardByPriority,
                        v
                    )),
                muted_note(rust_i18n::t!("sort_standard_by_priority_explain")),
                cb(app.sort_tiebreak_recent)
                    .label(rust_i18n::t!("sort_tiebreak_recent"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::SortTiebreakRecent,
                        v
                    )),
                Space::new().height(5),
                pick_row(
                    rust_i18n::t!("settings_paused_tasks"),
                    crate::config::PausedSortBehavior::iter().collect(),
                    Some(app.paused_sort_behavior),
                    Message::SetPausedSortBehavior,
                    Length::Fill,
                ),
                Space::new().height(5),
                pick_row(
                    rust_i18n::t!("sorting_preset_label"),
                    crate::config::SortPreset::iter().collect(),
                    Some(app.sort_preset),
                    Message::SetSortPreset,
                    Length::Fill,
                ),
                muted_note(rust_i18n::t!("settings_sort_preset_explain")),
                Space::new().height(10),
                pick_row(
                    rust_i18n::t!("first_day_of_week"),
                    crate::config::FirstDayOfWeek::iter().collect(),
                    Some(app.first_day_of_week),
                    Message::SetFirstDayOfWeek,
                    Length::Fill,
                ),
                Space::new().height(10),
                text(rust_i18n::t!("settings_urgent_and_timeframes"))
                    .size(16)
                    .style(|t: &Theme| text::Style {
                        color: Some(t.extended_palette().primary.base.color)
                    }),
                text(rust_i18n::t!("settings_urgent_definition")).size(18),
                numeric_row(
                    rust_i18n::t!("due_within_days"),
                    150.0,
                    "1",
                    &app.ob_urgent_days_input,
                    NumericField::UrgentDays,
                ),
                numeric_row(
                    rust_i18n::t!("priority_le"),
                    150.0,
                    "1",
                    &app.ob_urgent_prio_input,
                    NumericField::UrgentPrio,
                ),
                muted_note(rust_i18n::t!("settings_urgent_explain")),
                Space::new().height(10),
                text(rust_i18n::t!("settings_timeframes_cutoffs")).size(18),
                numeric_row(
                    rust_i18n::t!("priority_cutoff_days"),
                    150.0,
                    "30",
                    &app.ob_sort_days_input,
                    NumericField::SortDays,
                ),
                muted_note(rust_i18n::t!("settings_cutoff_explain")),
                numeric_row(
                    rust_i18n::t!("start_grace_days"),
                    150.0,
                    "1",
                    &app.ob_start_grace_input,
                    NumericField::StartGrace,
                ),
                muted_note(rust_i18n::t!("settings_start_grace_explain")),
                Space::new().height(10),
                text(rust_i18n::t!("settings_defaults")).size(18),
                numeric_row(
                    rust_i18n::t!("default_priority_label"),
                    150.0,
                    "5",
                    &app.ob_default_priority_input,
                    NumericField::DefaultPriority,
                ),
                muted_note(rust_i18n::t!("settings_default_prio_explain")),
                Space::new().height(10),
                text(rust_i18n::t!("display_limits")).size(18),
                numeric_row(
                    rust_i18n::t!("max_completed_tasks_root"),
                    200.0,
                    "20",
                    &app.ob_max_done_roots_input,
                    NumericField::MaxDoneRoots,
                ),
                muted_note(rust_i18n::t!("max_completed_tasks_root_explain")),
                Space::new().height(10),
                numeric_row(
                    rust_i18n::t!("max_completed_subtasks"),
                    200.0,
                    "5",
                    &app.ob_max_done_subtasks_input,
                    NumericField::MaxDoneSubtasks,
                ),
                muted_note(rust_i18n::t!("max_completed_subtasks_explain")),
                Space::new().height(10),
                text(rust_i18n::t!("pinned_actions_label")).size(18),
                {
                    let mut action_col = column![].spacing(2);
                    for action in crate::config::TaskAction::ALL {
                        let is_pinned = app.pinned_actions.contains(action);
                        let action_val = *action; // Copy for closure

                        let icon_char = match action_val {
                            crate::config::TaskAction::CompleteAndShift => icon::REPEAT,
                            crate::config::TaskAction::ToggleDetails => icon::INFO,
                            crate::config::TaskAction::ToggleTimer => icon::PLAY,
                            crate::config::TaskAction::StopTimer => icon::DEBUG_STOP,
                            crate::config::TaskAction::AddSession => icon::TIMER_PLUS,
                            crate::config::TaskAction::IncreasePriority => icon::PLUS,
                            crate::config::TaskAction::DecreasePriority => icon::MINUS,
                            crate::config::TaskAction::Cancel => icon::CROSS,
                            crate::config::TaskAction::Edit => icon::EDIT,
                            crate::config::TaskAction::EditTree => icon::EDIT_TREE,
                            crate::config::TaskAction::Delete
                            | crate::config::TaskAction::DeleteTree => icon::TRASH,
                            crate::config::TaskAction::Yank => icon::LINK,
                            crate::config::TaskAction::Focus => icon::FOCUS_FIELD,
                            crate::config::TaskAction::TogglePin => icon::THUMB_TACK,
                            crate::config::TaskAction::CreateSubtask => icon::CREATE_CHILD,
                            crate::config::TaskAction::DuplicateTree => icon::CLONE,
                            crate::config::TaskAction::CompleteTree => icon::LIST_CHECK,
                            crate::config::TaskAction::Promote => icon::ELEVATOR_UP,
                            crate::config::TaskAction::Move => icon::MOVE,
                            crate::config::TaskAction::OpenCoordinates => icon::MAP_LOCATION_DOT,
                            crate::config::TaskAction::OpenLocations => icon::MAP_MARKER_MULTIPLE,
                            crate::config::TaskAction::OpenUrl => icon::URL_CHECK,
                            crate::config::TaskAction::CopyUrl => icon::COPY,
                            crate::config::TaskAction::BrowseRelations => icon::LINK,
                        };

                        let check_icon = if is_pinned {
                            icon::CHECK_SQUARE
                        } else {
                            icon::SQUARE
                        };

                        let toggle_row = row![
                            icon::icon(check_icon).size(16),
                            icon::icon(icon_char).size(14).width(Length::Fixed(20.0)),
                            text(action.label()).size(14)
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center);

                        let row_btn = button(toggle_row)
                            .style(iced::widget::button::text)
                            .padding(4)
                            .width(Length::Fill)
                            .on_press(Message::TogglePinnedAction(action_val, !is_pinned));

                        action_col = action_col.push(row_btn);
                    }
                    action_col
                },
                Space::new().height(10),
                text(rust_i18n::t!("data_management")).size(18),
                numeric_row(
                    rust_i18n::t!("trash_retention_days_label"),
                    200.0,
                    "14",
                    &app.ob_trash_retention_input,
                    NumericField::TrashRetention,
                ),
                muted_note(rust_i18n::t!("trash_retention_explain")),
                Space::new().height(10),
                numeric_row(
                    rust_i18n::t!("implicit_goal_duration"),
                    200.0,
                    "60",
                    &app.ob_default_duration_goal_mins_input,
                    NumericField::DefaultDurationGoal,
                ),
                muted_note(rust_i18n::t!("implicit_goal_duration_explain")),
                Space::new().height(5),
                cb(app.sessions_count_as_completions)
                    .label(rust_i18n::t!("sessions_count_as_completions"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::SessionsCountAsCompletions,
                        v
                    )),
                Space::new().height(10),
                text(rust_i18n::t!("logging_label")).size(18),
                pick_row(
                    rust_i18n::t!("log_level_label"),
                    LogLevel::ALL.to_vec(),
                    Some(app.log_level),
                    Message::SetLogLevel,
                    Length::Fixed(120.0),
                ),
                muted_note(rust_i18n::t!("log_level_explain")),
                Space::new().height(10),
                text(rust_i18n::t!("quick_filter_title")).size(18),
                cb(app.show_quick_filter)
                    .label(rust_i18n::t!("quick_filter_show_button"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowQuickFilter,
                        v
                    )),
                cb(app.core_config.show_task_goals_in_sidebar)
                    .label(rust_i18n::t!("show_task_goals_in_sidebar"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::ShowTaskGoalsInSidebar,
                        v
                    )),
                row![
                    text(rust_i18n::t!("quick_filter_search_term")).width(Length::Fixed(150.0)),
                    text_input("is:ready", &app.ob_quick_filter_term_input)
                        .on_input(|v| Message::SetStringField(
                            crate::gui::message::StringField::QuickFilterTerm,
                            v
                        ))
                        .width(Length::Fill)
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                row![
                    text(rust_i18n::t!("quick_filter_icon")).width(Length::Fixed(150.0)),
                    text_input("f0fa9", &app.ob_quick_filter_icon_input)
                        .on_input(|v| Message::SetStringField(
                            crate::gui::message::StringField::QuickFilterIcon,
                            v
                        ))
                        .width(Length::Fill)
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                Space::new().height(10),
                text(rust_i18n::t!("server_connection")).size(18),
                row![
                    text(rust_i18n::t!("tls_client_cert_path")).width(Length::Fixed(200.0)),
                    text_input("", &app.ob_tls_client_cert_path)
                        .on_input(Message::SetTlsClientCertPath)
                        .width(Length::Fill)
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                row![
                    text(rust_i18n::t!("tls_client_key_path")).width(Length::Fixed(200.0)),
                    text_input("", &app.ob_tls_client_key_path)
                        .on_input(Message::SetTlsClientKeyPath)
                        .width(Length::Fill)
                        .padding(5)
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                Space::new().height(10),
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

    let goals_ui: Element<_> = if is_settings {
        let input_row = row![
            text_input(&rust_i18n::t!("alias_key_label"), &app.goal_input_key)
                .on_input(Message::GoalKeyInput)
                .padding(5)
                .width(Length::FillPortion(2)),
            iced::widget::pick_list(
                crate::config::GoalType::iter().collect::<Vec<_>>(),
                Some(app.goal_input_type),
                Message::GoalTypeChanged
            )
            .width(Length::FillPortion(2))
            .padding(5),
            text_input(&rust_i18n::t!("goal_input_target"), &app.goal_input_target)
                .on_input(Message::GoalTargetInput)
                .padding(5)
                .width(Length::FillPortion(1)),
            text_input(&rust_i18n::t!("goal_input_amount"), &app.goal_input_amount)
                .on_input(Message::GoalAmountChanged)
                .padding(5)
                .width(Length::FillPortion(1)),
            iced::widget::pick_list(
                crate::config::IntervalUnit::iter().collect::<Vec<_>>(),
                Some(app.goal_input_unit),
                Message::GoalUnitChanged
            )
            .width(Length::FillPortion(2))
            .padding(5),
            add_cancel_buttons(
                app.editing_goal_key.is_some(),
                Message::AddGoal,
                Message::CancelEditGoal,
            ),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let mut list_col = column![
            text(rust_i18n::t!("goals")).size(20),
            cb(app.show_goals_tab)
                .label(rust_i18n::t!("show_goals_tab"))
                .on_toggle(|v| Message::ToggleField(
                    crate::gui::message::BoolField::ShowGoalsTab,
                    v
                )),
            Space::new().height(5),
            input_row,
            iced::widget::rule::horizontal(1)
        ]
        .spacing(10);

        let mut sorted_goals: Vec<_> = app.core_config.goals.iter().collect();
        sorted_goals.sort_by_key(|(k, _)| k.to_string());

        for (key, goal) in sorted_goals {
            let is_editing_this = app.editing_goal_key.as_ref() == Some(key);

            let key_text = text(key)
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::Glyph)
                .style(editing_row_style(is_editing_this));

            let type_text = text(goal.goal_type.to_string())
                .width(Length::FillPortion(2))
                .style(editing_row_style(is_editing_this));

            let target_text = text(goal.target.to_string())
                .width(Length::FillPortion(1))
                .style(editing_row_style(is_editing_this));

            let period_text = text(goal.interval.format_short())
                .width(Length::FillPortion(2))
                .style(editing_row_style(is_editing_this));

            let row_item = row![
                key_text,
                type_text,
                target_text,
                period_text,
                button(icon::icon(icon::EDIT).size(12))
                    .style(button::secondary)
                    .padding(5)
                    .on_press(Message::EditGoal(key.clone(), goal.clone())),
                button(icon::icon(icon::CROSS).size(12))
                    .style(button::danger)
                    .padding(5)
                    .on_press(Message::RemoveGoal(key.clone()))
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);
            list_col = list_col.push(row_item);
        }

        let area = container(list_col)
            .padding(10)
            .style(|_| bordered(4.0, Color::from_rgb(0.3, 0.3, 0.3)));
        area.into()
    } else {
        Space::new().width(0).into()
    };

    let aliases_ui: Element<_> = if is_settings {
        let input_row = row![
            text_input(&rust_i18n::t!("alias_key_label"), &app.alias_input_key)
                .on_input(Message::AliasKeyInput)
                .padding(5)
                .width(Length::FillPortion(1)),
            text_input(&rust_i18n::t!("alias_value_label"), &app.alias_input_values)
                .on_input(Message::AliasValueInput)
                .on_submit(Message::AddAlias)
                .padding(5)
                .width(Length::FillPortion(2)),
            add_cancel_buttons(
                app.editing_alias_key.is_some(),
                Message::AddAlias,
                Message::CancelEditAlias,
            ),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let mut list_col = column![
            text(rust_i18n::t!("tag_aliases")).size(20),
            input_row,
            iced::widget::rule::horizontal(1)
        ]
        .spacing(10);

        let mut sorted_aliases: Vec<_> = app.tag_aliases.iter().collect();
        sorted_aliases.sort_by_key(|(k, _)| k.to_string());

        for (key, vals) in sorted_aliases {
            let val_str = vals.join(", ");
            let is_editing_this = app.editing_alias_key.as_ref() == Some(key);

            let key_text = text(if key.starts_with("@@") {
                key.to_string()
            } else {
                format!("#{}", key)
            })
            .width(Length::FillPortion(1))
            .wrapping(iced::widget::text::Wrapping::Glyph)
            .style(editing_row_style(is_editing_this));

            let val_text = text(val_str.clone())
                .width(Length::FillPortion(2))
                .wrapping(iced::widget::text::Wrapping::Glyph)
                .style(editing_row_style(is_editing_this));

            let row_item = row![
                key_text,
                text("->").width(Length::Fixed(20.0)),
                val_text,
                button(icon::icon(icon::EDIT).size(12))
                    .style(button::secondary)
                    .padding(5)
                    .on_press(Message::EditAlias(key.clone(), val_str)),
                button(icon::icon(icon::CROSS).size(12))
                    .style(button::danger)
                    .padding(5)
                    .on_press(Message::RemoveAlias(key.clone()))
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);
            list_col = list_col.push(row_item);
        }

        let area = container(list_col)
            .padding(10)
            .style(|_| bordered(4.0, Color::from_rgb(0.3, 0.3, 0.3)));
        area.into()
    } else {
        Space::new().width(0).into()
    };

    let collections_ui: Element<_> = if is_settings {
        let mut col = column![
            text(rust_i18n::t!("manage_collections")).size(20),
            row![
                cb(app.sort_collections_by_size)
                    .label(rust_i18n::t!("sort_collections_by_size"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::SortCollectionsBySize,
                        v
                    )),
            ]
            .align_y(iced::Alignment::Center)
        ]
        .spacing(10);

        let mut ordered_cals = Vec::new();
        for cal in &app.calendars {
            if crate::storage::is_system_calendar(&cal.href) {
                continue;
            }
            if cal.href.starts_with("local://") {
                if let Some(edit_cal) = app.local_cals_editing.iter().find(|c| c.href == cal.href) {
                    ordered_cals.push((edit_cal.clone(), true));
                }
            } else {
                if let Some(edit_cal) = app.remote_cals_editing.iter().find(|c| c.href == cal.href)
                {
                    ordered_cals.push((edit_cal.clone(), false));
                }
            }
        }
        for cal in &app.remote_cals_editing {
            if cal.href.starts_with("new_remote_") {
                ordered_cals.push((cal.clone(), false));
            }
        }

        let total = ordered_cals.len();
        for (index, (cal, is_local)) in ordered_cals.into_iter().enumerate() {
            let cal_href = cal.href.clone();
            let is_enabled = !app.disabled_calendars.contains(&cal_href);
            let is_default = cal_href == LOCAL_CALENDAR_HREF;
            let can_move_up = index > 0;
            let can_move_down = index < total - 1;

            let checkbox_elem = cb(is_enabled).label("").on_toggle({
                let h = cal_href.clone();
                move |v| Message::ToggleCalendarDisabled(h.clone(), !v)
            });

            let name_input = text_input(&rust_i18n::t!("name_label"), &cal.name)
                .on_input({
                    let h = cal_href.clone();
                    move |s| {
                        if is_local {
                            Message::LocalCalendarNameChanged(h.clone(), s)
                        } else {
                            Message::RemoteCalendarNameChanged(h.clone(), s)
                        }
                    }
                })
                .padding(5)
                .width(Length::Fill);

            let current_color = cal
                .color
                .as_ref()
                .and_then(|h| crate::color_utils::parse_hex_to_floats(h))
                .map(|(r, g, b)| Color::from_rgb(r, g, b))
                .unwrap_or(Color::from_rgb(0.2, 0.4, 0.8));

            let color_btn = button(
                text(icon::PALETTE_COLOR.to_string())
                    .font(icon::FONT)
                    .size(16)
                    .color(current_color),
            )
            .padding(5)
            .style(button::text)
            .on_press(Message::OpenColorPicker(cal_href.clone(), current_color));

            let color_widget: Element<_> =
                if app.color_picker_active_href.as_ref() == Some(&cal_href) {
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

            let up_btn: Element<_> = if app.sort_collections_by_size {
                Space::new().width(Length::Fixed(24.0)).into()
            } else if can_move_up {
                button(icon::icon(icon::ARROW_EXPAND_UP).size(14))
                    .style(button::text)
                    .padding(5)
                    .on_press(Message::MoveCalendar(cal_href.clone(), -1))
                    .into()
            } else {
                button(icon::icon(icon::ARROW_EXPAND_UP).size(14))
                    .style(button::text)
                    .padding(5)
                    .into()
            };
            let down_btn: Element<_> = if app.sort_collections_by_size {
                Space::new().width(Length::Fixed(24.0)).into()
            } else if can_move_down {
                button(icon::icon(icon::ARROW_EXPAND_DOWN).size(14))
                    .style(button::text)
                    .padding(5)
                    .on_press(Message::MoveCalendar(cal_href.clone(), 1))
                    .into()
            } else {
                button(icon::icon(icon::ARROW_EXPAND_DOWN).size(14))
                    .style(button::text)
                    .padding(5)
                    .into()
            };

            let (export_btn, import_btn): (Element<_>, Element<_>) = (
                button(
                    row![
                        icon::icon(icon::EXPORT).size(14),
                        text(rust_i18n::t!("export")).size(10)
                    ]
                    .spacing(3)
                    .align_y(iced::Alignment::Center),
                )
                .padding(5)
                .style(button::secondary)
                .on_press(Message::ExportLocalIcs(cal_href.clone()))
                .into(),
                button(
                    row![
                        icon::icon(icon::IMPORT).size(14),
                        text(rust_i18n::t!("import_action")).size(10)
                    ]
                    .spacing(3)
                    .align_y(iced::Alignment::Center),
                )
                .padding(5)
                .style(button::secondary)
                .on_press(Message::ImportLocalIcs(cal_href.clone()))
                .into(),
            );

            let delete_btn: Element<_> = if is_local && !is_default {
                button(icon::icon(icon::TRASH).size(14))
                    .style(button::danger)
                    .padding(5)
                    .on_press(Message::DeleteLocalCalendar(cal_href.clone()))
                    .into()
            } else {
                Space::new().width(Length::Fixed(22.0)).into()
            };

            let save_btn: Element<_> = if !is_local {
                let original_cal = app.calendars.iter().find(|c| c.href == cal_href);
                let has_changes = original_cal.is_none_or(|orig| orig.name != cal.name);
                if has_changes {
                    button(
                        icon::icon(icon::CHECK)
                            .size(16)
                            .color(app.theme().extended_palette().primary.base.color),
                    )
                    .style(button::text)
                    .padding(5)
                    .on_press(Message::SubmitRemoteCalendar(cal_href.clone()))
                    .into()
                } else {
                    Space::new().width(Length::Fixed(26.0)).into()
                }
            } else {
                Space::new().width(0).into()
            };

            let tag = if is_local {
                cal_tag(rust_i18n::t!("local_label"), Color::from_rgb(0.3, 0.3, 0.3))
            } else {
                cal_tag(
                    rust_i18n::t!("remote_label"),
                    Color::from_rgb(0.2, 0.4, 0.8),
                )
            };

            col = col.push(
                row![
                    up_btn,
                    down_btn,
                    checkbox_elem,
                    tag,
                    name_input,
                    export_btn,
                    import_btn,
                    color_widget,
                    save_btn,
                    delete_btn
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center),
            );
        }

        let add_buttons_row = row![
            button(
                row![
                    icon::icon(icon::PLUS).size(16),
                    text(rust_i18n::t!("create_new_local_calendar"))
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
            )
            .style(button::secondary)
            .width(Length::Fill)
            .on_press(Message::AddLocalCalendar),
            button(
                row![
                    icon::icon(icon::PLUS).size(16),
                    text(rust_i18n::t!("create_new_remote_calendar"))
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
            )
            .style(button::secondary)
            .width(Length::Fill)
            .on_press(Message::AddRemoteCalendar),
        ]
        .spacing(10);

        col = col.push(add_buttons_row);

        container(col)
            .padding(10)
            .style(|_| bordered(4.0, Color::from_rgba(0.5, 0.5, 0.5, 0.2)))
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

    let insecure_check = cb(app.ob_insecure)
        .label(rust_i18n::t!("allow_insecure_ssl"))
        .on_toggle(|v| Message::ToggleField(crate::gui::message::BoolField::ObInsecure, v))
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
                muted_note(rust_i18n::t!("caldav_url_hint")),
                text(rust_i18n::t!("username")),
                text_input(&rust_i18n::t!("username"), &app.ob_user)
                    .on_input(Message::ObUserChanged)
                    .padding(10),
                text(rust_i18n::t!("password")),
                row![
                    text_input(&rust_i18n::t!("password"), &app.ob_pass)
                        .on_input(Message::ObPassChanged)
                        .secure(!app.ob_password_visible)
                        .padding(10)
                        .width(Length::Fill),
                    button(
                        icon::icon(if app.ob_password_visible {
                            icon::EYE_CLOSED
                        } else {
                            icon::EYE
                        })
                        .size(20)
                    )
                    .style(button::text)
                    .padding(10)
                    .on_press(Message::ToggleObPasswordVisibility)
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                insecure_check,
                cb(app.sync_settings)
                    .label(rust_i18n::t!("sync_settings"))
                    .on_toggle(|v| Message::ToggleField(
                        crate::gui::message::BoolField::SyncSettings,
                        v
                    )),
                save_connect_btn
            ]
            .spacing(15)
        )
        .padding(10)
        .style(|_| bordered(6.0, Color::from_rgba(0.5, 0.5, 0.5, 0.2))),
        // 2. Preferences
        lang_picker,  // <-- language picker added to the form layout
        theme_picker, // <-- theme picker added to the form layout
        picker,
        collections_ui,
        notifications_ui,
        aliases_ui,
        goals_ui,
        advanced_ui,
        // 3. Bottom Actions
        offline_button_or_space,
    ]
    .spacing(20)
    .max_width(500);

    let scrollable_content = column![banners, form]
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
