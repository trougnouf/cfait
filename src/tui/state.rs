// Manages the application state for the TUI.
use crate::model::{CalendarListEntry, Task};
use crate::store::{FilterOptions, TaskStore};
use crate::system::SystemEvent;
use crate::tui::action::SidebarMode;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc; // Add import

#[derive(PartialEq, Clone, Copy)]
pub enum Focus {
    Sidebar,
    Main,
}

#[derive(PartialEq, Clone, Copy)]
pub enum InputMode {
    Normal,
    Creating,
    Searching,
    Editing,
    EditingDescription,
    Moving,
    SelectingExportSource,
    Exporting,
    Snoozing,
    RelationshipBrowsing, // NEW: For navigating through task relationships
}

pub struct AppState {
    // Data
    pub store: TaskStore,
    pub tasks: Vec<Task>,
    pub calendars: Vec<CalendarListEntry>,

    // UI State
    pub list_state: ListState,
    pub cal_state: ListState,
    pub active_focus: Focus,
    pub mode: InputMode,
    pub message: String,
    pub loading: bool,

    // Filter State
    pub sidebar_mode: SidebarMode,
    pub active_cal_href: Option<String>,
    pub hidden_calendars: HashSet<String>,
    pub disabled_calendars: HashSet<String>,
    pub selected_categories: HashSet<String>,
    pub selected_locations: HashSet<String>, // NEW
    pub match_all_categories: bool,
    pub hide_completed: bool,
    pub hide_fully_completed_tags: bool,
    pub sort_cutoff_months: Option<u32>,

    pub urgent_days: u32,
    pub urgent_prio: u8,
    pub default_priority: u8,
    pub start_grace_period_days: u32,

    // Snooze configuration
    pub snooze_short_mins: u32,
    pub snooze_long_mins: u32,

    // Input Buffers
    pub input_buffer: String,
    pub active_search_query: String, // Holds the committed search term
    pub cursor_position: usize,
    pub editing_index: Option<usize>,
    pub move_selection_state: ListState,
    pub move_targets: Vec<CalendarListEntry>,
    pub export_source_selection_state: ListState,
    pub export_source_calendars: Vec<CalendarListEntry>,
    pub export_selection_state: ListState,
    pub export_targets: Vec<CalendarListEntry>,

    pub yanked_uid: Option<String>,
    pub creating_child_of: Option<String>,
    pub show_full_help: bool,
    pub tag_aliases: HashMap<String, Vec<String>>,

    // Relationship browsing state
    pub relationship_items: Vec<(String, String)>, // (uid, display_name)
    pub relationship_selection_state: ListState,

    // Track unsynced status
    pub unsynced_changes: bool,
    pub alarm_actor_tx: Option<mpsc::Sender<SystemEvent>>,
    pub active_alarm: Option<(Task, String)>, // (Task, AlarmUID) to render popup
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let mut l_state = ListState::default();
        l_state.select(Some(0));
        let mut c_state = ListState::default();
        c_state.select(Some(0));

