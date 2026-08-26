// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/tui/state.rs
// Manages the application state for the TUI.
use crate::context::AppContext;
use crate::model::{AppIntent, CalendarListEntry, Task};
use crate::store::{FilterOptions, TaskListItem, TaskStore};
use crate::system::SystemEvent;
use crate::tui::action::SidebarMode;
use fastrand;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc; // Add import

const GOAL_ICONS: &[char] = &[
    '\u{ebf8}',
    '\u{f04fe}',
    '\u{f0a77}',
    '\u{f4de}',
    '\u{f11e}',
    '\u{f023c}',
    '\u{f140}',
    '\u{f05dd}',
    '\u{f08c9}',
    '\u{f295}',
    '\u{f1a04}',
    '\u{f029a}',
    '\u{f0873}',
    '\u{f0874}',
    '\u{f0875}',
    '\u{f0995}',
];

const JOURNAL_ICONS: &[char] = &[
    '\u{f02d}',
    '\u{ede2}',
    '\u{f14f7}',
    '\u{f05da}',
    '\u{f125f}',
    '\u{edf7}',
    '\u{ee34}',
    '\u{f06d3}',
    '\u{e7d8}',
    '\u{f040}',
    '\u{f064f}',
    '\u{f0776}',
];

#[derive(PartialEq, Clone, Copy)]
pub enum Focus {
    Sidebar,
    Main,
}

#[derive(PartialEq, Clone)]
pub enum InputMode {
    Normal,
    Creating,
    Searching,
    Editing,
    EditingDescription,
    EditingTree(String),
    ViewingDetails,
    Moving,
    SelectingExportSource,
    Exporting,
    Snoozing,
    RelationshipBrowsing,
    AddingSession,
    ManagingSessions,
    EditingSession(String, usize),
    JumpingToDate,
    ActionMenu,
    Help(crate::help::HelpTab),
}

pub struct AppState {
    // Data
    pub ctx: Arc<dyn AppContext>,
    pub store: TaskStore,
    pub tasks: Vec<TaskListItem>,
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
    pub local_mode_enabled: bool,
    pub selected_categories: HashSet<String>,
    pub selected_locations: HashSet<String>, // NEW
    pub match_all_categories: bool,
    pub hide_completed: bool,
    pub hide_fully_completed_tags: bool,
    pub hide_aliases_in_sidebar: bool,
    pub show_inline_descriptions: bool,
    pub strikethrough_completed: bool,
    pub show_priority_numbers: bool,
    pub sort_cutoff_days: Option<u32>,
    pub sort_standard_by_priority: bool,
    pub paused_sort_behavior: crate::config::PausedSortBehavior,
    pub sort_tiebreak_recent: bool,

    pub theme: crate::config::AppTheme,

    pub quick_filter_term: String,
    pub quick_filter_icon: String,
    pub show_quick_filter: bool,
    pub show_calendars_tab: bool,
    pub show_tags_tab: bool,
    pub show_locations_tab: bool,
    pub show_goals_tab: bool,
    pub show_journal_tab: bool,
    pub goal_icon: char,
    pub journal_icon: char,

    // Cached sidebar values (derived from the last filter result)
    pub cached_categories: Vec<crate::store::AggregateItem>,
    pub cached_locations: Vec<crate::store::AggregateItem>,

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
    pub focused_task_uid: Option<String>,
    pub edit_scroll_offset: u16,
    pub edit_scroll_x: u16,
    pub details_scroll: u16,
    pub editing_uid: Option<String>,
    pub editing_tree_uid: Option<String>,
    pub move_selection_state: ListState,
    pub move_targets: Vec<CalendarListEntry>,
    pub moving_tree: bool,
    pub moving_task_uid: Option<String>,
    pub export_source_selection_state: ListState,
    pub export_source_calendars: Vec<CalendarListEntry>,
    pub export_selection_state: ListState,
    pub export_targets: Vec<CalendarListEntry>,

