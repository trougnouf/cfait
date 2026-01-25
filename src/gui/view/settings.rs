// File: ./src/gui/view/settings.rs
// Renders the settings and onboarding screens.
use crate::gui::icon;
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use iced::widget::{
    MouseArea, Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{Color, Element, Length};
#[cfg(feature = "gui")]
use iced_aw::color_picker;
use strum::IntoEnumIterator;
use crate::config::AppTheme;
use crate::storage::LOCAL_CALENDAR_HREF;

pub fn view_settings(app: &GuiApp) -> Element<'_, Message> {
    let is_settings = matches!(app.state, AppState::Settings);

    // --- Header ---
    let title_text = text(if is_settings { "Settings" } else { "Welcome" }).size(40);
    let title_row = if is_settings {
        row![
            button(icon::icon(icon::ARROW_LEFT).size(24)).style(button::text).on_press(Message::CancelSettings),
            title_text,
            Space::new().width(Length::Fill)
        ].spacing(20).align_y(iced::Alignment::Center)
    } else {
        row![title_text, Space::new().width(Length::Fill)]
    };

    let title_drag_area: Element<_> = MouseArea::new(
        container(title_row).width(Length::Fill).padding(20)
    ).on_press(Message::WindowDragged).into();

    let error = if let Some(e) = &app.error_msg {
        text(e).color(Color::from_rgb(1.0, 0.0, 0.0))
    } else { text("") };

    // --- ACCOUNTS UI ---
    let accounts_section: Element<_> = if app.editing_account_id.is_some() {
        // --- EDIT MODE ---
        let is_new = app.editing_account_id.as_deref() == Some("new");
        let header_txt = if is_new { "Add Account" } else { "Edit Account" };

        container(column![
            text(header_txt).size(20),
            text("Account Name:"),
            text_input("Work, Personal...", &app.ob_name).on_input(Message::ObNameChanged).padding(10),
            text("Server URL:"),
            text_input("https://...", &app.ob_url).on_input(Message::ObUrlChanged).padding(10),
            text("Username:"),
            text_input("User", &app.ob_user).on_input(Message::ObUserChanged).padding(10),
            text("Password:"),
            text_input("Password", &app.ob_pass).on_input(Message::ObPassChanged).secure(true).padding(10),
            checkbox(app.ob_insecure).label("Allow insecure SSL").on_toggle(Message::ObInsecureToggled),

            row![
                button("Cancel").style(button::secondary).on_press(Message::CancelEditAccount),
                button("Save Account").on_press(Message::SaveAccount)
            ].spacing(10)
        ].spacing(10))
        .padding(10)
        .style(|_| container::Style {
            border: iced::Border { radius: 6.0.into(), width: 1.0, color: Color::from_rgb(0.5,0.5,0.5) },
            ..Default::default()
        })
        .into()
    } else {
        // --- LIST MODE ---
        let mut col = column![
            row![
                text("Accounts").size(20),
                Space::new().width(Length::Fill),
                button("Add Account").style(button::secondary).on_press(Message::AddNewAccount)
            ].align_y(iced::Alignment::Center)
        ].spacing(10);

        for acc in &app.accounts {
            let row_item = row![
                column![
                    text(&acc.name).size(16),
                    text(&acc.username).size(12).color(Color::from_rgb(0.5, 0.5, 0.5))
                ].width(Length::Fill),
                button(icon::icon(icon::EDIT).size(16)).style(button::text).on_press(Message::EditAccount(acc.id.clone())),
                button(icon::icon(icon::TRASH).size(16)).style(button::danger).on_press(Message::DeleteAccount(acc.id.clone()))
            ].spacing(10).align_y(iced::Alignment::Center);

            col = col.push(container(row_item).padding(5).style(|_| container::Style {
                background: Some(Color::from_rgba(0.5, 0.5, 0.5, 0.1).into()),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            }));
        }

        if app.accounts.is_empty() {
            col = col.push(text("No accounts configured."));
        }

        col.into()
    };

    // --- Main Connect Button ---
    let connect_btn: Element<_> = if app.editing_account_id.is_none() {
        button(if is_settings { "Save All & Reconnect" } else { "Connect" })
            .padding(10).width(Length::Fill).on_press(Message::ObSubmit)
            .into()
    } else {
        Space::new().height(0).into()
    };

    // --- Other UI sections that were missing ---
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

    // FIX: Re-structured this block to resolve the type inference error.
    let prefs: Element<'_, Message> = if is_settings {
        let mut col = column![
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
            checkbox(app.hide_completed)
                .label("Hide completed and canceled tasks")
                .on_toggle(Message::ToggleHideCompleted),
        ]
        .spacing(10);

        if !app.hide_completed {
            col = col.push(
                checkbox(app.hide_fully_completed_tags)
                    .label("Hide tags containing only completed tasks")
                    .on_toggle(Message::ToggleHideFullyCompletedTags),
            );
        }

        container(col).into()
    } else {
        Space::new().width(0).into()
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
                ].spacing(10).align_y(iced::Alignment::Center),
                text("Tasks starting within this period won't be pushed to 'Future'")
                    .size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(15),
                text("Urgency Rules (Shown at top):").size(16),
                row![
                    text("Due within (days):").width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_urgent_days_input).on_input(Message::ObUrgentDaysChanged).width(Length::Fixed(60.0)).padding(5)
                ].spacing(10).align_y(iced::Alignment::Center),
                row![
                    text("Priority <= (!):").width(Length::Fixed(150.0)),
                    text_input("1", &app.ob_urgent_prio_input).on_input(Message::ObUrgentPrioChanged).width(Length::Fixed(60.0)).padding(5)
                ].spacing(10).align_y(iced::Alignment::Center),
                Space::new().height(15),
                text("Priority Settings:").size(16),
                row![
                    text("Default Priority (!):").width(Length::Fixed(150.0)),
                    text_input("5", &app.ob_default_priority_input).on_input(Message::ObDefaultPriorityChanged).width(Length::Fixed(60.0)).padding(5)
                ].spacing(10).align_y(iced::Alignment::Center),
                text("(Tasks without priority (0) sort as this value)").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
                Space::new().height(10),
                row![
                    text("Priority cutoff (months):").width(Length::Fixed(150.0)),
                    text_input("6", &app.ob_sort_months_input).on_input(Message::ObSortMonthsChanged).width(Length::Fixed(100.0)).padding(5)
                ].spacing(10).align_y(iced::Alignment::Center),
                text("(Tasks due within this range are shown first. Blank = all timed first)").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
            ].spacing(5),
        ).padding(15).style(container::rounded_box).into()
    } else {
        Space::new().width(0).into()
    };

    let notifications_ui: Element<_> = if is_settings {
        column![
            text("Notifications & Reminders").size(20),
            checkbox(app.auto_reminders).label("Auto-remind on Start/Due dates (if no alarms set)").on_toggle(Message::SetAutoReminders),
            row![
                text("Default reminder time (HH:MM):").width(Length::Fixed(200.0)),
                text_input("09:00", &app.default_reminder_time).on_input(Message::SetDefaultReminderTime).width(Length::Fixed(80.0)).padding(5)
            ].spacing(10).align_y(iced::Alignment::Center),
            text("Snooze Presets:").size(14),
            row![
                text("Short:"),
                text_input("1h", &app.ob_snooze_short_input).on_input(Message::SetSnoozeShort).width(Length::Fixed(60.0)).padding(5),
                text("Long:"),
                text_input("1d", &app.ob_snooze_long_input).on_input(Message::SetSnoozeLong).width(Length::Fixed(60.0)).padding(5)
            ].spacing(10).align_y(iced::Alignment::Center),
            text("").size(5),
            text("Calendar Integration").size(20),
            {
                let cb = checkbox(app.create_events_for_tasks).label("Create calendar events (VEVENT) for tasks with dates");
                if !app.deleting_events { cb.on_toggle(Message::SetCreateEventsForTasks) } else { cb }
            },
            text("Events will be retroactively created. Use +cal or -cal in task input to override per-task").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
            text("").size(5),
            {
                let cb = checkbox(app.delete_events_on_completion).label("Delete events when tasks are completed");
                if !app.deleting_events { cb.on_toggle(Message::SetDeleteEventsOnCompletion) } else { cb }
            },
            text("Regardless, events are always deleted when tasks are deleted.").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
            {
                let btn = button("Delete all calendar events");
                if !app.deleting_events { btn.on_press(Message::DeleteAllCalendarEvents) } else { btn }
            },
            if app.deleting_events {
                text("Deleting events...").size(12).color(Color::from_rgb(0.6, 0.6, 0.6))
            } else { text("") },
        ].spacing(10).into()
    } else {
        Space::new().width(0).into()
    };

    let aliases_ui: Element<_> = if is_settings {
        let mut list_col = column![text("Tag aliases").size(20)].spacing(10);
        for (key, vals) in &app.tag_aliases {
            let val_str = vals.join(", ");
            let row_item = row![
                text(if key.starts_with("@@") { key.to_string() } else { format!("#{}", key) }).width(Length::FillPortion(1)),
                text("->").width(Length::Fixed(20.0)),
                text(val_str).width(Length::FillPortion(2)),
                button(icon::icon(icon::CROSS).size(12)).style(button::danger).padding(5).on_press(Message::RemoveAlias(key.clone()))
            ].spacing(10).align_y(iced::Alignment::Center);
            list_col = list_col.push(row_item);
        }
        let input_row = row![
            text_input("Alias (#tag or @@loc)", &app.alias_input_key).on_input(Message::AliasKeyInput).padding(5).width(Length::FillPortion(1)),
            text_input("#tag,@@loc,!3", &app.alias_input_values).on_input(Message::AliasValueInput).padding(5).width(Length::FillPortion(2)),
            button("Add").padding(5).on_press(Message::AddAlias)
        ].spacing(10);
        container(column![list_col, iced::widget::rule::horizontal(1), input_row].spacing(15)).padding(10).style(|_| container::Style {
            border: iced::Border { radius: 4.0.into(), width: 1.0, color: Color::from_rgb(0.3, 0.3, 0.3), },
            ..Default::default()
        }).into()
    } else {
        Space::new().width(0).into()
    };

    let cal_mgmt_ui: Element<_> = if is_settings && !app.calendars.is_empty() {
        let mut col = column![text("Manage calendars").size(20)].spacing(10);
        for cal in &app.calendars {
            let is_enabled = !app.disabled_calendars.contains(&cal.href);
            col = col.push(
                row![checkbox(is_enabled).label(&cal.name).on_toggle(move |v| Message::ToggleCalendarDisabled(cal.href.clone(), !v)).width(Length::Fill)]
                .spacing(10).align_y(iced::Alignment::Center)
            );
        }
        container(col).padding(10).style(|_| container::Style {
            border: iced::Border { radius: 4.0.into(), width: 1.0, color: Color::from_rgb(0.3, 0.3, 0.3), },
            ..Default::default()
        }).into()
    } else {
        Space::new().width(0).into()
    };

    let local_cal_ui: Element<_> = if is_settings {
        let mut local_cal_col = column![
            text("Local Calendars").size(20),
            text("Manage your offline calendars here.").size(12).color(Color::from_rgb(0.6, 0.6, 0.6)),
        ].spacing(10);

        for cal in &app.local_cals_editing {
            let href = cal.href.clone();
            let is_default = href == LOCAL_CALENDAR_HREF;
            let name_input = text_input("Name", &cal.name).on_input(move |s| Message::LocalCalendarNameChanged(href.clone(), s)).padding(5).width(Length::FillPortion(3));
            let export_href = cal.href.clone();
            let export_btn = button(row![icon::icon(icon::EXPORT).size(14), text("Export").size(10)].spacing(3).align_y(iced::Alignment::Center)).padding(5).style(button::secondary).on_press(Message::ExportLocalIcs(export_href));
            let import_href = cal.href.clone();
            let import_btn = button(row![icon::icon(icon::IMPORT).size(14), text("Import").size(10)].spacing(3).align_y(iced::Alignment::Center)).padding(5).style(button::secondary).on_press(Message::ImportLocalIcs(import_href));
            let current_color = cal.color.as_ref().and_then(|h| crate::color_utils::parse_hex_to_floats(h)).map(|(r, g, b)| Color::from_rgb(r, g, b)).unwrap_or(Color::from_rgb(0.5, 0.5, 0.5));
            let color_btn = button(text(icon::PALETTE_COLOR.to_string()).font(icon::FONT).size(16).color(current_color)).padding(5).style(button::text).on_press(Message::OpenColorPicker(cal.href.clone(), current_color));

            let color_widget: Element<_> = if app.color_picker_active_href.as_ref() == Some(&cal.href) {
                color_picker::ColorPicker::new(true, current_color, color_btn, Message::CancelColorPicker, Message::SubmitColorPicker).into()
            } else { color_btn.into() };

            let delete_btn: Element<_> = if !is_default {
                let h = cal.href.clone();
                button(icon::icon(icon::TRASH).size(14)).style(button::danger).padding(5).on_press(Message::DeleteLocalCalendar(h)).into()
            } else { Space::new().width(Length::Fixed(22.0)).into() };
            local_cal_col = local_cal_col.push(row![name_input, export_btn, import_btn, color_widget, delete_btn].spacing(5).align_y(iced::Alignment::Center));
        }
        local_cal_col = local_cal_col.push(button("Add Local Calendar").style(button::secondary).on_press(Message::AddLocalCalendar));
        container(local_cal_col).padding(10).style(|_| container::Style {
            border: iced::Border { radius: 6.0.into(), width: 1.0, color: Color::from_rgba(0.5, 0.5, 0.5, 0.2), },
            ..Default::default()
        }).into()
    } else { Space::new().width(0).into() };

    let form_col = column![
        accounts_section,
        connect_btn,
        if is_settings { Element::from(picker) } else { Space::new().width(0).into() },
        if is_settings { Element::from(cal_mgmt_ui) } else { Space::new().width(0).into() },
        if is_settings { Element::from(local_cal_ui) } else { Space::new().width(0).into() },
        if is_settings { Element::from(prefs) } else { Space::new().width(0).into() },
        if is_settings { Element::from(notifications_ui) } else { Space::new().width(0).into() },
        if is_settings { Element::from(sorting_ui) } else { Space::new().width(0).into() },
        if is_settings { Element::from(aliases_ui) } else { Space::new().width(0).into() },
    ].spacing(20);

    let scrollable_content = column![error, form_col].spacing(20).align_x(iced::Alignment::Center);
    let main_col = column![title_drag_area, scrollable(container(scrollable_content).width(Length::Fill).padding(20).center_x(Length::Fill))];

    container(main_col).width(Length::Fill).height(Length::Fill).into()
}
