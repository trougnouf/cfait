// File: ./src/gui/state.rs
// Manages the application state for the GUI (Iced).
use crate::client::ClientManager;
use crate::config::{AppTheme, AccountConfig};
use crate::gui::icon;
use crate::model::{Alarm, CalendarListEntry, Task as TodoTask};
use crate::store::TaskStore;
use crate::system::SystemEvent;
use iced::widget::text_editor;
use std::collections::{HashMap, HashSet};
use strum::IntoEnumIterator;
use tokio::sync::mpsc;

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum AppState {
    #[default]
    Loading,
    Onboarding,
    Active,
    Settings,
    Help,
}

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum SidebarMode {
    #[default]
    Calendars,
    Categories,
    Locations, // NEW
}

#[derive(Debug, Clone, Copy)]
pub enum ResizeDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

pub struct GuiApp {
    pub state: AppState,
    pub store: TaskStore,
    pub tasks: Vec<TodoTask>,
    pub calendars: Vec<CalendarListEntry>,
    pub client: Option<ClientManager>,

    // --- Account State ---
    pub accounts: Vec<AccountConfig>,
    pub editing_account_id: Option<String>, // If Some, we are in "Edit/Create Account" mode

    // Reuse existing `ob_` fields for the form:
    // ob_url, ob_user, ob_pass, ob_insecure
    // Add one new field for the Account Name:
    pub ob_name: String,

    pub tag_aliases: HashMap<String, Vec<String>>,

    // Cached Sidebar Data (computed once, not in view())
    pub cached_categories: Vec<(String, usize)>,
    pub cached_locations: Vec<(String, usize)>,

    // Cache for O(1) parent lookup in view_task_row
    // Map<TaskUID, (ParentTags, ParentLocation)>
    pub parent_attributes_cache: HashMap<String, (HashSet<String>, Option<String>)>,

    // --- Stable ID Cache ---
    // Maps Task UID -> Iced Widget ID. Ensures the View and Update loops use the exact same ID instance.
    pub task_ids: HashMap<String, iced::widget::Id>,

    // UI State
    pub sidebar_mode: SidebarMode,
    pub active_cal_href: Option<String>,
    pub hidden_calendars: HashSet<String>,
    pub disabled_calendars: HashSet<String>,
    pub selected_categories: HashSet<String>,
    pub selected_locations: HashSet<String>,
    pub match_all_categories: bool,
    pub yanked_uid: Option<String>,

    pub hovered_tag_uid: Option<String>,

    // Track selected task for highlighting
    pub selected_uid: Option<String>,

    // Preferences
    pub hide_completed: bool,
    pub hide_fully_completed_tags: bool,
    pub sort_cutoff_months: Option<u32>,
    pub current_theme: AppTheme,

    // Store the resolved random theme for this session
    pub resolved_random_theme: AppTheme,

    // Filter State
    pub filter_min_duration: Option<u32>,
    pub filter_max_duration: Option<u32>,
    pub filter_include_unset_duration: bool,

    // Inputs - Main
    pub input_value: text_editor::Content,
    pub description_value: text_editor::Content,
    pub search_value: String,
    pub editing_uid: Option<String>,
    pub creating_child_of: Option<String>,
    pub expanded_tasks: HashSet<String>,
    pub unsynced_changes: bool,

    // Computed State (Persisted for view borrowing)
    pub current_placeholder: String,

    // UI Visuals
    pub location_tab_icon: char,
    pub random_icon: char, // NEW

    // Inputs - Settings (Aliases)
    pub alias_input_key: String,
    pub alias_input_values: String,

    // System
    pub loading: bool,
    pub error_msg: Option<String>,

    // Onboarding / Config
    pub ob_url: String,
    pub ob_user: String,
    pub ob_pass: String,
    pub ob_default_cal: Option<String>,
    pub ob_sort_months_input: String,
    pub ob_insecure: bool,
    /// If true, the config file exists but is invalid. We must block overwrites.
    pub config_was_corrupted: bool,

    // Local Calendar Management
    pub local_cals_editing: Vec<CalendarListEntry>,
    pub color_picker_active_href: Option<String>,
    pub temp_color: iced::Color,
    pub scrollable_id: iced::widget::Id,
    pub sidebar_scrollable_id: iced::widget::Id,

    // Window Resizing State
    pub resize_direction: Option<ResizeDirection>,
    pub current_window_size: iced::Size,
    pub ob_urgent_days_input: String,
    pub ob_urgent_prio_input: String,
    pub ob_default_priority_input: String,
    pub ob_start_grace_input: String,
    pub urgent_days: u32,
    pub urgent_prio: u8,
    pub default_priority: u8,
    pub start_grace_period_days: u32,
    pub alarm_tx: Option<mpsc::Sender<SystemEvent>>, // Send tasks to actor
    pub ringing_tasks: Vec<(TodoTask, Alarm)>,       // Stack of firing alarms

    // Snooze Custom Input
    pub snooze_custom_input: String,

    // ICS Import Dialog State
    pub ics_import_dialog_open: bool,
    pub ics_import_file_path: Option<String>,
    pub ics_import_content: Option<String>,
    pub ics_import_selected_calendar: Option<String>,
    pub ics_import_task_count: Option<usize>,