    pub yanked_uid: Option<String>,
    pub yank_lock_active: bool,
    pub creating_child_of: Option<String>,
    pub creating_with_desc: bool,
    pub new_task_title: String,
    pub tag_aliases: HashMap<String, Vec<String>>,

    // Relationship browsing state
    pub relationship_items: Vec<(String, String, String)>, // (uid, display_name, rel_type)
    pub relationship_selection_state: ListState,

    // Session management state (for quick-log and session editor)
    pub session_items: Vec<(usize, String)>,
    pub session_selection_state: ListState,

    // Action menu state
    pub available_actions: Vec<crate::config::TaskAction>,
    pub action_menu_items: Vec<crate::config::TaskAction>,
    pub action_selection_state: ListState,
    pub action_filter: String,

    // Track unsynced status
    pub unsynced_changes: bool,
    pub alarm_actor_tx: Option<mpsc::Sender<SystemEvent>>,
    pub active_alarm: Option<(Task, String)>, // (Task, AlarmUID) to render popup

    // Expanded Done Groups (keys are parent UIDs; empty string for root group)
    pub expanded_done_groups: HashSet<String>,
    pub expanded_tags: HashSet<String>,
    pub expanded_locations: HashSet<String>,
    pub search_collapsed_tasks: HashSet<String>,
    pub journal_date: chrono::NaiveDate,
    pub first_day_of_week: crate::config::FirstDayOfWeek,
    pub journal_editing_uid: Option<String>,
    pub cached_journal_pages: Vec<(String, String, usize)>,
    pub goals: HashMap<String, crate::config::Goal>,
    pub cached_goals_progress: HashMap<String, (u32, Vec<f32>)>,
    pub cached_task_goals: Vec<(String, String, crate::config::Goal, u32, Vec<f32>)>,
    pub needs_redraw: bool,

    pub undo_stack: Vec<crate::journal::UndoRecord>,
    pub redo_stack: Vec<crate::journal::UndoRecord>,

    pub text_undo_stack: Vec<String>,
    pub text_redo_stack: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        // Backwards compatible default for codepaths that still call `AppState::default()`.
        // This uses the platform default context; prefer constructing with an explicit context.
        Self::new()
    }
}

impl AppState {
    /// Creates a new AppState with the default platform context.
    pub fn new() -> Self {
        // Provide a convenient no-arg constructor that uses the platform default context.
        // Call sites that need test isolation or custom roots should call `new_with_ctx`.
        let ctx = Arc::new(crate::context::StandardContext::new(None));
        Self::new_with_ctx(ctx)
    }

