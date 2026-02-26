// File: src/mobile.rs
use crate::alarm_index::AlarmIndex;
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::{AppContext, StandardContext};
use crate::controller::TaskController;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::model::{AlarmTrigger, DateType, Task};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use crate::store::{FilterOptions, TaskStore, UNCATEGORIZED_ID};
use chrono::{DateTime, Local, NaiveTime, Utc};
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

// --- Additions for Tokio Runtime ---
use std::sync::OnceLock;
use tokio::runtime::Runtime;

#[cfg(target_os = "android")]
use std::io::{Read, Write};

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

// This will be the runtime for all mobile async operations.
static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[uniffi::export]
pub fn init_tokio_runtime() -> Result<(), MobileError> {
    if TOKIO_RUNTIME.get().is_none() {
        let runtime = Runtime::new().map_err(|e| MobileError::from(e.to_string()))?;
        if TOKIO_RUNTIME.set(runtime).is_err() {
            #[cfg(target_os = "android")]
            log::warn!("Tokio runtime was already initialized by another thread.");
        } else {
            #[cfg(target_os = "android")]
            log::debug!("Tokio runtime initialized.");
        }
    }
    Ok(())
}

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
    Calendar,
    Filter,
    Operator,
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
            SyntaxType::Calendar => MobileSyntaxType::Calendar,
            SyntaxType::Filter => MobileSyntaxType::Filter,
            SyntaxType::Operator => MobileSyntaxType::Operator,
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
    // NEW FIELD
    pub completed_date_iso: Option<String>,
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
    // NEW FIELDS: Tasks that this task is blocking (successors)
    pub blocking_uids: Vec<String>,
    pub blocking_names: Vec<String>,
    pub related_to_uids: Vec<String>,
    pub related_to_names: Vec<String>,
    pub is_paused: bool,
    pub location: Option<String>,
    pub url: Option<String>,
    pub geo: Option<String>,

    // Time-tracking fields exposed to mobile clients
    pub time_spent_seconds: u64,
    pub last_started_at: Option<i64>,

    // Virtual task hinting for clients to render expand/collapse rows
    // Values are:
    //  - "none"     -> not a virtual row
    //  - "expand"   -> an expand placeholder; `virtual_payload` contains parent key
    //  - "collapse" -> a collapse placeholder; `virtual_payload` contains parent key
    pub virtual_type: String,
    pub virtual_payload: String,

    pub visible_categories: Vec<String>,
    pub visible_location: Option<String>,
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
pub struct MobileViewData {
    pub tasks: Vec<MobileTask>,
    pub tags: Vec<MobileTag>,
    pub locations: Vec<MobileLocation>,
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
    pub create_events_for_tasks: bool,
    pub delete_events_on_completion: bool,
    pub auto_refresh_interval: u32,
    pub trash_retention: u32,
    pub max_done_roots: u32,
    pub max_done_subtasks: u32,
}

#[derive(uniffi::Record)]
pub struct MobileHelpItem {
    pub keys: String,
    pub desc: String,
    pub example: String,
}

#[derive(uniffi::Record)]
pub struct MobileHelpSection {
    pub title: String,
    pub items: Vec<MobileHelpItem>,
}

