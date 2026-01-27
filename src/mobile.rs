/* cfait/src/mobile.rs
 *
 * UniFFI interface for exposing core logic to mobile platforms (Android).
 */

use crate::alarm_index::AlarmIndex;
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::config::Config;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::model::{AlarmTrigger, DateType, Task};
use crate::paths::AppPaths;
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use crate::store::{FilterOptions, TaskStore, UNCATEGORIZED_ID};
use chrono::{DateTime, Local, NaiveTime, Utc};
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
#[cfg(target_os = "android")]
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

// Android-only types are referenced with fully-qualified paths inside platform-specific blocks.
// This avoids top-level platform-gated imports that can become unused on some targets.

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    Generic(String),
}
impl From<String> for MobileError {
    fn from(e: String) -> Self {
        Self::Generic(e)
    }
}
impl From<&str> for MobileError {
    fn from(e: &str) -> Self {
        Self::Generic(e.to_string())
    }
}
impl From<anyhow::Error> for MobileError {
    fn from(e: anyhow::Error) -> Self {
        Self::Generic(e.to_string())
    }
}
impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MobileError::Generic(s) => s,
            }
        )
    }
}
impl std::error::Error for MobileError {}

#[derive(uniffi::Enum)]
pub enum MobileSyntaxType {
    Text,
    Priority,
    DueDate,
    StartDate,
    Recurrence,
    Duration,
    Tag,
    Location,
    Url,
    Geo,
    Description,
    Reminder,
}

impl From<SyntaxType> for MobileSyntaxType {
    fn from(t: SyntaxType) -> Self {
        match t {
            SyntaxType::Text => MobileSyntaxType::Text,
            SyntaxType::Priority => MobileSyntaxType::Priority,
            SyntaxType::DueDate => MobileSyntaxType::DueDate,
            SyntaxType::StartDate => MobileSyntaxType::StartDate,
            SyntaxType::Recurrence => MobileSyntaxType::Recurrence,
            SyntaxType::Duration => MobileSyntaxType::Duration,
            SyntaxType::Tag => MobileSyntaxType::Tag,
            SyntaxType::Location => MobileSyntaxType::Location,
            SyntaxType::Url => MobileSyntaxType::Url,
            SyntaxType::Geo => MobileSyntaxType::Geo,
            SyntaxType::Description => MobileSyntaxType::Description,
            SyntaxType::Reminder => MobileSyntaxType::Reminder,
        }
    }
}

#[derive(uniffi::Record)]
pub struct MobileSyntaxToken {
    pub kind: MobileSyntaxType,
    // Change u32 -> i32 for compatibility with Kotlin String indices
    pub start: i32,
    pub end: i32,
}

#[derive(uniffi::Record)]
pub struct MobileTask {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub is_done: bool,
    pub priority: u8,
    pub due_date_iso: Option<String>,
    pub is_allday_due: bool, // NEW
    pub start_date_iso: Option<String>,
    pub is_allday_start: bool,
    pub has_alarms: bool,
    pub is_future_start: bool,
    pub duration_mins: Option<u32>,
    pub duration_max_mins: Option<u32>,
    pub calendar_href: String,
    pub categories: Vec<String>,
    pub is_recurring: bool,
    pub parent_uid: Option<String>,
    pub smart_string: String,
    pub depth: u32,
    pub is_blocked: bool,
    pub status_string: String,
    pub blocked_by_names: Vec<String>,
    pub blocked_by_uids: Vec<String>,
    pub related_to_uids: Vec<String>,
    pub related_to_names: Vec<String>,
    pub is_paused: bool,
    pub location: Option<String>,
    pub url: Option<String>,
    pub geo: Option<String>,
}

#[derive(uniffi::Record)]
pub struct MobileCalendar {
    pub name: String,
    pub href: String,
    pub color: Option<String>,
    pub is_visible: bool,
    pub is_local: bool,
    pub is_disabled: bool,
}

#[derive(uniffi::Record)]
pub struct MobileTag {
    pub name: String,
    pub count: u32,
    pub is_uncategorized: bool,
}

#[derive(uniffi::Record)]
pub struct MobileRelatedTask {
    pub uid: String,
    pub summary: String,
}

#[derive(uniffi::Record)]
pub struct MobileLocation {
    pub name: String,
    pub count: u32,
}

#[derive(uniffi::Record)]
pub struct MobileAlarmInfo {
    pub task_uid: String,
    pub alarm_uid: String,
    pub title: String,
    pub body: String,
}

#[derive(uniffi::Record)]
pub struct MobileConfig {
    pub url: String,
    pub username: String,
    pub default_calendar: Option<String>,
    pub allow_insecure: bool,
    pub hide_completed: bool,
    pub tag_aliases: HashMap<String, Vec<String>>,
    pub disabled_calendars: Vec<String>,
    pub sort_cutoff_months: Option<u32>,
    pub urgent_days: u32,
    pub urgent_prio: u8,
    pub default_priority: u8,
    pub start_grace_period_days: u32,
    pub auto_reminders: bool,
    pub default_reminder_time: String,
    pub snooze_short: u32,
    pub snooze_long: u32,
    pub create_events_for_tasks: bool,
    pub delete_events_on_completion: bool,
}

fn task_to_mobile(t: &Task, store: &TaskStore) -> MobileTask {
    let smart = t.to_smart_string();
    let status_str = format!("{:?}", t.status);

    // CHANGE: Use cached field populated by filter()
    let is_blocked = t.is_blocked;
    let blocked_by_names = t
        .dependencies
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();

    let related_to_names = t
        .related_to
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();

    let (due_iso, due_allday) = match &t.due {
        Some(DateType::AllDay(d)) => (Some(d.format("%Y-%m-%d").to_string()), true),
        Some(DateType::Specific(dt)) => (Some(dt.to_rfc3339()), false),
        None => (None, false),
    };

    let (start_iso, start_allday) = match &t.dtstart {
        Some(DateType::AllDay(d)) => (Some(d.format("%Y-%m-%d").to_string()), true),
        Some(DateType::Specific(dt)) => (Some(dt.to_rfc3339()), false),
        None => (None, false),
    };

    let has_alarms = !t
        .alarms
        .iter()
        .all(|a| a.acknowledged.is_some() || a.is_snooze());

    // Calculate future start flag
    let now = Utc::now();
    let is_future_start = if let Some(start) = &t.dtstart {
        start.to_start_comparison_time() > now
    } else {
        false
    };

    MobileTask {
        uid: t.uid.clone(),
        summary: t.summary.clone(),
        description: t.description.clone(),
        is_done: t.status.is_done(),
        priority: t.priority,
        due_date_iso: due_iso,
        is_allday_due: due_allday,
        start_date_iso: start_iso,
        is_allday_start: start_allday,
        has_alarms,
        is_future_start,
        duration_mins: t.estimated_duration,
        duration_max_mins: t.estimated_duration_max,
        calendar_href: t.calendar_href.clone(),
        categories: t.categories.clone(),
        is_recurring: t.rrule.is_some(),
        parent_uid: t.parent_uid.clone(),
        smart_string: smart,
        depth: t.depth as u32,
        is_blocked,
        status_string: status_str,
        blocked_by_names,
        blocked_by_uids: t.dependencies.clone(),
        related_to_uids: t.related_to.clone(),
        related_to_names,
        is_paused: t.is_paused(),
        location: t.location.clone(),
        url: t.url.clone(),
        geo: t.geo.clone(),
    }
}