    /// Creates a new AppState with an explicit AppContext.
    pub fn new_with_ctx(ctx: Arc<dyn AppContext>) -> Self {
        let mut l_state = ListState::default();
        l_state.select(Some(0));
        let mut c_state = ListState::default();
        c_state.select(Some(0));

        let config = crate::config::Config::load(ctx.as_ref()).unwrap_or_default();

        Self {
            ctx: ctx.clone(),
            store: TaskStore::new(ctx.clone()),
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
            local_mode_enabled: true,
            selected_categories: HashSet::new(),
            selected_locations: HashSet::new(), // Init
            match_all_categories: true,
            hide_completed: false,
            strikethrough_completed: false,
            hide_fully_completed_tags: false,
            hide_aliases_in_sidebar: true,
            show_inline_descriptions: config.show_inline_descriptions,
            show_priority_numbers: true,
            quick_filter_term: "is:ready".to_string(),
            quick_filter_icon: "f0fa9".to_string(),
            show_quick_filter: true,
            show_calendars_tab: config.show_calendars_tab,
            show_tags_tab: config.show_tags_tab,
            show_locations_tab: config.show_locations_tab,
            show_goals_tab: config.show_goals_tab,
            show_journal_tab: config.show_journal_tab,
            goal_icon: GOAL_ICONS[fastrand::usize(..GOAL_ICONS.len())],
            journal_icon: JOURNAL_ICONS[fastrand::usize(..JOURNAL_ICONS.len())],
            sort_cutoff_days: Some(30),
            sort_standard_by_priority: false,
            paused_sort_behavior: crate::config::PausedSortBehavior::default(),
            sort_tiebreak_recent: config.sort_tiebreak_recent,
            theme: crate::config::AppTheme::default(),
            // Initialize sidebar caches as empty; they will be populated by refresh_filtered_view()
            cached_categories: Vec::new(),
            cached_locations: Vec::new(),
            urgent_days: 1,
            urgent_prio: 1,
            default_priority: 5,
            start_grace_period_days: 1,

            snooze_short_mins: 60,
            snooze_long_mins: 1440,

            input_buffer: String::new(),
            active_search_query: String::new(),
            cursor_position: 0,
            focused_task_uid: None,
            edit_scroll_offset: 0,
            edit_scroll_x: 0,
            details_scroll: 0,
            editing_uid: None,
            editing_tree_uid: None,
            move_selection_state: ListState::default(),
            move_targets: Vec::new(),
            moving_tree: false,
            moving_task_uid: None,
            yanked_uid: None,
            yank_lock_active: false,
            creating_child_of: None,
            creating_with_desc: false,
            new_task_title: String::new(),

            tag_aliases: HashMap::new(),
            export_source_selection_state: ListState::default(),
            export_source_calendars: Vec::new(),
            export_selection_state: ListState::default(),
            export_targets: Vec::new(),

            relationship_items: Vec::new(),
            relationship_selection_state: ListState::default(),
            session_items: Vec::new(),
            session_selection_state: ListState::default(),

            available_actions: Vec::new(),
            action_menu_items: Vec::new(),
            action_selection_state: ListState::default(),
            action_filter: String::new(),

            unsynced_changes: false, // Default false
            alarm_actor_tx: None,
            active_alarm: None,

            // Track expanded completed groups (keys are parent UIDs, empty string for roots)
            expanded_done_groups: HashSet::new(),
            expanded_tags: HashSet::new(),
            expanded_locations: HashSet::new(),
            search_collapsed_tasks: HashSet::new(),
            journal_date: chrono::Local::now().date_naive(),
            first_day_of_week: config.first_day_of_week,
            journal_editing_uid: None,
            cached_journal_pages: Vec::new(),
            goals: config.goals,
            cached_goals_progress: HashMap::new(),
            cached_task_goals: Vec::new(),
            needs_redraw: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            text_undo_stack: Vec::new(),
            text_redo_stack: Vec::new(),
        }
    }