        Self {
            store: TaskStore::new(),
            tasks: vec![],
            calendars: vec![],
            list_state: l_state,
            cal_state: c_state,
            active_focus: Focus::Main,
            mode: InputMode::Normal,
            message: "Loading...".to_string(),
            loading: true,

            sidebar_mode: SidebarMode::Calendars,
            active_cal_href: None,
            hidden_calendars: HashSet::new(),
            disabled_calendars: HashSet::new(),
            selected_categories: HashSet::new(),
            selected_locations: HashSet::new(), // Init
            match_all_categories: false,
            hide_completed: false,
            hide_fully_completed_tags: false,
            sort_cutoff_months: Some(2),
            urgent_days: 1,
            urgent_prio: 1,
            default_priority: 5,
            start_grace_period_days: 1,

            snooze_short_mins: 60,
            snooze_long_mins: 1440,

            input_buffer: String::new(),
            active_search_query: String::new(),
            cursor_position: 0,
            editing_index: None,
            move_selection_state: ListState::default(),
            move_targets: Vec::new(),
            yanked_uid: None,
            creating_child_of: None,
            show_full_help: false,

            tag_aliases: HashMap::new(),
            export_source_selection_state: ListState::default(),
            export_source_calendars: Vec::new(),
            export_selection_state: ListState::default(),
            export_targets: Vec::new(),

            relationship_items: Vec::new(),
            relationship_selection_state: ListState::default(),

            unsynced_changes: false, // Default false
            alarm_actor_tx: None,
            active_alarm: None,
        }
    }

    pub fn get_filtered_calendars(&self) -> Vec<&CalendarListEntry> {
        self.calendars
            .iter()
            .filter(|c| !self.disabled_calendars.contains(&c.href))
            .collect()
    }

    pub fn refresh_filtered_view(&mut self) {
        let search_term = if self.mode == InputMode::Searching {
            &self.input_buffer
        } else {
            &self.active_search_query
        };

        let cutoff_date = if let Some(months) = self.sort_cutoff_months {
            let now = chrono::Utc::now();
            let days = months as i64 * 30;
            Some(now + chrono::Duration::days(days))
        } else {
            None
        };

        let mut effective_hidden = self.hidden_calendars.clone();
        effective_hidden.extend(self.disabled_calendars.clone());

        self.tasks = self.store.filter(FilterOptions {
            active_cal_href: None, // Logic handled by hidden_calendars
            selected_categories: &self.selected_categories,
            selected_locations: &self.selected_locations, // Pass locations
            match_all_categories: self.match_all_categories,
            hidden_calendars: &effective_hidden,
            search_term,
            hide_completed_global: self.hide_completed,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: self.urgent_days,
            urgent_prio: self.urgent_prio,
            default_priority: self.default_priority,
            start_grace_period_days: self.start_grace_period_days,
        });

        let len = self.tasks.len();
        if len == 0 {
            self.list_state.select(None);
        } else {
            let current = self.list_state.selected().unwrap_or(0);
            if current >= len {
                self.list_state.select(Some(len - 1)); // Clamp
            } else {
                self.list_state.select(Some(current));
            }
        }
    }

    pub fn get_selected_task(&self) -> Option<&Task> {
        if let Some(idx) = self.list_state.selected() {
            self.tasks.get(idx)
        } else {
            None
        }
    }

    // --- INPUT HELPERS ---
    pub fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.cursor_position.saturating_sub(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_left);
    }
    pub fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.cursor_position.saturating_add(1);
        self.cursor_position = self.clamp_cursor(cursor_moved_right);
    }
    pub fn enter_char(&mut self, new_char: char) {
        // Safe insertion for UTF-8 strings
        let byte_index = self
            .input_buffer
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_position)
            .unwrap_or(self.input_buffer.len());

        self.input_buffer.insert(byte_index, new_char);
        self.move_cursor_right();
    }
    pub fn delete_char(&mut self) {
        if self.cursor_position != 0 {
            let current_index = self.cursor_position;
            let before = self.input_buffer.chars().take(current_index - 1);
            let after = self.input_buffer.chars().skip(current_index);
            self.input_buffer = before.chain(after).collect();
            self.move_cursor_left();
        }
    }
    pub fn reset_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_position = 0;
    }
    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input_buffer.chars().count())
    }

    // --- HELPER FOR SIDEBAR LENGTH ---
    fn get_sidebar_len(&self) -> usize {
        match self.sidebar_mode {
            SidebarMode::Calendars => self
                .calendars
                .iter()
                .filter(|c| !self.disabled_calendars.contains(&c.href))
                .count(),
            SidebarMode::Categories => self
                .store
                .get_all_categories(
                    self.hide_completed,
                    self.hide_fully_completed_tags,
                    &self.selected_categories,
                    &self.hidden_calendars,
                )
                .len(),
            SidebarMode::Locations => self
                .store
                .get_all_locations(self.hide_completed, &self.hidden_calendars)
                .len(), // NEW
        }
    }

    // --- NAVIGATION ---
    pub fn next(&mut self) {
        match self.active_focus {
            Focus::Main => {
                if self.tasks.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i >= self.tasks.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len == 0 {
                    return;
                }
                let i = match self.cal_state.selected() {
                    Some(i) => {
                        if i >= len - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.cal_state.select(Some(i));
            }
        }
    }
    pub fn previous(&mut self) {
        match self.active_focus {
            Focus::Main => {
                if self.tasks.is_empty() {
                    return;
                }
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.tasks.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.list_state.select(Some(i));
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len == 0 {
                    return;
                }
                let i = match self.cal_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            len - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.cal_state.select(Some(i));
            }
        }
    }
    pub fn jump_forward(&mut self, step: usize) {
        match self.active_focus {
            Focus::Main => {
                if !self.tasks.is_empty() {
                    let current = self.list_state.selected().unwrap_or(0);
                    self.list_state
                        .select(Some((current + step).min(self.tasks.len() - 1)));
                }
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len > 0 {
                    let current = self.cal_state.selected().unwrap_or(0);
                    self.cal_state.select(Some((current + step).min(len - 1)));
                }
            }
        }
    }
    pub fn jump_backward(&mut self, step: usize) {
        match self.active_focus {
            Focus::Main => {
                if !self.tasks.is_empty() {
                    let current = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some(current.saturating_sub(step)));
                }
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len > 0 {
                    let current = self.cal_state.selected().unwrap_or(0);
                    self.cal_state.select(Some(current.saturating_sub(step)));
                }
            }
        }
    }
    pub fn toggle_focus(&mut self) {
        self.active_focus = match self.active_focus {
            Focus::Main => Focus::Sidebar,
            Focus::Sidebar => Focus::Main,
        }
    }
    pub fn next_move_target(&mut self) {
        if self.move_targets.is_empty() {
            return;
        }
        let i = match self.move_selection_state.selected() {
            Some(i) => {
                if i >= self.move_targets.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.move_selection_state.select(Some(i));
    }

    pub fn previous_move_target(&mut self) {
        if self.move_targets.is_empty() {
            return;
        }
        let i = match self.move_selection_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.move_targets.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.move_selection_state.select(Some(i));
    }
    pub fn next_export_source(&mut self) {
        if self.export_source_calendars.is_empty() {
            return;
        }
        let i = match self.export_source_selection_state.selected() {
            Some(i) => {
                if i >= self.export_source_calendars.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.export_source_selection_state.select(Some(i));
    }

    pub fn previous_export_source(&mut self) {
        if self.export_source_calendars.is_empty() {
            return;
        }
        let i = match self.export_source_selection_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.export_source_calendars.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.export_source_selection_state.select(Some(i));
    }

    pub fn next_export_target(&mut self) {
        if self.export_targets.is_empty() {
            return;
        }
        let i = match self.export_selection_state.selected() {
            Some(i) => {
                if i >= self.export_targets.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.export_selection_state.select(Some(i));
    }

    pub fn previous_export_target(&mut self) {
        if self.export_targets.is_empty() {
            return;
        }
        let i = match self.export_selection_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.export_targets.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.export_selection_state.select(Some(i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn dummy_task() -> Task {
        Task::new("test", &HashMap::new(), None)
    }

    #[test]
    fn test_navigation_next_wraps() {
        let mut state = AppState::new();
        // Add 3 dummy tasks
        state.tasks = vec![dummy_task(), dummy_task(), dummy_task()];

        // Start at 0
        state.list_state.select(Some(0));

        state.next(); // 1
        assert_eq!(state.list_state.selected(), Some(1));

        state.next(); // 2
        assert_eq!(state.list_state.selected(), Some(2));

        state.next(); // Wrap to 0
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn test_navigation_previous_wraps() {
        let mut state = AppState::new();
        state.tasks = vec![dummy_task(), dummy_task(), dummy_task()];

        state.list_state.select(Some(0));

        state.previous(); // Wrap to last (2)
        assert_eq!(state.list_state.selected(), Some(2));

        state.previous(); // 1
        assert_eq!(state.list_state.selected(), Some(1));
    }

    #[test]
    fn test_navigation_empty_list_safety() {
        let mut state = AppState::new();
        state.tasks = vec![]; // Empty

        // Should not panic
        state.next();
        state.previous();

        // Selection should stay None or safe default, but definitely no panic
    }

    #[test]
    fn test_cursor_clamping() {
        let mut state = AppState::new();
        state.input_buffer = "abc".to_string(); // len 3
        state.cursor_position = 0;

        state.move_cursor_right(); // 1
        state.move_cursor_right(); // 2
        state.move_cursor_right(); // 3 (after 'c')
        state.move_cursor_right(); // Should stay 3

        assert_eq!(state.cursor_position, 3);

        state.move_cursor_left(); // 2
        state.move_cursor_left(); // 1
        state.move_cursor_left(); // 0
        state.move_cursor_left(); // Should stay 0

        assert_eq!(state.cursor_position, 0);
    }
}