#[derive(uniffi::Object)]
pub struct CfaitMobile {
    client: Arc<Mutex<Option<RustyClient>>>,
    store: Arc<Mutex<TaskStore>>,
    /// In-memory cache of the alarm index to avoid repeated disk reads
    /// Updated whenever tasks are modified or loaded from cache
    alarm_index_cache: Arc<Mutex<Option<AlarmIndex>>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl CfaitMobile {
    #[uniffi::constructor]
    pub fn new(android_files_dir: String) -> Self {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("CfaitRust"),
        );
        AppPaths::init_android_path(android_files_dir);
        Self {
            client: Arc::new(Mutex::new(None)),
            store: Arc::new(Mutex::new(TaskStore::new())),
            alarm_index_cache: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create_debug_export(&self) -> Result<String, MobileError> {
        // Thin exported wrapper - calls internal implementation which is platform-specific.
        self.create_debug_export_internal()
    }

    // Internal implementation not exported via UniFFI. Platform-specific behavior is gated here.
    fn create_debug_export_internal(&self) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        {
            let data_dir =
                AppPaths::get_data_dir().map_err(|e| MobileError::from(e.to_string()))?;
            let cache_dir =
                AppPaths::get_cache_dir().map_err(|e| MobileError::from(e.to_string()))?;
            // --- FIX: Get the config directory ---
            let config_dir =
                AppPaths::get_config_dir().map_err(|e| MobileError::from(e.to_string()))?;

            let export_path = cache_dir.join("cfait_debug_export.zip");
            let file = std::fs::File::create(&export_path)
                .map_err(|e| MobileError::from(e.to_string()))?;
            let mut zip = zip::ZipWriter::new(file);

            let options: zip::write::FileOptions<'_, ()> = {
                let o = zip::write::FileOptions::default();
                o.compression_method(zip::CompressionMethod::Deflated)
                    .unix_permissions(0o755)
            };

            // Helper to add a directory's contents to the zip
            let mut add_dir = |dir: &std::path::Path, prefix: &str| -> Result<(), MobileError> {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();

                            // Skip lock files and the export file itself
                            if file_name.ends_with(".lock") || file_name == "cfait_debug_export.zip"
                            {
                                continue;
                            }

                            let zip_path = format!("{}{}", prefix, file_name);
                            zip.start_file(zip_path, options)
                                .map_err(|e| MobileError::from(e.to_string()))?;

                            // Special handling for config.toml to redact credentials
                            if file_name == "config.toml" {
                                let mut config = Config::load().unwrap_or_default();
                                config.username = "[REDACTED]".to_string();
                                // If Config has a password field, redact it. Otherwise ignore.
                                // Use match to avoid compile error if field absent (but it exists in this crate).
                                config.password = "[REDACTED]".to_string();
                                // Serialize sanitized config
                                let toml_str = toml::to_string_pretty(&config).unwrap_or_default();
                                zip.write_all(toml_str.as_bytes())
                                    .map_err(|e| MobileError::from(e.to_string()))?;
                            } else {
                                // Copy raw file
                                let mut f = std::fs::File::open(&path)
                                    .map_err(|e| MobileError::from(e.to_string()))?;
                                let mut buffer = Vec::new();
                                f.read_to_end(&mut buffer)
                                    .map_err(|e| MobileError::from(e.to_string()))?;
                                zip.write_all(&buffer)
                                    .map_err(|e| MobileError::from(e.to_string()))?;
                            }
                        }
                    }
                }
                Ok(())
            };

            // Add Data Directory (journals, local calendars)
            add_dir(&data_dir, "data/")?;

            // --- FIX: Add the Config directory ---
            add_dir(&config_dir, "config/")?;

            // Add Cache Directory (remote calendar caches)
            add_dir(&cache_dir, "cache/")?;

            zip.finish().map_err(|e| MobileError::from(e.to_string()))?;

            return Ok(export_path.to_string_lossy().to_string());
        }

        #[cfg(not(target_os = "android"))]
        {
            // Debug export not available on non-Android targets.
            Err(MobileError::from(
                "Debug export is only available on Android",
            ))
        }
    }

    pub async fn add_alias(&self, key: String, tags: Vec<String>) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();

        crate::model::validate_alias_integrity(&key, &tags, &c.tag_aliases)
            .map_err(MobileError::from)?;

        c.tag_aliases.insert(key.clone(), tags.clone());
        c.save().map_err(MobileError::from)?;

        let mut store = self.store.lock().await;
        let modified = store.apply_alias_retroactively(&key, &tags);
        drop(store);

        if !modified.is_empty() {
            let client_guard = self.client.lock().await;
            if let Some(client) = &*client_guard {
                for mut t in modified {
                    let _ = client.update_task(&mut t).await;
                }
            } else {
                for t in modified {
                    let _ = crate::journal::Journal::push(crate::journal::Action::Update(t));
                }
            }
        }
        Ok(())
    }

    pub fn has_unsynced_changes(&self) -> bool {
        !crate::journal::Journal::load().is_empty()
    }

    pub fn export_local_ics(&self, calendar_href: String) -> Result<String, MobileError> {
        let tasks = LocalStorage::load_for_href(&calendar_href)
            .map_err(|e| MobileError::from(e.to_string()))?;
        Ok(LocalStorage::to_ics_string(&tasks))
    }

    pub fn import_local_ics(
        &self,
        calendar_href: String,
        ics_content: String,
    ) -> Result<String, MobileError> {
        let count = LocalStorage::import_from_ics(&calendar_href, &ics_content)
            .map_err(|e| MobileError::from(e.to_string()))?;

        Ok(format!("Successfully imported {} task(s)", count))
    }

    pub fn parse_smart_string(&self, input: String) -> Vec<MobileSyntaxToken> {
        let tokens = tokenize_smart_input(&input);

        let mut byte_to_utf16 = std::collections::BTreeMap::new();
        let mut byte_pos = 0;
        let mut utf16_pos = 0;

        byte_to_utf16.insert(0, 0);

        for c in input.chars() {
            byte_pos += c.len_utf8();
            utf16_pos += c.len_utf16();
            byte_to_utf16.insert(byte_pos, utf16_pos as i32); // Cast to i32 here
        }

        tokens
            .into_iter()
            .map(|t| {
                let start_16 = *byte_to_utf16.get(&t.start).unwrap_or(&0);
                let end_16 = *byte_to_utf16.get(&t.end).unwrap_or(&start_16);

                MobileSyntaxToken {
                    kind: MobileSyntaxType::from(t.kind),
                    start: start_16,
                    end: end_16,
                }
            })
            .collect()
    }

    pub fn get_config(&self) -> MobileConfig {
        let c = Config::load().unwrap_or_default();
        MobileConfig {
            url: c.url,
            username: c.username,
            default_calendar: c.default_calendar,
            allow_insecure: c.allow_insecure_certs,
            hide_completed: c.hide_completed,
            tag_aliases: c.tag_aliases,
            disabled_calendars: c.disabled_calendars,
            sort_cutoff_months: c.sort_cutoff_months,
            urgent_days: c.urgent_days_horizon,
            urgent_prio: c.urgent_priority_threshold,
            default_priority: c.default_priority,
            start_grace_period_days: c.start_grace_period_days,
            auto_reminders: c.auto_reminders,
            default_reminder_time: c.default_reminder_time,
            snooze_short: c.snooze_short_mins,
            snooze_long: c.snooze_long_mins,
            create_events_for_tasks: c.create_events_for_tasks,
            delete_events_on_completion: c.delete_events_on_completion,
        }
    }

    pub fn parse_duration_string(&self, val: String) -> Option<u32> {
        crate::model::parser::parse_duration(&val)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_config(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
        hide_completed: bool,
        disabled_calendars: Vec<String>,
        sort_cutoff_months: Option<u32>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
        start_grace_period_days: u32,
        auto_reminders: bool,
        default_reminder_time: String,
        snooze_short: u32,
        snooze_long: u32,
        create_events_for_tasks: bool,
        delete_events_on_completion: bool,
    ) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.url = url;
        c.username = user;
        if !pass.is_empty() {
            c.password = pass;
        }
        c.allow_insecure_certs = insecure;
        c.hide_completed = hide_completed;
        c.disabled_calendars = disabled_calendars;
        c.sort_cutoff_months = sort_cutoff_months;
        c.urgent_days_horizon = urgent_days;
        c.urgent_priority_threshold = urgent_prio;
        c.default_priority = default_priority;
        c.start_grace_period_days = start_grace_period_days;
        c.auto_reminders = auto_reminders;
        c.default_reminder_time = default_reminder_time;
        c.snooze_short_mins = snooze_short;
        c.snooze_long_mins = snooze_long;
        c.create_events_for_tasks = create_events_for_tasks;
        c.delete_events_on_completion = delete_events_on_completion;
        c.save().map_err(MobileError::from)
    }

    pub async fn delete_all_calendar_events(&self) -> Result<u32, MobileError> {
        let all_tasks: Vec<_> = {
            let store = self.store.lock().await;
            store.calendars.values().flatten().cloned().collect()
        };

        let client = {
            let client_opt = self.client.lock().await;
            client_opt
                .as_ref()
                .ok_or_else(|| MobileError::from("Offline"))?
                .clone()
        };

        // NEW CONCURRENT LOGIC
        let futures = all_tasks.into_iter().map(|task| {
            let c = client.clone();
            async move {
                match c.sync_task_companion_event(&task, false).await {
                    Ok(true) => 1,
                    _ => 0,
                }
            }
        });

        // Run 8 concurrent requests
        let count = stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<u32>>()
            .await
            .iter()
            .sum();

        Ok(count)
    }

    pub async fn create_missing_calendar_events(&self) -> Result<u32, MobileError> {
        let all_tasks: Vec<_> = {
            let store = self.store.lock().await;
            store.calendars.values().flatten().cloned().collect()
        };

        let client = {
            let client_opt = self.client.lock().await;
            client_opt
                .as_ref()
                .ok_or_else(|| MobileError::from("Offline"))?
                .clone()
        };

        // NEW CONCURRENT LOGIC
        let futures = all_tasks.into_iter().map(|task| {
            let c = client.clone();
            async move {
                match c.sync_task_companion_event(&task, true).await {
                    Ok(true) => 1,
                    _ => 0,
                }
            }
        });

        // Run 8 concurrent requests
        let count = stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<u32>>()
            .await
            .iter()
            .sum();

        Ok(count)
    }

    pub async fn add_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        if task_uid == blocker_uid {
            return Err(MobileError::from("Cannot depend on self"));
        }
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.add_dependency(id, blocker_uid)
        })
        .await
    }

    pub async fn remove_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.remove_dependency(id, &blocker_uid)
        })
        .await
    }

    pub async fn set_parent(
        &self,
        child_uid: String,
        parent_uid: Option<String>,
    ) -> Result<(), MobileError> {
        if let Some(p) = &parent_uid
            && *p == child_uid
        {
            return Err(MobileError::from("Cannot be child of self"));
        }
        self.apply_store_mutation(child_uid, |store: &mut TaskStore, id: &str| {
            store.set_parent(id, parent_uid)
        })
        .await
    }

    pub async fn add_related_to(
        &self,
        task_uid: String,
        related_uid: String,
    ) -> Result<(), MobileError> {
        if task_uid == related_uid {
            return Err(MobileError::from("Cannot relate to self"));
        }
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.add_related_to(id, related_uid)
        })
        .await
    }

    pub async fn remove_related_to(
        &self,
        task_uid: String,
        related_uid: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.remove_related_to(id, &related_uid)
        })
        .await
    }

    /// Get all tasks that have a related_to link to the given task
    pub async fn get_tasks_related_to(&self, uid: String) -> Vec<MobileRelatedTask> {
        let store = self.store.lock().await;
        store
            .get_tasks_related_to(&uid)
            .into_iter()
            .map(|(uid, summary)| MobileRelatedTask { uid, summary })
            .collect()
    }

    pub fn isolate_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        let all_cals = self.get_calendars();
        let mut new_hidden = vec![];
        for cal in all_cals {
            if cal.href != href {
                new_hidden.push(cal.href.clone());
            }
        }
        config.hidden_calendars = new_hidden;
        config.default_calendar = Some(href);
        config.save().map_err(MobileError::from)
    }

    pub fn remove_alias(&self, key: String) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.tag_aliases.remove(&key);
        c.save().map_err(MobileError::from)
    }

    pub fn set_default_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        config.default_calendar = Some(href.clone());
        // Ensure the write calendar is always visible
        config.hidden_calendars.retain(|h| h != &href);
        config.save().map_err(MobileError::from)
    }

    pub fn set_calendar_visibility(&self, href: String, visible: bool) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        if visible {
            config.hidden_calendars.retain(|h| h != &href);
        } else if !config.hidden_calendars.contains(&href) {
            config.hidden_calendars.push(href);
        }
        config.save().map_err(MobileError::from)
    }

    pub fn load_from_cache(&self) {
        let mut store = self.store.blocking_lock();
        store.clear();

        // Load all local calendars
        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                match LocalStorage::load_for_href(&loc.href) {
                    Ok(mut tasks) => {
                        crate::journal::Journal::apply_to_tasks(&mut tasks, &loc.href);
                        store.insert(loc.href, tasks);
                    }
                    Err(e) => {
                        #[cfg(target_os = "android")]
                        log::error!(
                            "Failed to load {} - this may indicate data corruption or format incompatibility: {}",
                            loc.href,
                            e
                        );
                        #[cfg(not(target_os = "android"))]
                        eprintln!(
                            "Failed to load {} - this may indicate data corruption or format incompatibility: {}",
                            loc.href, e
                        );
                    }
                }
            }
        }

        // Load remote calendars
        if let Ok(cals) = Cache::load_calendars() {
            for cal in cals {
                if cal.href.starts_with("local://") {
                    continue; // Skip locals, already loaded from registry
                }
                if let Ok((mut tasks, _)) = Cache::load(&cal.href) {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &cal.href);
                    store.insert(cal.href, tasks);
                }
            }
        }

        // Rebuild the alarm index after loading all calendars
        let config = Config::load().unwrap_or_default();
        let index = AlarmIndex::rebuild_from_tasks(
            &store.calendars,
            config.auto_reminders,
            &config.default_reminder_time,
        );
        if let Err(e) = index.save() {
            #[cfg(target_os = "android")]
            log::warn!("Failed to save alarm index: {}", e);
            #[cfg(not(target_os = "android"))]
            let _ = e; // Suppress unused variable warning
        } else {
            #[cfg(target_os = "android")]
            log::debug!("Alarm index rebuilt with {} alarms", index.len());
        }

        // Cache the index in memory to avoid double disk reads
        *self.alarm_index_cache.blocking_lock() = Some(index);
    }

    pub async fn sync(&self) -> Result<String, MobileError> {
        let config = Config::load().map_err(MobileError::from)?;
        self.apply_connection(config).await
    }

    pub async fn connect(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
    ) -> Result<String, MobileError> {
        let mut config = Config::load().unwrap_or_default();
        config.url = url;
        config.username = user;
        if !pass.is_empty() {
            config.password = pass;
        }
        config.allow_insecure_certs = insecure;
        self.apply_connection(config).await
    }

    pub fn get_calendars(&self) -> Vec<MobileCalendar> {
        let config = Config::load().unwrap_or_default();
        let disabled_set: HashSet<String> = config.disabled_calendars.iter().cloned().collect();
        let mut result = Vec::new();

        // Load local registry
        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                result.push(MobileCalendar {
                    name: loc.name,
                    href: loc.href.clone(),
                    color: loc.color,
                    is_visible: !config.hidden_calendars.contains(&loc.href),
                    is_local: true,
                    is_disabled: disabled_set.contains(&loc.href),
                });
            }
        }

        // Load remote calendars
        if let Ok(cals) = crate::cache::Cache::load_calendars() {
            for c in cals {
                if c.href.starts_with("local://") {
                    continue; // Skip locals, already added from registry
                }
                result.push(MobileCalendar {
                    name: c.name,
                    href: c.href.clone(),
                    color: c.color,
                    is_visible: !config.hidden_calendars.contains(&c.href),
                    is_local: false,
                    is_disabled: disabled_set.contains(&c.href),
                });
            }
        }
        result
    }

    pub async fn get_all_tags(&self) -> Vec<MobileTag> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let empty_includes = HashSet::new();
        let mut hidden_cals: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden_cals.extend(config.disabled_calendars);
        store
            .get_all_categories(
                config.hide_completed,
                config.hide_fully_completed_tags,
                &empty_includes,
                &hidden_cals,
            )
            .into_iter()
            .map(|(name, count)| MobileTag {
                name: name.clone(),
                count: count as u32,
                is_uncategorized: name == UNCATEGORIZED_ID,
            })
            .collect()
    }

    pub async fn get_all_locations(&self) -> Vec<MobileLocation> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let mut hidden_cals: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden_cals.extend(config.disabled_calendars);

        store
            .get_all_locations(config.hide_completed, &hidden_cals)
            .into_iter()
            .map(|(name, count)| MobileLocation {
                name,
                count: count as u32,
            })
            .collect()
    }

    pub async fn get_view_tasks(
        &self,
        filter_tags: Vec<String>,
        filter_locations: Vec<String>,
        search_query: String,
    ) -> Vec<MobileTask> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();

        let mut selected_categories = HashSet::new();
        for tag in filter_tags {
            selected_categories.insert(tag);
        }
        let mut selected_locations = HashSet::new();
        for l in filter_locations {
            selected_locations.insert(l);
        }

        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        let cutoff_date = config
            .sort_cutoff_months
            .map(|months| chrono::Utc::now() + chrono::Duration::days(months as i64 * 30));

        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &selected_categories,
            selected_locations: &selected_locations,
            match_all_categories: false,
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
        });

        filtered
            .into_iter()
            .map(|t| task_to_mobile(&t, &store))
            .collect()
    }

    pub async fn get_random_task_uid(
        &self,
        filter_tags: Vec<String>,
        filter_locations: Vec<String>,
        search_query: String,
    ) -> Option<String> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();

        let mut selected_categories = HashSet::new();
        for tag in filter_tags {
            selected_categories.insert(tag);
        }
        let mut selected_locations = HashSet::new();
        for l in filter_locations {
            selected_locations.insert(l);
        }

        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        let cutoff_date = config
            .sort_cutoff_months
            .map(|months| chrono::Utc::now() + chrono::Duration::days(months as i64 * 30));

        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &selected_categories,
            selected_locations: &selected_locations,
            match_all_categories: false,
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
        });

        // Use the shared core logic which handles weights and ignores done tasks
        let idx = crate::store::select_weighted_random_index(&filtered, config.default_priority)?;
        filtered.get(idx).map(|t| t.uid.clone())
    }

    pub async fn yank_task(&self, _uid: String) -> Result<(), MobileError> {
        Ok(())
    }

    pub async fn add_task_smart(&self, input: String) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        log::debug!("add_task_smart() called with input: '{}'", input);

        // Load mutable config so we can update aliases if present
        let mut config = Config::load().unwrap_or_default();

        // 1. Extract aliases from inline input (e.g., "#alias:=#real")
        // Returns (clean_input, new_aliases_map)
        let (clean_input, new_aliases) = crate::model::extract_inline_aliases(&input);

        if !new_aliases.is_empty() {
            // Validate
            for (k, v) in &new_aliases {
                crate::model::validate_alias_integrity(k, v, &config.tag_aliases)
                    .map_err(MobileError::from)?;
            }

            // Update config
            config.tag_aliases.extend(new_aliases.clone());
            config.save().map_err(MobileError::from)?;

            // Apply retroactive updates to store
            let mut store = self.store.lock().await;
            let mut all_modified = Vec::new();

            for (key, tags) in &new_aliases {
                let modified = store.apply_alias_retroactively(key, tags);
                all_modified.extend(modified);
            }
            drop(store);

            // Sync modified tasks
            if !all_modified.is_empty() {
                let client_guard = self.client.lock().await;
                if let Some(client) = &*client_guard {
                    for mut t in all_modified {
                        let _ = client.update_task(&mut t).await;
                    }
                } else {
                    for t in all_modified {
                        let _ = crate::journal::Journal::push(crate::journal::Action::Update(t));
                    }
                }
            }

            // CHECK: Was this a pure definition?
            // If the remaining input is just the alias key itself (single token starting with #, @@, or loc:),
            // and no other text, we assume the user only wanted to define the alias, not create a task named "#alias".
            let trimmed = clean_input.trim();
            let is_pure_definition = trimmed.is_empty()
                || (!trimmed.contains(' ')
                    && (trimmed.starts_with('#')
                        || trimmed.starts_with("@@")
                        || trimmed.to_lowercase().starts_with("loc:")));

            if is_pure_definition {
                return Ok("ALIAS_UPDATED".to_string());
            }
        }

        // If input is empty after extraction (no aliases or not handled above), we are done
        if clean_input.trim().is_empty() {
            return Ok("".to_string());
        }

        // Parse time from config
        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

        // Create task with cleaned input and updated aliases
        let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);

        #[cfg(target_os = "android")]
        log::debug!(
            "Created task: uid={}, summary='{}', has_alarms={}",
            task.uid,
            task.summary,
            !task.alarms.is_empty()
        );

        let target_href = config
            .default_calendar
            .clone()
            .unwrap_or(LOCAL_CALENDAR_HREF.to_string());
        task.calendar_href = target_href.clone();

        self.store.lock().await.add_task(task.clone());

        let guard = self.client.lock().await;
        let mut network_success = false;

        if let Some(client) = &*guard
            && client.create_task(&mut task).await.is_ok()
        {
            // Assign a placeholder etag to prevent ghost pruning.
            if task.etag.is_empty() {
                task.etag = "pending_refresh".to_string();
            }

            self.store.lock().await.update_or_add_task(task.clone());
            network_success = true;

            #[cfg(target_os = "android")]
            log::debug!(
                "Task {} created on network, assigned placeholder etag to prevent ghost pruning",
                task.uid
            );
        }

        if !network_success {
            if task.calendar_href.starts_with("local://") {
                let mut all = LocalStorage::load_for_href(&task.calendar_href).unwrap_or_default();
                all.push(task.clone());
                LocalStorage::save_for_href(&task.calendar_href, &all)
                    .map_err(MobileError::from)?;
            } else {
                crate::journal::Journal::push(crate::journal::Action::Create(task.clone()))
                    .map_err(MobileError::from)?;
            }
            self.store.lock().await.update_or_add_task(task.clone());
        }

        // Rebuild alarm index after adding task
        #[cfg(target_os = "android")]
        log::debug!("Rebuilding alarm index after adding task {}", task.uid);

        let store_for_rebuild = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store_for_rebuild);

        #[cfg(target_os = "android")]
        log::debug!("Alarm index rebuilt. Returning task uid: {}", task.uid);

        Ok(task.uid)
    }

    pub async fn change_priority(&self, uid: String, delta: i8) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.change_priority(id, delta)
        })
        .await
    }

    pub async fn set_status_process(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status(id, crate::model::TaskStatus::InProcess)
        })
        .await
    }

    pub async fn set_status_cancelled(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status(id, crate::model::TaskStatus::Cancelled)
        })
        .await
    }

    pub async fn pause_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| store.pause_task(id))
            .await
    }

    pub async fn stop_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| store.stop_task(id))
            .await
    }

    pub async fn start_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status_in_process(id)
        })
        .await
    }

    pub async fn update_task_smart(
        &self,
        uid: String,
        smart_input: String,
    ) -> Result<(), MobileError> {
        let config = Config::load().unwrap_or_default();
        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

        self.apply_store_mutation(uid, |t: &mut TaskStore, id: &str| {
            if let Some((task, _)) = t.get_task_mut(id) {
                task.apply_smart_input(&smart_input, &config.tag_aliases, def_time);
                Some(task.clone())
            } else {
                None
            }
        })
        .await
    }

    pub async fn update_task_description(
        &self,
        uid: String,
        description: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |t: &mut TaskStore, id: &str| {
            if let Some((task, _)) = t.get_task_mut(id) {
                task.description = description;
                Some(task.clone())
            } else {
                None
            }
        })
        .await
    }

    pub async fn toggle_task(&self, uid: String) -> Result<(), MobileError> {
        let mut store = self.store.lock().await;
        let (task, _) = store
            .get_task_mut(&uid)
            .ok_or(MobileError::from("Task not found"))?;

        if task.status.is_done() {
            task.status = crate::model::TaskStatus::NeedsAction;
            task.percent_complete = None;
        } else {
            task.status = crate::model::TaskStatus::Completed;
            task.percent_complete = Some(100);
            // Add COMPLETED date property so mobile/offline toggles include the timestamp
            let now_str = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            task.unmapped_properties.retain(|p| p.key != "COMPLETED");
            task.unmapped_properties.push(crate::model::RawProperty {
                key: "COMPLETED".to_string(),
                value: now_str,
                params: vec![],
            });
        }

        let mut task_for_net = task.clone();
        drop(store);

        let client_guard = self.client.lock().await;
        let mut network_success = false;

        if let Some(client) = &*client_guard
            && let Ok((_, next_task_opt, _)) = client.toggle_task(&mut task_for_net).await
        {
            // Lock the store once, update any history item and then the recycled task exactly once
            let mut store = self.store.lock().await;
            if let Some(next_task) = next_task_opt {
                store.update_or_add_task(next_task);
            }
            // Save recycled task (clone to satisfy borrow checker)
            store.update_or_add_task(task_for_net.clone());
            network_success = true;
        }

        if !network_success {
            if task_for_net.calendar_href.starts_with("local://") {
                let cal_href = task_for_net.calendar_href.clone();
                let mut local = LocalStorage::load_for_href(&cal_href).unwrap_or_default();
                if let Some(idx) = local.iter().position(|t| t.uid == task_for_net.uid) {
                    local[idx] = task_for_net;
                    LocalStorage::save_for_href(&cal_href, &local).map_err(MobileError::from)?;
                }
            } else {
                crate::journal::Journal::push(crate::journal::Action::Update(task_for_net))
                    .map_err(MobileError::from)?;
            }
        }

        // Rebuild alarm index after toggling task
        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);

        Ok(())
    }

    pub async fn move_task(&self, uid: String, new_cal_href: String) -> Result<(), MobileError> {
        let client_guard = self.client.lock().await;
        let mut store = self.store.lock().await;

        // Use the atomic store API that returns both the original (pre-mutation)
        // and updated (post-mutation) task so we don't need to clone/look up
        // the original separately and risk races.
        let (original_task, _updated_task) = store
            .move_task(&uid, new_cal_href.clone())
            .ok_or(MobileError::from("Task not found"))?;

        drop(store);

        let mut network_success = false;
        // Pass the original (pre-mutation) task to the client so the backend
        // / journal can identify the source calendar.
        if let Some(client) = &*client_guard
            && client
                .move_task(&original_task, &new_cal_href)
                .await
                .is_ok()
        {
            network_success = true;
        }

        if !network_success && !new_cal_href.starts_with("local://") {
            // Pass the original (pre-mutation) task to the Journal
            crate::journal::Journal::push(crate::journal::Action::Move(
                original_task,
                new_cal_href,
            ))
            .map_err(MobileError::from)?;
        }

        // Rebuild alarm index after moving task
        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);

        Ok(())
    }

    pub async fn delete_task(&self, uid: String) -> Result<(), MobileError> {
        let mut store = self.store.lock().await;
        let (task, href) = store
            .delete_task(&uid)
            .ok_or(MobileError::from("Task not found"))?;
        drop(store);

        let client_guard = self.client.lock().await;
        let mut network_success = false;

        if let Some(client) = &*client_guard
            && client.delete_task(&task).await.is_ok()
        {
            network_success = true;
        }

        if !network_success && !href.starts_with("local://") {
            crate::journal::Journal::push(crate::journal::Action::Delete(task))
                .map_err(MobileError::from)?;
        }

        // Rebuild alarm index after deleting task
        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);

        Ok(())
    }

    pub async fn migrate_local_to(
        &self,
        source_calendar_href: String,
        target_calendar_href: String,
    ) -> Result<String, MobileError> {
        let client_guard = self.client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or(MobileError::from("Client not connected"))?;

        let local_tasks = LocalStorage::load_for_href(&source_calendar_href)
            .map_err(|e| MobileError::from(e.to_string()))?;
        if local_tasks.is_empty() {
            return Ok("No local tasks to migrate.".to_string());
        }

        let count = client
            .migrate_tasks(local_tasks, &target_calendar_href)
            .await
            .map_err(MobileError::from)?;

        Ok(format!("Successfully migrated {} tasks.", count))
    }

    pub async fn create_local_calendar(
        &self,
        name: String,
        color: Option<String>,
    ) -> Result<String, MobileError> {
        let mut locals =
            LocalCalendarRegistry::load().map_err(|e| MobileError::from(e.to_string()))?;

        let id = Uuid::new_v4().to_string();
        let href = format!("local://{}", id);

        let new_cal = crate::model::CalendarListEntry {
            name,
            href: href.clone(),
            color,
        };

        locals.push(new_cal);
        LocalCalendarRegistry::save(&locals).map_err(|e| MobileError::from(e.to_string()))?;

        // Update in-memory store
        let mut store = self.store.lock().await;
        store.insert(href.clone(), vec![]);

        Ok(href)
    }

    pub async fn update_local_calendar(
        &self,
        href: String,
        name: String,
        color: Option<String>,
    ) -> Result<(), MobileError> {
        let mut locals =
            LocalCalendarRegistry::load().map_err(|e| MobileError::from(e.to_string()))?;

        if let Some(cal) = locals.iter_mut().find(|c| c.href == href) {
            cal.name = name;
            cal.color = color;
            LocalCalendarRegistry::save(&locals).map_err(|e| MobileError::from(e.to_string()))?;

            // Note: We don't need to update the Store for name/color changes as those are metadata,
            // but we might want to trigger a refresh in the UI.
            return Ok(());
        }

        Err(MobileError::from("Calendar not found"))
    }

    pub async fn delete_local_calendar(&self, href: String) -> Result<(), MobileError> {
        if href == LOCAL_CALENDAR_HREF {
            return Err(MobileError::from("Cannot delete default local calendar"));
        }

        let mut locals =
            LocalCalendarRegistry::load().map_err(|e| MobileError::from(e.to_string()))?;

        if let Some(idx) = locals.iter().position(|c| c.href == href) {
            locals.remove(idx);
            LocalCalendarRegistry::save(&locals).map_err(|e| MobileError::from(e.to_string()))?;

            // Remove data file
            if let Some(path) = LocalStorage::get_path_for_href(&href) {
                let _ = std::fs::remove_file(path);
            }

            // Update in-memory store
            let mut store = self.store.lock().await;
            store.remove(&href);

            // Rebuild alarm index to remove alarms from deleted calendar
            self.rebuild_alarm_index_sync(&store);

            return Ok(());
        }

        Err(MobileError::from("Calendar not found"))
    }

    pub async fn snooze_alarm(
        &self,
        task_uid: String,
        alarm_uid: String,
        minutes: u32,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(task_uid.clone(), |store, id| {
            if let Some((task, _)) = store.get_task_mut(id) {
                // Check for implicit alarm
                if alarm_uid.starts_with("implicit_") {
                    let parts: Vec<&str> = alarm_uid.split('|').collect();
                    if parts.len() >= 2
                        && let Ok(dt) = DateTime::parse_from_rfc3339(parts[1])
                    {
                        let utc_dt = dt.with_timezone(&Utc);
                        let desc = if alarm_uid.contains("implicit_due") {
                            "Due now"
                        } else {
                            "Starting"
                        };
                        task.snooze_implicit_alarm(utc_dt, desc.to_string(), minutes);
                        return Some(task.clone());
                    }
                }

                if task.snooze_alarm(&alarm_uid, minutes) {
                    return Some(task.clone());
                }
            }
            None
        })
        .await?;

        // Rebuild alarm index after snoozing alarm
        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);

        Ok(())
    }

    pub async fn dismiss_alarm(
        &self,
        task_uid: String,
        alarm_uid: String,
    ) -> Result<(), MobileError> {
        #[cfg(target_os = "android")]
        log::debug!(
            "dismiss_alarm called: task_uid={}, alarm_uid={}",
            task_uid,
            alarm_uid
        );

        self.apply_store_mutation(task_uid.clone(), |store, id| {
            if let Some((task, _)) = store.get_task_mut(id) {
                #[cfg(target_os = "android")]
                log::debug!(
                    "Found task '{}' with {} alarms before dismiss",
                    task.summary,
                    task.alarms.len()
                );

                // Implicit Alarm Handling
                if alarm_uid.starts_with("implicit_") {
                    let parts: Vec<&str> = alarm_uid.split('|').collect();
                    if parts.len() >= 2
                        && let Ok(dt) = DateTime::parse_from_rfc3339(parts[1])
                    {
                        let utc_dt = dt.with_timezone(&Utc);
                        let desc = if alarm_uid.contains("implicit_due") {
                            "Due now"
                        } else {
                            "Starting"
                        };
                        task.dismiss_implicit_alarm(utc_dt, desc.to_string());
                        #[cfg(target_os = "android")]
                        log::debug!("Dismissed implicit alarm: {}", alarm_uid);
                        return Some(task.clone());
                    }
                }

                if task.dismiss_alarm(&alarm_uid) {
                    #[cfg(target_os = "android")]
                    log::debug!(
                        "Successfully dismissed explicit alarm, task now has {} alarms",
                        task.alarms.len()
                    );
                    return Some(task.clone());
                } else {
                    #[cfg(target_os = "android")]
                    log::warn!("dismiss_alarm returned false - alarm not found or not dismissible");
                }
            } else {
                #[cfg(target_os = "android")]
                log::warn!("Task {} not found in store during dismiss", id);
            }
            None
        })
        .await?;

        // Rebuild alarm index after dismissing alarm
        #[cfg(target_os = "android")]
        log::debug!("Rebuilding alarm index after dismissing alarm");

        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);

        #[cfg(target_os = "android")]
        log::debug!("dismiss_alarm completed successfully");

        Ok(())
    }

    /// Used by Android WorkManager to schedule the next wakeup
    /// Returns: timestamp (seconds) of the very next alarm across ALL tasks
    pub async fn get_next_global_alarm_time(&self) -> Option<i64> {
        let store = self.store.lock().await;
        let mut global_earliest: Option<i64> = None;

        for tasks in store.calendars.values() {
            for task in tasks {
                // Skip completed tasks? Usually yes for alarms.
                if task.status.is_done() {
                    continue;
                }

                if let Some(ts) = task.next_trigger_timestamp() {
                    match global_earliest {
                        Some(current) if ts < current => global_earliest = Some(ts),
                        None => global_earliest = Some(ts),
                        _ => {}
                    }
                }
            }
        }
        global_earliest
    }

    /// Returns the timestamp (seconds) of the next alarm (explicit or implicit).
    /// Used by Android to schedule AlarmManager.
    ///
    /// PERFORMANCE OPTIMIZATION: Uses the alarm index cache for fast lookups.
    /// - Without cache: O(N) - must scan all tasks
    /// - With cache: O(1) - direct lookup of next alarm from sorted index
    ///
    /// Falls back to full scan if index is missing.
    pub fn get_next_alarm_timestamp(&self) -> Option<i64> {
        // Try in-memory cache first (avoids disk read)
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(ref index) = *cached
            && !index.is_empty()
        {
            if let Some(timestamp) = index.get_next_alarm_timestamp() {
                #[cfg(target_os = "android")]
                log::debug!("Next alarm timestamp from cached index: {}", timestamp);
                return Some(timestamp as i64);
            } else {
                #[cfg(target_os = "android")]
                log::debug!("No future alarms in cached index");
                return None;
            }
        }
        drop(cached);

        // Fallback: Load from disk if cache miss
        let index = AlarmIndex::load();
        if !index.is_empty() {
            if let Some(timestamp) = index.get_next_alarm_timestamp() {
                #[cfg(target_os = "android")]
                log::debug!("Next alarm timestamp from disk index: {}", timestamp);
                // Update cache for next time
                *self.alarm_index_cache.blocking_lock() = Some(index);
                return Some(timestamp as i64);
            } else {
                #[cfg(target_os = "android")]
                log::debug!("No future alarms in disk index");
                return None;
            }
        }

        // Final fallback: Index doesn't exist, do full scan
        #[cfg(target_os = "android")]
        log::warn!("Alarm index not available for next_alarm_timestamp, falling back to full scan");

        let store = self.store.blocking_lock();
        let config = Config::load().unwrap_or_default();
        let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

        let now = Utc::now();
        let mut global_earliest: Option<i64> = None;

        let check_ts = |ts: i64, current_earliest: &mut Option<i64>| {
            if ts > now.timestamp() {
                match current_earliest {
                    Some(e) if ts < *e => *current_earliest = Some(ts),
                    None => *current_earliest = Some(ts),
                    _ => {}
                }
            }
        };

        for tasks in store.calendars.values() {
            for task in tasks {
                if task.status.is_done() {
                    continue;
                }

                // 1. Explicit Alarms
                if let Some(ts) = task.next_trigger_timestamp() {
                    check_ts(ts, &mut global_earliest);
                }

                // 2. Implicit Alarms
                if config.auto_reminders {
                    // Only check if no active explicit alarms exist
                    let has_active_explicit = task
                        .alarms
                        .iter()
                        .any(|a| a.acknowledged.is_none() && !a.is_snooze());

                    if !has_active_explicit {
                        if let Some(due) = &task.due {
                            let dt = match due {
                                DateType::Specific(t) => *t,
                                DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(Local)
                                    .unwrap()
                                    .with_timezone(&Utc),
                            };
                            // Only if not dismissed
                            if !task.has_alarm_at(dt) {
                                check_ts(dt.timestamp(), &mut global_earliest);
                            }
                        }
                        if let Some(start) = &task.dtstart {
                            let dt = match start {
                                DateType::Specific(t) => *t,
                                DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(Local)
                                    .unwrap()
                                    .with_timezone(&Utc),
                            };
                            if !task.has_alarm_at(dt) {
                                check_ts(dt.timestamp(), &mut global_earliest);
                            }
                        }
                    }
                }
            }
        }
        global_earliest
    }

    /// Called by Android when the AlarmManager wakes up.
    /// Returns all alarms that should be firing NOW (with a small grace period).
    ///
    /// PERFORMANCE OPTIMIZATION: Uses the alarm index cache for fast lookups.
    /// - Without cache: O(N) - must parse all tasks (~2-3 seconds for 1000 tasks)
    /// - With cache: O(1) or O(log N) - direct lookup (~30-50ms)
    ///
    /// Falls back to full scan if index is missing or corrupted.
    pub fn get_firing_alarms(&self) -> Vec<MobileAlarmInfo> {
        // Try in-memory cache first (avoids disk read)
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(ref index) = *cached
            && !index.is_empty()
        {
            #[cfg(target_os = "android")]
            log::debug!(
                "Using cached alarm index for fast lookup ({} alarms indexed)",
                index.len()
            );
            let firing = index.get_firing_alarms();

            if !firing.is_empty() {
                #[cfg(target_os = "android")]
                log::info!("Found {} firing alarm(s) via cached index", firing.len());
                return firing
                    .into_iter()
                    .map(|entry| MobileAlarmInfo {
                        task_uid: entry.task_uid,
                        alarm_uid: entry.alarm_uid,
                        title: entry.task_title,
                        body: entry.description.unwrap_or_else(|| "Reminder".to_string()),
                    })
                    .collect();
            } else {
                #[cfg(target_os = "android")]
                log::debug!("No firing alarms found in cached index");
                return Vec::new();
            }
        }
        drop(cached);

        // Fallback: Load from disk if cache miss
        let index = AlarmIndex::load();
        if !index.is_empty() {
            #[cfg(target_os = "android")]
            log::debug!(
                "Using disk alarm index for fast lookup ({} alarms indexed)",
                index.len()
            );
            let firing = index.get_firing_alarms();

            if !firing.is_empty() {
                #[cfg(target_os = "android")]
                log::info!("Found {} firing alarm(s) via disk index", firing.len());
                // Update cache for next time
                *self.alarm_index_cache.blocking_lock() = Some(index);
                return firing
                    .into_iter()
                    .map(|entry| MobileAlarmInfo {
                        task_uid: entry.task_uid,
                        alarm_uid: entry.alarm_uid,
                        title: entry.task_title,
                        body: entry.description.unwrap_or_else(|| "Reminder".to_string()),
                    })
                    .collect();
            } else {
                #[cfg(target_os = "android")]
                log::debug!("No firing alarms found in disk index");
                return Vec::new();
            }
        }

        // Final fallback: Index doesn't exist or is empty, do full scan
        #[cfg(target_os = "android")]
        log::warn!("Alarm index not available, falling back to full task scan (slow)");

        let store = self.store.blocking_lock();
        let config = Config::load().unwrap_or_default();
        let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

        let now = Utc::now();
        let mut results = Vec::new();

        for tasks in store.calendars.values() {
            for task in tasks {
                if task.status.is_done() {
                    continue;
                }

                // Check Explicit
                for alarm in &task.alarms {
                    // Do NOT filter out snoozes here.
                    if alarm.acknowledged.is_some() {
                        continue;
                    }

                    let trigger_dt = match alarm.trigger {
                        AlarmTrigger::Absolute(dt) => dt,
                        AlarmTrigger::Relative(mins) => {
                            let anchor = if let Some(DateType::Specific(d)) = task.due {
                                d
                            } else if let Some(DateType::Specific(s)) = task.dtstart {
                                s
                            } else {
                                continue;
                            };
                            anchor + chrono::Duration::minutes(mins as i64)
                        }
                    };

                    // Fire if in past (within 2 hours grace to avoid old spam)
                    if trigger_dt <= now && (now - trigger_dt).num_minutes() < 120 {
                        results.push(MobileAlarmInfo {
                            task_uid: task.uid.clone(),
                            alarm_uid: alarm.uid.clone(),
                            title: task.summary.clone(),
                            body: alarm
                                .description
                                .clone()
                                .unwrap_or_else(|| "Reminder".to_string()),
                        });
                    }
                }

                // Check Implicit
                if config.auto_reminders {
                    // Count snooze alarms as active so we don't double-fire
                    let has_active_explicit = task.alarms.iter().any(|a| a.acknowledged.is_none());
                    if !has_active_explicit {
                        // Helper for check
                        let mut check_implicit = |dt: DateTime<Utc>, desc: &str, type_key: &str| {
                            if !task.has_alarm_at(dt) && dt <= now && (now - dt).num_minutes() < 120
                            {
                                let ts_str = dt.to_rfc3339();
                                // Synthetic ID generation matching `system.rs`
                                let synth_id =
                                    format!("implicit_{}:|{}|{}", type_key, ts_str, task.uid);
                                results.push(MobileAlarmInfo {
                                    task_uid: task.uid.clone(),
                                    alarm_uid: synth_id,
                                    title: task.summary.clone(),
                                    body: desc.to_string(),
                                });
                            }
                        };

                        if let Some(due) = &task.due {
                            let dt = match due {
                                DateType::Specific(t) => *t,
                                DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(Local)
                                    .unwrap()
                                    .with_timezone(&Utc),
                            };
                            check_implicit(dt, "Due now", "due");
                        }
                        if let Some(start) = &task.dtstart {
                            let dt = match start {
                                DateType::Specific(t) => *t,
                                DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(Local)
                                    .unwrap()
                                    .with_timezone(&Utc),
                            };
                            check_implicit(dt, "Task starting", "start");
                        }
                    }
                }
            }
        }
        results
    }
}