    pub fn get_filtered_calendars(&self) -> Vec<&CalendarListEntry> {
        self.calendars
            .iter()
            .filter(|c| self.local_mode_enabled || !c.href.starts_with("local://"))
            .filter(|c| !self.disabled_calendars.contains(&c.href))
            .filter(|c| {
                if c.href == crate::storage::LOCAL_TRASH_HREF || c.href == "local://recovery" {
                    self.store
                        .calendars
                        .get(&c.href)
                        .is_some_and(|map| !map.is_empty())
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn sort_calendars(&mut self) {
        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
        let order = config.collection_order.clone();
        let sort_by_size = config.sort_collections_by_size;
        let mut sizes = std::collections::HashMap::new();
        if sort_by_size {
            for cal in &self.calendars {
                let count = self
                    .store
                    .calendars
                    .get(&cal.href)
                    .map(|m| m.len())
                    .unwrap_or(0);
                sizes.insert(cal.href.clone(), count);
            }
        }
        self.calendars.sort_by(|a, b| {
            if sort_by_size {
                let count_a = sizes.get(&a.href).unwrap_or(&0);
                let count_b = sizes.get(&b.href).unwrap_or(&0);
                crate::model::compare_calendars_with_size(
                    &a.href, &a.name, *count_a, &b.href, &b.name, *count_b, &order,
                )
            } else {
                crate::model::compare_calendars(&a.href, &a.name, &b.href, &b.name, &order)
            }
        });
    }

    pub fn refresh_filtered_view(&mut self) {
        self.sort_calendars();

        let search_term = if self.mode == InputMode::Searching {
            &self.input_buffer
        } else {
            &self.active_search_query
        };

        let cutoff_date = if let Some(days) = self.sort_cutoff_days {
            let now = chrono::Utc::now();
            Some(now + chrono::Duration::days(days as i64))
        } else {
            None
        };

        let mut effective_hidden = self.hidden_calendars.clone();
        effective_hidden.extend(self.disabled_calendars.clone());
        if !self.local_mode_enabled {
            for href in self.store.calendars.keys() {
                if href.starts_with("local://") {
                    effective_hidden.insert(href.clone());
                }
            }
        }

        // Load config to get limits
        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();

        // Use the store.filter() that returns a FilterResult so we can populate
        // both the task list and the sidebar caches for categories/locations.
        let filter_res = self.store.filter(FilterOptions {
            active_cal_href: None, // Logic handled by hidden_calendars
            hidden_calendars: &effective_hidden,
            selected_categories: &self.selected_categories,
            selected_locations: &self.selected_locations,
            match_all_categories: self.match_all_categories,
            search_term,
            hide_completed_global: self.hide_completed,
            hide_fully_completed_tags: self.hide_fully_completed_tags,
            hide_aliases_in_sidebar: self.hide_aliases_in_sidebar,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: self.urgent_days,
            urgent_prio: self.urgent_prio,
            default_priority: self.default_priority,
            start_grace_period_days: self.start_grace_period_days,
            sort_standard_by_priority: self.sort_standard_by_priority,
            sort_preset: config.sort_preset,
            paused_sort_behavior: self.paused_sort_behavior,
            sort_tiebreak_recent: self.sort_tiebreak_recent,
            expanded_done_groups: &self.expanded_done_groups,
            expanded_tags: &self.expanded_tags,
            expanded_locations: &self.expanded_locations,
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
            tag_aliases: &config.tag_aliases,
            search_collapsed_tasks: &self.search_collapsed_tasks,
            focused_task_uid: self.focused_task_uid.as_deref(),
        });

        self.tasks = filter_res.items;
        self.cached_categories = filter_res.categories;
        self.cached_locations = filter_res.locations;

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

        let mut goals_progress = HashMap::new();
        for (key, goal) in &self.goals {
            let prog = self.store.calculate_goal_progress(key, goal);
            let history = self.store.calculate_goal_history(key, goal, 7);
            goals_progress.insert(key.clone(), (prog, history));
        }
        self.cached_goals_progress = goals_progress;

        let mut task_goals = Vec::new();
        if config.show_task_goals_in_sidebar {
            for (href, map) in self.store.calendars.iter() {
                if self.hidden_calendars.contains(href)
                    || self.disabled_calendars.contains(href)
                    || href == crate::storage::LOCAL_TRASH_HREF
                    || href == "local://recovery"
                {
                    continue;
                }
                for t in map.values() {
                    if t.unmapped_properties
                        .iter()
                        .any(|p| p.key == "X-CFAIT-HISTORY-OF")
                    {
                        continue;
                    }
                    if let Some(goal) = &t.goal {
                        let progress = self
                            .store
                            .calculate_goal_progress(&format!("task:{}", t.uid), goal);
                        let history =
                            self.store
                                .calculate_goal_history(&format!("task:{}", t.uid), goal, 7);
                        task_goals.push((
                            t.uid.clone(),
                            t.summary.clone(),
                            goal.clone(),
                            progress,
                            history,
                        ));
                    }
                }
            }
        }
        task_goals.sort_by(|a, b| a.1.cmp(&b.1));
        self.cached_task_goals = task_goals;

        let mut pages = Vec::new();
        for (href, map) in self.store.calendars.iter() {
            if effective_hidden.contains(href)
                || href == crate::storage::LOCAL_TRASH_HREF
                || href == "local://recovery"
            {
                continue;
            }
            for t in map.values() {
                if t.is_journal && t.dtstart.is_none() {
                    pages.push(t);
                }
            }
        }
        pages.sort_by(|a, b| a.summary.cmp(&b.summary));

        let mut children_map: std::collections::HashMap<String, Vec<&crate::model::Task>> =
            std::collections::HashMap::new();
        let mut roots = Vec::new();
        let page_uids: std::collections::HashSet<String> =
            pages.iter().map(|p| p.uid.clone()).collect();

        for p in &pages {
            if let Some(parent) = &p.parent_uid
                && page_uids.contains(parent)
            {
                children_map.entry(parent.clone()).or_default().push(p);
                continue;
            }
            roots.push(p);
        }

        let mut flat_pages = Vec::new();
        fn flatten_pages(
            node: &crate::model::Task,
            children_map: &std::collections::HashMap<String, Vec<&crate::model::Task>>,
            depth: usize,
            out: &mut Vec<(String, String, usize)>,
        ) {
            let title = if node.summary.is_empty() {
                rust_i18n::t!("untitled_page", default = "Untitled page").to_string()
            } else {
                node.summary.clone()
            };
            out.push((node.uid.clone(), title, depth));
            if let Some(children) = children_map.get(&node.uid) {
                for child in children {
                    flatten_pages(child, children_map, depth + 1, out);
                }
            }
        }

        for root in roots {
            flatten_pages(root, &children_map, 0, &mut flat_pages);
        }
        self.cached_journal_pages = flat_pages;
    }

    pub fn get_selected_task(&self) -> Option<&Task> {
        if self.sidebar_mode == SidebarMode::Journal && self.active_focus == Focus::Main {
            return None;
        }
        if let Some(idx) = self.list_state.selected() {
            match &self.tasks.get(idx) {
                Some(TaskListItem::Task(task)) => Some(task),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the task at a specific index, returning None for control items
    pub fn get_task_at_index(&self, idx: usize) -> Option<&Task> {
        match &self.tasks.get(idx) {
            Some(TaskListItem::Task(task)) => Some(task),
            _ => None,
        }
    }

    /// Find the index of a task by UID, ignoring control items
    pub fn find_task_index_by_uid(&self, uid: &str) -> Option<usize> {
        self.tasks.iter().position(|item| {
            if let TaskListItem::Task(task) = item {
                task.uid == uid
            } else {
                false
            }
        })
    }

    /// Handles Intents directly to keep state synced without a controller
    pub fn apply_task_intent(
        &mut self,
        intent: &AppIntent,
        config: &crate::config::Config,
    ) -> Vec<crate::journal::Action> {
        if let AppIntent::FocusTaskTree { uid } = intent {
            self.focused_task_uid = uid.clone();
            return Vec::new();
        }
        let (forward, reverse, desc, primary_uid) = self.store.apply_task_intent(intent, config);
        if !forward.is_empty() {
            self.undo_stack.push(crate::journal::UndoRecord {
                description: desc,
                primary_uid,
                forward: forward.clone(),
                reverse,
            });
            self.redo_stack.clear();
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
        }
        forward
    }

    /// Get all real tasks (excluding control items)
    pub fn get_all_real_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter_map(|item| {
                if let TaskListItem::Task(task) = item {
                    Some(task.as_ref())
                } else {
                    None
                }
            })
            .collect()
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
        let old = self.input_buffer.clone();
        // Safe insertion for UTF-8 strings
        let byte_index = self
            .input_buffer
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_position)
            .unwrap_or(self.input_buffer.len());

        self.input_buffer.insert(byte_index, new_char);
        self.move_cursor_right();
        if old != self.input_buffer {
            self.text_undo_stack.push(old);
            self.text_redo_stack.clear();
            if self.text_undo_stack.len() > 50 {
                self.text_undo_stack.remove(0);
            }
        }
    }
    pub fn delete_char(&mut self) {
        if self.cursor_position != 0 {
            let old = self.input_buffer.clone();
            let current_index = self.cursor_position;
            let before = self.input_buffer.chars().take(current_index - 1);
            let after = self.input_buffer.chars().skip(current_index);
            self.input_buffer = before.chain(after).collect();
            self.move_cursor_left();
            if old != self.input_buffer {
                self.text_undo_stack.push(old);
                self.text_redo_stack.clear();
                if self.text_undo_stack.len() > 50 {
                    self.text_undo_stack.remove(0);
                }
            }
        }
    }
    pub fn reset_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.text_undo_stack.clear();
        self.text_redo_stack.clear();
    }
    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input_buffer.chars().count())
    }

    // --- HELPER FOR SIDEBAR LENGTH ---
    fn get_sidebar_len(&self) -> usize {
        match self.sidebar_mode {
            SidebarMode::Calendars => self.get_filtered_calendars().len(),
            SidebarMode::Categories => self.cached_categories.len(),
            SidebarMode::Locations => self.cached_locations.len(),
            SidebarMode::Journal => self.cached_journal_pages.len(),
            SidebarMode::Goals => self.goals.len(),
        }
    }

    // --- NAVIGATION ---
    pub fn next(&mut self) {
        match self.active_focus {
            Focus::Main => {
                if self.sidebar_mode == SidebarMode::Journal {
                    self.details_scroll = self.details_scroll.saturating_add(1);
                    return;
                }
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
                self.details_scroll = 0;
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

                if self.sidebar_mode == SidebarMode::Journal
                    && let Some(page) = self.cached_journal_pages.get(i)
                {
                    let uid = page.0.clone();
                    self.journal_editing_uid = Some(uid.clone());
                    if let Some(task) = self.store.get_task_ref(&uid) {
                        self.active_cal_href = Some(task.calendar_href.clone());
                    }
                }
            }
        }
    }
    pub fn previous(&mut self) {
        match self.active_focus {
            Focus::Main => {
                if self.sidebar_mode == SidebarMode::Journal {
                    self.details_scroll = self.details_scroll.saturating_sub(1);
                    return;
                }
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
                self.details_scroll = 0;
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

                if self.sidebar_mode == SidebarMode::Journal
                    && let Some(page) = self.cached_journal_pages.get(i)
                {
                    let uid = page.0.clone();
                    self.journal_editing_uid = Some(uid.clone());
                    if let Some(task) = self.store.get_task_ref(&uid) {
                        self.active_cal_href = Some(task.calendar_href.clone());
                    }
                }
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
                    self.details_scroll = 0;
                }
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len > 0 {
                    let current = self.cal_state.selected().unwrap_or(0);
                    let i = (current + step).min(len - 1);
                    self.cal_state.select(Some(i));

                    if self.sidebar_mode == SidebarMode::Journal
                        && let Some(page) = self.cached_journal_pages.get(i)
                    {
                        let uid = page.0.clone();
                        self.journal_editing_uid = Some(uid.clone());
                        if let Some(task) = self.store.get_task_ref(&uid) {
                            self.active_cal_href = Some(task.calendar_href.clone());
                        }
                    }
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
                    self.details_scroll = 0;
                }
            }
            Focus::Sidebar => {
                let len = self.get_sidebar_len();
                if len > 0 {
                    let current = self.cal_state.selected().unwrap_or(0);
                    let i = current.saturating_sub(step);
                    self.cal_state.select(Some(i));

                    if self.sidebar_mode == SidebarMode::Journal
                        && let Some(page) = self.cached_journal_pages.get(i)
                    {
                        let uid = page.0.clone();
                        self.journal_editing_uid = Some(uid.clone());
                        if let Some(task) = self.store.get_task_ref(&uid) {
                            self.active_cal_href = Some(task.calendar_href.clone());
                        }
                    }
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
        state.tasks = vec![
            TaskListItem::Task(Box::new(dummy_task())),
            TaskListItem::Task(Box::new(dummy_task())),
            TaskListItem::Task(Box::new(dummy_task())),
        ];

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
        state.tasks = vec![
            TaskListItem::Task(Box::new(dummy_task())),
            TaskListItem::Task(Box::new(dummy_task())),
            TaskListItem::Task(Box::new(dummy_task())),
        ];

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
