// File: ./src/gui/update/common.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Common utilities for GUI update handlers.
//!
//! This module implements shared helper routines used by the GUI update flow:
//!   - building the filtered task list for the UI,
//!   - saving configuration back to disk,
//!   - applying alias changes retroactively,
//!   - and helper scrolling/focus utilities used by the view layer.
//!
//! Key design notes:
//!   - The filter pipeline is intentionally split so that most predicate checks
//!     operate on &Task references (no clones). Only the final, visible tasks are
//!     cloned for UI rendering. This keeps memory churn low on frequent refreshes.
//!   - We maintain a small parent-attribute cache here (tags/location) to avoid
//!     repeated lookups while rendering task rows.
//!   - The scrolling helpers attempt a bounds-aware snap (pixel-accurate) when a
//!     widget Id and its layout bounds are available; they fall back to index-based
//!     heuristics when bounds are not yet registered.

use crate::config::Config;
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::gui::view::focusable::{clear_focus_bounds, get_all_focus_bounds, get_focus_bounds};

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
/// 4) Notify the alarm actor with the full task set (for alarm scheduling).
pub fn refresh_filtered_tasks(app: &mut GuiApp) {
    // Clear focus bounds before rebuilding the list.
    clear_focus_bounds();

    let config = &app.core_config;

    // Sync specific Iced state to SessionState
    app.session.active_calendar_href = app.active_cal_href.clone();
    app.session.search_term = app.search_value.text();

    // Delegate entirely to session state
    let filter_res = app.session.get_filtered_view(&app.store, config);

    app.tasks = filter_res.items;
    app.cached_categories = filter_res.categories;
    app.cached_locations = filter_res.locations;

    for item in &mut app.tasks {
        if let crate::store::TaskListItem::Task(task) = item {
            app.task_ids
                .entry(task.uid.clone())
                .or_insert_with(iced::widget::Id::unique);
        }
    }

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
    let mut cfg = app.core_config.clone();

    cfg.url = app.ob_url.clone();
    cfg.username = app.ob_user.clone();
    cfg.password = app.ob_pass.clone();
    cfg.default_calendar = app.ob_default_cal.clone();
    cfg.allow_insecure_certs = app.ob_insecure;
    cfg.hidden_calendars = app.hidden_calendars.iter().cloned().collect();
    cfg.disabled_calendars = app.disabled_calendars.iter().cloned().collect();
    cfg.hide_completed = app.hide_completed;
    cfg.hide_fully_completed_tags = app.hide_fully_completed_tags;
    cfg.sort_standard_by_priority = app.sort_standard_by_priority;
    cfg.show_priority_numbers = app.show_priority_numbers;
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
    cfg.pinned_actions = app.pinned_actions.clone();

    cfg.max_done_roots = app.ob_max_done_roots_input.parse().unwrap_or(20);
    cfg.max_done_subtasks = app.ob_max_done_subtasks_input.parse().unwrap_or(5);

    cfg.quick_filter_term = app.quick_filter_term.clone();
    cfg.quick_filter_icon = app.quick_filter_icon.clone();
    cfg.show_quick_filter = app.show_quick_filter;
    cfg.sidebar_is_hidden = app.sidebar_is_hidden;
    cfg.log_level = app.log_level;

    // Cache the updated config in memory
    app.core_config = cfg.clone();

    // --- ASYNC SAVE FIX ---
    let ctx_clone = app.ctx.clone();
    let cfg_clone = cfg.clone();
    std::thread::spawn(move || {
        let _ = cfg_clone.save_with_credentials(ctx_clone.as_ref());
    });
    // ----------------------

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
) -> Vec<crate::model::Task> {
    let modified_tasks = app.store.apply_alias_retroactively(alias_key, target_tags);

    if modified_tasks.is_empty() {
        return Vec::new();
    }

    refresh_filtered_tasks(app);
    modified_tasks
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
        let idx_opt = app.find_task_index_by_uid(uid);

        // If the task was completely filtered out (e.g. search didn't match), do NOT try to scroll to it.
        // Doing so would query a floating `Id` that is not attached to the layout tree!
        if idx_opt.is_none() {
            return Task::none();
        }

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
            // Index-only centering as a last resort, using dynamic height estimation.
            let mut content_h = 0.0;
            let mut item_center = 0.0;

            for (i, item) in app.tasks.iter().enumerate() {
                let mut h = 36.0; // Base row height

                if let crate::store::TaskListItem::Task(t) = item {
                    // Add height for wrapped summary (approx 60 chars per line)
                    h += (t.summary.len().saturating_sub(60) as f32 / 60.0).floor() * 20.0;

                    // Add height for expanded details
                    if app.expanded_tasks.contains(&t.uid) {
                        h += 15.0; // Margin
                        if !t.description.is_empty() {
                            h += t.description.lines().count() as f32 * 18.0;
                        }
                        h += t.dependencies.len() as f32 * 24.0;
                        h += t.related_to.len() as f32 * 24.0;
                        h += app.store.get_tasks_blocking(&t.uid).len() as f32 * 24.0;
                        h += app.store.get_tasks_related_to(&t.uid).len() as f32 * 24.0;
                        if !t.sessions.is_empty() || app.adding_session_uid.as_ref() == Some(&t.uid)
                        {
                            h += 30.0 + (t.sessions.len().min(3) as f32 * 20.0);
                        }
                    }
                } else {
                    h = 28.0; // Expand/Collapse virtual row height
                }

                if i < idx {
                    item_center += h;
                } else if i == idx {
                    item_center += h / 2.0;
                }
                content_h += h;
            }

            content_h = content_h.max(1.0);
            let viewport_h = (app.current_window_size.height - 180.0).max(100.0);
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
                    tokio::time::sleep(StdDuration::from_millis(50)).await;
                }
            },
            move |_| Message::SnapToSelected { focus },
        );
    }

    // Batch a few increasing delays to give the UI multiple chances to register the row.
    Task::batch(vec![
        Task::perform(
            async {
                tokio::time::sleep(StdDuration::from_millis(120)).await;
            },
            move |_| Message::SnapToSelected { focus },
        ),
        Task::perform(
            async {
                tokio::time::sleep(StdDuration::from_millis(360)).await;
            },
            move |_| Message::SnapToSelected { focus },
        ),
        Task::perform(
            async {
                tokio::time::sleep(StdDuration::from_millis(720)).await;
            },
            move |_| Message::SnapToSelected { focus },
        ),
    ])
}

