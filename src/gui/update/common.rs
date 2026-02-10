// File: ./src/gui/update/common.rs
// Common utility functions for GUI updates.

use crate::config::Config;
use crate::gui::async_ops::*;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::focusable::{clear_focus_bounds, get_all_focus_bounds, get_focus_bounds};
use crate::store::FilterOptions;
use crate::system::SystemEvent;

use iced::Task;
use iced::widget::operation;
use iced::widget::scrollable::RelativeOffset;
use std::time::Duration as StdDuration;

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
    // --- FIX: Clear the focus bounds cache before filtering ---
    // This prevents stale layout data from breaking scroll calculations.
    clear_focus_bounds();

    let cutoff_date = app
        .sort_cutoff_months
        .map(|m| chrono::Utc::now() + chrono::Duration::days(m as i64 * 30));

    // Load config so we can respect the global limits for showing completed groups/subtasks.
    let config = Config::load(app.ctx.as_ref()).unwrap_or_default();

    app.tasks = app.store.filter(FilterOptions {
        // FIX: Pass None instead of active_cal_href.
        // Passing active_cal_href forces the store to filter EXCLUSIVELY to that calendar.
        // We want to show all calendars that aren't hidden (unified view).
        active_cal_href: None,
        hidden_calendars: &app.hidden_calendars,
        selected_categories: &app.selected_categories,
        selected_locations: &app.selected_locations,
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
        expanded_done_groups: &app.expanded_done_groups,
        max_done_roots: config.max_done_roots,
        max_done_subtasks: config.max_done_subtasks,
    });

    // 2. Build Parent Attribute Cache (O(N))
    // We do this here once per update so the view loop is O(1)
    app.parent_attributes_cache.clear();

    // Create a temporary lookup for all tasks in the store
    // This allows us to resolve parents even if they aren't in the filtered view
    let mut quick_lookup: std::collections::HashMap<String, &crate::model::Task> =
        std::collections::HashMap::new();
    for map in app.store.calendars.values() {
        for t in map.values() {
            quick_lookup.insert(t.uid.clone(), t);
        }
    }

    // --- UPDATE ID CACHE ---
    for task in &app.tasks {
        app.task_ids
            .entry(task.uid.clone())
            .or_insert_with(iced::widget::Id::unique);

        if let Some(p_uid) = &task.parent_uid
            && let Some(parent) = quick_lookup.get(p_uid)
        {
            let p_tags: std::collections::HashSet<String> =
                parent.categories.iter().cloned().collect();
            let p_loc = parent.location.clone();
            app.parent_attributes_cache
                .insert(p_uid.clone(), (p_tags, p_loc));
        }
    }

    // Update sidebar cache after filtering
    refresh_sidebar_cache(app);

    if let Some(tx) = &app.alarm_tx {
        // We need to send the FULL list (store.calendars.values().flat_map), not just filtered view
        // But for simplicity, let's just send the filtered list if that's what we have handy,
        // OR better: construct full list. The actor needs ALL tasks to check alarms properly.
        let all_tasks: Vec<crate::model::Task> = app
            .store
            .calendars
            .values()
            .flat_map(|m| m.values())
            .cloned()
            .collect();
        let _ = tx.try_send(SystemEvent::UpdateTasks(all_tasks));
    }
}

