// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/config.rs
// Handles configuration loading, saving, and defaults.
use crate::context::AppContext;
use crate::storage::LocalStorage;
use anyhow::{Error, Result};
use chrono;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use strum::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
#[serde(rename_all = "lowercase")]
pub enum GoalType {
    Count,
    Duration,
}

impl fmt::Display for GoalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalType::Count => write!(f, "Count"),
            GoalType::Duration => write!(f, "Duration"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
#[serde(rename_all = "lowercase")]
pub enum IntervalUnit {
    Days,
    Weeks,
    Months,
    Years,
}

impl fmt::Display for IntervalUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntervalUnit::Days => write!(f, "Days"),
            IntervalUnit::Weeks => write!(f, "Weeks"),
            IntervalUnit::Months => write!(f, "Months"),
            IntervalUnit::Years => write!(f, "Years"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval {
    pub amount: u32,
    pub unit: IntervalUnit,
}

impl Interval {
    pub fn get_period_bounds(&self, now: chrono::DateTime<chrono::Utc>, offset: i32) -> (i64, i64) {
        use chrono::{Datelike, NaiveDate, NaiveTime};
        let local_now = now.with_timezone(&chrono::Local).date_naive();

        let reference_date = match self.unit {
            IntervalUnit::Days => {
                local_now + chrono::Duration::days((offset * self.amount as i32) as i64)
            }
            IntervalUnit::Weeks => {
                local_now + chrono::Duration::days((offset * self.amount as i32 * 7) as i64)
            }
            IntervalUnit::Months => {
                let mut m = local_now.month0() as i32 + offset * self.amount as i32;
                let mut y = local_now.year();
                while m < 0 {
                    m += 12;
                    y -= 1;
                }
                while m > 11 {
                    m -= 12;
                    y += 1;
                }
                NaiveDate::from_ymd_opt(y, (m as u32) + 1, 1).unwrap_or(local_now)
            }
            IntervalUnit::Years => {
                NaiveDate::from_ymd_opt(local_now.year() + offset * self.amount as i32, 1, 1)
                    .unwrap_or(local_now)
            }
        };

        let start_date = match self.unit {
            IntervalUnit::Days => {
                let epoch_days =
                    (reference_date - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days();
                let cycle = (epoch_days / self.amount as i64) * self.amount as i64;
                NaiveDate::from_ymd_opt(1970, 1, 1).unwrap() + chrono::Duration::days(cycle)
            }
            IntervalUnit::Weeks => {
                let days_since =
                    (reference_date - NaiveDate::from_ymd_opt(1970, 1, 5).unwrap()).num_days();
                let weeks = if days_since >= 0 {
                    days_since / 7
                } else {
                    (days_since - 6) / 7
                };
                let cycle = (weeks / self.amount as i64) * self.amount as i64;
                NaiveDate::from_ymd_opt(1970, 1, 5).unwrap() + chrono::Duration::days(cycle * 7)
            }
            IntervalUnit::Months => {
                let months = (reference_date.year() - 1970) * 12 + reference_date.month0() as i32;
                let cycle = (months / self.amount as i32) * self.amount as i32;
                let y = 1970 + cycle / 12;
                let m = (cycle % 12) as u32 + 1;
                NaiveDate::from_ymd_opt(y, m, 1).unwrap()
            }
            IntervalUnit::Years => {
                let years = reference_date.year() - 1970;
                let cycle = (years / self.amount as i32) * self.amount as i32;
                NaiveDate::from_ymd_opt(1970 + cycle, 1, 1).unwrap()
            }
        };

        let end_date = match self.unit {
            IntervalUnit::Days => start_date + chrono::Duration::days(self.amount as i64),
            IntervalUnit::Weeks => start_date + chrono::Duration::days((self.amount * 7) as i64),
            IntervalUnit::Months => {
                let m = start_date.month0() + self.amount;
                let y = start_date.year() + (m / 12) as i32;
                NaiveDate::from_ymd_opt(y, (m % 12) + 1, 1).unwrap()
            }
            IntervalUnit::Years => {
                NaiveDate::from_ymd_opt(start_date.year() + self.amount as i32, 1, 1).unwrap()
            }
        };

        let start_ts = crate::model::item::safe_local_to_utc(
            start_date,
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        )
        .timestamp();
        let end_ts = crate::model::item::safe_local_to_utc(
            end_date,
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        )
        .timestamp();
        (start_ts, end_ts)
    }

    pub fn format_short(&self) -> String {
        let unit_str = match self.unit {
            IntervalUnit::Days => "d",
            IntervalUnit::Weeks => "w",
            IntervalUnit::Months => "mo",
            IntervalUnit::Years => "y",
        };
        if self.amount == 1 {
            unit_str.to_string()
        } else {
            format!("{}{}", self.amount, unit_str)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Goal {
    pub goal_type: GoalType,
    pub target: u32,
    pub interval: Interval,
}

impl Goal {
    pub fn format_target_display(&self, target_str: &str) -> String {
        if target_str == "1" && self.interval.amount == 1 {
            match self.interval.unit {
                IntervalUnit::Days => rust_i18n::t!("goal_period_daily").to_string(),
                IntervalUnit::Weeks => rust_i18n::t!("goal_period_weekly").to_string(),
                IntervalUnit::Months => rust_i18n::t!("goal_period_monthly").to_string(),
                IntervalUnit::Years => rust_i18n::t!("goal_period_yearly").to_string(),
            }
        } else {
            format!("{}/{}", target_str, self.interval.format_short())
        }
    }
}

impl<'de> Deserialize<'de> for Goal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum GoalFormat {
            Modern {
                goal_type: GoalType,
                target: u32,
                interval: Interval,
            },
            Legacy {
                goal_type: GoalType,
                target: u32,
                period: String,
            },
        }
        match GoalFormat::deserialize(deserializer)? {
            GoalFormat::Modern {
                goal_type,
                target,
                interval,
            } => Ok(Goal {
                goal_type,
                target,
                interval,
            }),
            GoalFormat::Legacy {
                goal_type,
                target,
                period,
            } => {
                let mut amount = 1;
                let unit = match period.to_lowercase().as_str() {
                    "daily" => IntervalUnit::Days,
                    "monthly" => IntervalUnit::Months,
                    "quarterly" => {
                        amount = 3;
                        IntervalUnit::Months
                    }
                    "halfyearly" | "semiannual" => {
                        amount = 6;
                        IntervalUnit::Months
                    }
                    "yearly" => IntervalUnit::Years,
                    _ => IntervalUnit::Weeks,
                };
                Ok(Goal {
                    goal_type,
                    target,
                    interval: Interval { amount, unit },
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, EnumIter)]
pub enum LogLevel {
    #[default]
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Error => write!(f, "Error"),
            LogLevel::Warn => write!(f, "Warn"),
            LogLevel::Info => write!(f, "Info"),
            LogLevel::Debug => write!(f, "Debug"),
            LogLevel::Trace => write!(f, "Trace"),
        }
    }
}

impl LogLevel {
    pub const ALL: &'static [LogLevel] = &[
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    pub fn to_level_filter(&self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_cutoff() -> Option<u32> {
    Some(2)
}

// Configuration version constant for migration handling
const CURRENT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskAction {
    OpenUrl,
    ToggleDetails,
    CompleteAndShift,
    ToggleTimer,
    StopTimer,
    AddSession,
    IncreasePriority,
    DecreasePriority,
    Edit,
    Yank,
    CreateSubtask,
    DuplicateTree,
    Promote,
    Move,
    Cancel,
    Delete,
    DeleteTree,
    OpenLocations,
    OpenCoordinates,
    ExtractSubtasks,
    TogglePin,
}

impl TaskAction {
    pub const ALL: &'static [TaskAction] = &[
        TaskAction::OpenUrl,         // First - link out
        TaskAction::OpenCoordinates, // Second - single coordinates
        TaskAction::OpenLocations,   // Third - multiple coordinates (GPX)
        TaskAction::ToggleDetails,
        TaskAction::CompleteAndShift,
        TaskAction::ToggleTimer,
        TaskAction::StopTimer,
        TaskAction::AddSession,
        TaskAction::IncreasePriority,
        TaskAction::DecreasePriority,
        TaskAction::Edit,
        TaskAction::Yank,
        TaskAction::CreateSubtask,
        TaskAction::ExtractSubtasks,
        TaskAction::TogglePin,
        TaskAction::DuplicateTree,
        TaskAction::Promote,
        TaskAction::Move,
        TaskAction::Cancel,
        TaskAction::Delete,
        TaskAction::DeleteTree,
    ];

    pub fn label(&self) -> String {
        match self {
            TaskAction::ExtractSubtasks => rust_i18n::t!("action_extract_subtasks").to_string(),
            TaskAction::CompleteAndShift => rust_i18n::t!("action_complete_and_shift").to_string(),
            TaskAction::ToggleDetails => rust_i18n::t!("show_details").to_string(),
            TaskAction::ToggleTimer => rust_i18n::t!("start_task").to_string(),
            TaskAction::StopTimer => rust_i18n::t!("stop_reset").to_string(),
            TaskAction::AddSession => rust_i18n::t!("help_metadata_log_time").to_string(),
            TaskAction::IncreasePriority => rust_i18n::t!("increase_priority").to_string(),
            TaskAction::DecreasePriority => rust_i18n::t!("menu_decrease_prio").to_string(),
            TaskAction::Edit => rust_i18n::t!("edit").to_string(),
            TaskAction::Yank => rust_i18n::t!("yank_copy_id").to_string(),
            TaskAction::CreateSubtask => rust_i18n::t!("create_subtask").to_string(),
            TaskAction::DuplicateTree => rust_i18n::t!("duplicate_task").to_string(),
            TaskAction::Promote => rust_i18n::t!("promote_remove_parent").to_string(),
            TaskAction::Move => rust_i18n::t!("menu_move").to_string(),
            TaskAction::Cancel => rust_i18n::t!("cancel").to_string(),
            TaskAction::Delete => rust_i18n::t!("delete").to_string(),
            TaskAction::DeleteTree => rust_i18n::t!("delete_task_tree").to_string(),
            TaskAction::OpenLocations => rust_i18n::t!("action_open_locations").to_string(),
            TaskAction::OpenCoordinates => rust_i18n::t!("open_coordinates").to_string(),
            TaskAction::OpenUrl => rust_i18n::t!("open_url").to_string(),
            TaskAction::TogglePin => rust_i18n::t!("action_toggle_pin").to_string(),
        }
    }
}

/// Controls the priority order of task sorting within the "urgent" bucket (rank 1-3).
/// This determines which criterion (urgent priority, started status, or due soon) takes precedence.
/// - `UrgentStartedDue`: Urgent tasks first, then started, then due soon
/// - `UrgentDueStarted`: Urgent tasks first, then due soon, then started
/// - `StartedUrgentDue`: Started tasks first, then urgent, then due soon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, EnumIter)]
pub enum SortPreset {
    #[default]
    UrgentStartedDue,
    UrgentDueStarted,
    StartedUrgentDue,
}

impl fmt::Display for SortPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortPreset::UrgentStartedDue => write!(f, "Urgent > Started > Due Soon"),
            SortPreset::UrgentDueStarted => write!(f, "Urgent > Due Soon > Started"),
            SortPreset::StartedUrgentDue => write!(f, "Started > Urgent > Due Soon"),
        }
    }
}

impl std::str::FromStr for SortPreset {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Urgent > Started > Due Soon" => Ok(SortPreset::UrgentStartedDue),
            "Urgent > Due Soon > Started" => Ok(SortPreset::UrgentDueStarted),
            "Started > Urgent > Due Soon" => Ok(SortPreset::StartedUrgentDue),
            _ => Err(()),
        }
    }
}

fn default_pinned_actions() -> Vec<TaskAction> {
    vec![
        TaskAction::OpenUrl, // First action - open URL
        TaskAction::OpenCoordinates,
        TaskAction::ToggleDetails,
        TaskAction::ToggleTimer,
        TaskAction::IncreasePriority,
        TaskAction::DecreasePriority,
        TaskAction::Cancel,
        TaskAction::Edit,
        TaskAction::Yank,
        TaskAction::CreateSubtask,
    ]
}

pub fn set_locale_with_fallback(locale: &str) {
    let available = rust_i18n::available_locales!();
    if available.iter().any(|l| l == locale) {
        rust_i18n::set_locale(locale);
    } else if let Some(primary) = locale.split(&['_', '-'][..]).next() {
        if available.iter().any(|l| l == primary) {
            rust_i18n::set_locale(primary);
        } else {
            rust_i18n::set_locale(locale);
        }
    } else {
        rust_i18n::set_locale(locale);
    }
}

pub fn init_locale(ctx: &dyn crate::context::AppContext) {
    // Initialize locale using the persistent config if present, otherwise fall back
    // to the system locale (primary language subtag). Android will pass its locale
    // at startup via UniFFI so this will pick that up if it's saved in the Config.
    let config = Config::load(ctx).unwrap_or_default();
    if let Some(lang) = config.language {
        set_locale_with_fallback(&lang);
    } else if let Some(sys_lang) = sys_locale::get_locale() {
        set_locale_with_fallback(&sys_lang);
    }
}

fn default_urgent_days() -> u32 {
    1
}
fn default_urgent_prio() -> u8 {
    1
}

fn default_enable_local_mode() -> bool {
    true
}

fn default_start_grace_period() -> u32 {
    1
}

fn default_priority() -> u8 {
    5
}

fn default_auto_remind() -> bool {
    true
}
fn default_remind_time() -> String {
    "08:00".to_string()
}
fn default_snooze_1() -> u32 {
    60
}
fn default_snooze_2() -> u32 {
    1440
}

fn default_create_events() -> bool {
    false
}

fn default_delete_events_on_completion() -> bool {
    false
}

fn default_refresh_interval() -> u32 {
    30
}

fn default_max_done_roots() -> usize {
    20
}

fn default_ui_scale() -> f32 {
    1.0
}
fn default_max_done_subtasks() -> usize {
    5
}

// Quick Filter defaults
fn default_quick_filter_term() -> String {
    "is:ready".to_string()
}
fn default_quick_filter_icon() -> String {
    "f0fa9".to_string()
}

// Default trash retention moved to advanced settings; default shortened to 14 days.
fn default_trash_retention() -> u32 {
    14
}

fn default_duration_goal_mins() -> u32 {
    60
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, EnumIter)]
pub enum AppTheme {
    #[default]
    RustyDark,
    Random,
    Light,
    Dark,
    Dracula,
    Nord,
    SolarizedLight,
    SolarizedDark,
    GruvboxLight,
    GruvboxDark,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
    CatppuccinMocha,
    TokyoNight,
    TokyoNightStorm,
    TokyoNightLight,
    KanagawaWave,
    KanagawaDragon,
    KanagawaLotus,
    Moonfly,
    Nightfly,
    Oxocarbon,
    Ferra,
}

impl AppTheme {
    pub fn is_dark(&self) -> bool {
        !matches!(
            self,
            AppTheme::Light
                | AppTheme::SolarizedLight
                | AppTheme::GruvboxLight
                | AppTheme::CatppuccinLatte
                | AppTheme::TokyoNightLight
        )
    }
}

impl fmt::Display for AppTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppTheme::RustyDark => write!(f, "Rusty Dark"),
            AppTheme::Random => write!(f, "Random"),
            AppTheme::Light => write!(f, "Light"),
            AppTheme::Dark => write!(f, "Dark"),
            AppTheme::Dracula => write!(f, "Dracula"),
            AppTheme::Nord => write!(f, "Nord"),
            AppTheme::SolarizedLight => write!(f, "Solarized Light"),
            AppTheme::SolarizedDark => write!(f, "Solarized Dark"),
            AppTheme::GruvboxLight => write!(f, "Gruvbox Light"),
            AppTheme::GruvboxDark => write!(f, "Gruvbox Dark"),
            AppTheme::CatppuccinLatte => write!(f, "Catppuccin Latte"),
            AppTheme::CatppuccinFrappe => write!(f, "Catppuccin Frappé"),
            AppTheme::CatppuccinMacchiato => write!(f, "Catppuccin Macchiato"),
            AppTheme::CatppuccinMocha => write!(f, "Catppuccin Mocha"),
            AppTheme::TokyoNight => write!(f, "Tokyo Night"),
            AppTheme::TokyoNightStorm => write!(f, "Tokyo Night Storm"),
            AppTheme::TokyoNightLight => write!(f, "Tokyo Night Light"),
            AppTheme::KanagawaWave => write!(f, "Kanagawa Wave"),
            AppTheme::KanagawaDragon => write!(f, "Kanagawa Dragon"),
            AppTheme::KanagawaLotus => write!(f, "Kanagawa Lotus"),
            AppTheme::Moonfly => write!(f, "Moonfly"),
            AppTheme::Nightfly => write!(f, "Nightfly"),
            AppTheme::Oxocarbon => write!(f, "Oxocarbon"),
            AppTheme::Ferra => write!(f, "Ferra"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Config {
    /// IMPORTANT FOR DEVELOPERS:
    /// If you add a new action to `default_pinned_actions()` or modify a default
    /// collection that existing users should inherit, you MUST increment
    /// `CURRENT_CONFIG_VERSION` and add a migration step in `Config::load()`.
    /// Otherwise, existing users' saved configs will override your new defaults.
    #[serde(default)]
    pub config_version: u32,

    pub url: String,
    pub username: String,

    // Skip saving to disk, but allow reading (with default) for migration!
    #[serde(skip_serializing, default)]
    pub password: String,

    #[serde(default)]
    pub allow_insecure_certs: bool,
    #[serde(default)]
    pub disabled_calendars: Vec<String>,

    pub default_calendar: Option<String>,
    #[serde(default = "default_enable_local_mode")]
    pub enable_local_mode: bool,
    #[serde(default)]
    pub hide_completed: bool,
    #[serde(default)]
    pub strikethrough_completed: bool,
    #[serde(default = "default_true")]
    pub hide_fully_completed_tags: bool,
    #[serde(default = "default_true")]
    pub hide_aliases_in_sidebar: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_cutoff")]
    pub sort_cutoff_months: Option<u32>,
    /// When `true`, rank-4 (standard tasks with a due date within the cutoff) are sorted
    /// by priority first, then by due date.  Default is `false` (date-first).
    #[serde(default)]
    pub sort_standard_by_priority: bool,
    /// Priority order for sorting tasks within the urgent/due soon/started ranks.
    /// See `SortPreset` enum for available options.
    #[serde(default)]
    pub sort_preset: SortPreset,
    #[serde(default)]
    pub theme: AppTheme,

    // Optional language/locale selection. None = use system default.
    #[serde(default)]
    pub language: Option<String>,

    #[serde(default = "default_urgent_days")]
    pub urgent_days_horizon: u32,
    #[serde(default = "default_urgent_prio")]
    pub urgent_priority_threshold: u8,
    #[serde(default = "default_priority")]
    pub default_priority: u8,
    #[serde(default = "default_start_grace_period")]
    pub start_grace_period_days: u32,

    #[serde(default = "default_auto_remind")]
    pub auto_reminders: bool,
    #[serde(default = "default_remind_time")]
    pub default_reminder_time: String, // Format "HH:MM"

    #[serde(default = "default_snooze_1")]
    pub snooze_short_mins: u32,
    #[serde(default = "default_snooze_2")]
    pub snooze_long_mins: u32,

    #[serde(default = "default_create_events")]
    pub create_events_for_tasks: bool,

    #[serde(default = "default_delete_events_on_completion")]
    pub delete_events_on_completion: bool,

    #[serde(default = "default_refresh_interval")]
    pub auto_refresh_interval_mins: u32,

    // Default retention for local trash. This setting has been moved to the
    // Advanced Settings UI; default value reduced to 14 days.
    #[serde(default = "default_trash_retention")]
    pub trash_retention_days: u32, // Integer: Days to keep items in local trash before permanent delete. 0 to disable trash.

    #[serde(default = "default_duration_goal_mins")]
    pub default_duration_goal_mins: u32,

    #[serde(default)]
    pub sessions_count_as_completions: bool,

    #[serde(default = "default_max_done_roots")]
    pub max_done_roots: usize,
    #[serde(default = "default_max_done_subtasks")]
    pub max_done_subtasks: usize,

    #[serde(default = "default_true")]
    pub show_ongoing_notifications: bool,
    #[serde(default = "default_true")]
    pub show_priority_numbers: bool,

    #[serde(default = "default_pinned_actions")]
    pub pinned_actions: Vec<TaskAction>,

    #[serde(default = "default_quick_filter_term")]
    pub quick_filter_term: String,
    #[serde(default = "default_quick_filter_icon")]
    pub quick_filter_icon: String,
    #[serde(default = "default_true")]
    pub show_quick_filter: bool,

    #[serde(default = "default_true")]
    pub show_goals_tab: bool,

    #[serde(default = "default_true")]
    pub show_task_goals_in_sidebar: bool,

    #[serde(default)]
    pub sidebar_is_hidden: bool,

    #[serde(default)]
    pub description_editor: String,

    // Logging level for both file and terminal output
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,

    // Maps are typically at the end in TOML
    #[serde(default)]
    pub hidden_calendars: Vec<String>,
    #[serde(default)]
    pub tag_aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub goals: HashMap<String, Goal>,

    // UI State
    #[serde(default)]
    pub expanded_tags: Vec<String>,
    #[serde(default)]
    pub expanded_locations: Vec<String>,
    #[serde(default)]
    pub expanded_done_groups: Vec<String>,

    #[serde(default = "default_true")]
    pub sync_settings: bool,
    #[serde(default)]
    pub settings_updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SyncableConfig {
    #[serde(default)]
    pub tag_aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub goals: HashMap<String, Goal>,
    #[serde(default)]
    pub hide_completed: bool,
    #[serde(default)]
    pub hide_fully_completed_tags: bool,
    #[serde(default)]
    pub hide_aliases_in_sidebar: bool,
    #[serde(default)]
    pub sort_cutoff_months: Option<u32>,
    #[serde(default)]
    pub sort_standard_by_priority: bool,
    #[serde(default)]
    pub sort_preset: SortPreset,
    #[serde(default = "default_urgent_days")]
    pub urgent_days_horizon: u32,
    #[serde(default = "default_urgent_prio")]
    pub urgent_priority_threshold: u8,
    #[serde(default = "default_priority")]
    pub default_priority: u8,
    #[serde(default = "default_start_grace_period")]
    pub start_grace_period_days: u32,
    #[serde(default = "default_auto_remind")]
    pub auto_reminders: bool,
    #[serde(default = "default_remind_time")]
    pub default_reminder_time: String,
    #[serde(default = "default_snooze_1")]
    pub snooze_short_mins: u32,
    #[serde(default = "default_snooze_2")]
    pub snooze_long_mins: u32,
    #[serde(default = "default_create_events")]
    pub create_events_for_tasks: bool,
    #[serde(default = "default_delete_events_on_completion")]
    pub delete_events_on_completion: bool,
    #[serde(default = "default_trash_retention")]
    pub trash_retention_days: u32,
    #[serde(default = "default_duration_goal_mins")]
    pub default_duration_goal_mins: u32,
    #[serde(default)]
    pub sessions_count_as_completions: bool,
    #[serde(default = "default_max_done_roots")]
    pub max_done_roots: usize,
    #[serde(default = "default_max_done_subtasks")]
    pub max_done_subtasks: usize,
    #[serde(default = "default_true")]
    pub show_ongoing_notifications: bool,
    #[serde(default = "default_true")]
    pub show_priority_numbers: bool,
    #[serde(default = "default_pinned_actions")]
    pub pinned_actions: Vec<TaskAction>,
    #[serde(default = "default_quick_filter_term")]
    pub quick_filter_term: String,
    #[serde(default = "default_quick_filter_icon")]
    pub quick_filter_icon: String,
    #[serde(default = "default_true")]
    pub show_quick_filter: bool,
    #[serde(default = "default_true")]
    pub show_goals_tab: bool,
    #[serde(default = "default_true")]
    pub show_task_goals_in_sidebar: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SettingsPayload {
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub config: SyncableConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            default_calendar: None,
            enable_local_mode: true,
            allow_insecure_certs: false,
            hidden_calendars: Vec::new(),
            disabled_calendars: Vec::new(),
            hide_completed: false,
            hide_fully_completed_tags: true,
            hide_aliases_in_sidebar: true,
            ui_scale: 1.0,
            sort_cutoff_months: Some(2),
            sort_standard_by_priority: false,
            sort_preset: SortPreset::default(),
            tag_aliases: HashMap::new(),
            language: None,
            theme: AppTheme::default(),
            urgent_days_horizon: 1,
            urgent_priority_threshold: 1,
            default_priority: 5,
            start_grace_period_days: 1,
            auto_reminders: true,
            default_reminder_time: "08:00".to_string(),
            snooze_short_mins: 60,
            snooze_long_mins: 1440,
            create_events_for_tasks: false,
            delete_events_on_completion: false,
            auto_refresh_interval_mins: 30,
            trash_retention_days: 14,
            default_duration_goal_mins: 60,
            sessions_count_as_completions: false,
            strikethrough_completed: false,
            max_done_roots: 20,
            max_done_subtasks: 5,
            show_ongoing_notifications: true,
            show_priority_numbers: true,
            pinned_actions: default_pinned_actions(),
            quick_filter_term: default_quick_filter_term(),
            quick_filter_icon: default_quick_filter_icon(),
            show_quick_filter: true,
            show_goals_tab: true,
            show_task_goals_in_sidebar: true,
            sidebar_is_hidden: false,
            description_editor: String::new(),
            log_level: default_log_level(),
            expanded_tags: Vec::new(),
            expanded_locations: Vec::new(),
            expanded_done_groups: Vec::new(),
            sync_settings: true,
            settings_updated_at: 0,
            goals: HashMap::new(),
        }
    }
}

impl Config {
    pub fn get_syncable(&self) -> SyncableConfig {
        SyncableConfig {
            tag_aliases: self.tag_aliases.clone(),
            goals: self.goals.clone(),
            hide_completed: self.hide_completed,
            hide_fully_completed_tags: self.hide_fully_completed_tags,
            hide_aliases_in_sidebar: self.hide_aliases_in_sidebar,
            sort_cutoff_months: self.sort_cutoff_months,
            sort_standard_by_priority: self.sort_standard_by_priority,
            sort_preset: self.sort_preset,
            urgent_days_horizon: self.urgent_days_horizon,
            urgent_priority_threshold: self.urgent_priority_threshold,
            default_priority: self.default_priority,
            start_grace_period_days: self.start_grace_period_days,
            auto_reminders: self.auto_reminders,
            default_reminder_time: self.default_reminder_time.clone(),
            snooze_short_mins: self.snooze_short_mins,
            snooze_long_mins: self.snooze_long_mins,
            create_events_for_tasks: self.create_events_for_tasks,
            delete_events_on_completion: self.delete_events_on_completion,
            trash_retention_days: self.trash_retention_days,
            default_duration_goal_mins: self.default_duration_goal_mins,
            sessions_count_as_completions: self.sessions_count_as_completions,
            max_done_roots: self.max_done_roots,
            max_done_subtasks: self.max_done_subtasks,
            show_ongoing_notifications: self.show_ongoing_notifications,
            show_priority_numbers: self.show_priority_numbers,
            pinned_actions: self.pinned_actions.clone(),
            quick_filter_term: self.quick_filter_term.clone(),
            quick_filter_icon: self.quick_filter_icon.clone(),
            show_quick_filter: self.show_quick_filter,
            show_goals_tab: self.show_goals_tab,
            show_task_goals_in_sidebar: self.show_task_goals_in_sidebar,
        }
    }

    pub fn apply_syncable(&mut self, sync: SyncableConfig) {
        self.tag_aliases = sync.tag_aliases;
        self.goals = sync.goals;
        self.hide_completed = sync.hide_completed;
        self.hide_fully_completed_tags = sync.hide_fully_completed_tags;
        self.hide_aliases_in_sidebar = sync.hide_aliases_in_sidebar;
        self.sort_cutoff_months = sync.sort_cutoff_months;
        self.sort_standard_by_priority = sync.sort_standard_by_priority;
        self.sort_preset = sync.sort_preset;
        self.urgent_days_horizon = sync.urgent_days_horizon;
        self.urgent_priority_threshold = sync.urgent_priority_threshold;
        self.default_priority = sync.default_priority;
        self.start_grace_period_days = sync.start_grace_period_days;
        self.auto_reminders = sync.auto_reminders;
        self.default_reminder_time = sync.default_reminder_time;
        self.snooze_short_mins = sync.snooze_short_mins;
        self.snooze_long_mins = sync.snooze_long_mins;
        self.create_events_for_tasks = sync.create_events_for_tasks;
        self.delete_events_on_completion = sync.delete_events_on_completion;
        self.trash_retention_days = sync.trash_retention_days;
        self.default_duration_goal_mins = sync.default_duration_goal_mins;
        self.sessions_count_as_completions = sync.sessions_count_as_completions;
        self.max_done_roots = sync.max_done_roots;
        self.max_done_subtasks = sync.max_done_subtasks;
        self.show_ongoing_notifications = sync.show_ongoing_notifications;
        self.show_priority_numbers = sync.show_priority_numbers;
        self.pinned_actions = sync.pinned_actions;
        self.quick_filter_term = sync.quick_filter_term;
        self.quick_filter_icon = sync.quick_filter_icon;
        self.show_quick_filter = sync.show_quick_filter;
        self.show_goals_tab = sync.show_goals_tab;
        self.show_task_goals_in_sidebar = sync.show_task_goals_in_sidebar;
    }

    pub fn update_sync_timestamp_if_changed(&mut self, old: &Config) {
        if self.get_syncable() != old.get_syncable() {
            self.settings_updated_at = chrono::Utc::now().timestamp();
        }
    }

    /// Load the configuration from disk using an explicit context.
    pub fn load(ctx: &dyn AppContext) -> Result<Self> {
        let path = ctx.get_config_file_path()?;

        if !path.exists() {
            return Err(anyhow::anyhow!("Config file not found"));
        }

        let contents = fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e)
        })?;

        let mut config: Config = toml::from_str(&contents).map_err(|e| {
            anyhow::anyhow!("Failed to parse config file '{}': {}", path.display(), e)
        })?;

        // --- CONFIG MIGRATIONS ---
        // Apply migrations for older configs to ensure they receive new features/defaults.
        // If you bump `CURRENT_CONFIG_VERSION`, add a new `if` block here.
        if config.config_version == 0 {
            // Migrate OpenCoordinates (added after v0.5.8)
            if !config.pinned_actions.contains(&TaskAction::OpenCoordinates) {
                config.pinned_actions.insert(0, TaskAction::OpenCoordinates);
            }
            // Migrate OpenUrl (added after v0.5.8)
            if !config.pinned_actions.contains(&TaskAction::OpenUrl) {
                config.pinned_actions.insert(0, TaskAction::OpenUrl);
            }
            // Note: Inserting both at 0 one after the other places OpenUrl first,
            // then OpenCoordinates, perfectly matching `default_pinned_actions()`.

            config.config_version = 1;
        }

        Ok(config)
    }

    /// Load the configuration from disk and fetch the password from the OS keyring.
    /// Use this ONLY during app startup, explicit syncing, or opening the settings panel.
    pub fn load_with_credentials(ctx: &dyn AppContext) -> Result<Self> {
        let mut config = Self::load(ctx)?;

        let user_key = if config.username.is_empty() {
            "default"
        } else {
            &config.username
        };

        match keyring_core::Entry::new("cfait", user_key) {
            Ok(entry) => {
                if !config.password.is_empty() {
                    // Migration: plaintext password found in config.toml!
                    // Move it securely into the OS keyring.
                    if let Err(err) = entry.set_password(&config.password) {
                        log::warn!(
                            "Failed to migrate password into keyring for user '{}': {}",
                            user_key,
                            err
                        );
                    }
                } else {
                    match entry.get_password() {
                        Ok(pw) => {
                            // Normal run: fetch the password from the OS keyring.
                            config.password = pw;
                        }
                        Err(keyring_core::Error::NoEntry) => {}
                        Err(err) => {
                            log::warn!(
                                "Failed to load password from keyring for user '{}': {}",
                                user_key,
                                err
                            );
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!(
                    "Failed to initialize keyring entry for user '{}': {}",
                    user_key,
                    err
                );
            }
        }

        Ok(config)
    }

    pub fn is_missing_config_error(err: &Error) -> bool {
        if err.to_string().contains("Config file not found") {
            return true;
        }
        if let Some(io_err) = err.downcast_ref::<std::io::Error>()
            && io_err.kind() == std::io::ErrorKind::NotFound
        {
            return true;
        }
        for cause in err.chain() {
            if let Some(io_err) = cause.downcast_ref::<std::io::Error>()
                && io_err.kind() == std::io::ErrorKind::NotFound
            {
                return true;
            }
        }
        false
    }

    /// Save configuration using an explicit context.
    /// This method post-processes the TOML string to inject documentation comments.
    pub fn save(&self, ctx: &dyn AppContext) -> Result<()> {
        let path = ctx.get_config_file_path()?;

        LocalStorage::with_lock(&path, || {
            let toml_str = toml::to_string_pretty(self)?;
            let documented_toml = Self::inject_documentation(&toml_str);
            LocalStorage::atomic_write(&path, documented_toml)?;
            Ok(())
        })?;
        Ok(())
    }

    /// Save configuration and update the OS keyring credential.
    pub fn save_with_credentials(&self, ctx: &dyn AppContext) -> Result<()> {
        let user_key = if self.username.is_empty() {
            "default"
        } else {
            &self.username
        };

        match keyring_core::Entry::new("cfait", user_key) {
            Ok(entry) => {
                if !self.password.is_empty() {
                    if let Err(err) = entry.set_password(&self.password) {
                        log::warn!(
                            "Failed to save password to keyring for user '{}': {}",
                            user_key,
                            err
                        );
                    }
                } else if let Err(err) = entry.delete_credential() {
                    // Delete credential if the user cleared the password.
                    // Missing entries are fine; anything else is worth logging.
                    if !matches!(err, keyring_core::Error::NoEntry) {
                        log::warn!(
                            "Failed to delete keyring credential for user '{}': {}",
                            user_key,
                            err
                        );
                    }
                }
            }
            Err(err) => {
                log::warn!(
                    "Failed to initialize keyring entry for user '{}': {}",
                    user_key,
                    err
                );
            }
        }

        self.save(ctx)
    }

    /// Get the path string using an explicit context.
    pub fn get_path_string(ctx: &dyn AppContext) -> Result<String> {
        let path = ctx.get_config_file_path()?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Post-process raw TOML string to add comments and headers.
    fn inject_documentation(raw_toml: &str) -> String {
        let mut out = String::with_capacity(raw_toml.len() + 2048);

        // Header Comment
        out.push_str("# Cfait Configuration\n\n");

        // Connection Header (Assumed to be at top based on struct order)
        out.push_str("# --- Connection Settings ---\n");

        for line in raw_toml.lines() {
            let trimmed = line.trim();

            // -- Section Headers --
            if trimmed.starts_with("default_calendar =") {
                out.push_str("\n# --- UI & Behavior ---\n");
            } else if trimmed.starts_with("sort_cutoff_months =") {
                out.push_str("\n# --- Sorting & Ranking Logic ---\n");
            } else if trimmed.starts_with("auto_reminders =") {
                out.push_str("\n# --- Notifications & Reminders ---\n");
            } else if trimmed.starts_with("create_events_for_tasks =") {
                out.push_str("\n# --- Calendar Integration (VEVENT Sync) ---\n");
            } else if trimmed.starts_with("max_done_roots =") {
                out.push_str("\n# --- Advanced Settings ---\n");
            } else if trimmed.starts_with("[tag_aliases]") {
                out.push_str("\n# --- Aliases (Global Templates) ---\n");
                out.push_str("# Map shortcuts to sets of tags/locations/priorities.\n");
                out.push_str("# Example: \"#gardening\" = [\"#fun\", \"@@home\"]\n");
            } else if trimmed.starts_with("[goals]") {
                out.push_str("\n# --- Goals & Habit Tracking ---\n");
                out.push_str("# Set tracking goals for specific tags or locations.\n");
                out.push_str("# Example: [goals.\"#reading\"]\n");
                out.push_str("#          goal_type = \"count\" # or \"duration\"\n");
                out.push_str("#          target = 5\n");
                out.push_str("#          period = \"weekly\" # daily, weekly, monthly, yearly\n");
            }

            // -- Inline or Block Comments for specific keys --

            if trimmed.starts_with("url =") {
                out.push_str("# URL: The full address to your CalDAV server endpoint.\n");
                out.push_str(line);
            } else if trimmed.starts_with("allow_insecure_certs =") {
                out.push_str(line);
                out.push_str(
                    " # Boolean: Set true to bypass SSL verification (e.g. self-signed certs).",
                );
            } else if trimmed.starts_with("sync_settings =") {
                out.push_str(line);
                out.push_str(
                    " # Boolean: If true, sync settings and aliases as a hidden task on the server.",
                );
            } else if trimmed.starts_with("disabled_calendars =") {
                out.push_str("# List of calendar HREFs (strings) to completely disable/ignore.\n");
                out.push_str(line);
            } else if trimmed.starts_with("default_calendar =") {
                out.push_str(line);
                out.push_str(
                    " # String: Target for new tasks. HREF or 'local://default' when local mode is enabled.",
                );
            } else if trimmed.starts_with("enable_local_mode =") {
                out.push_str(line);
                out.push_str(
                    " # Boolean: If false, TUI local/offline calendars are hidden and new tasks target remote calendars only.",
                );
            } else if trimmed.starts_with("hide_completed =") {
                out.push_str(line);
                out.push_str(" # Boolean: If true, Completed/Cancelled tasks are hidden globally.");
            } else if trimmed.starts_with("strikethrough_completed =") {
                out.push_str(line);
                out.push_str(" # Boolean: Apply strikethrough styling to completed task titles.");
            } else if trimmed.starts_with("hide_fully_completed_tags =") {
                out.push_str(line);
                out.push_str(" # Boolean: Hide tags in sidebar if all their tasks are completed.");
            } else if trimmed.starts_with("hide_aliases_in_sidebar =") {
                out.push_str(line);
                out.push_str(" # Boolean: Hide shorthand aliases from the sidebar (showing only their targets).");
            } else if trimmed.starts_with("ui_scale =") {
                out.push_str(line);
                out.push_str(
                    " # Float: Global UI scale factor (0.5–3.0). Ctrl+/Ctrl-/scroll to change.",
                );
            } else if trimmed.starts_with("theme =") {
                out.push_str(line);
                out.push_str(" # String: App Theme (RustyDark, Light, Dark, etc). In the TUI, light themes adapt text contrast for light terminal backgrounds.");
            } else if trimmed.starts_with("sort_cutoff_months =") {
                out.push_str(line);
                out.push_str(
                    " # Integer/None: Tasks due beyond this many months are ranked lower.",
                );
            } else if trimmed.starts_with("urgent_days_horizon =") {
                out.push_str(line);
                out.push_str(
                    " # Integer: Tasks due within this many days are considered 'Urgent'.",
                );
            } else if trimmed.starts_with("urgent_priority_threshold =") {
                out.push_str(line);
                out.push_str(" # Integer (1-9): Priorities <= this value are 'Urgent'. (1=High)");
            } else if trimmed.starts_with("default_priority =") {
                out.push_str(line);
                out.push_str(" # Integer (1-9): Default priority for new tasks. 0 maps to this.");
            } else if trimmed.starts_with("start_grace_period_days =") {
                out.push_str(line);
                out.push_str(
                    " # Integer: Future tasks appear in the list this many days before start.",
                );
            } else if trimmed.starts_with("auto_reminders =") {
                out.push_str(line);
                out.push_str(
                    " # Boolean: Auto-remind on Due/Start dates if no explicit alarms exist.",
                );
            } else if trimmed.starts_with("default_reminder_time =") {
                out.push_str(line);
                out.push_str(" # String (HH:MM): Default time for date-only auto-reminders.");
            } else if trimmed.starts_with("snooze_short_mins =") {
                out.push_str(line);
                out.push_str(" # Integer: Minutes for the 'Short Snooze' button.");
            } else if trimmed.starts_with("snooze_long_mins =") {
                out.push_str(line);
                out.push_str(" # Integer: Minutes for the 'Long Snooze' button.");
            } else if trimmed.starts_with("auto_refresh_interval_mins =") {
                out.push_str(line);
                out.push_str(" # Integer: Background sync interval in minutes. 0 to disable.");
            } else if trimmed.starts_with("create_events_for_tasks =") {
                out.push_str(line);
                out.push_str(
                    " # Boolean: Create companion VEVENTs on server for tasks with dates.",
                );
            } else if trimmed.starts_with("delete_events_on_completion =") {
                out.push_str(line);
                out.push_str(" # Boolean: Remove the companion VEVENT when task is completed.");
            } else if trimmed.starts_with("max_done_roots =") {
                out.push_str(line);
                out.push_str(
                    " # Integer: Limit completed root tasks shown before 'Expand' button.",
                );
            } else if trimmed.starts_with("max_done_subtasks =") {
                out.push_str(line);
                out.push_str(
                    " # Integer: Limit completed subtasks shown in a parent before 'Expand'.",
                );
            } else if trimmed.starts_with("show_ongoing_notifications =") {
                out.push_str(line);
                out.push_str(" # Boolean: Display ongoing timer notification for active tasks.");
            } else if trimmed.starts_with("quick_filter_term =") {
                out.push_str(line);
                out.push_str(" # String: The search term toggled by the quick filter button.");
            } else if trimmed.starts_with("quick_filter_icon =") {
                out.push_str(line);
                out.push_str(" # String: Hex code or character for the quick filter button icon.");
            } else if trimmed.starts_with("show_quick_filter =") {
                out.push_str(line);
                out.push_str(" # Boolean: Display the quick filter button in the search bar.");
            } else if trimmed.starts_with("show_goals_tab =") {
                out.push_str(line);
                out.push_str(" # Boolean: Display the Goals tab in the sidebar.");
            } else if trimmed.starts_with("show_task_goals_in_sidebar =") {
                out.push_str(line);
                out.push_str(" # Boolean: Display task-specific goals alongside global goals.");
            } else if trimmed.starts_with("sidebar_is_hidden =") {
                out.push_str(line);
                out.push_str(" # Boolean: Hide the left sidebar collections panel.");
            } else if trimmed.starts_with("description_editor =") {
                out.push_str(line);
                out.push_str(" # String: Editor for task descriptions. Empty uses $VISUAL/$EDITOR. 'builtin' forces internal editor.");
            } else if trimmed.starts_with("show_priority_numbers =") {
                out.push_str(line);
                out.push_str(" # Boolean: Render priority numbers (!X) visually next to tags.");
            } else if trimmed.starts_with("hidden_calendars =") {
                out.push_str("# List of calendar HREFs currently toggled 'off' in the sidebar.\n");
                out.push_str(line);
            } else if trimmed.starts_with("expanded_tags =") {
                out.push_str("\n# --- UI Memory State ---\n");
                out.push_str("# Arrays remembering which tree folders are currently expanded.\n");
                out.push_str(line);
            } else if trimmed.starts_with("trash_retention_days =") {
                out.push_str(line);
                out.push_str(" # Integer: Days to keep deleted items in local trash before permanent delete. 0 disables trash.");
            } else if trimmed.starts_with("log_level =") {
                out.push_str(line);
                out.push_str(" # String: Logging verbosity level (Error, Warn, Info, Debug, Trace). Applies to both log file and terminal.");
            } else if trimmed.starts_with("config_version =") {
                out.push_str(
                    "# Internal version for configuration migrations. Do not edit manually.",
                );
                out.push_str(line);
            } else {
                // Pass through unhandled lines
                out.push_str(line);
            }
            out.push('\n');
        }

        out
    }
}