#[uniffi::export]
impl CfaitMobile {
    pub fn get_syntax_help(&self) -> Vec<MobileHelpSection> {
        crate::help::get_syntax_help()
            .into_iter()
            .map(|s| MobileHelpSection {
                title: s.title,
                items: s
                    .items
                    .into_iter()
                    .map(|i| MobileHelpItem {
                        keys: i.keys,
                        desc: i.desc,
                        example: i.example,
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn get_available_locales(&self) -> Vec<String> {
        rust_i18n::available_locales!()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
}

fn task_to_mobile(
    t: &Task,
    store: &TaskStore,
    aliases: &HashMap<String, Vec<String>>,
) -> MobileTask {
    let smart = t.to_smart_string();
    let status_str = format!("{:?}", t.status);

    let is_blocked = t.is_blocked;
    let blocked_by_names = t
        .dependencies
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();

    // Determine tasks that THIS task is blocking (successors) via reverse index
    let blocking_pairs = store.get_tasks_blocking(&t.uid);
    // Unzip into parallel vectors: (uids, names)
    let (blocking_uids, blocking_names): (Vec<String>, Vec<String>) =
        blocking_pairs.into_iter().unzip();

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
    // expose completion date to mobile clients
    let completed_date_iso = t.completion_date().map(|d| d.to_rfc3339());

    let has_alarms = !t
        .alarms
        .iter()
        .all(|a| a.acknowledged.is_some() || a.is_snooze());

    let now = Utc::now();
    let is_future_start = if let Some(start) = &t.dtstart {
        start.to_start_comparison_time() > now
    } else {
        false
    };

    let (parent_tags, parent_loc) = if let Some(p_uid) = &t.parent_uid
        && let Some(parent) = store.get_task_ref(p_uid)
    {
        (
            parent.categories.iter().cloned().collect(),
            parent.location.clone(),
        )
    } else {
        (std::collections::HashSet::new(), None)
    };

    let (visible_categories, visible_location) =
        t.resolve_visual_attributes(&parent_tags, &parent_loc, aliases);

    // Map the internal virtual state to simple strings for mobile clients
    let (v_type, v_payload) = match &t.virtual_state {
        crate::model::VirtualState::None => ("none".to_string(), "".to_string()),
        crate::model::VirtualState::Expand(k) => ("expand".to_string(), k.clone()),
        crate::model::VirtualState::Collapse(k) => ("collapse".to_string(), k.clone()),
    };

    MobileTask {
        uid: t.uid.clone(),
        summary: t.summary.clone(),
        description: t.description.clone(),
        is_done: t.status.is_done(),
        priority: t.priority,
        due_date_iso: due_iso,
        // Include completed date field
        completed_date_iso,
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
        // Expose blocking (successors) to mobile clients
        blocking_uids,
        blocking_names,
        related_to_uids: t.related_to.clone(),
        related_to_names,
        is_paused: t.is_paused(),
        location: t.location.clone(),
        url: t.url.clone(),
        geo: t.geo.clone(),

        // Time-tracking values
        time_spent_seconds: t.time_spent_seconds,
        last_started_at: t.last_started_at,

        // Virtual hint fields for mobile/GUI clients
        virtual_type: v_type,
        virtual_payload: v_payload,

        visible_categories,
        visible_location,
    }
}

#[derive(uniffi::Object)]
pub struct CfaitMobile {
    controller: TaskController,
    alarm_index_cache: Arc<Mutex<Option<AlarmIndex>>>,
    ctx: Arc<dyn AppContext>,
}

type MultiTaskMutator = Box<dyn Fn(&mut TaskStore, &str) -> Vec<Task> + Send>;

// Module-scope boxed helper for persisting multiple tasks returned by store mutators.
// This lives at module scope so it can be used from the exported uniffi impl block
// without embedding complicated generic closure bounds in the exported impl.
async fn apply_store_mutation_multi_boxed(
    this: &CfaitMobile,
    uid: &str,
    mutator: MultiTaskMutator,
) -> Result<(), MobileError> {
    let mut store = this.controller.store.lock().await;
    let tasks_to_save = (mutator)(&mut store, uid);
    drop(store);

    for task in tasks_to_save {
        this.controller
            .update_task(task)
            .await
            .map_err(MobileError::from)?;
    }

    let store_locked = this.controller.store.lock().await;
    this.rebuild_alarm_index_sync(&store_locked);
    Ok(())
}

#[uniffi::export]
impl CfaitMobile {
    #[uniffi::constructor]
    pub fn new(android_files_dir: String) -> Self {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("CfaitRust"),
        );
        let ctx: Arc<dyn AppContext> =
            Arc::new(StandardContext::new(Some(PathBuf::from(android_files_dir))));

        let store = Arc::new(Mutex::new(TaskStore::new(ctx.clone())));
        let client = Arc::new(Mutex::new(None));
        let controller = TaskController::new(store, client, ctx.clone());

        // Trigger prune on startup
        let c_clone = controller.clone();
        // This is the critical change. We must explicitly use the runtime we created.
        if let Some(runtime) = TOKIO_RUNTIME.get() {
            runtime.spawn(async move {
                let _ = c_clone.prune_trash().await;
            });
        } else {
            // Log an error if the runtime wasn't initialized.
            #[cfg(target_os = "android")]
            log::error!("Tokio runtime not initialized before CfaitMobile::new() was called!");
        }

        Self {
            controller,
            alarm_index_cache: Arc::new(Mutex::new(None)),
            ctx,
        }
    }

    pub fn create_debug_export(&self) -> Result<String, MobileError> {
        self.create_debug_export_internal()
    }

    // Expose locale switching to Kotlin
    pub fn set_locale(&self, locale: String) {
        rust_i18n::set_locale(&locale);
    }

    pub fn has_unsynced_changes(&self) -> bool {
        !crate::journal::Journal::load(self.ctx.as_ref()).is_empty()
    }

    /// Returns true if the in-memory store contains any tasks across any calendars.
    /// This is useful for clients to distinguish "no data at all" from "filters produced no results".
    pub fn has_any_tasks(&self) -> bool {
        self.controller.store.blocking_lock().has_any_tasks()
    }

    pub fn export_local_ics(&self, calendar_href: String) -> Result<String, MobileError> {
        let tasks = LocalStorage::load_for_href(self.ctx.as_ref(), &calendar_href)
            .map_err(|e| MobileError::from(e.to_string()))?;
        Ok(LocalStorage::to_ics_string(&tasks))
    }

    pub fn import_local_ics(
        &self,
        calendar_href: String,
        ics_content: String,
    ) -> Result<String, MobileError> {
        let count = LocalStorage::import_from_ics(self.ctx.as_ref(), &calendar_href, &ics_content)
            .map_err(|e| MobileError::from(e.to_string()))?;
        Ok(format!("Successfully imported {} task(s)", count))
    }

    pub fn parse_smart_string(&self, input: String, is_search: bool) -> Vec<MobileSyntaxToken> {
        let tokens = tokenize_smart_input(&input, is_search);
        let mut byte_to_utf16 = std::collections::BTreeMap::new();
        let mut byte_pos = 0;
        let mut utf16_pos = 0;
        byte_to_utf16.insert(0, 0);
        for c in input.chars() {
            byte_pos += c.len_utf8();
            utf16_pos += c.len_utf16();
            byte_to_utf16.insert(byte_pos, utf16_pos as i32);
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
        let c = Config::load(self.ctx.as_ref()).unwrap_or_default();
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
            create_events_for_tasks: c.create_events_for_tasks,
            delete_events_on_completion: c.delete_events_on_completion,
            auto_refresh_interval: c.auto_refresh_interval_mins,
            trash_retention: c.trash_retention_days,
            max_done_roots: c.max_done_roots as u32,
            max_done_subtasks: c.max_done_subtasks as u32,
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
        create_events_for_tasks: bool,
        delete_events_on_completion: bool,
        auto_refresh_interval: u32,
        trash_retention: u32,
        // NEW ARGUMENTS
        max_done_roots: u32,
        max_done_subtasks: u32,
    ) -> Result<(), MobileError> {
        let mut c = Config::load(self.ctx.as_ref()).unwrap_or_default();
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
        c.create_events_for_tasks = create_events_for_tasks;
        c.delete_events_on_completion = delete_events_on_completion;
        c.auto_refresh_interval_mins = auto_refresh_interval;
        c.trash_retention_days = trash_retention;

        // Save new values
        c.max_done_roots = max_done_roots as usize;
        c.max_done_subtasks = max_done_subtasks as usize;

        c.save(self.ctx.as_ref()).map_err(MobileError::from)
    }

    pub fn get_calendars(&self) -> Vec<MobileCalendar> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let disabled_set: HashSet<String> = config.disabled_calendars.iter().cloned().collect();
        let mut result = Vec::new();
        // Acquire a blocking lock to inspect local store counts for the trash calendar
        let store = self.controller.store.blocking_lock();
        if let Ok(locals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
            for loc in locals {
                // Special-case: hide empty trash and recovery calendars from mobile list
                if loc.href == crate::storage::LOCAL_TRASH_HREF || loc.href == "local://recovery" {
                    if let Some(map) = store.calendars.get(crate::storage::LOCAL_TRASH_HREF) {
                        if map.is_empty() {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
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
        if let Ok(cals) = crate::cache::Cache::load_calendars(self.ctx.as_ref()) {
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

    pub fn isolate_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        config.hidden_calendars = self
            .get_calendars()
            .iter()
            .filter(|c| c.href != href)
            .map(|c| c.href.clone())
            .collect();
        config.default_calendar = Some(href);
        config.save(self.ctx.as_ref()).map_err(MobileError::from)
    }

    pub fn remove_alias(&self, key: String) -> Result<(), MobileError> {
        let mut c = Config::load(self.ctx.as_ref()).unwrap_or_default();
        c.tag_aliases.remove(&key);
        c.save(self.ctx.as_ref()).map_err(MobileError::from)
    }

    pub fn set_default_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        config.default_calendar = Some(href.clone());
        config.hidden_calendars.retain(|h| h != &href);
        config.save(self.ctx.as_ref()).map_err(MobileError::from)
    }

    pub fn set_calendar_visibility(&self, href: String, visible: bool) -> Result<(), MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        if visible {
            config.hidden_calendars.retain(|h| h != &href);
        } else if !config.hidden_calendars.contains(&href) {
            config.hidden_calendars.push(href);
        }
        config.save(self.ctx.as_ref()).map_err(MobileError::from)
    }

    pub fn load_from_cache(&self) {
        let mut store = self.controller.store.blocking_lock();
        store.clear();
        if let Ok(locals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
            for loc in locals {
                match LocalStorage::load_for_href(self.ctx.as_ref(), &loc.href) {
                    Ok(mut tasks) => {
                        crate::journal::Journal::apply_to_tasks(
                            self.ctx.as_ref(),
                            &mut tasks,
                            &loc.href,
                        );
                        store.insert(loc.href, tasks);
                    }
                    Err(e) => {
                        #[cfg(target_os = "android")]
                        log::error!("Failed to load {} - data corruption: {}", loc.href, e);
                        #[cfg(not(target_os = "android"))]
                        eprintln!("Failed to load {} - data corruption: {}", loc.href, e);
                    }
                }
            }
        }
        if let Ok(cals) = Cache::load_calendars(self.ctx.as_ref()) {
            for cal in cals {
                if cal.href.starts_with("local://") {
                    continue;
                }
                if let Ok((mut tasks, _)) = Cache::load(self.ctx.as_ref(), &cal.href) {
                    crate::journal::Journal::apply_to_tasks(
                        self.ctx.as_ref(),
                        &mut tasks,
                        &cal.href,
                    );
                    store.insert(cal.href, tasks);
                }
            }
        }
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let index = AlarmIndex::rebuild_from_tasks(
            &store.calendars,
            config.auto_reminders,
            &config.default_reminder_time,
        );
        if let Err(e) = index.save(self.ctx.as_ref()) {
            #[cfg(target_os = "android")]
            log::warn!("Failed to save alarm index: {}", e);
            #[cfg(not(target_os = "android"))]
            let _ = e;
        } else {
            #[cfg(target_os = "android")]
            log::debug!("Alarm index rebuilt with {} alarms", index.len());
        }
        *self.alarm_index_cache.blocking_lock() = Some(index);
    }

    pub fn get_next_alarm_timestamp(&self) -> Option<i64> {
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(ref index) = *cached
            && !index.is_empty()
        {
            if let Some(timestamp) = index.get_next_alarm_timestamp() {
                return Some(timestamp as i64);
            }
            return None;
        }
        drop(cached);
        let index = AlarmIndex::load(self.ctx.as_ref());
        if !index.is_empty() {
            if let Some(timestamp) = index.get_next_alarm_timestamp() {
                *self.alarm_index_cache.blocking_lock() = Some(index);
                return Some(timestamp as i64);
            }
            return None;
        }
        let store = self.controller.store.blocking_lock();
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let now = Utc::now();
        let mut global_earliest: Option<i64> = None;

        let check_ts = |ts: i64, current_earliest: &mut Option<i64>| {
            if ts > now.timestamp()
                && (current_earliest.is_none() || ts < current_earliest.unwrap())
            {
                *current_earliest = Some(ts);
            }
        };

        for tasks_map in store.calendars.values() {
            for task in tasks_map.values() {
                // UPDATE: Skip InProcess here
                if task.status.is_done() || task.status == crate::model::TaskStatus::InProcess {
                    continue;
                }

                if let Some(ts) = task.next_trigger_timestamp() {
                    check_ts(ts, &mut global_earliest);
                }
                if config.auto_reminders
                    && !task
                        .alarms
                        .iter()
                        .any(|a| a.acknowledged.is_none() && !a.is_snooze())
                {
                    // Check implicit logic using explicitly referenced parameters
                    let mut check_implicit = |dt: DateTime<Utc>| {
                        if !task.has_alarm_at(dt) {
                            check_ts(dt.timestamp(), &mut global_earliest);
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
                        check_implicit(dt);
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
                        check_implicit(dt);
                    }
                }
            }
        }
        global_earliest
    }

    pub fn get_firing_alarms(&self) -> Vec<MobileAlarmInfo> {
        let cached = self.alarm_index_cache.blocking_lock();
        if let Some(ref index) = *cached
            && !index.is_empty()
        {
            let firing = index.get_firing_alarms();
            if !firing.is_empty() {
                return firing
                    .into_iter()
                    .map(|e| MobileAlarmInfo {
                        task_uid: e.task_uid,
                        alarm_uid: e.alarm_uid,
                        title: e.task_title,
                        body: e.description.unwrap_or("Reminder".to_string()),
                    })
                    .collect();
            } else {
                return Vec::new();
            }
        }
        drop(cached);
        let index = AlarmIndex::load(self.ctx.as_ref());
        if !index.is_empty() {
            let firing = index.get_firing_alarms();
            if !firing.is_empty() {
                *self.alarm_index_cache.blocking_lock() = Some(index);
                return firing
                    .into_iter()
                    .map(|e| MobileAlarmInfo {
                        task_uid: e.task_uid,
                        alarm_uid: e.alarm_uid,
                        title: e.task_title,
                        body: e.description.unwrap_or("Reminder".to_string()),
                    })
                    .collect();
            } else {
                return Vec::new();
            }
        }
        let store = self.controller.store.blocking_lock();
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        let now = Utc::now();
        let mut results = Vec::new();
        for tasks_map in store.calendars.values() {
            for task in tasks_map.values() {
                // UPDATE: Skip InProcess here
                if task.status.is_done() || task.status == crate::model::TaskStatus::InProcess {
                    continue;
                }

                for alarm in &task.alarms {
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
                    if trigger_dt <= now && (now - trigger_dt).num_minutes() < 120 {
                        results.push(MobileAlarmInfo {
                            task_uid: task.uid.clone(),
                            alarm_uid: alarm.uid.clone(),
                            title: task.summary.clone(),
                            body: alarm.description.clone().unwrap_or("Reminder".to_string()),
                        });
                    }
                }
                if config.auto_reminders
                    && !task
                        .alarms
                        .iter()
                        .any(|a| a.acknowledged.is_none() && !a.is_snooze())
                {
                    let mut check_implicit = |dt: DateTime<Utc>, desc: &str, type_key: &str| {
                        if !task.has_alarm_at(dt) && dt <= now && (now - dt).num_minutes() < 120 {
                            let synth_id =
                                format!("implicit_{}:|{}|{}", type_key, dt.to_rfc3339(), task.uid);
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
                        // Use rust_i18n lookup for localized description (Single Source of Truth)
                        let alarm_due_now = rust_i18n::t!("alarm_due_now");
                        check_implicit(dt, alarm_due_now.as_ref(), "due");
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
                        // Use rust_i18n lookup for localized description (Single Source of Truth)
                        let alarm_task_starting = rust_i18n::t!("alarm_task_starting");
                        check_implicit(dt, alarm_task_starting.as_ref(), "start");
                    }
                }
            }
        }
        results
    }
}

// Block 2: Asynchronous functions
#[uniffi::export(async_runtime = "tokio")]
impl CfaitMobile {
    pub async fn add_alias(&self, key: String, tags: Vec<String>) -> Result<(), MobileError> {
        let mut c = Config::load(self.ctx.as_ref()).unwrap_or_default();
        crate::model::validate_alias_integrity(&key, &tags, &c.tag_aliases)
            .map_err(MobileError::from)?;
        c.tag_aliases.insert(key.clone(), tags.clone());
        c.save(self.ctx.as_ref()).map_err(MobileError::from)?;
        let mut store = self.controller.store.lock().await;
        let modified = store.apply_alias_retroactively(&key, &tags);
        drop(store);
        if !modified.is_empty() {
            for t in modified {
                // Delegate to Controller
                self.controller
                    .update_task(t)
                    .await
                    .map_err(MobileError::from)?;
            }
        }
        Ok(())
    }

    pub async fn add_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        if task_uid == blocker_uid {
            return Err(MobileError::from("Cannot depend on self"));
        }
        self.apply_store_mutation(&task_uid, |store, id| store.add_dependency(id, blocker_uid))
            .await
    }

    pub async fn remove_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(&task_uid, |store, id| {
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
        self.apply_store_mutation(&child_uid, |store, id| store.set_parent(id, parent_uid))
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
        self.apply_store_mutation(&task_uid, |store, id| store.add_related_to(id, related_uid))
            .await
    }

    pub async fn remove_related_to(
        &self,
        task_uid: String,
        related_uid: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(&task_uid, |store, id| {
            store.remove_related_to(id, &related_uid)
        })
        .await
    }

    pub async fn get_tasks_related_to(&self, uid: String) -> Vec<MobileRelatedTask> {
        self.controller
            .store
            .lock()
            .await
            .get_tasks_related_to(&uid)
            .into_iter()
            .map(|(uid, summary)| MobileRelatedTask { uid, summary })
            .collect()
    }

    pub async fn sync(&self) -> Result<String, MobileError> {
        let config = Config::load(self.ctx.as_ref()).map_err(MobileError::from)?;
        self.apply_connection(config).await
    }

    pub async fn connect(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
    ) -> Result<String, MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        config.url = url;
        config.username = user;
        if !pass.is_empty() {
            config.password = pass;
        }
        config.allow_insecure_certs = insecure;
        self.apply_connection(config).await
    }

    // get_all_tags removed: tags are now derived from view data returned by `get_view_data`.
    // Keep a lightweight compatibility shim that returns an empty vector so callers that
    // haven't been migrated won't panic (prefer updating callers to use `get_view_data`).
    pub async fn get_all_tags(&self) -> Vec<MobileTag> {
        Vec::new()
    }

    // get_all_locations removed: locations are now derived from view data returned by `get_view_data`.
    // Compatibility shim returning empty vector to avoid panics for callers that were not updated yet.
    pub async fn get_all_locations(&self) -> Vec<MobileLocation> {
        Vec::new()
    }

    // Direct lookup to support opening specific tasks (e.g. via notification or deep link)
    // regardless of current view filters (hidden/completed/collapsed).
    pub async fn get_task_by_uid(&self, uid: String) -> Option<MobileTask> {
        let store = self.controller.store.lock().await;
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();

        if let Some(task) = store.get_task_ref(&uid) {
            Some(task_to_mobile(task, &store, &config.tag_aliases))
        } else {
            None
        }
    }

    // get_view_tasks returns tasks + contextual tags/locations (kept MobileViewData return).
    pub async fn get_view_tasks(
        &self,
        filter_tags: Vec<String>,
        filter_locations: Vec<String>,
        search_query: String,
        // Caller-provided list of expanded done-group keys
        expanded_groups: Vec<String>,
        match_all_categories: bool, // <--- ADDED PARAM
    ) -> MobileViewData {
        let store = self.controller.store.lock().await;
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);

        // Convert expanded vector into a set for efficient lookup
        let expanded_set: HashSet<String> = expanded_groups.into_iter().collect();

        let cutoff_date = config
            .sort_cutoff_months
            .map(|m| Utc::now() + chrono::Duration::days(m as i64 * 30));
        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &filter_tags.into_iter().collect(),
            selected_locations: &filter_locations.into_iter().collect(),
            match_all_categories, // <--- PASSED IN
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            hide_fully_completed_tags: config.hide_fully_completed_tags,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,

            // Pass expansion state and configured limits into the store filter
            expanded_done_groups: &expanded_set,
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
        });

        let tasks = filtered
            .tasks
            .into_iter()
            .map(|t| task_to_mobile(&t, &store, &config.tag_aliases))
            .collect();

        let tags = filtered
            .categories
            .into_iter()
            .map(|(name, count)| MobileTag {
                name: name.clone(),
                count: count as u32,
                is_uncategorized: name == UNCATEGORIZED_ID,
            })
            .collect();

        let locations = filtered
            .locations
            .into_iter()
            .map(|(name, count)| MobileLocation {
                name,
                count: count as u32,
            })
            .collect();

        MobileViewData {
            tasks,
            tags,
            locations,
        }
    }

    pub async fn get_random_task_uid(
        &self,
        filter_tags: Vec<String>,
        filter_locations: Vec<String>,
        search_query: String,
    ) -> Option<String> {
        let store = self.controller.store.lock().await;
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        let cutoff_date = config
            .sort_cutoff_months
            .map(|m| Utc::now() + chrono::Duration::days(m as i64 * 30));
        let filter_res = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &filter_tags.into_iter().collect(),
            selected_locations: &filter_locations.into_iter().collect(),
            match_all_categories: false,
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            hide_fully_completed_tags: config.hide_fully_completed_tags,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
            expanded_done_groups: &HashSet::new(),
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
        });
        let filtered = filter_res.tasks;
        let idx = crate::store::select_weighted_random_index(&filtered, config.default_priority)?;
        filtered.get(idx).map(|t| t.uid.clone())
    }

    pub async fn yank_task(&self, _uid: String) -> Result<(), MobileError> {
        Ok(())
    }

    pub async fn add_task_smart(&self, input: String) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        log::debug!("add_task_smart: '{}'", input);
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let (clean_input, new_aliases) = crate::model::extract_inline_aliases(&input);
        if !new_aliases.is_empty() {
            for (k, v) in &new_aliases {
                crate::model::validate_alias_integrity(k, v, &config.tag_aliases)
                    .map_err(MobileError::from)?;
            }
            config.tag_aliases.extend(new_aliases.clone());
            config.save(self.ctx.as_ref()).map_err(MobileError::from)?;
            let mut store = self.controller.store.lock().await;
            let all_modified: Vec<_> = new_aliases
                .iter()
                .flat_map(|(key, tags)| store.apply_alias_retroactively(key, tags))
                .collect();
            drop(store);
            if !all_modified.is_empty() {
                for t in all_modified {
                    self.controller
                        .update_task(t)
                        .await
                        .map_err(MobileError::from)?;
                }
            }
            let trimmed = clean_input.trim();
            if trimmed.is_empty()
                || (!trimmed.contains(' ')
                    && (trimmed.starts_with('#')
                        || trimmed.starts_with("@@")
                        || trimmed.to_lowercase().starts_with("loc:")))
            {
                return Ok("ALIAS_UPDATED".to_string());
            }
        }
        if clean_input.trim().is_empty() {
            return Ok("".to_string());
        }
        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
        let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);
        #[cfg(target_os = "android")]
        log::debug!(
            "Created task: uid={}, summary='{}', alarms={}",
            task.uid,
            task.summary,
            !task.alarms.is_empty()
        );
        task.calendar_href = config
            .default_calendar
            .clone()
            .unwrap_or(LOCAL_CALENDAR_HREF.to_string());

        let uid = self
            .controller
            .create_task(task)
            .await
            .map_err(MobileError::from)?;

        #[cfg(target_os = "android")]
        log::debug!("Rebuilding alarm index after adding {}", uid);
        let store_for_rebuild = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store_for_rebuild);
        #[cfg(target_os = "android")]
        log::debug!("Alarm index rebuilt. Returning uid: {}", uid);
        Ok(uid)
    }

    pub async fn change_priority(&self, uid: String, delta: i8) -> Result<(), MobileError> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        self.apply_store_mutation(&uid, |store, id| {
            store.change_priority(id, delta, config.default_priority)
        })
        .await
    }

    pub async fn set_status_process(&self, uid: String) -> Result<(), MobileError> {
        apply_store_mutation_multi_boxed(
            self,
            &uid,
            Box::new(|store, id| store.set_status_in_process(id)),
        )
        .await
    }

    pub async fn set_status_cancelled(&self, uid: String) -> Result<(), MobileError> {
        let mut store = self.controller.store.lock().await;
        // Apply cancellation logic in the store (may produce history + next + reset children)
        let (primary, secondary, children) = store
            .set_status(&uid, crate::model::TaskStatus::Cancelled)
            .ok_or(MobileError::from("Task not found"))?;
        drop(store);

        // Persist resulting tasks via the controller's persistence/update API.
        // The store already updated in-memory state; here we instruct the controller
        // to persist those changes (online or journal fallback).
        if let Some(sec) = secondary {
            // Recurring: primary is history (treated as created snapshot), sec is next instance.
            // Use create_task for new history, update_task for existing next instance.
            self.controller
                .create_task(primary)
                .await
                .map_err(MobileError::from)?;
            self.controller
                .update_task(sec)
                .await
                .map_err(MobileError::from)?;
        } else {
            // Non-recurring: primary is the updated existing task
            self.controller
                .update_task(primary)
                .await
                .map_err(MobileError::from)?;
        }

        // Persist any children that were auto-reset by the store.
        for child in children {
            self.controller
                .update_task(child)
                .await
                .map_err(MobileError::from)?;
        }

        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(())
    }

    pub async fn pause_task(&self, uid: String) -> Result<(), MobileError> {
        apply_store_mutation_multi_boxed(self, &uid, Box::new(|s, id| s.pause_task(id))).await
    }
    pub async fn stop_task(&self, uid: String) -> Result<(), MobileError> {
        apply_store_mutation_multi_boxed(self, &uid, Box::new(|s, id| s.stop_task(id))).await
    }
    pub async fn start_task(&self, uid: String) -> Result<(), MobileError> {
        apply_store_mutation_multi_boxed(self, &uid, Box::new(|s, id| s.set_status_in_process(id)))
            .await
    }

    pub async fn update_task_smart(
        &self,
        uid: String,
        smart_input: String,
    ) -> Result<(), MobileError> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let def_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();
        self.apply_store_mutation(&uid, |t, id| {
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
        self.apply_store_mutation(&uid, |t, id| {
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
        self.controller
            .toggle_task(&uid)
            .await
            .map_err(MobileError::from)?;
        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(())
    }

    pub async fn move_task(&self, uid: String, new_cal_href: String) -> Result<(), MobileError> {
        self.controller
            .move_task(&uid, &new_cal_href)
            .await
            .map_err(MobileError::from)?;

        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(())
    }

    pub async fn delete_task(&self, uid: String) -> Result<(), MobileError> {
        self.controller
            .delete_task(&uid)
            .await
            .map_err(MobileError::from)?;
        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(())
    }

    pub async fn migrate_local_to(
        &self,
        source_href: String,
        target_href: String,
    ) -> Result<String, MobileError> {
        let client = self
            .controller
            .client
            .lock()
            .await
            .as_ref()
            .ok_or(MobileError::from("Client not connected"))?
            .clone();
        let tasks = LocalStorage::load_for_href(self.ctx.as_ref(), &source_href)
            .map_err(|e| MobileError::from(e.to_string()))?;
        if tasks.is_empty() {
            return Ok("No tasks to migrate.".to_string());
        }
        let count = client
            .migrate_tasks(tasks, &target_href)
            .await
            .map_err(MobileError::from)?;
        Ok(format!("Migrated {} tasks.", count))
    }

    pub async fn create_local_calendar(
        &self,
        name: String,
        color: Option<String>,
    ) -> Result<String, MobileError> {
        let mut locals = LocalCalendarRegistry::load(self.ctx.as_ref())
            .map_err(|e| MobileError::from(e.to_string()))?;
        let href = format!("local://{}", Uuid::new_v4());
        locals.push(crate::model::CalendarListEntry {
            name,
            href: href.clone(),
            color,
        });
        LocalCalendarRegistry::save(self.ctx.as_ref(), &locals)
            .map_err(|e| MobileError::from(e.to_string()))?;
        self.controller
            .store
            .lock()
            .await
            .insert(href.clone(), vec![]);
        Ok(href)
    }

    pub async fn update_local_calendar(
        &self,
        href: String,
        name: String,
        color: Option<String>,
    ) -> Result<(), MobileError> {
        let mut locals = LocalCalendarRegistry::load(self.ctx.as_ref())
            .map_err(|e| MobileError::from(e.to_string()))?;
        if let Some(cal) = locals.iter_mut().find(|c| c.href == href) {
            cal.name = name;
            cal.color = color;
            LocalCalendarRegistry::save(self.ctx.as_ref(), &locals)
                .map_err(|e| MobileError::from(e.to_string()))?;
            Ok(())
        } else {
            Err(MobileError::from("Calendar not found"))
        }
    }

    // apply_store_mutation_multi_boxed moved to module scope above so it is callable
    // from the exported impl without generic-bound issues.

    pub async fn delete_local_calendar(&self, href: String) -> Result<(), MobileError> {
        if href == LOCAL_CALENDAR_HREF {
            return Err(MobileError::from("Cannot delete default calendar"));
        }
        let mut locals = LocalCalendarRegistry::load(self.ctx.as_ref())
            .map_err(|e| MobileError::from(e.to_string()))?;
        if let Some(idx) = locals.iter().position(|c| c.href == href) {
            locals.remove(idx);
            LocalCalendarRegistry::save(self.ctx.as_ref(), &locals)
                .map_err(|e| MobileError::from(e.to_string()))?;
            if let Some(path) = LocalStorage::get_path_for_href(self.ctx.as_ref(), &href) {
                let _ = std::fs::remove_file(path);
            }
            let mut store = self.controller.store.lock().await;
            store.remove(&href);
            self.rebuild_alarm_index_sync(&store);
            Ok(())
        } else {
            Err(MobileError::from("Calendar not found"))
        }
    }

    pub async fn snooze_alarm(
        &self,
        task_uid: String,
        alarm_uid: String,
        minutes: u32,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(&task_uid, |store, id| {
            if let Some((task, _)) = store.get_task_mut(id)
                && task.handle_snooze(&alarm_uid, minutes)
            {
                return Some(task.clone());
            }
            None
        })
        .await?;
        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        Ok(())
    }

    pub async fn dismiss_alarm(
        &self,
        task_uid: String,
        alarm_uid: String,
    ) -> Result<(), MobileError> {
        #[cfg(target_os = "android")]
        log::debug!("dismiss_alarm: task={}, alarm={}", task_uid, alarm_uid);
        self.apply_store_mutation(&task_uid, |store, id| {
            if let Some((task, _)) = store.get_task_mut(id)
                && task.handle_dismiss(&alarm_uid)
            {
                return Some(task.clone());
            }
            None
        })
        .await?;
        #[cfg(target_os = "android")]
        log::debug!("Rebuilding alarm index after dismiss");
        let store = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store);
        #[cfg(target_os = "android")]
        log::debug!("Dismiss successful");
        Ok(())
    }

    pub async fn get_next_global_alarm_time(&self) -> Option<i64> {
        let store = self.controller.store.lock().await;
        let mut earliest: Option<i64> = None;
        for map in store.calendars.values() {
            for task in map.values() {
                if task.status.is_done() {
                    continue;
                }
                if let Some(ts) = task.next_trigger_timestamp()
                    && (earliest.is_none() || ts < earliest.unwrap())
                {
                    earliest = Some(ts);
                }
            }
        }
        earliest
    }

    pub async fn delete_all_calendar_events(&self) -> Result<u32, MobileError> {
        let client = {
            self.controller
                .client
                .lock()
                .await
                .as_ref()
                .ok_or(MobileError::from("Offline"))?
                .clone()
        };

        // Get a list of all remote calendars we know about
        let cals: Vec<String> = {
            self.controller
                .store
                .lock()
                .await
                .calendars
                .keys()
                .filter(|h| !h.starts_with("local://"))
                .cloned()
                .collect()
        };

        let mut total = 0;
        for cal_href in cals {
            if let Ok(count) = client.delete_all_companion_events(&cal_href).await {
                total += count as u32;
            }
        }

        Ok(total)
    }

    pub async fn create_missing_calendar_events(&self) -> Result<u32, MobileError> {
        let all_tasks: Vec<_> = {
            self.controller
                .store
                .lock()
                .await
                .calendars
                .values()
                .flat_map(|m| m.values())
                .cloned()
                .collect()
        };
        let client = {
            self.controller
                .client
                .lock()
                .await
                .as_ref()
                .ok_or(MobileError::from("Offline"))?
                .clone()
        };
        let futures = all_tasks.into_iter().map(|t| {
            let c = client.clone();
            async move {
                if c.sync_task_companion_event(&t, true).await.unwrap_or(false) {
                    1
                } else {
                    0
                }
            }
        });
        Ok(stream::iter(futures)
            .buffer_unordered(8)
            .collect::<Vec<u32>>()
            .await
            .iter()
            .sum())
    }
}

// Separated impl block for internal non-exported methods
impl CfaitMobile {
    // Generic mutator helper
    async fn apply_store_mutation<F>(&self, uid: &str, mutator: F) -> Result<(), MobileError>
    where
        F: FnOnce(&mut TaskStore, &str) -> Option<Task>,
    {
        // Lock store, apply mutation to get updated task
        let mut store = self.controller.store.lock().await;
        let task_to_save = mutator(&mut store, uid).ok_or(MobileError::from("Task not found"))?;
        drop(store); // Unlock

        // Send to Controller
        self.controller
            .update_task(task_to_save)
            .await
            .map_err(MobileError::from)?;

        let store_locked = self.controller.store.lock().await;
        self.rebuild_alarm_index_sync(&store_locked);

        Ok(())
    }

    // ... [Read operations like get_view_tasks use self.controller.store.lock().await] ...
    async fn apply_connection(&self, config: Config) -> Result<String, MobileError> {
        let (client, cals, _, _, warning) =
            RustyClient::connect_with_fallback(self.ctx.clone(), config, Some("Android"))
                .await
                .map_err(MobileError::from)?;
        *self.controller.client.lock().await = Some(client.clone());
        let fetch_result = client.get_all_tasks(&cals).await;
        let mut store = self.controller.store.lock().await;
        store.clear();
        if let Ok(locals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
            for loc in locals {
                match LocalStorage::load_for_href(self.ctx.as_ref(), &loc.href) {
                    Ok(mut tasks) => {
                        crate::journal::Journal::apply_to_tasks(
                            self.ctx.as_ref(),
                            &mut tasks,
                            &loc.href,
                        );
                        store.insert(loc.href, tasks);
                    }
                    Err(e) => {
                        #[cfg(target_os = "android")]
                        log::error!("Failed to load {} - data corruption: {}", loc.href, e);
                        #[cfg(not(target_os = "android"))]
                        eprintln!("Failed to load {} - data corruption: {}", loc.href, e);
                    }
                }
            }
        }
        match fetch_result {
            Ok(results) => {
                let mut fetched_hrefs = HashSet::new();
                for (href, mut tasks) in results {
                    crate::journal::Journal::apply_to_tasks(self.ctx.as_ref(), &mut tasks, &href);
                    store.insert(href.clone(), tasks);
                    fetched_hrefs.insert(href);
                }
                for cal in &cals {
                    if !cal.href.starts_with("local://")
                        && !fetched_hrefs.contains(&cal.href)
                        && let Ok((mut cached, _)) = Cache::load(self.ctx.as_ref(), &cal.href)
                    {
                        crate::journal::Journal::apply_to_tasks(
                            self.ctx.as_ref(),
                            &mut cached,
                            &cal.href,
                        );
                        store.insert(cal.href.clone(), cached);
                    }
                }
            }
            Err(e) => {
                for cal in &cals {
                    if !cal.href.starts_with("local://")
                        && !store.calendars.contains_key(&cal.href)
                        && let Ok((mut cached, _)) = Cache::load(self.ctx.as_ref(), &cal.href)
                    {
                        crate::journal::Journal::apply_to_tasks(
                            self.ctx.as_ref(),
                            &mut cached,
                            &cal.href,
                        );
                        store.insert(cal.href.clone(), cached);
                    }
                }
                self.rebuild_alarm_index_sync(&store);
                return Err(MobileError::from(e));
            }
        }
        self.rebuild_alarm_index_sync(&store);
        Ok(warning.unwrap_or_else(|| "Connected".to_string()))
    }

    fn rebuild_alarm_index_sync(&self, store: &TaskStore) {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let index = AlarmIndex::rebuild_from_tasks(
            &store.calendars,
            config.auto_reminders,
            &config.default_reminder_time,
        );
        match index.save(self.ctx.as_ref()) {
            Ok(_) => {
                #[cfg(target_os = "android")]
                log::debug!("Alarm index rebuilt with {} alarms", index.len());
                *self.alarm_index_cache.blocking_lock() = Some(index);
            }
            Err(e) => {
                #[cfg(target_os = "android")]
                log::warn!("Failed to save alarm index: {}", e);
                #[cfg(not(target_os = "android"))]
                let _ = e;
            }
        }
    }

    // Internal implementation not exported via UniFFI. Platform-specific behavior is gated here.
    fn create_debug_export_internal(&self) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        {
            let data_dir = self
                .ctx
                .get_data_dir()
                .map_err(|e| MobileError::from(e.to_string()))?;
            let cache_dir = self
                .ctx
                .get_cache_dir()
                .map_err(|e| MobileError::from(e.to_string()))?;
            let config_dir = self
                .ctx
                .get_config_dir()
                .map_err(|e| MobileError::from(e.to_string()))?;
            let export_path = cache_dir.join("cfait_debug_export.zip");
            let file = std::fs::File::create(&export_path)
                .map_err(|e| MobileError::from(e.to_string()))?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755);
            let mut add_dir = |dir: &std::path::Path, prefix: &str| -> Result<(), MobileError> {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();
                            if file_name.ends_with(".lock") || file_name == "cfait_debug_export.zip"
                            {
                                continue;
                            }
                            zip.start_file(format!("{}{}", prefix, file_name), options)
                                .map_err(|e| MobileError::from(e.to_string()))?;
                            if file_name == "config.toml" {
                                let mut config =
                                    Config::load(self.ctx.as_ref()).unwrap_or_default();
                                config.username = "[REDACTED]".to_string();
                                config.password = "[REDACTED]".to_string();
                                zip.write_all(
                                    toml::to_string_pretty(&config)
                                        .unwrap_or_default()
                                        .as_bytes(),
                                )
                                .map_err(|e| MobileError::from(e.to_string()))?;
                            } else {
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
            add_dir(&data_dir, "data/")?;
            add_dir(&config_dir, "config/")?;
            add_dir(&cache_dir, "cache/")?;
            zip.finish().map_err(|e| MobileError::from(e.to_string()))?;
            return Ok(export_path.to_string_lossy().to_string());
        }
        #[cfg(not(target_os = "android"))]
        {
            Err(MobileError::from(
                "Debug export is only available on Android",
            ))
        }
    }
}