pub fn save_config(app: &mut GuiApp) -> Config {
    let mut cfg = Config::load(app.ctx.as_ref()).unwrap_or_default();

    cfg.url = app.ob_url.clone();
    cfg.username = app.ob_user.clone();
    cfg.password = app.ob_pass.clone();
    cfg.default_calendar = app.ob_default_cal.clone();
    cfg.allow_insecure_certs = app.ob_insecure;
    cfg.hidden_calendars = app.hidden_calendars.iter().cloned().collect();
    cfg.disabled_calendars = app.disabled_calendars.iter().cloned().collect();
    cfg.hide_completed = app.hide_completed;
    cfg.hide_fully_completed_tags = app.hide_fully_completed_tags;
    cfg.tag_aliases = app.tag_aliases.clone();
    cfg.sort_cutoff_months = app.sort_cutoff_months;
    cfg.theme = app.current_theme;
    cfg.urgent_days_horizon = app.urgent_days;
    cfg.urgent_priority_threshold = app.urgent_prio;
    cfg.default_priority = app.default_priority;
    cfg.start_grace_period_days = app.start_grace_period_days;
    cfg.auto_reminders = app.auto_reminders;
    cfg.default_reminder_time = app.default_reminder_time.clone();
    cfg.snooze_short_mins = app.snooze_short_mins;
    cfg.snooze_long_mins = app.snooze_long_mins;
    cfg.create_events_for_tasks = app.create_events_for_tasks;
    cfg.delete_events_on_completion = app.delete_events_on_completion;
    cfg.auto_refresh_interval_mins = app.auto_refresh_interval_mins;

    // Save new values from Advanced Settings inputs
    cfg.max_done_roots = app.ob_max_done_roots_input.parse().unwrap_or(20);
    cfg.max_done_subtasks = app.ob_max_done_subtasks_input.parse().unwrap_or(5);

    let _ = cfg.save(app.ctx.as_ref());
    cfg
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
///
/// Strategy:
/// 1) If both a stable widget Id is registered and the task index is known, perform both:
///    focus the widget id and snap the scrollable to the relative offset (batched).
/// 2) If only Id is present, focus it (ensures keyboard focus).
/// 3) If only index known, snap_to relative offset as a fallback.
pub fn scroll_to_selected(app: &GuiApp, focus: bool) -> Task<Message> {
    if let Some(uid) = &app.selected_uid {
        let id_opt = app.task_ids.get(uid).cloned();
        let idx_opt = app.tasks.iter().position(|t| t.uid == *uid);

        // If we have both an Id and an index, prefer to compute a pixel-accurate scroll offset
        // and then focus. We try bounds-based centering first (most accurate), and fall back
        // to an index-based center estimate if bounds are not available.
        if let (Some(id), Some(idx)) = (id_opt.clone(), idx_opt) {
            // Try bounds-based centering when available.
            if let Some(rect) = get_focus_bounds(&id) {
                // Compute content top/min and content bottom/max from registered bounds.
                // This allows computing content height as max_y - min_y, and deriving the
                // target item's center relative to the content origin (min_y).
                let all = get_all_focus_bounds();
                let mut min_y: f32 = f32::INFINITY;
                let mut max_y: f32 = 0.0;
                for (_k, r) in all.iter() {
                    min_y = min_y.min(r.y);
                    max_y = max_y.max(r.y + r.height);
                }

                if min_y.is_finite() && max_y > min_y {
                    let content_h = max_y - min_y;
                    let viewport_h = (app.current_window_size.height - 180.0).max(100.0);

                    // Item center relative to content top (min_y)
                    let item_center_rel = (rect.y - min_y) + rect.height / 2.0;

                    // Maximum scroll offset (px) is content_h - viewport_h.
                    let max_scroll = (content_h - viewport_h).max(0.0);
                    // Desired offset so the item center is positioned in the middle of the viewport.
                    let desired_offset_px =
                        (item_center_rel - viewport_h / 2.0).clamp(0.0, max_scroll);

                    // Use an absolute pixel scroll to position precisely, then focus the id.
                    // Convert desired pixel offset into a relative fraction for snap_to.
                    // max_scroll = content_height - viewport_height
                    let max_scroll_px = (content_h - viewport_h).max(0.0);
                    let y = if max_scroll_px > 0.0 {
                        (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let snap =
                        operation::snap_to(app.scrollable_id.clone(), RelativeOffset { x: 0.0, y });

                    if focus {
                        return Task::batch(vec![snap, operation::focus(id)]);
                    } else {
                        return snap;
                    }
                }
            }

            // Fallback: index-centered heuristic (estimate pixels using avg row height).
            let avg_item_h: f32 = 34.0; // tuned default row height
            let total_items = app.tasks.len() as f32;
            let content_h = (avg_item_h * total_items).max(1.0);
            let viewport_h = (app.current_window_size.height - 180.0).max(100.0);
            let item_center = (idx as f32 + 0.5) * avg_item_h;
            let max_scroll = (content_h - viewport_h).max(0.0);
            let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll);

            // Convert desired pixel offset into a relative fraction for snap_to.
            let max_scroll_px = (content_h - viewport_h).max(0.0);
            let y = if max_scroll_px > 0.0 {
                (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let snap = operation::snap_to(app.scrollable_id.clone(), RelativeOffset { x: 0.0, y });

            if focus {
                return Task::batch(vec![snap, operation::focus(id)]);
            } else {
                return snap;
            }
        }

        // If we only have the index, center using the same index-based pixel heuristic.
        if let Some(idx) = idx_opt {
            let avg_item_h: f32 = 34.0;
            let total_items = app.tasks.len() as f32;
            let content_h = (avg_item_h * total_items).max(1.0);
            let viewport_h = (app.current_window_size.height - 180.0).max(100.0);
            let item_center = (idx as f32 + 0.5) * avg_item_h;
            let max_scroll = (content_h - viewport_h).max(0.0);
            let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll);

            // Convert desired pixel offset into a relative fraction for snap_to.
            let max_scroll_px = (content_h - viewport_h).max(0.0);
            let y = if max_scroll_px > 0.0 {
                (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
            } else {
                0.0
            };

            return operation::snap_to(app.scrollable_id.clone(), RelativeOffset { x: 0.0, y });
        }

        // If we only have the widget Id, focus it (last resort).
        if let Some(id) = id_opt
            && focus
        {
            return operation::focus(id);
        }
    }

    //eprintln!("scroll_to_selected: no selected uid");
    Task::none()
}

/// Helper: Waits a small amount of time (allowing the View to rebuild)
/// and then triggers one or more `SnapToSelected` messages (retries).
///
/// Rationale:
/// A single delayed attempt may still miss on some platforms/conditions. Emit a small batch
/// of delayed triggers (at increasing delays) so the focus attempt has multiple chances across
/// subsequent frames to succeed.
pub fn scroll_to_selected_delayed(_app: &GuiApp, focus: bool) -> Task<Message> {
    // Try to emit SnapToSelected as soon as the focusable row reports its layout bounds.
    // Strategy:
    // 1. If we know which UID is selected and we have a cached widget Id for it, poll the
    //    focus bounds registry for that Id and emit SnapToSelected as soon as a bounds entry
    //    is seen (up to a timeout). This yields the most precise timing to focus the widget
    //    right after it has been registered by the View's operate traversal.
    // 2. Fallback: if no selected UID / id is available, or the poll times out, emit a
    //    series of delayed SnapToSelected messages as before.

    // If we have an explicitly-selected task and a cached widget Id, try the bounds-aware path.
    if let Some(uid) = &_app.selected_uid
        && let Some(id) = _app.task_ids.get(uid).cloned()
    {
        // Spawn a single perform task that polls for the bounds to exist for this Id.
        // We will wait up to ~1s (20 * 50ms) checking every 50ms; as soon as bounds exist
        // we emit SnapToSelected. If polling times out, emit SnapToSelected once anyway.
        return Task::perform(
            async move {
                let mut attempts = 0u8;
                // Poll loop: check for bounds up to N attempts.
                loop {
                    // If bounds exist, we can stop waiting.
                    if crate::gui::view::focusable::get_focus_bounds(&id).is_some() {
                        break;
                    }
                    attempts = attempts.saturating_add(1);
                    if attempts >= 20 {
                        // timed out
                        break;
                    }
                    std::thread::sleep(StdDuration::from_millis(50));
                }
            },
            move |_| Message::SnapToSelected { focus },
        );
    }

    // Fallback: emit a small batch of delayed attempts (in case no id was cached yet).
    Task::batch(vec![
        Task::perform(
            async {
                std::thread::sleep(StdDuration::from_millis(120));
            },
            move |_| Message::SnapToSelected { focus },
        ),
        Task::perform(
            async {
                std::thread::sleep(StdDuration::from_millis(360));
            },
            move |_| Message::SnapToSelected { focus },
        ),
        Task::perform(
            async {
                std::thread::sleep(StdDuration::from_millis(720));
            },
            move |_| Message::SnapToSelected { focus },
        ),
    ])
}
