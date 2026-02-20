// File: src/gui/update/common.rs
/*
 Common utilities for GUI update handlers.

 This module implements shared helper routines used by the GUI update flow:
  - building the filtered task list for the UI,
  - saving configuration back to disk,
  - applying alias changes retroactively,
  - and helper scrolling/focus utilities used by the view layer.

 Key design notes:
  - The filter pipeline is intentionally split so that most predicate checks
    operate on &Task references (no clones). Only the final, visible tasks are
    cloned for UI rendering. This keeps memory churn low on frequent refreshes.
  - We maintain a small parent-attribute cache here (tags/location) to avoid
    repeated lookups while rendering task rows.
  - The scrolling helpers attempt a bounds-aware snap (pixel-accurate) when a
    widget Id and its layout bounds are available; they fall back to index-based
    heuristics when bounds are not yet registered.
*/

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

/// Build the visible task list and update UI caches.
///
/// Strategy:
/// 1) Clear any previously stored focus bounds (these can be stale after major updates).
/// 2) Build an effective set of hidden calendars (user-hidden + disabled).
/// 3) Run `store.filter(...)` which operates on references and returns the final cloned list.
/// 4) Build a small parent-attribute cache used by task row rendering (inheritance of tags/location).
/// 5) Notify the alarm actor with the full task set (for alarm scheduling).
pub fn refresh_filtered_tasks(app: &mut GuiApp) {
    // Clear focus bounds before rebuilding the list. This prevents stale layout
    // information from interfering with subsequent focus/scroll computations.
    clear_focus_bounds();

    let cutoff_date = app
        .sort_cutoff_months
        .map(|m| chrono::Utc::now() + chrono::Duration::days(m as i64 * 30));

    // Load configuration so the filter honors the current advanced limits (max done roots/subtasks).
    let config = Config::load(app.ctx.as_ref()).unwrap_or_default();

    // Effective hidden calendars: union of explicitly hidden and disabled calendars.
    // This set is passed to the filter to exclude those calendars efficiently.
    let mut effective_hidden = app.hidden_calendars.clone();
    effective_hidden.extend(app.disabled_calendars.clone());

    let search_text = app.search_value.text();

    let filter_res = app.store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &effective_hidden,
        selected_categories: &app.selected_categories,
        selected_locations: &app.selected_locations,
        match_all_categories: app.match_all_categories,
        search_term: &search_text,
        hide_completed_global: app.hide_completed,
        hide_fully_completed_tags: app.hide_fully_completed_tags,
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

    app.tasks = filter_res.tasks;
    app.cached_categories = filter_res.categories;
    app.cached_locations = filter_res.locations;

    // Rebuild a tiny parent attribute cache (tags + location). This allows task rows
    // to inherit visual attributes without performing repeated map lookups.
    app.parent_attributes_cache.clear();

    let mut quick_lookup: std::collections::HashMap<String, &crate::model::Task> =
        std::collections::HashMap::new();
    // Create an O(1) lookup table for tasks across all calendars so that parent attribute
    // resolution can work even if the parent is not in the current filtered view.
    for map in app.store.calendars.values() {
        for t in map.values() {
            quick_lookup.insert(t.uid.clone(), t);
        }
    }

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

    // Notify the alarm actor with the complete task set so scheduling/enablement can run off the latest data.
    if let Some(tx) = &app.alarm_tx {
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

/// Persist GUI-level config values back to the central Config object and save to disk.
///
/// This function collects the various UI-bound fields, converts them into the
/// Config structure and saves. It's intentionally permissive because the GUI may
/// leave some fields empty while the user types; callers should ensure validation
/// where necessary.
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
    cfg.strikethrough_completed = app.strikethrough_completed;
    cfg.auto_refresh_interval_mins = app.auto_refresh_interval_mins;
    cfg.trash_retention_days = app.trash_retention_days;

    cfg.max_done_roots = app.ob_max_done_roots_input.parse().unwrap_or(20);
    cfg.max_done_subtasks = app.ob_max_done_subtasks_input.parse().unwrap_or(5);

    let _ = cfg.save(app.ctx.as_ref());
    cfg
}

/// Apply a new alias mapping retroactively across tasks.
///
/// If there are modified tasks, refresh the filtered view and optionally issue
/// network update commands to persist changes to a remote server.
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

/// Scroll the main list to the selected task.
///
/// This helper prefers a bounds-aware pixel-accurate scroll when we have a widget Id
/// and the view has registered layout bounds for it. When bounds are not available
/// (e.g. the widget hasn't rendered yet) it falls back to an index-based heuristic
/// using an average row height.
pub fn scroll_to_selected(app: &GuiApp, focus: bool) -> Task<Message> {
    if let Some(uid) = &app.selected_uid {
        let id_opt = app.task_ids.get(uid).cloned();
        let idx_opt = app.tasks.iter().position(|t| t.uid == *uid);

        if let (Some(id), Some(idx)) = (id_opt.clone(), idx_opt) {
            if let Some(rect) = get_focus_bounds(&id) {
                // When available, use the union of all registered bounds to compute
                // an absolute content height and derive a fractional scroll offset.
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

                    // Maximum scroll = content_h - viewport_h
                    let max_scroll = (content_h - viewport_h).max(0.0);
                    // Desired offset so the item center is positioned in the middle of the viewport.
                    let desired_offset_px =
                        (item_center_rel - viewport_h / 2.0).clamp(0.0, max_scroll);

                    // Convert desired pixel offset into a relative fraction for snap_to.
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
            // Tuned default row height is used when bounds are not available.
            let avg_item_h: f32 = 34.0;
            let total_items = app.tasks.len() as f32;
            let content_h = (avg_item_h * total_items).max(1.0);
            let viewport_h = (app.current_window_size.height - 180.0).max(100.0);
            let item_center = (idx as f32 + 0.5) * avg_item_h;
            let max_scroll = (content_h - viewport_h).max(0.0);
            let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll);

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

        if let Some(idx) = idx_opt {
            // Index-only centering as a last resort.
            let avg_item_h: f32 = 34.0;
            let total_items = app.tasks.len() as f32;
            let content_h = (avg_item_h * total_items).max(1.0);
            let viewport_h = (app.current_window_size.height - 180.0).max(100.0);
            let item_center = (idx as f32 + 0.5) * avg_item_h;
            let max_scroll = (content_h - viewport_h).max(0.0);
            let desired_offset_px = (item_center - viewport_h / 2.0).clamp(0.0, max_scroll);

            let max_scroll_px = (content_h - viewport_h).max(0.0);
            let y = if max_scroll_px > 0.0 {
                (desired_offset_px / max_scroll_px).clamp(0.0, 1.0)
            } else {
                0.0
            };

            return operation::snap_to(app.scrollable_id.clone(), RelativeOffset { x: 0.0, y });
        }

        if let Some(id) = id_opt
            && focus
        {
            // If we only have a widget Id, focus it as a last step.
            return operation::focus(id);
        }
    }

    Task::none()
}

/// Try to focus the selected row after a short delay. This function emits a small
/// batch of delayed attempts to increase the chance the focusable row has been
/// registered by the view traversal on slower platforms or complex layouts.
pub fn scroll_to_selected_delayed(_app: &GuiApp, focus: bool) -> Task<Message> {
    if let Some(uid) = &_app.selected_uid
        && let Some(id) = _app.task_ids.get(uid).cloned()
    {
        return Task::perform(
            async move {
                let mut attempts = 0u8;
                loop {
                    if crate::gui::view::focusable::get_focus_bounds(&id).is_some() {
                        break;
                    }
                    attempts = attempts.saturating_add(1);
                    if attempts >= 20 {
                        break;
                    }
                    std::thread::sleep(StdDuration::from_millis(50));
                }
            },
            move |_| Message::SnapToSelected { focus },
        );
    }

    // Batch a few increasing delays to give the UI multiple chances to register the row.
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
