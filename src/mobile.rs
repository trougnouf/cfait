/*
 cfait/src/mobile.rs

 Mobile UniFFI bridge with multi-server account management.

 This file exposes account management and core mobile methods to the Kotlin/Swift
 layers via UniFFI. It supports multiple server accounts, testing connections,
 saving/deleting accounts, and performing a full sync that loads all accounts
 from disk and connects them.
*/

use crate::alarm_index::AlarmIndex;
use crate::cache::Cache;
use crate::client::ClientManager;
use crate::config::{Config, AccountConfig};
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::model::{DateType, Task};
use crate::paths::AppPaths;
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use crate::store::{FilterOptions, TaskStore, UNCATEGORIZED_ID};
use chrono::{NaiveTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(target_os = "android")]
use android_logger::Config as LogConfig;
#[cfg(target_os = "android")]
use log::LevelFilter;

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
    pub is_allday_due: bool,
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

#[derive(uniffi::Record, Clone)]
pub struct MobileAccount {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub allow_insecure: bool,
}

#[derive(uniffi::Record)]
pub struct MobileConfigUpdate {
    pub hide_completed: bool,
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
    let is_blocked = t.is_blocked;
    let blocked_by_names: Vec<String> = t
        .dependencies
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();
    let related_to_names: Vec<String> = t
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
    let has_alarms = !t.alarms.iter().all(|a| a.acknowledged.is_some() || a.is_snooze());
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
    client: Arc<Mutex<Option<ClientManager>>>,
    store: Arc<Mutex<TaskStore>>,
    alarm_index_cache: Arc<Mutex<Option<AlarmIndex>>>,
}

