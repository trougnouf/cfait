// Handles configuration loading, saving, and defaults.
use crate::paths::AppPaths;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use strum::EnumIter;

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

fn default_start_grace_period() -> u32 {
    1
} // 1 day grace period for start dates

fn default_priority() -> u8 {
    5
} // Normal priority

// Add default helpers
fn default_auto_remind() -> bool {
    true
}
fn default_remind_time() -> String {
    "08:00".to_string()
}
fn default_snooze_1() -> u32 {
    60
} // 1 hour
fn default_snooze_2() -> u32 {
    1440
} // 1 day

fn default_create_events() -> bool {
    false
}

fn default_delete_events_on_completion() -> bool {
    false
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
            default_priority: 5,
            start_grace_period_days: 1,
            auto_reminders: true,
            default_reminder_time: "08:00".to_string(),
            snooze_short_mins: 60,
            snooze_long_mins: 1440,
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
