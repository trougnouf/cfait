// File: src/tui/handlers.rs
// Handles keyboard input and system events for the TUI.
use crate::config::Config;
use crate::model::parser::{extract_inline_aliases, validate_alias_integrity};
use crate::model::{Task, TaskStatus};
use crate::storage::{LOCAL_CALENDAR_HREF, LOCAL_TRASH_HREF, LocalCalendarRegistry};
use crate::system::SystemEvent;
use crate::tui::action::{Action, AppEvent, SidebarMode};
use crate::tui::state::{AppState, Focus, InputMode};
use chrono::NaiveTime;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;

// Weighted-random helper from the shared store
use crate::store::select_weighted_random_index;

pub fn handle_app_event(state: &mut AppState, event: AppEvent, default_cal: &Option<String>) {
    match event {
        AppEvent::Status(s) => state.message = s,
        AppEvent::Error(s) => {
            state.message = format!("Error: {}", s);
            state.loading = false;
        }
        AppEvent::CalendarsLoaded(cals) => {
            state.calendars = cals;

            if let Some(def) = default_cal
                && let Some(found) = state
                    .calendars
                    .iter()
                    .find(|c| c.name == *def || c.href == *def)
            {
                if state.hidden_calendars.contains(&found.href) {
                    state.hidden_calendars.remove(&found.href);
                }
                state.active_cal_href = Some(found.href.clone());
            }

            if state.active_cal_href.is_none() {
                state.active_cal_href = Some(LOCAL_CALENDAR_HREF.to_string());
            }
            state.refresh_filtered_view();
        }
        AppEvent::TasksLoaded(results) => {
            for (href, tasks) in results {
                state.store.insert(href, tasks);
            }
            state.refresh_filtered_view();
            state.loading = false;
        }
    }
}

// Helper to notify the alarm system of local changes immediately
fn update_alarms(state: &AppState) {
    if let Some(tx) = &state.alarm_actor_tx {
        let all = state
            .store
            .calendars
            .values()
            .flat_map(|m| m.values())
            .cloned()
            .collect();
        let _ = tx.try_send(SystemEvent::UpdateTasks(all));
    }
}

