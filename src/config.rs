// File: src/config.rs
use crate::paths::AppPaths;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;

fn default_true() -> bool {
    true
}
fn default_cutoff() -> Option<u32> {
    Some(2)
}

fn default_urgent_days() -> u32 {
    1
} // Tomorrow
fn default_urgent_prio() -> u8 {
    1
} // !1

// Add default helpers
fn default_auto_remind() -> bool {
    true
}
fn default_remind_time() -> String {
    "08:00".to_string()
}
fn default_snooze_1() -> u32 {
    15
} // 15 min
fn default_snooze_2() -> u32 {
    60
} // 1 hour

fn default_create_events() -> bool {
    false
}

fn default_delete_events_on_completion() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppTheme {
    Dark,
    #[default]
    RustyDark,
}

impl fmt::Display for AppTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppTheme::Dark => write!(f, "Default Dark"),
            AppTheme::RustyDark => write!(f, "Rusty Dark"),
        }
    }
}

impl AppTheme {
    pub const ALL: [AppTheme; 2] = [AppTheme::Dark, AppTheme::RustyDark];
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Config {
    pub url: String,
    pub username: String,
    pub password: String,
    pub default_calendar: Option<String>,
    #[serde(default)]
    pub allow_insecure_certs: bool,
    #[serde(default)]
    pub hidden_calendars: Vec<String>,
    #[serde(default)]
    pub disabled_calendars: Vec<String>,
    #[serde(default)]
    pub hide_completed: bool,
    #[serde(default = "default_true")]
    pub hide_fully_completed_tags: bool,
    #[serde(default = "default_cutoff")]
    pub sort_cutoff_months: Option<u32>,
    #[serde(default)]
    pub tag_aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub theme: AppTheme,

    #[serde(default = "default_urgent_days")]
    pub urgent_days_horizon: u32,
    #[serde(default = "default_urgent_prio")]
    pub urgent_priority_threshold: u8,

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            password: String::new(),
            default_calendar: None,
            allow_insecure_certs: false,
            hidden_calendars: Vec::new(),
            disabled_calendars: Vec::new(),
            hide_completed: false,
            // Match the serde defaults
            hide_fully_completed_tags: true,
            sort_cutoff_months: Some(2),
            tag_aliases: HashMap::new(),
            theme: AppTheme::default(),
            urgent_days_horizon: 1,
            urgent_priority_threshold: 1,
            auto_reminders: true,
            default_reminder_time: "08:00".to_string(),
            snooze_short_mins: 15,
            snooze_long_mins: 60,
            create_events_for_tasks: false,
            delete_events_on_completion: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = AppPaths::get_config_file_path()?;
        if path.exists() {
            let contents = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&contents)?;
            return Ok(config);
        }
        Err(anyhow::anyhow!("Config file not found"))
    }

    pub fn save(&self) -> Result<()> {
        let path = AppPaths::get_config_file_path()?;
        LocalStorage::with_lock(&path, || {
            let toml_str = toml::to_string_pretty(self)?;
            LocalStorage::atomic_write(&path, toml_str)?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn get_path_string() -> Result<String> {
        let path = AppPaths::get_config_file_path()?;
        Ok(path.to_string_lossy().to_string())
    }
}