    // Double click tracking
    pub last_click: Option<(std::time::Instant, String)>, // Added

    // Config cache (New fields)
    pub auto_reminders: bool,
    pub default_reminder_time: String,
    pub snooze_short_mins: u32,
    pub snooze_long_mins: u32,
    pub create_events_for_tasks: bool,
    pub delete_events_on_completion: bool,
    pub deleting_events: bool,

    // Settings input buffers for duration strings
    pub ob_snooze_short_input: String,
    pub ob_snooze_long_input: String,

    // ADDED: Force Server-Side Decorations
    pub force_ssd: bool,
}

impl Default for GuiApp {
    fn default() -> Self {
        // Randomize Location Icon
        let loc_icons = [
            icon::LOCATION,
            icon::LOCATION,
            icon::LOCATION,
            icon::EARTH_ASIA,
            icon::EARTH_AMERICAS,
            icon::EARTH_AFRICA,
            icon::EARTH_GENERIC,
            icon::PLANET,
            icon::GALAXY,
            icon::ISLAND,
            icon::COMPASS,
            icon::MOUNTAINS,
            icon::GLOBE,
            icon::GLOBEMODEL,
            icon::MOON,
        ];

        let mut rng = fastrand::Rng::new();

        let location_tab_icon = loc_icons[rng.usize(..loc_icons.len())];

        // Pick initial random icon for the random-jump button
        let random_icon =
            crate::gui::icon::RANDOM_ICONS[rng.usize(..crate::gui::icon::RANDOM_ICONS.len())];

        // Select a random theme (excluding Random itself)
        let themes: Vec<AppTheme> = AppTheme::iter()
            .filter(|&t| t != AppTheme::Random)
            .collect();
        let resolved_random_theme = if !themes.is_empty() {
            themes[rng.usize(..themes.len())]
        } else {
            // Fallback if the themes list is somehow empty
            AppTheme::RustyDark
        };

        Self {
            state: AppState::Loading,
            store: TaskStore::new(),
            tasks: vec![],
            calendars: vec![],
            client: None,
            // --- Account State ---
            accounts: Vec::new(),
            editing_account_id: None,
            ob_name: String::new(),
            tag_aliases: HashMap::new(),

            cached_categories: Vec::new(),
            cached_locations: Vec::new(),

            // Initialize:
            parent_attributes_cache: HashMap::new(),
            task_ids: HashMap::new(), // Init new field

            sidebar_mode: SidebarMode::Calendars,
            active_cal_href: None,
            hidden_calendars: HashSet::new(),
            disabled_calendars: HashSet::new(),
            selected_categories: HashSet::new(),
            selected_locations: HashSet::new(),
            match_all_categories: false,
            yanked_uid: None,
            selected_uid: None,

            hovered_tag_uid: None,

            hide_completed: false,
            hide_fully_completed_tags: true,
            sort_cutoff_months: Some(2),
            ob_sort_months_input: "2".to_string(),
            current_theme: AppTheme::default(),
            resolved_random_theme,

            filter_min_duration: None,
            filter_max_duration: None,
            filter_include_unset_duration: true,

            input_value: text_editor::Content::new(),
            description_value: text_editor::Content::new(),
            search_value: String::new(),
            editing_uid: None,
            creating_child_of: None,
            expanded_tasks: HashSet::new(),
            unsynced_changes: false,

            current_placeholder: "Add a task...".to_string(),

            location_tab_icon, // Add this
            random_icon,       // NEW
            alias_input_key: String::new(),
            alias_input_values: String::new(),

            loading: true,
            error_msg: None,
            ob_url: String::new(),
            ob_user: String::new(),
            ob_pass: String::new(),
            ob_default_cal: None,
            ob_insecure: false,
            config_was_corrupted: false,

            local_cals_editing: vec![],
            color_picker_active_href: None,
            temp_color: iced::Color::WHITE,
            scrollable_id: iced::widget::Id::unique(),
            sidebar_scrollable_id: iced::widget::Id::unique(),

            resize_direction: None,
            current_window_size: iced::Size::new(800.0, 600.0),
            ob_urgent_days_input: "1".to_string(),
            ob_urgent_prio_input: "1".to_string(),
            ob_default_priority_input: "5".to_string(),
            ob_start_grace_input: "1".to_string(),
            urgent_days: 1,
            urgent_prio: 1,
            default_priority: 5,
            start_grace_period_days: 1,
            alarm_tx: None,
            ringing_tasks: Vec::new(),
            snooze_custom_input: String::new(),

            // Default config values
            auto_reminders: true,
            default_reminder_time: "09:00".to_string(),
            snooze_short_mins: 60,
            snooze_long_mins: 1440,
            create_events_for_tasks: false,
            delete_events_on_completion: false,
            deleting_events: false,
            ob_snooze_short_input: "1h".to_string(),
            ob_snooze_long_input: "1d".to_string(),

            // ADDED: Force Server-Side Decorations
            force_ssd: false,

            // Double click tracking
            last_click: None, // Added

            // ICS Import Dialog
            ics_import_dialog_open: false,
            ics_import_file_path: None,
            ics_import_content: None,
            ics_import_selected_calendar: None,
            ics_import_task_count: None,
        }
    }
}