pub async fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    action_tx: &Sender<Action>,
) -> Option<Action> {
    // --- ALARM INTERCEPTION ---
    if let Some((task, alarm_uid)) = state.active_alarm.clone() {
        // If in Snoozing mode, let the input handler deal with it
        if state.mode == InputMode::Snoozing {
            // Fall through to normal input handling below
        } else {
            match key.code {
                KeyCode::Char('D') | KeyCode::Char('d') => {
                    if let Some((t, _)) = state.store.get_task_mut(&task.uid)
                        && t.handle_dismiss(&alarm_uid)
                    {
                        let t_clone = t.clone();
                        // Update UI
                        state.active_alarm = None;
                        state.refresh_filtered_view();
                        // Push update to backend
                        let _ = action_tx.send(Action::UpdateTask(t_clone.clone())).await;
                        // Push update to alarm actor
                        update_alarms(state);
                    }
                    return None;
                }
                KeyCode::Char('1') => {
                    // Snooze short preset
                    if let Some((t, _)) = state.store.get_task_mut(&task.uid)
                        && t.handle_snooze(&alarm_uid, state.snooze_short_mins)
                    {
                        let t_clone = t.clone();
                        state.active_alarm = None;
                        state.refresh_filtered_view();
                        let _ = action_tx.send(Action::UpdateTask(t_clone.clone())).await;
                        update_alarms(state);
                    }
                    return None;
                }
                KeyCode::Char('2') => {
                    // Snooze long preset
                    if let Some((t, _)) = state.store.get_task_mut(&task.uid)
                        && t.handle_snooze(&alarm_uid, state.snooze_long_mins)
                    {
                        let t_clone = t.clone();
                        state.active_alarm = None;
                        state.refresh_filtered_view();
                        let _ = action_tx.send(Action::UpdateTask(t_clone.clone())).await;
                        update_alarms(state);
                    }
                    return None;
                }
                KeyCode::Char('c') => {
                    /* Complete */
                    if let Some((t, _)) = state.store.get_task_mut(&task.uid) {
                        // Dismiss alarm first (optional but clean)
                        t.dismiss_alarm(&alarm_uid);
                        let uid = t.uid.clone();
                        // Close popup
                        state.active_alarm = None;
                        // Compute changes locally in the store
                        if let Some((primary, secondary, children)) = state.store.toggle_task(&uid)
                        {
                            state.refresh_filtered_view();
                            update_alarms(state);

                            // Dispatch explicit persistence actions to the network actor
                            if let Some(sec) = secondary {
                                // Recurring: create history, update next instance
                                let _ = action_tx.send(Action::CreateTask(primary)).await;
                                let _ = action_tx.send(Action::UpdateTask(sec)).await;
                            } else {
                                // Non-recurring: update the primary task
                                let _ = action_tx.send(Action::UpdateTask(primary)).await;
                            }

                            // Persist any children that were auto-reset by the store
                            for child in children {
                                let _ = action_tx.send(Action::UpdateTask(child)).await;
                            }

                            // We've dispatched persistence actions; no single intent to return.
                            return None;
                        }
                    }
                    return None;
                }
                KeyCode::Char('x') => {
                    /* Cancel */
                    if let Some((t, _)) = state.store.get_task_mut(&task.uid) {
                        t.dismiss_alarm(&alarm_uid);
                        let uid = t.uid.clone();
                        state.active_alarm = None;

                        // Apply cancel logic in-store (may produce history + next + reset children)
                        if let Some((primary, secondary, children)) =
                            state.store.set_status(&uid, TaskStatus::Cancelled)
                        {
                            state.refresh_filtered_view();
                            update_alarms(state);

                            // Persist results via explicit actions
                            if let Some(sec) = secondary {
                                let _ = action_tx.send(Action::CreateTask(primary)).await;
                                let _ = action_tx.send(Action::UpdateTask(sec)).await;
                            } else {
                                let _ = action_tx.send(Action::UpdateTask(primary)).await;
                            }

                            for child in children {
                                let _ = action_tx.send(Action::UpdateTask(child)).await;
                            }
                        }
                    }
                    return None;
                }
                KeyCode::Char('S') | KeyCode::Char('s') => {
                    // Enter custom snooze mode
                    state.mode = InputMode::Snoozing;
                    state.reset_input();
                    return None;
                }
                _ => return None, // Block other input while alarm is ringing
            }
        }
    }
    // --------------------------
    // --- SANITY CHECK ---
    // Prevent out-of-bounds panics if cursor drift happened
    let char_count = state.input_buffer.chars().count();
    if state.cursor_position > char_count {
        state.cursor_position = char_count;
    }

    match state.mode {
        InputMode::Creating => match key.code {
            KeyCode::Enter if !state.input_buffer.is_empty() => {
                let (clean_input, new_aliases): (String, HashMap<String, Vec<String>>) =
                    extract_inline_aliases(&state.input_buffer);

                if !new_aliases.is_empty() {
                    for (key, tags) in new_aliases {
                        if let Err(e) = validate_alias_integrity(&key, &tags, &state.tag_aliases) {
                            state.message = format!("Alias Error: {}", e);
                            return None;
                        }

                        state.tag_aliases.insert(key.clone(), tags.clone());
                        let modified = state.store.apply_alias_retroactively(&key, &tags);

                        for t in modified {
                            let _ = action_tx.send(Action::UpdateTask(t)).await;
                        }
                    }
                    if let Ok(mut cfg) = Config::load(state.ctx.as_ref()) {
                        cfg.tag_aliases = state.tag_aliases.clone();
                        let _ = cfg.save(state.ctx.as_ref());
                    }
                }

                if clean_input.starts_with('#')
                    && !clean_input.trim().contains(' ')
                    && state.creating_child_of.is_none()
                {
                    let was_alias_def = state.input_buffer.contains(":=");

                    if !was_alias_def {
                        let tag = clean_input.trim().trim_start_matches('#').to_string();
                        if !tag.is_empty() {
                            state.sidebar_mode = SidebarMode::Categories;
                            state.selected_categories.clear();
                            state.selected_categories.insert(tag);
                            state.mode = InputMode::Normal;
                            state.reset_input();
                            state.refresh_filtered_view();
                            return None;
                        }
                    } else {
                        state.mode = InputMode::Normal;
                        state.reset_input();
                        state.message = "Alias updated.".to_string();
                        return None;
                    }
                }

                let is_loc = clean_input.starts_with("@@") || clean_input.starts_with("loc:");
                if is_loc && !clean_input.trim().contains(' ') && state.creating_child_of.is_none()
                {
                    let raw = if clean_input.starts_with("@@") {
                        clean_input.trim_start_matches("@@")
                    } else {
                        clean_input.trim_start_matches("loc:")
                    };
                    let loc = crate::model::parser::strip_quotes(raw);

                    if !loc.is_empty() {
                        state.sidebar_mode = SidebarMode::Locations;
                        state.selected_locations.clear();
                        state.selected_locations.insert(loc);
                        state.mode = InputMode::Normal;
                        state.reset_input();
                        state.refresh_filtered_view();
                        return None;
                    }
                }

                let target_href = state
                    .active_cal_href
                    .clone()
                    .or_else(|| state.calendars.first().map(|c| c.href.clone()));

                if let Some(href) = target_href {
                    // Load config to get time
                    let config = Config::load(state.ctx.as_ref()).unwrap_or_default();
                    let def_time =
                        NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

                    let mut task = Task::new(&clean_input, &state.tag_aliases, def_time);
                    task.calendar_href = href.clone();
                    task.parent_uid = state.creating_child_of.clone();

                    let new_uid = task.uid.clone();
                    state.store.add_task(task.clone());
                    state.refresh_filtered_view();
                    update_alarms(state);

                    if let Some(idx) = state.tasks.iter().position(|t| t.uid == new_uid) {
                        state.list_state.select(Some(idx));
                    }

                    state.mode = InputMode::Normal;
                    state.reset_input();
                    state.creating_child_of = None;
                    return Some(Action::CreateTask(task));
                }
                state.mode = InputMode::Normal;
                state.reset_input();
            }
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.reset_input();
            }
            KeyCode::Char(c) => state.enter_char(c),
            KeyCode::Backspace => state.delete_char(),
            KeyCode::Left => state.move_cursor_left(),
            KeyCode::Right => state.move_cursor_right(),
            _ => {}
        },
        InputMode::Editing => match key.code {
            KeyCode::Enter => {
                let (clean_input, new_aliases): (String, HashMap<String, Vec<String>>) =
                    extract_inline_aliases(&state.input_buffer);
                if !new_aliases.is_empty() {
                    for (k, v) in new_aliases {
                        if let Err(e) = validate_alias_integrity(&k, &v, &state.tag_aliases) {
                            state.message = format!("Alias Error: {}", e);
                            return None;
                        }

                        state.tag_aliases.insert(k.clone(), v.clone());
                        let modified = state.store.apply_alias_retroactively(&k, &v);
                        for mod_t in modified {
                            let _ = action_tx.send(Action::UpdateTask(mod_t)).await;
                        }
                    }
                    if let Ok(mut cfg) = Config::load(state.ctx.as_ref()) {
                        cfg.tag_aliases = state.tag_aliases.clone();
                        let _ = cfg.save(state.ctx.as_ref());
                    }
                }

                let target_uid: Option<String> = state
                    .editing_index
                    .and_then(|idx| state.tasks.get(idx).map(|t| t.uid.clone()));

                if let Some(uid) = target_uid
                    && let Some((t, _)) = state.store.get_task_mut(&uid)
                {
                    let config = Config::load(state.ctx.as_ref()).unwrap_or_default();
                    let def_time =
                        NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
                    t.apply_smart_input(&clean_input, &state.tag_aliases, def_time);
                    let clone = t.clone();
                    state.refresh_filtered_view();
                    update_alarms(state);
                    state.mode = InputMode::Normal;
                    state.reset_input();
                    return Some(Action::UpdateTask(clone));
                }
                state.mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.reset_input();
            }
            KeyCode::Char(c) => state.enter_char(c),
            KeyCode::Backspace => state.delete_char(),
            KeyCode::Left => state.move_cursor_left(),
            KeyCode::Right => state.move_cursor_right(),
            _ => {}
        },
        InputMode::EditingDescription => match key.code {
            // Enter inserts a newline
            KeyCode::Enter => {
                state.enter_char('\n');
            }
            // Save: Ctrl+S
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let target_uid: Option<String> = state
                    .editing_index
                    .and_then(|idx| state.tasks.get(idx).map(|t| t.uid.clone()));

                if let Some(uid) = target_uid
                    && let Some((t, _)) = state.store.get_task_mut(&uid)
                {
                    t.description = state.input_buffer.clone();
                    let clone = t.clone();
                    state.refresh_filtered_view();
                    state.mode = InputMode::Normal;
                    state.reset_input();
                    return Some(Action::UpdateTask(clone));
                }
                state.mode = InputMode::Normal;
                state.reset_input();
            }
            // Cancel: Esc
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.reset_input();
                state.message = "Editing cancelled.".to_string();
            }
            // Editing & Navigation
            KeyCode::Char(c) => {
                // FIX: Sanitize control characters and normalize tabs to spaces.
                // Convert Tab to 4 spaces to ensure cursor math matches rendering.
                if c == '\t' {
                    for _ in 0..4 {
                        state.enter_char(' ');
                    }
                } else if !c.is_control() || c == '\n' {
                    state.enter_char(c);
                }
            }
            KeyCode::Backspace => state.delete_char(),
            KeyCode::Left => state.move_cursor_left(),
            KeyCode::Right => state.move_cursor_right(),
            KeyCode::Up => {
                // Move cursor to same column on previous line (or start)
                let current_idx = state.cursor_position;
                let chars: Vec<char> = state.input_buffer.chars().collect();

                // 1. Find start of current line
                let mut line_start = current_idx;
                while line_start > 0 && chars[line_start - 1] != '\n' {
                    line_start -= 1;
                }

                // 2. Calculate column offset
                let col = current_idx - line_start;

                if line_start > 0 {
                    // 3. Find start of previous line
                    let mut prev_line_start = line_start - 1;
                    while prev_line_start > 0 && chars[prev_line_start - 1] != '\n' {
                        prev_line_start -= 1;
                    }

                    // 4. Determine length of previous line
                    // (line_start - 1) is the newline char itself
                    let prev_line_len = (line_start - 1) - prev_line_start;

                    // 5. Move to min(col, prev_len)
                    let new_col = col.min(prev_line_len);
                    state.cursor_position = prev_line_start + new_col;
                } else {
                    state.cursor_position = 0;
                }
            }
            KeyCode::Down => {
                let current_idx = state.cursor_position;
                let chars: Vec<char> = state.input_buffer.chars().collect();
                let total = chars.len();

                // 1. Find start of current line and column
                let mut line_start = current_idx;
                while line_start > 0 && chars[line_start - 1] != '\n' {
                    line_start -= 1;
                }
                let col = current_idx - line_start;

                // 2. Find end of current line (start of next)
                let mut next_line_start = current_idx;
                while next_line_start < total && chars[next_line_start] != '\n' {
                    next_line_start += 1;
                }

                if next_line_start < total {
                    next_line_start += 1; // Skip the newline

                    // 3. Find length of next line
                    let mut next_line_end = next_line_start;
                    while next_line_end < total && chars[next_line_end] != '\n' {
                        next_line_end += 1;
                    }
                    let next_line_len = next_line_end - next_line_start;

                    // 4. Move
                    let new_col = col.min(next_line_len);
                    state.cursor_position = next_line_start + new_col;
                } else {
                    state.cursor_position = total;
                }
            }
            _ => {}
        },
        InputMode::Snoozing => match key.code {
            KeyCode::Enter if !state.input_buffer.is_empty() => {
                // Parse custom snooze duration
                if let Some(mins) = crate::model::parser::parse_duration(&state.input_buffer) {
                    if let Some((task, alarm_uid)) = state.active_alarm.clone()
                        && let Some((t, _)) = state.store.get_task_mut(&task.uid)
                        && t.snooze_alarm(&alarm_uid, mins)
                    {
                        let t_clone = t.clone();
                        state.active_alarm = None;
                        state.mode = InputMode::Normal;
                        state.reset_input();
                        state.refresh_filtered_view();
                        let _ = action_tx.send(Action::UpdateTask(t_clone)).await;
                        update_alarms(state);
                    }
                } else {
                    state.message = format!("Invalid duration: '{}'", state.input_buffer);
                }
                return None;
            }
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.reset_input();
                // Return to alarm display
                return None;
            }
            KeyCode::Char(c) => {
                state.enter_char(c);
            }
            KeyCode::Backspace => {
                state.delete_char();
            }
            KeyCode::Left => {
                state.move_cursor_left();
            }
            KeyCode::Right => {
                state.move_cursor_right();
            }
            _ => {}
        },
        InputMode::Searching => match key.code {
            KeyCode::Enter => {
                if state.input_buffer.starts_with('#') && !state.input_buffer.contains(' ') {
                    let tag = state.input_buffer.trim_start_matches('#').to_string();
                    state.sidebar_mode = SidebarMode::Categories;
                    state.selected_categories.clear();
                    state.selected_categories.insert(tag);
                    state.active_search_query.clear();
                } else if (state.input_buffer.starts_with("@@")
                    || state.input_buffer.starts_with("loc:"))
                    && !state.input_buffer.contains(' ')
                {
                    let raw = if state.input_buffer.starts_with("@@") {
                        state.input_buffer.trim_start_matches("@@")
                    } else {
                        state.input_buffer.trim_start_matches("loc:")
                    };
                    let loc = crate::model::parser::strip_quotes(raw);

                    state.sidebar_mode = SidebarMode::Locations;
                    state.selected_locations.clear();
                    state.selected_locations.insert(loc);
                    state.active_search_query.clear();
                } else {
                    state.active_search_query = state.input_buffer.clone();
                }

                state.mode = InputMode::Normal;
                state.reset_input();
                state.refresh_filtered_view();
            }
            KeyCode::Esc => {
                state.active_search_query.clear();
                state.mode = InputMode::Normal;
                state.reset_input();
                state.refresh_filtered_view();
            }
            KeyCode::Char(c) => {
                state.enter_char(c);
                state.refresh_filtered_view();
            }
            KeyCode::Backspace => {
                state.delete_char();
                state.refresh_filtered_view();
            }
            KeyCode::Left => state.move_cursor_left(),
            KeyCode::Right => state.move_cursor_right(),
            KeyCode::Down => state.next(),
            KeyCode::Up => state.previous(),
            KeyCode::PageDown => state.jump_forward(10),
            KeyCode::PageUp => state.jump_backward(10),
            _ => {}
        },
        InputMode::Normal => match key.code {
            KeyCode::Esc => {
                let mut needs_refresh = false;
                if state.yanked_uid.is_some() {
                    state.yanked_uid = None;
                    state.message = "Yank cleared.".to_string();
                } else if !state.active_search_query.is_empty() {
                    state.active_search_query.clear();
                    needs_refresh = true;
                } else if !state.selected_categories.is_empty() {
                    state.selected_categories.clear();
                    needs_refresh = true;
                }
                if needs_refresh {
                    state.refresh_filtered_view();
                }
            }
            KeyCode::Char('?') => state.show_full_help = !state.show_full_help,
            KeyCode::Char('q') => return Some(Action::Quit),
            KeyCode::Char('r') => return Some(Action::Refresh),
            KeyCode::Char('R') => {
                // Weighted-random jump to a task (uppercase R)
                if let Some(idx) =
                    select_weighted_random_index(&state.tasks, state.default_priority)
                {
                    state.list_state.select(Some(idx));
                    state.message = "Jumped to random task".to_string();
                }
            }

            KeyCode::Char(' ') => {
                if state.active_focus == Focus::Main {
                    if let Some(uid) = state.get_selected_task().map(|t| t.uid.clone())
                        && let Some((primary, secondary, children)) = state.store.toggle_task(&uid)
                    {
                        state.refresh_filtered_view();
                        update_alarms(state);

                        // Dispatch persistence actions computed by the store
                        if let Some(sec) = secondary {
                            let _ = action_tx.send(Action::CreateTask(primary)).await;
                            let _ = action_tx.send(Action::UpdateTask(sec)).await;
                        } else {
                            let _ = action_tx.send(Action::UpdateTask(primary)).await;
                        }

                        for child in children {
                            let _ = action_tx.send(Action::UpdateTask(child)).await;
                        }

                        // Local state already updated; no single intent to return.
                        return None;
                    }
                } else if state.active_focus == Focus::Sidebar
                    && state.sidebar_mode == SidebarMode::Calendars
                {
                    let target_href = if let Some(idx) = state.cal_state.selected() {
                        let filtered = state.get_filtered_calendars();
                        filtered.get(idx).map(|c| c.href.clone())
                    } else {
                        None
                    };

                    if let Some(href) = target_href
                        && state.active_cal_href.as_ref() != Some(&href)
                    {
                        if state.hidden_calendars.contains(&href) {
                            state.hidden_calendars.remove(&href);
                            let _ = action_tx.send(Action::ToggleCalendarVisibility(href)).await;
                        } else {
                            state.hidden_calendars.insert(href);
                        }
                        state.refresh_filtered_view();
                    }
                }
            }
            KeyCode::Char('s') => {
                if let Some(task) = state.get_selected_task() {
                    let uid = task.uid.clone();
                    let updated_tasks = if task.status == TaskStatus::InProcess {
                        state.store.pause_task(&uid)
                    } else {
                        state.store.set_status_in_process(&uid)
                    };

                    if !updated_tasks.is_empty() {
                        state.refresh_filtered_view();
                        for t in updated_tasks {
                            let tx = action_tx.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Action::UpdateTask(t)).await;
                            });
                        }
                        return None;
                    }
                }
            }
            KeyCode::Char('S') => {
                if let Some(uid) = state.get_selected_task().map(|t| t.uid.clone()) {
                    let updated_tasks = state.store.stop_task(&uid);
                    if !updated_tasks.is_empty() {
                        state.refresh_filtered_view();
                        for t in updated_tasks {
                            let tx = action_tx.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Action::UpdateTask(t)).await;
                            });
                        }
                        return None;
                    }
                }
            }
            KeyCode::Char('x') => {
                if let Some(uid) = state.get_selected_task().map(|t| t.uid.clone())
                    && let Some((primary, secondary, children)) =
                        state.store.set_status(&uid, TaskStatus::Cancelled)
                {
                    state.refresh_filtered_view();
                    update_alarms(state);

                    // Persist the computed mutations
                    if let Some(sec) = secondary {
                        let _ = action_tx.send(Action::CreateTask(primary)).await;
                        let _ = action_tx.send(Action::UpdateTask(sec)).await;
                    } else {
                        let _ = action_tx.send(Action::UpdateTask(primary)).await;
                    }

                    for child in children {
                        let _ = action_tx.send(Action::UpdateTask(child)).await;
                    }
                }
            }
            KeyCode::Char('+') => {
                if let Some(uid) = state.get_selected_task().map(|t| t.uid.clone())
                    && let Some(updated) =
                        state.store.change_priority(&uid, 1, state.default_priority)
                {
                    state.refresh_filtered_view();
                    return Some(Action::UpdateTask(updated));
                }
            }
            KeyCode::Char('-') => {
                if let Some(uid) = state.get_selected_task().map(|t| t.uid.clone())
                    && let Some(updated) =
                        state
                            .store
                            .change_priority(&uid, -1, state.default_priority)
                {
                    state.refresh_filtered_view();
                    return Some(Action::UpdateTask(updated));
                }
            }
            KeyCode::Char('d') => {
                if let Some(view_task) = state.get_selected_task() {
                    let uid = view_task.uid.clone();
                    let is_trash = view_task.calendar_href == LOCAL_TRASH_HREF;

                    // Load config to check retention
                    let config = Config::load(state.ctx.as_ref()).unwrap_or_default();

                    if config.trash_retention_days > 0 && !is_trash {
                        // --- SOFT DELETE ---
                        // 1. Ensure Registry
                        let _ =
                            LocalCalendarRegistry::ensure_trash_calendar_exists(state.ctx.as_ref());
                        // Update UI calendar list if needed
                        if !state.calendars.iter().any(|c| c.href == LOCAL_TRASH_HREF)
                            && let Ok(cals) = LocalCalendarRegistry::load(state.ctx.as_ref())
                        {
                            // Merge intelligently
                            for c in cals {
                                if !state
                                    .calendars
                                    .iter()
                                    .any(|existing| existing.href == c.href)
                                {
                                    state.calendars.push(c);
                                }
                            }
                        }

                        // 2. Ensure Store Map
                        state
                            .store
                            .calendars
                            .entry(LOCAL_TRASH_HREF.to_string())
                            .or_default();

                        // 3. Move in Store
                        if let Some((original, mut updated)) =
                            state.store.move_task(&uid, LOCAL_TRASH_HREF.to_string())
                        {
                            // 4. Stamp Date
                            let now_str = chrono::Utc::now().to_rfc3339();
                            updated
                                .unmapped_properties
                                .retain(|p| p.key != "X-TRASHED-DATE");
                            updated.unmapped_properties.push(crate::model::RawProperty {
                                key: "X-TRASHED-DATE".to_string(),
                                value: now_str,
                                params: vec![],
                            });

                            // 5. Save Trash Copy
                            state.store.update_or_add_task(updated.clone());
                            state.refresh_filtered_view();
                            update_alarms(state);

                            // 6. Delete Original
                            // We emit DeleteTask for the original. The network actor will see it as a delete
                            // and remove it from the server. The local trash copy is safe.
                            return Some(Action::DeleteTask(original));
                        }
                    } else {
                        // --- HARD DELETE ---
                        if let Some((deleted, _)) = state.store.delete_task(&uid) {
                            state.refresh_filtered_view();
                            update_alarms(state);
                            return Some(Action::DeleteTask(deleted));
                        }
                    }
                }
            }
            KeyCode::Char('c') => {
                let data = if let Some(parent_uid) = &state.yanked_uid {
                    state
                        .get_selected_task()
                        .map(|view_task| (view_task.uid.clone(), parent_uid.clone()))
                } else {
                    None
                };

                if let Some((child_uid, parent_uid)) = data {
                    if child_uid == parent_uid {
                        state.message = "Cannot be child of self!".to_string();
                    } else if let Some(updated) =
                        state.store.set_parent(&child_uid, Some(parent_uid))
                    {
                        state.yanked_uid = None;
                        state.refresh_filtered_view();
                        return Some(Action::UpdateTask(updated));
                    }
                }
            }
            KeyCode::Char('C') => {
                if state.active_focus == Focus::Main
                    && let Some(task) = state.get_selected_task()
                {
                    // Fix: Define these inside the block to resolve scope errors
                    let uid = task.uid.clone();
                    let summary = task.summary.clone();

                    let mut initial_input = String::new();
                    for cat in &task.categories {
                        // Parity: Use quote_value to handle spaces correctly
                        initial_input
                            .push_str(&format!("#{} ", crate::model::parser::quote_value(cat)));
                    }
                    // Parity: Add Location inheritance
                    if let Some(loc) = &task.location {
                        initial_input
                            .push_str(&format!("@@{} ", crate::model::parser::quote_value(loc)));
                    }

                    state.input_buffer = initial_input;
                    state.cursor_position = state.input_buffer.chars().count();

                    state.mode = InputMode::Creating;
                    state.creating_child_of = Some(uid);
                    state.message = format!("New Child of '{}'...", summary);
                }
            }
            KeyCode::Char('y') => {
                if let Some(t) = state.get_selected_task() {
                    let uid = t.uid.clone();
                    let summary = t.summary.clone();
                    state.yanked_uid = Some(uid);
                    state.message = format!("Yanked: {}", summary);
                }
            }
            KeyCode::Char('b') => {
                let data = if let Some(yanked) = &state.yanked_uid {
                    state
                        .get_selected_task()
                        .map(|current| (current.uid.clone(), yanked.clone()))
                } else {
                    None
                };

                if let Some((curr_uid, yanked_uid)) = data {
                    if curr_uid == yanked_uid {
                        state.message = "Cannot depend on self!".to_string();
                    } else if let Some(updated) = state.store.add_dependency(&curr_uid, yanked_uid)
                    {
                        state.yanked_uid = None;
                        state.refresh_filtered_view();
                        return Some(Action::UpdateTask(updated));
                    }
                }
            }
            KeyCode::Char('l') => {
                let data = if let Some(yanked) = &state.yanked_uid {
                    state
                        .get_selected_task()
                        .map(|current| (current.uid.clone(), yanked.clone()))
                } else {
                    None
                };

                if let Some((curr_uid, yanked_uid)) = data {
                    if curr_uid == yanked_uid {
                        state.message = "Cannot relate to self!".to_string();
                    } else if let Some(updated) = state.store.add_related_to(&curr_uid, yanked_uid)
                    {
                        state.yanked_uid = None;
                        state.refresh_filtered_view();
                        return Some(Action::UpdateTask(updated));
                    }
                }
            }
            KeyCode::Char('.') | KeyCode::Char('>') => {
                if state.active_focus == Focus::Main
                    && let Some(idx) = state.list_state.selected()
                    && idx > 0
                    && idx < state.tasks.len()
                {
                    let parent_uid = state.tasks[idx - 1].uid.clone();
                    let current_uid = state.tasks[idx].uid.clone();
                    if let Some(updated) = state.store.set_parent(&current_uid, Some(parent_uid)) {
                        state.refresh_filtered_view();
                        return Some(Action::UpdateTask(updated));
                    }
                }
            }
            KeyCode::Char(',') | KeyCode::Char('<') => {
                if state.active_focus == Focus::Main
                    && let Some(view_task) = state.get_selected_task()
                    && view_task.parent_uid.is_some()
                {
                    let uid = view_task.uid.clone();
                    if let Some(updated) = state.store.set_parent(&uid, None) {
                        state.refresh_filtered_view();
                        return Some(Action::UpdateTask(updated));
                    }
                }
            }
            KeyCode::Char('X') => {
                // Step 1: Select source local calendar
                state.export_source_calendars = state
                    .calendars
                    .iter()
                    .filter(|c| {
                        c.href.starts_with("local://")
                            && !state.disabled_calendars.contains(&c.href)
                    })
                    .cloned()
                    .collect();
                if !state.export_source_calendars.is_empty() {
                    state.export_source_selection_state.select(Some(0));
                    state.mode = InputMode::SelectingExportSource;
                    state.message = "Select source local calendar to export from.".to_string();
                }
            }
            KeyCode::Char('M') => {
                if let Some(task) = state.get_selected_task() {
                    let current_href = task.calendar_href.clone();
                    state.move_targets = state
                        .calendars
                        .iter()
                        .filter(|c| {
                            c.href != current_href && !state.disabled_calendars.contains(&c.href)
                        })
                        .cloned()
                        .collect();
                    if !state.move_targets.is_empty() {
                        state.move_selection_state.select(Some(0));
                        state.mode = InputMode::Moving;
                        state.message = "Select a calendar and press Enter.".to_string();
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => state.next(),
            KeyCode::Up | KeyCode::Char('k') => state.previous(),
            KeyCode::PageDown => state.jump_forward(10),
            KeyCode::PageUp => state.jump_backward(10),
            KeyCode::Tab => state.toggle_focus(),
            KeyCode::Char('1') => {
                state.sidebar_mode = SidebarMode::Calendars;
                state.refresh_filtered_view();
            }
            KeyCode::Char('2') => {
                state.sidebar_mode = SidebarMode::Categories;
                state.refresh_filtered_view();
            }
            KeyCode::Char('3') => {
                state.sidebar_mode = SidebarMode::Locations;
                state.refresh_filtered_view();
            }
            KeyCode::Char('m') => {
                state.match_all_categories = !state.match_all_categories;
                state.refresh_filtered_view();
            }
            KeyCode::Char('H') => {
                state.hide_completed = !state.hide_completed;
                state.refresh_filtered_view();
            }
            KeyCode::Char('L') => {
                // Enter relationship browsing mode to navigate to linked tasks
                if let Some(task) = state.get_selected_task() {
                    let mut items = Vec::new();

                    // Add blocked-by dependencies
                    for dep_uid in &task.dependencies {
                        let name = state
                            .store
                            .get_summary(dep_uid)
                            .unwrap_or_else(|| "Unknown task".to_string());
                        let is_done = state.store.is_task_done(dep_uid).unwrap_or(false);
                        let check = if is_done { "[x]" } else { "[ ]" };
                        items.push((dep_uid.clone(), format!("⬆ {} {}", check, name)));
                    }

                    // Add outgoing relations
                    for related_uid in &task.related_to {
                        let name = state
                            .store
                            .get_summary(related_uid)
                            .unwrap_or_else(|| "Unknown task".to_string());
                        items.push((related_uid.clone(), format!("→ {}", name)));
                    }

                    // Add incoming relations
                    let incoming_related = state.store.get_tasks_related_to(&task.uid);
                    for (related_uid, related_name) in incoming_related {
                        items.push((related_uid, format!("← {}", related_name)));
                    }

                    if !items.is_empty() {
                        state.relationship_items = items;
                        state.relationship_selection_state.select(Some(0));
                        state.mode = InputMode::RelationshipBrowsing;
                        state.message =
                            "Select task to jump to (Enter) or Esc to cancel".to_string();
                    } else {
                        state.message = "No related tasks to browse.".to_string();
                    }
                }
            }
            KeyCode::Char('*') => {
                if state.active_focus == Focus::Sidebar {
                    match state.sidebar_mode {
                        SidebarMode::Calendars => {
                            let are_all_visible = state
                                .calendars
                                .iter()
                                .filter(|c| !state.disabled_calendars.contains(&c.href))
                                .filter(|c| c.href != "local://trash")
                                .all(|c| !state.hidden_calendars.contains(&c.href));

                            if are_all_visible {
                                for cal in &state.calendars {
                                    if state.active_cal_href.as_ref() != Some(&cal.href) {
                                        state.hidden_calendars.insert(cal.href.clone());
                                    }
                                }
                            } else {
                                state.hidden_calendars.clear();
                                // Re-hide trash if not active
                                if state.active_cal_href.as_deref() != Some("local://trash") {
                                    state.hidden_calendars.insert("local://trash".to_string());
                                }
                                let _ = action_tx.send(Action::Refresh).await;
                            }
                        }
                        SidebarMode::Categories => {
                            state.selected_categories.clear();
                        }
                        SidebarMode::Locations => {
                            state.selected_locations.clear();
                        }
                    }
                }
            }
            KeyCode::Right => {
                if state.active_focus == Focus::Sidebar {
                    match state.sidebar_mode {
                        SidebarMode::Calendars => {
                            let target_href = if let Some(idx) = state.cal_state.selected() {
                                let filtered = state.get_filtered_calendars();
                                filtered.get(idx).map(|c| c.href.clone())
                            } else {
                                None
                            };

                            if let Some(href) = target_href {
                                state.active_cal_href = Some(href.clone());
                                state.hidden_calendars.clear();
                                for c in &state.calendars {
                                    if c.href != href {
                                        state.hidden_calendars.insert(c.href.clone());
                                    }
                                }
                                state.refresh_filtered_view();
                                if href != LOCAL_CALENDAR_HREF {
                                    return Some(Action::IsolateCalendar(href));
                                }
                            }
                        }
                        SidebarMode::Categories => {
                            // Use cached categories derived from the last `filter()` call
                            let cats = &state.cached_categories;
                            if let Some(idx) = state.cal_state.selected()
                                && let Some((c, _)) = cats.get(idx)
                            {
                                state.selected_categories.clear();
                                state.selected_categories.insert(c.clone());
                                state.refresh_filtered_view();
                            }
                        }
                        SidebarMode::Locations => {
                            // Use cached locations derived from the last `filter()` call
                            let locs = &state.cached_locations;
                            if let Some(idx) = state.cal_state.selected()
                                && let Some((l, _)) = locs.get(idx)
                            {
                                state.selected_locations.clear();
                                state.selected_locations.insert(l.clone());
                                state.refresh_filtered_view();
                            }
                        }
                    }
                } else if state.mode == InputMode::Editing {
                    state.move_cursor_right();
                }
            }
            KeyCode::Enter => {
                // If the main list has focus, handle virtual expand/collapse rows first.
                if state.active_focus == Focus::Main {
                    // Clone the virtual state to drop the immutable borrow of `state`
                    let virtual_state_opt =
                        state.get_selected_task().map(|t| t.virtual_state.clone());

                    if let Some(virtual_state) = virtual_state_opt {
                        match virtual_state {
                            crate::model::VirtualState::Expand(key) => {
                                state.expanded_done_groups.insert(key);
                                state.refresh_filtered_view();
                                return None;
                            }
                            crate::model::VirtualState::Collapse(key) => {
                                state.expanded_done_groups.remove(&key);
                                state.refresh_filtered_view();
                                return None;
                            }
                            _ => {}
                        }
                    }
                } else if state.active_focus == Focus::Sidebar {
                    match state.sidebar_mode {
                        SidebarMode::Calendars => {
                            let target_href = if let Some(idx) = state.cal_state.selected() {
                                let filtered = state.get_filtered_calendars();
                                filtered.get(idx).map(|c| c.href.clone())
                            } else {
                                None
                            };

                            if let Some(href) = target_href {
                                state.active_cal_href = Some(href.clone());
                                state.hidden_calendars.remove(&href);
                                state.refresh_filtered_view();
                                if href != LOCAL_CALENDAR_HREF {
                                    return Some(Action::SwitchCalendar(href));
                                }
                            }
                        }
                        SidebarMode::Categories => {
                            // Use cached categories derived from the last `filter()` call
                            let cats = &state.cached_categories;
                            if let Some(idx) = state.cal_state.selected()
                                && let Some((c, _)) = cats.get(idx)
                            {
                                let c_clone = c.clone();
                                if state.selected_categories.contains(&c_clone) {
                                    state.selected_categories.remove(&c_clone);
                                } else {
                                    state.selected_categories.insert(c_clone);
                                }
                                state.refresh_filtered_view();
                            }
                        }
                        SidebarMode::Locations => {
                            // Use cached locations derived from the last `filter()` call
                            let locs = &state.cached_locations;
                            if let Some(idx) = state.cal_state.selected()
                                && let Some((l, _)) = locs.get(idx)
                            {
                                let l_clone = l.clone();
                                if state.selected_locations.contains(&l_clone) {
                                    state.selected_locations.remove(&l_clone);
                                } else {
                                    state.selected_locations.insert(l_clone);
                                }
                                state.refresh_filtered_view();
                            }
                        }
                    }
                }
            }
            KeyCode::Char('/') => {
                state.mode = InputMode::Searching;
                state.reset_input();
            }
            KeyCode::Char('a') => {
                state.mode = InputMode::Creating;
                state.reset_input();
                state.message = "New Task...".to_string();
            }
            KeyCode::Char('e') => {
                if let Some(t) = state.get_selected_task() {
                    state.input_buffer = t.to_smart_string();
                    state.cursor_position = state.input_buffer.chars().count();
                    state.editing_index = state.list_state.selected();
                    state.mode = InputMode::Editing;
                }
            }
            KeyCode::Char('E') => {
                if state.active_focus == Focus::Main
                    && let Some(t) = state.get_selected_task()
                {
                    // Load description into input buffer for manual editing
                    state.input_buffer = t.description.clone();
                    state.cursor_position = state.input_buffer.chars().count();

                    // Reset BOTH scroll offsets (vertical + horizontal)
                    state.edit_scroll_offset = 0;
                    state.edit_scroll_x = 0;

                    state.editing_index = state.list_state.selected();
                    state.mode = InputMode::EditingDescription;
                }
            }
            _ => {}
        },
        InputMode::Moving => match key.code {
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.message = String::new();
            }
            KeyCode::Down | KeyCode::Char('j') => state.next_move_target(),
            KeyCode::Up | KeyCode::Char('k') => state.previous_move_target(),
            KeyCode::Enter => {
                let data = if let Some(task) = state.get_selected_task() {
                    if let Some(idx) = state.move_selection_state.selected() {
                        state
                            .move_targets
                            .get(idx)
                            .map(|target_cal| (task.clone(), target_cal.href.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Use the atomic store API that returns both the original (pre-mutation)
                // and updated (post-mutation) tasks so we don't rely on separate
                // lookups/clones and avoid races.
                if let Some((ref selected_task, ref target_href)) = data
                    && let Some((original_task, _updated_task)) = state
                        .store
                        .move_task(&selected_task.uid, target_href.clone())
                {
                    state.refresh_filtered_view();
                    // Update alarms immediately if needed (task moved, though move doesn't clear completion)
                    // Moving a task keeps its alarms but might change visibility.
                    // For safety, re-sync alarms.
                    update_alarms(state);

                    state.message = "Moving task...".to_string();
                    state.mode = InputMode::Normal;
                    return Some(Action::MoveTask(original_task, target_href.clone()));
                }
                state.mode = InputMode::Normal;
            }
            _ => {}
        },
        InputMode::SelectingExportSource => match key.code {
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.message = String::new();
            }
            KeyCode::Down | KeyCode::Char('j') => state.next_export_source(),
            KeyCode::Up | KeyCode::Char('k') => state.previous_export_source(),
            KeyCode::Enter => {
                if let Some(idx) = state.export_source_selection_state.selected()
                    && let Some(source) = state.export_source_calendars.get(idx)
                {
                    // Step 2: Now select destination remote calendar
                    state.export_targets = state
                        .calendars
                        .iter()
                        .filter(|c| {
                            !c.href.starts_with("local://")
                                && !state.disabled_calendars.contains(&c.href)
                        })
                        .cloned()
                        .collect();
                    if !state.export_targets.is_empty() {
                        state.export_selection_state.select(Some(0));
                        state.mode = InputMode::Exporting;
                        state.message = format!(
                            "Exporting from '{}'. Select destination calendar.",
                            source.name
                        );
                    } else {
                        state.mode = InputMode::Normal;
                        state.message = "No remote calendars available for export.".to_string();
                    }
                }
            }
            _ => {}
        },
        InputMode::Exporting => match key.code {
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.message = String::new();
            }
            KeyCode::Down | KeyCode::Char('j') => state.next_export_target(),
            KeyCode::Up | KeyCode::Char('k') => state.previous_export_target(),
            KeyCode::Enter => {
                if let Some(source_idx) = state.export_source_selection_state.selected()
                    && let Some(source) = state.export_source_calendars.get(source_idx)
                    && let Some(target_idx) = state.export_selection_state.selected()
                    && let Some(target) = state.export_targets.get(target_idx)
                {
                    let source_href = source.href.clone();
                    let target_href = target.href.clone();
                    state.mode = InputMode::Normal;
                    state.message = String::new();
                    return Some(Action::MigrateLocal(source_href, target_href));
                }
            }
            _ => {}
        },
        InputMode::RelationshipBrowsing => match key.code {
            KeyCode::Esc => {
                state.mode = InputMode::Normal;
                state.message = String::new();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = state.relationship_items.len();
                if len > 0 {
                    let current = state.relationship_selection_state.selected().unwrap_or(0);
                    let next = if current >= len - 1 { 0 } else { current + 1 };
                    state.relationship_selection_state.select(Some(next));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = state.relationship_items.len();
                if len > 0 {
                    let current = state.relationship_selection_state.selected().unwrap_or(0);
                    let prev = if current == 0 { len - 1 } else { current - 1 };
                    state.relationship_selection_state.select(Some(prev));
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = state.relationship_selection_state.selected()
                    && let Some((target_uid, _)) = state.relationship_items.get(idx)
                {
                    // Jump to the target task
                    let target_uid = target_uid.clone();

                    // Find which calendar this task belongs to
                    if let Some(href) = state.store.index.get(&target_uid).cloned() {
                        // Clear filters that might hide the task
                        state.active_search_query.clear();
                        state.selected_categories.clear();
                        state.selected_locations.clear();

                        // Switch calendar if needed and unhide
                        if state.active_cal_href.as_ref() != Some(&href) {
                            state.active_cal_href = Some(href.clone());
                            state.hidden_calendars.remove(&href);
                        }

                        state.refresh_filtered_view();

                        // Find and select the target task in the list
                        if let Some(task_idx) = state.tasks.iter().position(|t| t.uid == target_uid)
                        {
                            state.list_state.select(Some(task_idx));
                        }

                        state.mode = InputMode::Normal;
                        state.message = "Jumped to task".to_string();
                    } else {
                        state.message = "Task not found".to_string();
                        state.mode = InputMode::Normal;
                    }
                }
            }
            _ => {}
        },
    }
    None
}
