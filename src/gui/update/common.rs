// Common utility functions for GUI updates.
use crate::config::Config;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::store::FilterOptions;
use crate::system::SystemEvent;
use chrono::{Duration, Utc};
use iced::Task;
use iced::widget::{operation, scrollable::RelativeOffset};

pub fn refresh_sidebar_cache(app: &mut GuiApp) {
    // Cache categories
    app.cached_categories = app.store.get_all_categories(
        app.hide_completed,
        app.hide_fully_completed_tags,
        &app.selected_categories,
        &app.hidden_calendars,
    );

    // Cache locations
    app.cached_locations = app
        .store
        .get_all_locations(app.hide_completed, &app.hidden_calendars);
}

pub fn refresh_filtered_tasks(app: &mut GuiApp) {
    let cutoff_date = if let Some(months) = app.sort_cutoff_months {
        let now = Utc::now();
        let days = months as i64 * 30;
        Some(now + Duration::days(days))
    } else {
        None
    };

    app.tasks = app.store.filter(FilterOptions {
        active_cal_href: None, // This is now handled by hidden_calendars
        hidden_calendars: &app.hidden_calendars,
        selected_categories: &app.selected_categories,
        selected_locations: &app.selected_locations, // NEW
        match_all_categories: app.match_all_categories,
        search_term: &app.search_value,
        hide_completed_global: app.hide_completed,
        cutoff_date,
        min_duration: app.filter_min_duration,
        max_duration: app.filter_max_duration,
        include_unset_duration: app.filter_include_unset_duration,
        urgent_days: app.urgent_days,
        urgent_prio: app.urgent_prio,
        default_priority: app.default_priority,
        start_grace_period_days: app.start_grace_period_days,
    });

    // Update sidebar cache after filtering
    refresh_sidebar_cache(app);

    if let Some(tx) = &app.alarm_tx {
        // We need to send the FULL list (store.calendars.values().flat_map), not just filtered view
        // But for simplicity, let's just send the filtered list if that's what we have handy,
        // OR better: construct full list. The actor needs ALL tasks to check alarms properly.
        let all_tasks: Vec<crate::model::Task> =
            app.store.calendars.values().flatten().cloned().collect();
        let _ = tx.try_send(SystemEvent::UpdateTasks(all_tasks));
    }
}

pub fn save_config(app: &GuiApp) {
    let _ = Config {
        url: app.ob_url.clone(),
        username: app.ob_user.clone(),
        password: app.ob_pass.clone(),
        default_calendar: app.ob_default_cal.clone(),
        hide_completed: app.hide_completed,
        hide_fully_completed_tags: app.hide_fully_completed_tags,
        allow_insecure_certs: app.ob_insecure,
        hidden_calendars: app.hidden_calendars.iter().cloned().collect(),
        disabled_calendars: app.disabled_calendars.iter().cloned().collect(),
        tag_aliases: app.tag_aliases.clone(),
        sort_cutoff_months: app.sort_cutoff_months,
        theme: app.current_theme,
        urgent_days_horizon: app.urgent_days,
        urgent_priority_threshold: app.urgent_prio,
        default_priority: app.default_priority,
        start_grace_period_days: app.start_grace_period_days,

        // NEW FIELDS
        auto_reminders: app.auto_reminders,
        default_reminder_time: app.default_reminder_time.clone(),
        snooze_short_mins: app.snooze_short_mins,
        snooze_long_mins: app.snooze_long_mins,
        create_events_for_tasks: app.create_events_for_tasks,
        delete_events_on_completion: app.delete_events_on_completion,
    }
    .save();
}

pub fn apply_alias_retroactively(
    app: &mut GuiApp,
    alias_key: &str,
    target_tags: &[String],
) -> Option<Task<Message>> {
    let modified_tasks = app.store.apply_alias_retroactively(alias_key, target_tags);

    if modified_tasks.is_empty() {
        return None;
    }

    refresh_filtered_tasks(app);

    if let Some(client) = &app.client {
        let mut commands = Vec::new();
        for t in modified_tasks {
            commands.push(Task::perform(
                async_update_wrapper(client.clone(), t),
                Message::SyncSaved,
            ));
        }
        return Some(Task::batch(commands));
    }

    None
}

/// Helper: Generates a command to scroll the main list to the currently selected task.
pub fn scroll_to_selected(app: &GuiApp) -> Task<Message> {
    if let Some(uid) = &app.selected_uid
        && let Some(idx) = app.tasks.iter().position(|t| t.uid == *uid)
    {
        let len = app.tasks.len().max(1) as f32;
        return operation::snap_to(
            app.scrollable_id.clone(),
            RelativeOffset {
                x: 0.0,
                y: idx as f32 / len,
            },
        );
    }

    Task::none()
}