impl CfaitMobile {
    async fn apply_connection_internal(&self, config: Config) -> Result<String, MobileError> {
        let accounts = config.accounts.clone();

        let manager = ClientManager::new(&accounts, Some("Android")).await;
        *self.client.lock().await = Some(manager.clone());

        let cals = manager.get_all_calendars().await;
        let _ = Cache::save_calendars(&cals);

        let mut store = self.store.lock().await;
        store.clear();

        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                if let Ok(mut tasks) = LocalStorage::load_for_href(&loc.href) {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &loc.href);
                    store.insert(loc.href, tasks);
                }
            }
        }

        match manager.get_all_tasks(&cals).await {
            Ok(results) => {
                for (href, mut tasks) in results {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &href);
                    store.insert(href, tasks);
                }
            }
            Err(e) => return Err(MobileError::from(e)),
        }

        self.rebuild_alarm_index_sync(&store);
        Ok("Connected".to_string())
    }

    fn rebuild_alarm_index_sync(&self, store: &TaskStore) {
        let config = Config::load().unwrap_or_default();
        let index =
            AlarmIndex::rebuild_from_tasks(&store.calendars, config.auto_reminders, &config.default_reminder_time);
        let _ = index.save();
        // Update cached index (blocking lock used for quick sync with synchronous callers)
        *self.alarm_index_cache.blocking_lock() = Some(index);
    }

    fn resolve_account_id(&self, href: &str) -> String {
        if let Ok(cached) = Cache::load_calendars()
            && let Some(cal) = cached.iter().find(|c| c.href == href) {
                return cal.account_id.clone();
            }
        if let Ok(locals) = LocalCalendarRegistry::load()
            && let Some(cal) = locals.iter().find(|c| c.href == href) {
                return cal.account_id.clone();
            }
        "default".to_string()
    }

    async fn apply_store_mutation<F>(&self, uid: String, mutator: F) -> Result<(), MobileError>
    where
        F: FnOnce(&mut TaskStore, &str) -> Option<Task>,
    {
        let mut store = self.store.lock().await;
        let updated_task = mutator(&mut store, &uid).ok_or(MobileError::from("Task not found"))?;
        let mut task_for_net = updated_task.clone();
        drop(store);

        let acc_id = self.resolve_account_id(&task_for_net.calendar_href);
        let client_guard = self.client.lock().await;
        let mut network_success = false;

        if let Some(manager) = &*client_guard
            && let Some(client) = manager.get_client(&acc_id)
                && client.update_task(&mut task_for_net).await.is_ok() {
                    if task_for_net.etag.is_empty() {
                        task_for_net.etag = "pending_refresh".to_string();
                    }
                    let mut store = self.store.lock().await;
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
                crate::journal::Journal::push(crate::journal::Action::Update(task_for_net)).map_err(MobileError::from)?;
            }
        }
        Ok(())
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl CfaitMobile {
    #[uniffi::constructor]
    pub fn new(android_files_dir: String) -> Self {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            LogConfig::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("CfaitRust"),
        );
        AppPaths::init_android_path(android_files_dir);
        Self {
            client: Arc::new(Mutex::new(None)),
            store: Arc::new(Mutex::new(TaskStore::new())),
            alarm_index_cache: Arc::new(Mutex::new(None)),
        }
    }

    // Account Management
    pub fn get_accounts(&self) -> Vec<MobileAccount> {
        let config = Config::load().unwrap_or_default();
        config
            .accounts
            .into_iter()
            .map(|a| MobileAccount {
                id: a.id,
                name: a.name,
                url: a.url,
                username: a.username,
                password: a.password,
                allow_insecure: a.allow_insecure_certs,
            })
            .collect()
    }

    pub fn save_account(
        &self,
        id: String,
        name: String,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
    ) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        let new_acc = AccountConfig {
            id: if id.is_empty() { Uuid::new_v4().to_string() } else { id },
            name,
            url,
            username: user,
            password: pass,
            allow_insecure_certs: insecure,
        };

        if let Some(pos) = config.accounts.iter().position(|a| a.id == new_acc.id) {
            config.accounts[pos] = new_acc;
        } else {
            config.accounts.push(new_acc);
        }
        config.save().map_err(MobileError::from)
    }

    pub fn delete_account(&self, id: String) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        config.accounts.retain(|a| a.id != id);
        config.save().map_err(MobileError::from)
    }

    pub async fn validate_connection(&self, url: String, user: String, pass: String, insecure: bool) -> Result<String, MobileError> {
        // Test connection without saving
        match crate::client::RustyClient::new(&url, &user, &pass, insecure, Some("Android")) {
            Ok(client) => match client.get_calendars().await {
                Ok(cals) => Ok(format!("Success! Found {} calendars.", cals.len())),
                Err(e) => Err(MobileError::from(e)),
            },
            Err(e) => Err(MobileError::from(e)),
        }
    }

    // Main sync method now just loads config from disk and connects all accounts
    pub async fn sync(&self) -> Result<String, MobileError> {
        let config = Config::load().map_err(MobileError::from)?;
        self.apply_connection_internal(config).await
    }

    pub fn get_calendars(&self) -> Vec<MobileCalendar> {
        let config = Config::load().unwrap_or_default();
        let disabled_set: HashSet<String> = config.disabled_calendars.iter().cloned().collect();
        let mut result = Vec::new();

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
        if let Ok(cals) = Cache::load_calendars() {
            for c in cals {
                if c.href.starts_with("local://") {
                    continue;
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

    pub async fn add_alias(&self, key: String, tags: Vec<String>) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        crate::model::validate_alias_integrity(&key, &tags, &c.tag_aliases).map_err(MobileError::from)?;
        c.tag_aliases.insert(key.clone(), tags.clone());
        c.save().map_err(MobileError::from)?;
        let mut store = self.store.lock().await;
        let modified = store.apply_alias_retroactively(&key, &tags);
        drop(store);
        if !modified.is_empty() {
            let client_guard = self.client.lock().await;
            if let Some(manager) = &*client_guard {
                for mut t in modified {
                    let acc_id = self.resolve_account_id(&t.calendar_href);
                    if let Some(client) = manager.get_client(&acc_id) {
                        let _ = client.update_task(&mut t).await;
                    }
                }
            } else {
                for t in modified {
                    let _ = crate::journal::Journal::push(crate::journal::Action::Update(t));
                }
            }
        }
        Ok(())
    }

    pub fn load_from_cache(&self) {
        let mut store = self.store.blocking_lock();
        store.clear();

        if let Ok(locals) = LocalCalendarRegistry::load() {
            for loc in locals {
                if let Ok(mut tasks) = LocalStorage::load_for_href(&loc.href) {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &loc.href);
                    store.insert(loc.href, tasks);
                }
            }
        }
        if let Ok(cals) = Cache::load_calendars() {
            for cal in cals {
                if cal.href.starts_with("local://") {
                    continue;
                }
                if let Ok((mut tasks, _)) = Cache::load(&cal.href) {
                    crate::journal::Journal::apply_to_tasks(&mut tasks, &cal.href);
                    store.insert(cal.href, tasks);
                }
            }
        }
        self.rebuild_alarm_index_sync(&store);
    }

    pub async fn get_all_tags(&self) -> Vec<MobileTag> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let empty = HashSet::new();
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        store
            .get_all_categories(config.hide_completed, config.hide_fully_completed_tags, &empty, &hidden)
            .into_iter()
            .map(|(n, c)| MobileTag {
                name: n.clone(),
                count: c as u32,
                is_uncategorized: n == UNCATEGORIZED_ID,
            })
            .collect()
    }

    pub async fn get_all_locations(&self) -> Vec<MobileLocation> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        store
            .get_all_locations(config.hide_completed, &hidden)
            .into_iter()
            .map(|(n, c)| MobileLocation { name: n, count: c as u32 })
            .collect()
    }

    pub async fn get_view_tasks(&self, tags: Vec<String>, locs: Vec<String>, query: String) -> Vec<MobileTask> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let mut sc = HashSet::new();
        for t in tags {
            sc.insert(t);
        }
        let mut sl = HashSet::new();
        for l in locs {
            sl.insert(l);
        }
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        let cutoff = config.sort_cutoff_months.map(|m| Utc::now() + chrono::Duration::days(m as i64 * 30));

        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &sc,
            selected_locations: &sl,
            match_all_categories: false,
            search_term: &query,
            hide_completed_global: config.hide_completed,
            cutoff_date: cutoff,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
        });
        filtered.into_iter().map(|t| task_to_mobile(&t, &store)).collect()
    }

    pub async fn get_random_task_uid(&self, tags: Vec<String>, locs: Vec<String>, query: String) -> Option<String> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let mut sc = HashSet::new();
        for t in tags {
            sc.insert(t);
        }
        let mut sl = HashSet::new();
        for l in locs {
            sl.insert(l);
        }
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);

        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &sc,
            selected_locations: &sl,
            match_all_categories: false,
            search_term: &query,
            hide_completed_global: config.hide_completed,
            cutoff_date: None,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
        });

        let idx = crate::store::select_weighted_random_index(&filtered, config.default_priority)?;
        filtered.get(idx).map(|t| t.uid.clone())
    }

    pub fn get_config(&self) -> MobileConfig {
        let c = Config::load().unwrap_or_default();
        MobileConfig {
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

    pub fn save_config(&self, update: MobileConfigUpdate) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.hide_completed = update.hide_completed;
        c.disabled_calendars = update.disabled_calendars;
        c.sort_cutoff_months = update.sort_cutoff_months;
        c.urgent_days_horizon = update.urgent_days;
        c.urgent_priority_threshold = update.urgent_prio;
        c.default_priority = update.default_priority;
        c.start_grace_period_days = update.start_grace_period_days;
        c.auto_reminders = update.auto_reminders;
        c.default_reminder_time = update.default_reminder_time;
        c.snooze_short_mins = update.snooze_short;
        c.snooze_long_mins = update.snooze_long;
        c.create_events_for_tasks = update.create_events_for_tasks;
        c.delete_events_on_completion = update.delete_events_on_completion;
        c.save().map_err(MobileError::from)
    }

    pub async fn add_task_smart(&self, input: String) -> Result<String, MobileError> {
        let mut config = Config::load().unwrap_or_default();
        let (clean_input, new_aliases) = crate::model::extract_inline_aliases(&input);

        if !new_aliases.is_empty() {
            for (k, v) in &new_aliases {
                crate::model::validate_alias_integrity(k, v, &config.tag_aliases).map_err(MobileError::from)?;
            }
            config.tag_aliases.extend(new_aliases.clone());
            config.save().map_err(MobileError::from)?;
        }
        if clean_input.trim().is_empty() {
            return Ok("".to_string());
        }

        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
        let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);
        let target_href = config.default_calendar.clone().unwrap_or(LOCAL_CALENDAR_HREF.to_string());
        task.calendar_href = target_href.clone();

        self.store.lock().await.add_task(task.clone());
        let acc_id = self.resolve_account_id(&task.calendar_href);
        let mut network_success = false;

        let client_guard = self.client.lock().await;
        if let Some(manager) = &*client_guard
            && let Some(client) = manager.get_client(&acc_id)
                && client.create_task(&mut task).await.is_ok() {
                    if task.etag.is_empty() {
                        task.etag = "pending_refresh".to_string();
                    }
                    self.store.lock().await.update_or_add_task(task.clone());
                    network_success = true;
                }

        if !network_success {
            if task.calendar_href.starts_with("local://") {
                let mut all = LocalStorage::load_for_href(&task.calendar_href).unwrap_or_default();
                all.push(task.clone());
                LocalStorage::save_for_href(&task.calendar_href, &all).map_err(MobileError::from)?;
            } else {
                crate::journal::Journal::push(crate::journal::Action::Create(task.clone())).map_err(MobileError::from)?;
            }
            self.store.lock().await.update_or_add_task(task.clone());
        }
        let store = self.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(task.uid)
    }

    pub async fn change_priority(&self, uid: String, delta: i8) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |s, i| s.change_priority(i, delta)).await
    }

    pub async fn update_task_smart(&self, uid: String, input: String) -> Result<(), MobileError> {
        let config = Config::load().unwrap_or_default();
        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
        self.apply_store_mutation(uid, |store, id| {
            if let Some((task, _)) = store.get_task_mut(id) {
                task.apply_smart_input(&input, &config.tag_aliases, def_time);
                return Some(task.clone());
            }
            None
        })
        .await
    }

    pub async fn update_task_description(&self, uid: String, desc: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store, id| {
            if let Some((task, _)) = store.get_task_mut(id) {
                task.description = desc;
                return Some(task.clone());
            }
            None
        })
        .await
    }

    pub fn get_next_alarm_timestamp(&self) -> Option<i64> {
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(idx) = &*cached
            && !idx.is_empty() {
                return idx.get_next_alarm_timestamp().map(|t| t as i64);
            }
        drop(cached);
        let idx = AlarmIndex::load();
        if !idx.is_empty() {
            let ts = idx.get_next_alarm_timestamp().map(|t| t as i64);
            *self.alarm_index_cache.blocking_lock() = Some(idx);
            return ts;
        }
        None
    }

    pub fn get_firing_alarms(&self) -> Vec<MobileAlarmInfo> {
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(idx) = &*cached
            && !idx.is_empty() {
                return idx
                    .get_firing_alarms()
                    .into_iter()
                    .map(|a| MobileAlarmInfo {
                        task_uid: a.task_uid,
                        alarm_uid: a.alarm_uid,
                        title: a.task_title,
                        body: a.description.unwrap_or_default(),
                    })
                    .collect();
            }
        drop(cached);
        let idx = AlarmIndex::load();
        if !idx.is_empty() {
            let res = idx
                .get_firing_alarms()
                .into_iter()
                .map(|a| MobileAlarmInfo {
                    task_uid: a.task_uid,
                    alarm_uid: a.alarm_uid,
                    title: a.task_title,
                    body: a.description.unwrap_or_default(),
                })
                .collect();
            *self.alarm_index_cache.blocking_lock() = Some(idx);
            return res;
        }
        Vec::new()
    }

    pub fn tokenize_smart_input(&self, input: String) -> Vec<MobileSyntaxToken> {
        tokenize_smart_input(&input)
            .into_iter()
            .map(|t| MobileSyntaxToken {
                kind: MobileSyntaxType::from(t.kind),
                start: t.start as i32,
                end: t.end as i32,
            })
            .collect()
    }
}