use crate::model::AppIntent;

pub fn dispatch_intent(app: &mut GuiApp, intent: AppIntent) {
    let config = &app.core_config;

    // 1. Update UI filters synchronously
    app.session.apply_session_intent(&intent);

    // 2. Mutate in-memory store synchronously & extract persistence actions
    let actions = app.store.apply_task_intent(&intent, config);

    // 3. Update the UI rendering
    refresh_filtered_tasks(app);

    // 4. Send actions to background thread for disk/network persistence
    if !actions.is_empty()
        && let Some(tx) = &app.bg_tx
    {
        let _ = tx.try_send(crate::gui::async_ops::WorkerCommand::Batch(actions));
    }
}

/// Reloads the journal from disk and updates the unsynced UI state & tooltip.
pub fn update_journal_state(app: &mut GuiApp) {
    let journal = crate::journal::Journal::load(app.ctx.as_ref());
    app.unsynced_changes = !journal.is_empty();

    if app.unsynced_changes {
        let mut lines = vec![rust_i18n::t!("unsynced").to_string()];
        for (i, action) in journal.queue.iter().enumerate() {
            if i >= 10 {
                lines.push(
                    rust_i18n::t!("unsynced_and_more", count = journal.queue.len() - 10)
                        .to_string(),
                );
                break;
            }
            let (verb, summary) = match action {
                crate::journal::Action::Create(t) => (
                    rust_i18n::t!("unsynced_action_create").to_string(),
                    &t.summary,
                ),
                crate::journal::Action::Update(t) => (
                    rust_i18n::t!("unsynced_action_update").to_string(),
                    &t.summary,
                ),
                crate::journal::Action::Delete(t) => (
                    rust_i18n::t!("calendar_action_deleted").to_string(),
                    &t.summary,
                ),
                crate::journal::Action::Move(t, _) => (
                    rust_i18n::t!("unsynced_action_move").to_string(),
                    &t.summary,
                ),
            };
            let trunc_summary = if summary.chars().count() > 40 {
                format!("{}...", summary.chars().take(37).collect::<String>())
            } else {
                summary.clone()
            };
            lines.push(format!("• {}: {}", verb, trunc_summary));
        }
        app.unsynced_tooltip = lines.join("\n");
    } else {
        app.unsynced_tooltip = rust_i18n::t!("force_sync").to_string();
    }
}