impl CfaitMobile {
    /// Helper method to rebuild the alarm index from the current task store.
    /// Should be called whenever tasks are modified (add/update/delete/snooze/dismiss).
    /// Updates both the disk cache and in-memory cache.
    fn rebuild_alarm_index_sync(&self, store: &TaskStore) {
        let config = Config::load().unwrap_or_default();
        let index = AlarmIndex::rebuild_from_tasks(
            &store.calendars,
            config.auto_reminders,
            &config.default_reminder_time,
        );

        match index.save() {
            Ok(_) => {
                #[cfg(target_os = "android")]
                log::debug!("Alarm index rebuilt with {} alarms", index.len());
                // Update in-memory cache
                *self.alarm_index_cache.blocking_lock() = Some(index);
            }
            Err(e) => {
                #[cfg(target_os = "android")]
                log::warn!("Failed to save alarm index: {}", e);
                #[cfg(not(target_os = "android"))]
                let _ = e; // Suppress unused variable warning
            }
        }
    }

    async fn apply_connection(&self, config: Config) -> Result<String, MobileError> {
        let (client, cals, _, _, warning_from_fallback) =
            RustyClient::connect_with_fallback(config, Some("Android"))
                .await
                .map_err(MobileError::from)?;

        *self.client.lock().await = Some(client.clone());

        let fetch_result = client.get_all_tasks(&cals).await;

        let mut store = self.store.lock().await;
        store.clear();

        // Load all local calendars
        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                match LocalStorage::load_for_href(&loc.href) {
                    Ok(mut tasks) => {
                        crate::journal::Journal::apply_to_tasks(&mut tasks, &loc.href);
                        store.insert(loc.href, tasks);
                    }
                    Err(e) => {
                        #[cfg(target_os = "android")]
                        log::error!(
                            "Failed to load {} - this may indicate data corruption or format incompatibility: {}",
                            loc.href,
                            e
                        );
                        #[cfg(not(target_os = "android"))]
                        eprintln!(
                            "Failed to load {} - this may indicate data corruption or format incompatibility: {}",
                            loc.href, e
                        );
                    }
                }
            }
        }

        match fetch_result {
            Ok(results) => {
                let mut fetched_hrefs: HashSet<String> = HashSet::new();
                for (href, mut tasks) in results {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &href);
                    store.insert(href.clone(), tasks);
                    fetched_hrefs.insert(href);
                }
                for cal in &cals {
                    if !cal.href.starts_with("local://")
                        && !fetched_hrefs.contains(&cal.href)
                        && let Ok((mut cached, _)) = crate::cache::Cache::load(&cal.href)
                    {
                        crate::journal::Journal::apply_to_tasks(&mut cached, &cal.href);
                        store.insert(cal.href.clone(), cached);
                    }
                }
            }
            Err(e) => {
                for cal in &cals {
                    if !cal.href.starts_with("local://")
                        && !store.calendars.contains_key(&cal.href)
                        && let Ok((mut cached, _)) = crate::cache::Cache::load(&cal.href)
                    {
                        crate::journal::Journal::apply_to_tasks(&mut cached, &cal.href);
                        store.insert(cal.href.clone(), cached);
                    }
                }
                // Even on error, we must rebuild index for whatever data we loaded from cache
                self.rebuild_alarm_index_sync(&store);
                return Err(MobileError::from(e));
            }
        }

        // Rebuild the alarm index now that the store is updated
        self.rebuild_alarm_index_sync(&store);

        Ok(warning_from_fallback.unwrap_or_else(|| "Connected".to_string()))
    }

    async fn apply_store_mutation<F>(&self, uid: String, mutator: F) -> Result<(), MobileError>
    where
        F: FnOnce(&mut TaskStore, &str) -> Option<Task>,
    {
        let mut store = self.store.lock().await;
        let updated_task = mutator(&mut store, &uid)
            .ok_or(MobileError::from("Task not found or mutation failed"))?;

        let mut task_for_net = updated_task.clone();
        drop(store);

        let client_guard = self.client.lock().await;
        let mut network_success = false;

        if let Some(client) = &*client_guard
            && client.update_task(&mut task_for_net).await.is_ok()
        {
            // Assign a placeholder etag to prevent ghost pruning.
            // Similar to add_task_smart, we need to ensure the task has a non-empty etag
            // to survive ghost pruning when AlarmWorker creates a fresh CfaitMobile instance.
            if task_for_net.etag.is_empty() {
                task_for_net.etag = "pending_refresh".to_string();
            }

            let mut store = self.store.lock().await;
            store.update_or_add_task(task_for_net.clone());
            network_success = true;

            #[cfg(target_os = "android")]
            log::debug!(
                "Task {} updated on network, assigned placeholder etag to prevent ghost pruning",
                task_for_net.uid
            );
        }

        if !network_success {
            if task_for_net.calendar_href.starts_with("local://") {
                let cal_href = task_for_net.calendar_href.clone();
                let mut local = LocalStorage::load_for_href(&cal_href).unwrap_or_default();
                if let Some(idx) = local.iter().position(|t| t.uid == task_for_net.uid) {
                    local[idx] = task_for_net;
                    LocalStorage::save_for_href(&cal_href, &local).map_err(MobileError::from)?;
                }
            } else {
                crate::journal::Journal::push(crate::journal::Action::Update(task_for_net))
                    .map_err(MobileError::from)?;
            }
        }
        Ok(())
    }
}
