// File: ./src/mobile.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Mobile bindings and FFI interface for the Rust core.

use crate::alarm_index::AlarmIndex;
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::config::Config;
use crate::context::{AppContext, StandardContext};
use crate::controller::TaskController;
use crate::help::HelpTab;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::model::{AlarmTrigger, DateType, Task};
use crate::storage::{LOCAL_CALENDAR_HREF, LocalCalendarRegistry, LocalStorage};
use crate::store::{FilterOptions, TaskStore, UNCATEGORIZED_ID};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use std::sync::OnceLock;
use tokio::runtime::Runtime;

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

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_trougnouf_cfait_CfaitApplication_initNdkContext<'local>(
    mut unowned_env: jni::EnvUnowned<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
) {
    let _ = unowned_env.with_env(|env| -> jni::errors::Result<()> {
        let vm = env.get_java_vm()?;
        let global_context = env.new_global_ref(&context)?;
        unsafe {
            ndk_context::initialize_android_context(
                vm.get_raw() as *mut std::ffi::c_void,
                global_context.into_raw() as *mut std::ffi::c_void,
            );
        }
        Ok(())
    });
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
    Pin,
    Filter,
    Operator,
    Goal,
    Collection,
    WikiLink,
    Dependency,
    Relation,
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
            SyntaxType::Pin => MobileSyntaxType::Pin,
            SyntaxType::Filter => MobileSyntaxType::Filter,
            SyntaxType::Operator => MobileSyntaxType::Operator,
            SyntaxType::Goal => MobileSyntaxType::Goal,
            SyntaxType::Collection => MobileSyntaxType::Collection,
            SyntaxType::WikiLink => MobileSyntaxType::WikiLink,
            SyntaxType::Dependency => MobileSyntaxType::Dependency,
            SyntaxType::Relation => MobileSyntaxType::Relation,
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
pub struct MobileFilterOptions {
    pub filter_tags: Vec<String>,
    pub filter_locations: Vec<String>,
    pub search_query: String,
    pub expanded_groups: Vec<String>,
    pub match_all_categories: bool,
    pub expanded_tags: Vec<String>,
    pub expanded_locations: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct MobileWorkSession {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(uniffi::Record)]
pub struct MobileTask {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub is_done: bool,
    pub percent_complete: Option<u8>,
    pub priority: u8,
    pub due_date_iso: Option<String>,
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
    pub is_relative_recurrence: bool,
    pub parent_uid: Option<String>,
    pub smart_string: String,
    pub depth: u32,
    pub is_blocked: bool,
    pub status_string: String,
    pub blocked_by_names: Vec<String>,
    pub blocked_by_uids: Vec<String>,
    pub blocking_uids: Vec<String>,
    pub blocking_names: Vec<String>,
    pub related_to_uids: Vec<String>,
    pub related_to_names: Vec<String>,
    pub is_paused: bool,
    pub has_subtasks: bool,
    pub has_blocking_tasks: bool,
    pub has_related_tasks: bool,
    pub has_visible_subtasks: bool,
    pub tree_location_count: u32,
    pub location: Option<String>,
    pub url: Option<String>,
    pub geo: Option<String>,
    pub time_spent_seconds: u64,
    pub last_started_at: Option<i64>,
    pub sessions: Vec<MobileWorkSession>,
    pub virtual_type: String,
    pub virtual_payload: String,
    pub is_collapsed: bool,
    pub pinned: bool,
    pub has_extractable_subtasks: bool,
    pub created_date_iso: Option<String>,
    pub last_modified_date_iso: Option<String>,

    pub goal_progress_str: Option<String>,
    pub goal_target_str: Option<String>,
    pub goal_history: Vec<f32>,
    pub rrule_history_stat: Option<String>,

    // UI Visual resolution fields
    pub visible_categories: Vec<String>,
    pub visible_location: Option<String>,
    pub is_search_context: bool,
}

impl MobileTask {
    fn empty_virtual(vtype: &str, payload: &str, depth: u32) -> Self {
        Self {
            uid: format!("virtual-{}-{}", vtype, payload),
            summary: String::new(),
            description: String::new(),
            is_done: false,
            percent_complete: None,
            priority: 0,
            due_date_iso: None,
            completed_date_iso: None,
            is_allday_due: false,
            start_date_iso: None,
            is_allday_start: false,
            has_alarms: false,
            is_future_start: false,
            duration_mins: None,
            duration_max_mins: None,
            calendar_href: String::new(),
            categories: vec![],
            is_recurring: false,
            is_relative_recurrence: false,
            parent_uid: None,
            smart_string: String::new(),
            depth,
            is_blocked: false,
            status_string: String::new(),
            blocked_by_names: vec![],
            blocked_by_uids: vec![],
            blocking_uids: vec![],
            blocking_names: vec![],
            related_to_uids: vec![],
            related_to_names: vec![],
            is_paused: false,
            has_subtasks: false,
            has_blocking_tasks: false,
            has_related_tasks: false,
            has_visible_subtasks: false,
            tree_location_count: 0,
            location: None,
            url: None,
            geo: None,
            time_spent_seconds: 0,
            last_started_at: None,
            sessions: vec![],
            virtual_type: vtype.to_string(),
            virtual_payload: payload.to_string(),
            is_collapsed: false,
            pinned: false,
            has_extractable_subtasks: false,
            created_date_iso: None,
            last_modified_date_iso: None,
            goal_progress_str: None,
            goal_target_str: None,
            goal_history: vec![],
            rrule_history_stat: None,
            visible_categories: vec![],
            visible_location: None,
            is_search_context: false,
        }
    }
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
    pub display_name: String,
    pub count: u32,
    pub depth: u32,
    pub has_children: bool,
    pub is_expanded: bool,
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
    pub display_name: String,
    pub count: u32,
    pub depth: u32,
    pub has_children: bool,
    pub is_expanded: bool,
}

#[derive(uniffi::Record)]
pub struct MobileViewData {
    pub tasks: Vec<MobileTask>,
    pub tags: Vec<MobileTag>,
    pub locations: Vec<MobileLocation>,
    pub goals: Vec<MobileGoalProgress>,
    pub focused_task_uid: Option<String>,
}

#[derive(uniffi::Record)]
pub struct MobileAlarmInfo {
    pub task_uid: String,
    pub alarm_uid: String,
    pub title: String,
    pub body: String,
}

#[derive(uniffi::Enum)]
pub enum MobileGoalType {
    Count,
    Duration,
}

#[derive(uniffi::Enum)]
pub enum MobileIntervalUnit {
    Days,
    Weeks,
    Months,
    Years,
}

#[derive(uniffi::Record)]
pub struct MobileInterval {
    pub amount: u32,
    pub unit: MobileIntervalUnit,
}

#[derive(uniffi::Record)]
pub struct MobileGoal {
    pub goal_type: MobileGoalType,
    pub target: u32,
    pub interval: MobileInterval,
}

#[derive(uniffi::Record)]
pub struct MobileGoalProgress {
    pub key: String,
    pub progress_str: String,
    pub target_str: String,
    pub period_str: String,
    pub pct: f32,
    pub history: Vec<f32>,
}

#[derive(uniffi::Record)]
pub struct MobileConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub tls_client_cert_path: Option<String>,
    pub tls_client_key_path: Option<String>,
    pub default_calendar: Option<String>,
    pub allow_insecure: bool,
    pub hide_completed: bool,
    pub hide_aliases_in_sidebar: bool,
    pub tag_aliases: HashMap<String, Vec<String>>,
    pub disabled_calendars: Vec<String>,
    pub sort_cutoff_days: Option<u32>,
    pub sort_standard_by_priority: bool,
    pub sort_preset: String,
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
    pub show_ongoing_notifications: bool,
    pub show_quick_filter: bool,
    pub quick_filter_term: String,
    pub quick_filter_icon: String,
    pub sync_settings: bool,
    pub goals: HashMap<String, MobileGoal>,
    pub default_duration_goal_mins: u32,
    pub sessions_count_as_completions: bool,
    pub show_goals_tab: bool,
    pub show_task_goals_in_sidebar: bool,
    pub expanded_tags: Vec<String>,
    pub expanded_locations: Vec<String>,
    pub expanded_done_groups: Vec<String>,
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

#[derive(uniffi::Record)]
pub struct MobileHelpCategoryData {
    pub category: HelpTab,
    pub title: String,
    pub sections: Vec<MobileHelpSection>,
}

#[uniffi::export]
impl CfaitMobile {
    pub fn get_help_data(&self) -> Vec<MobileHelpCategoryData> {
        vec![
            MobileHelpCategoryData {
                category: HelpTab::Syntax,
                title: rust_i18n::t!("syntax_help").to_string(),
                sections: crate::help::get_syntax_help()
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
                    .collect(),
            },
            MobileHelpCategoryData {
                category: HelpTab::About,
                title: rust_i18n::t!("help_about").to_string(),
                sections: vec![],
            },
        ]
    }

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

    pub fn log_message(&self, level: String, tag: String, message: String) {
        match level.to_uppercase().as_str() {
            "ERROR" => log::error!("[{}] {}", tag, message),
            "WARN" => log::warn!("[{}] {}", tag, message),
            "INFO" => log::info!("[{}] {}", tag, message),
            "DEBUG" => log::debug!("[{}] {}", tag, message),
            _ => log::trace!("[{}] {}", tag, message),
        }
    }

    pub fn get_task_tree_markdown(&self, uid: String) -> String {
        let store = self.controller.store.blocking_lock();
        crate::model::extractor::serialize_task_tree(&store, &uid)
    }

    pub fn export_locations_gpx(&self, uid: String) -> Result<String, MobileError> {
        let store = self.controller.store.blocking_lock();
        let waypoints = store.get_tree_waypoints(&uid);

        if waypoints.is_empty() {
            return Err(MobileError::from(rust_i18n::t!("no_locations").to_string()));
        }

        let mut gpx = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Cfait\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
        );
        for (name, geo) in waypoints {
            let parts: Vec<&str> = geo.split(',').collect();
            if parts.len() >= 2 {
                let escaped_name = name
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                gpx.push_str(&format!(
                    "  <wpt lat=\"{}\" lon=\"{}\"><name>{}</name></wpt>\n",
                    parts[0].trim(),
                    parts[1].trim(),
                    escaped_name
                ));
            }
        }
        gpx.push_str("</gpx>");

        Ok(gpx)
    }
}

fn populate_transient(
    t: &mut Task,
    store: &TaskStore,
    aliases: &HashMap<String, Vec<String>>,
    parent_uids: &HashSet<String>,
) {
    t.has_blocking_tasks = store.has_tasks_blocking(&t.uid);
    t.has_related_tasks = store.has_tasks_related_to(&t.uid);
    t.has_subtasks = parent_uids.contains(&t.uid);
    let now = Utc::now();
    t.is_future_start = t
        .dtstart
        .as_ref()
        .map(|start| start.to_start_comparison_time() > now)
        .unwrap_or(false);
    t.is_overdue = t
        .due
        .as_ref()
        .map(|d| !t.status.is_done() && d.to_comparison_time() < now)
        .unwrap_or(false);
    t.is_blocked = store.is_blocked(t);
    let (p_tags, p_loc) = if let Some(p_uid) = &t.parent_uid {
        if let Some(p) = store.get_task_ref(p_uid) {
            (p.categories.iter().cloned().collect(), p.location.clone())
        } else {
            (HashSet::new(), None)
        }
    } else {
        (HashSet::new(), None)
    };
    let (visible_tags, visible_location) = t.resolve_visual_attributes(&p_tags, &p_loc, aliases);
    t.visible_categories = visible_tags;
    t.visible_location = visible_location;
}

fn task_to_mobile(t: &Task, store: &TaskStore) -> MobileTask {
    let smart = t.to_smart_string();
    let status_str = format!("{:?}", t.status);

    let blocked_by_names = t
        .dependencies
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();

    let blocking_pairs = store.get_tasks_blocking(&t.uid);
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
        Some(DateType::Month(y, m)) => (Some(format!("{:04}-{:02}", y, m)), true),
        Some(DateType::Year(y)) => (Some(format!("{:04}", y)), true),
        None => (None, false),
    };

    let (start_iso, start_allday) = match &t.dtstart {
        Some(DateType::AllDay(d)) => (Some(d.format("%Y-%m-%d").to_string()), true),
        Some(DateType::Specific(dt)) => (Some(dt.to_rfc3339()), false),
        Some(DateType::Month(y, m)) => (Some(format!("{:04}-{:02}", y, m)), true),
        Some(DateType::Year(y)) => (Some(format!("{:04}", y)), true),
        None => (None, false),
    };

    let completed_date_iso = t.completion_date().map(|d| d.to_rfc3339());
    let created_date_iso = t.created_date().map(|d| d.to_rfc3339());
    let last_modified_date_iso = t.last_modified_date().map(|d| d.to_rfc3339());

    let has_alarms = !t
        .alarms
        .iter()
        .all(|a| a.acknowledged.is_some() || a.is_snooze());

    let tree_location_count = store.count_tree_locations(&t.uid) as u32;

    let (v_type, v_payload) = ("none".to_string(), "".to_string());

    let mut goal_progress_str = None;
    let mut goal_target_str = None;
    let mut goal_history = Vec::new();
    let mut rrule_history_stat = None;

    if let Some(rrule) = &t.rrule {
        let (count, _, key) = store.get_completion_history_stats(&t.uid, rrule);
        if count > 0 {
            let window_str = rust_i18n::t!(key).to_string();
            let text = if count == 1 {
                rust_i18n::t!("habit_completed_in_past.one", window = window_str).to_string()
            } else {
                rust_i18n::t!(
                    "habit_completed_in_past.other",
                    count = count,
                    window = window_str
                )
                .to_string()
            };
            rrule_history_stat = Some(text);
        }
    }

    if let Some(goal) = &t.goal {
        let progress = store.calculate_goal_progress(&format!("task:{}", t.uid), goal);
        let (c_str, t_str) = if goal.goal_type == crate::config::GoalType::Duration {
            crate::model::parser::format_goal_duration(progress, goal.target)
        } else {
            (progress.to_string(), goal.target.to_string())
        };
        goal_progress_str = Some(c_str);
        goal_target_str = Some(goal.format_target_display(&t_str));
    }

    if let Some(goal) = t.get_effective_goal() {
        goal_history = store.calculate_goal_history(&format!("task:{}", t.uid), &goal, 7);
    }

    MobileTask {
        uid: t.uid.clone(),
        summary: t.summary.clone(),
        description: t.description.clone(),
        is_done: t.status.is_done(),
        percent_complete: t.percent_complete,
        priority: t.priority,
        due_date_iso: due_iso,
        completed_date_iso,
        is_allday_due: due_allday,
        start_date_iso: start_iso,
        is_allday_start: start_allday,
        has_alarms,
        is_future_start: t.is_future_start,
        duration_mins: t.estimated_duration,
        duration_max_mins: t.estimated_duration_max,
        calendar_href: t.calendar_href.clone(),
        categories: t.categories.clone(),
        is_recurring: t.rrule.is_some(),
        is_relative_recurrence: t.is_relative_recurrence(),
        parent_uid: t.parent_uid.clone(),
        smart_string: smart,
        depth: t.depth as u32,
        is_blocked: t.is_blocked,
        status_string: status_str,
        blocked_by_names,
        blocked_by_uids: t.dependencies.clone(),
        blocking_uids,
        blocking_names,
        related_to_uids: t.related_to.clone(),
        related_to_names,
        is_paused: t.is_paused(),
        has_subtasks: t.has_subtasks,
        has_blocking_tasks: t.has_blocking_tasks,
        has_related_tasks: t.has_related_tasks,
        has_visible_subtasks: t.has_visible_subtasks,
        tree_location_count,
        location: t.location.clone(),
        url: t.url.clone(),
        geo: t.geo.clone(),
        time_spent_seconds: t.time_spent_seconds,
        last_started_at: t.last_started_at,
        sessions: t
            .sessions
            .iter()
            .map(|s| MobileWorkSession {
                start_ms: s.start * 1000,
                end_ms: s.end * 1000,
            })
            .collect(),
        virtual_type: v_type,
        virtual_payload: v_payload,
        is_collapsed: t.collapsed,
        pinned: t.pinned,
        has_extractable_subtasks: t.has_extractable_subtasks(),
        created_date_iso,
        last_modified_date_iso,
        goal_progress_str,
        goal_target_str,
        goal_history,
        rrule_history_stat,
        visible_categories: t.visible_categories.clone(),
        visible_location: t.visible_location.clone(),
        is_search_context: t.is_search_context,
    }
}

#[derive(uniffi::Object)]
pub struct CfaitMobile {
    controller: TaskController,
    alarm_index_cache: Arc<Mutex<Option<AlarmIndex>>>,
    ctx: Arc<dyn AppContext>,
    session: Arc<Mutex<crate::model::SessionState>>,
}

fn load_mobile_config_with_credentials(ctx: &dyn AppContext) -> Config {
    match Config::load_with_credentials(ctx) {
        Ok(config) => config,
        Err(err) => {
            #[cfg(target_os = "android")]
            log::warn!(
                "Falling back to config-only load after credential load failure: {}",
                err
            );
            #[cfg(not(target_os = "android"))]
            let _ = err;
            Config::load(ctx).unwrap_or_default()
        }
    }
}

fn apply_mobile_credentials_update(config: &mut Config, user: &str, pass: &str) {
    let previous_username = config.username.clone();
    config.username = user.to_string();

    if !pass.is_empty() {
        config.password = pass.to_string();
    } else if !previous_username.is_empty() && previous_username != user {
        // The Android settings UI intentionally leaves stored passwords blank on reload.
        // Preserve credentials for the same account, but avoid reusing one account's
        // password for a different username when the field is left untouched.
        config.password.clear();
    }
}

#[uniffi::export]
impl CfaitMobile {
    #[uniffi::constructor]
    pub fn new(android_files_dir: String) -> Self {
        let ctx: Arc<dyn AppContext> =
            Arc::new(StandardContext::new(Some(PathBuf::from(android_files_dir))));

        let config = crate::config::Config::load(ctx.as_ref()).unwrap_or_default();
        crate::system::init_logging(
            ctx.as_ref(),
            false,
            Some(config.log_level.to_level_filter()),
        );
        crate::system::init_keyring();

        let store = Arc::new(Mutex::new(TaskStore::new(ctx.clone())));
        let client = Arc::new(Mutex::new(None));
        let controller = TaskController::new(store, client, ctx.clone());

        let config = crate::config::Config::load(ctx.as_ref()).unwrap_or_default();
        let session = crate::model::SessionState {
            expanded_tags: config.expanded_tags,
            expanded_locations: config.expanded_locations,
            ..Default::default()
        };

        let c_clone = controller.clone();
        if let Some(runtime) = TOKIO_RUNTIME.get() {
            runtime.spawn(async move {
                let _ = c_clone.prune_trash().await;
            });
        } else {
            #[cfg(target_os = "android")]
            log::error!("Tokio runtime not initialized before CfaitMobile::new() was called!");
        }

        Self {
            controller,
            alarm_index_cache: Arc::new(Mutex::new(None)),
            ctx,
            session: Arc::new(Mutex::new(session)),
        }
    }

    pub fn create_debug_export(&self) -> Result<String, MobileError> {
        self.create_debug_export_internal()
    }

    pub fn set_locale(&self, locale: String) {
        crate::config::set_locale_with_fallback(&locale);
    }

    pub fn has_unsynced_changes(&self) -> bool {
        !crate::journal::Journal::load(self.ctx.as_ref()).is_empty()
    }

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
        let msg = if count == 1 {
            rust_i18n::t!("import_success.one").to_string()
        } else {
            rust_i18n::t!("import_success.other", count = count).to_string()
        };
        Ok(msg)
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
        let c = load_mobile_config_with_credentials(self.ctx.as_ref());
        MobileConfig {
            url: c.url,
            username: c.username,
            password: c.password,
            tls_client_cert_path: c.tls_client_cert_path,
            tls_client_key_path: c.tls_client_key_path,
            default_calendar: c.default_calendar,
            allow_insecure: c.allow_insecure_certs,
            hide_completed: c.hide_completed,
            hide_aliases_in_sidebar: c.hide_aliases_in_sidebar,
            tag_aliases: c.tag_aliases,
            disabled_calendars: c.disabled_calendars,
            sort_cutoff_days: c.sort_cutoff_days,
            sort_standard_by_priority: c.sort_standard_by_priority,
            sort_preset: c.sort_preset.to_string(),
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
            show_ongoing_notifications: c.show_ongoing_notifications,
            show_quick_filter: c.show_quick_filter,
            quick_filter_term: c.quick_filter_term,
            quick_filter_icon: c.quick_filter_icon,
            sync_settings: c.sync_settings,
            goals: c
                .goals
                .into_iter()
                .map(|(k, v)| {
                    let mt = match v.goal_type {
                        crate::config::GoalType::Count => MobileGoalType::Count,
                        crate::config::GoalType::Duration => MobileGoalType::Duration,
                    };
                    let m_unit = match v.interval.unit {
                        crate::config::IntervalUnit::Days => MobileIntervalUnit::Days,
                        crate::config::IntervalUnit::Weeks => MobileIntervalUnit::Weeks,
                        crate::config::IntervalUnit::Months => MobileIntervalUnit::Months,
                        crate::config::IntervalUnit::Years => MobileIntervalUnit::Years,
                    };
                    (
                        k,
                        MobileGoal {
                            goal_type: mt,
                            target: v.target,
                            interval: MobileInterval {
                                amount: v.interval.amount,
                                unit: m_unit,
                            },
                        },
                    )
                })
                .collect(),
            default_duration_goal_mins: c.default_duration_goal_mins,
            sessions_count_as_completions: c.sessions_count_as_completions,
            show_goals_tab: c.show_goals_tab,
            show_task_goals_in_sidebar: c.show_task_goals_in_sidebar,
            expanded_tags: c.expanded_tags,
            expanded_locations: c.expanded_locations,
            expanded_done_groups: Vec::new(),
        }
    }

    pub fn parse_duration_string(&self, val: String) -> Option<u32> {
        crate::model::parser::parse_duration(&val)
    }

    pub async fn add_session(&self, uid: String, input: String) -> Result<(), MobileError> {
        if let Some(session) = crate::model::parser::parse_session_input(&input) {
            let mut store = self.controller.store.lock().await;
            if let Some((task, _)) = store.get_task_mut(&uid) {
                task.add_session(session);
                task.sequence += 1;
                let cloned = task.clone();
                drop(store);
                self.controller
                    .update_task(cloned)
                    .await
                    .map_err(MobileError::from)?;
                self.rebuild_alarm_index().await;
                Ok(())
            } else {
                Err(MobileError::from(
                    rust_i18n::t!("error_task_not_found").to_string(),
                ))
            }
        } else {
            Err(MobileError::from(
                rust_i18n::t!("error_format", msg = "Invalid time format").to_string(),
            ))
        }
    }

    pub async fn edit_session(
        &self,
        uid: String,
        index: u32,
        input: String,
    ) -> Result<(), MobileError> {
        if let Some(session) = crate::model::parser::parse_session_input(&input) {
            let mut store = self.controller.store.lock().await;
            if let Some((task, _)) = store.get_task_mut(&uid) {
                let idx = index as usize;
                task.remove_session(idx);
                task.add_session(session);
                task.sequence += 1;
                let cloned = task.clone();
                drop(store);
                self.controller
                    .update_task(cloned)
                    .await
                    .map_err(MobileError::from)?;
                self.rebuild_alarm_index().await;
                Ok(())
            } else {
                Err(MobileError::from(
                    rust_i18n::t!("error_task_not_found").to_string(),
                ))
            }
        } else {
            Err(MobileError::from(
                rust_i18n::t!("error_format", msg = "Invalid time format").to_string(),
            ))
        }
    }

    pub async fn delete_session(&self, uid: String, index: u32) -> Result<(), MobileError> {
        let mut store = self.controller.store.lock().await;
        if let Some((task, _)) = store.get_task_mut(&uid) {
            let idx = index as usize;
            task.remove_session(idx);
            task.sequence += 1;
            let cloned = task.clone();
            drop(store);
            self.controller
                .update_task(cloned)
                .await
                .map_err(MobileError::from)?;
            self.rebuild_alarm_index().await;
            Ok(())
        } else {
            Err(MobileError::from(
                rust_i18n::t!("error_task_not_found").to_string(),
            ))
        }
    }

    pub fn save_config(&self, config: MobileConfig) -> Result<(), MobileError> {
        let mut c = load_mobile_config_with_credentials(self.ctx.as_ref());
        let old_c = c.clone();
        c.url = config.url;
        apply_mobile_credentials_update(&mut c, &config.username, &config.password);
        c.tls_client_cert_path = config.tls_client_cert_path;
        c.tls_client_key_path = config.tls_client_key_path;
        c.allow_insecure_certs = config.allow_insecure;
        c.hide_completed = config.hide_completed;
        c.hide_aliases_in_sidebar = config.hide_aliases_in_sidebar;
        c.tag_aliases = config.tag_aliases;
        c.disabled_calendars = config.disabled_calendars;
        c.sort_cutoff_days = config.sort_cutoff_days;
        c.sort_standard_by_priority = config.sort_standard_by_priority;
        c.sort_preset = config.sort_preset.parse().unwrap_or_default();
        c.urgent_days_horizon = config.urgent_days;
        c.urgent_priority_threshold = config.urgent_prio;
        c.default_priority = config.default_priority;
        c.start_grace_period_days = config.start_grace_period_days;
        c.auto_reminders = config.auto_reminders;
        c.default_reminder_time = config.default_reminder_time;
        c.snooze_short_mins = config.snooze_short;
        c.create_events_for_tasks = config.create_events_for_tasks;
        c.delete_events_on_completion = config.delete_events_on_completion;
        c.auto_refresh_interval_mins = config.auto_refresh_interval;
        c.trash_retention_days = config.trash_retention;

        c.max_done_roots = config.max_done_roots as usize;
        c.max_done_subtasks = config.max_done_subtasks as usize;
        c.show_ongoing_notifications = config.show_ongoing_notifications;
        c.show_quick_filter = config.show_quick_filter;
        c.quick_filter_term = config.quick_filter_term;
        c.quick_filter_icon = config.quick_filter_icon;
        c.sync_settings = config.sync_settings;

        c.goals = config
            .goals
            .into_iter()
            .map(|(k, v)| {
                let t = match v.goal_type {
                    MobileGoalType::Count => crate::config::GoalType::Count,
                    MobileGoalType::Duration => crate::config::GoalType::Duration,
                };
                let p = match v.interval.unit {
                    MobileIntervalUnit::Days => crate::config::IntervalUnit::Days,
                    MobileIntervalUnit::Weeks => crate::config::IntervalUnit::Weeks,
                    MobileIntervalUnit::Months => crate::config::IntervalUnit::Months,
                    MobileIntervalUnit::Years => crate::config::IntervalUnit::Years,
                };
                (
                    k,
                    crate::config::Goal {
                        goal_type: t,
                        target: v.target,
                        interval: crate::config::Interval {
                            amount: v.interval.amount,
                            unit: p,
                        },
                    },
                )
            })
            .collect();
        c.default_duration_goal_mins = config.default_duration_goal_mins;
        c.sessions_count_as_completions = config.sessions_count_as_completions;
        c.show_goals_tab = config.show_goals_tab;

        c.expanded_tags = config.expanded_tags;
        c.expanded_locations = config.expanded_locations;

        c.update_sync_timestamp_if_changed(&old_c);

        c.save_with_credentials(self.ctx.as_ref())
            .map_err(MobileError::from)
    }

    pub fn move_calendar(&self, href: String, direction: i8) -> Result<(), MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let cals = self.get_calendars();

        let mut current_order = config.collection_order.clone();
        for cal in &cals {
            if !current_order.contains(&cal.href)
                && cal.href != crate::storage::LOCAL_TRASH_HREF
                && cal.href != "local://recovery"
            {
                current_order.push(cal.href.clone());
            }
        }

        if let Some(idx) = current_order.iter().position(|h| h == &href) {
            let new_idx =
                (idx as i32 + direction as i32).clamp(0, (current_order.len() - 1) as i32) as usize;
            if idx != new_idx {
                current_order.swap(idx, new_idx);
                config.collection_order = current_order;
                config.save(self.ctx.as_ref()).map_err(MobileError::from)?;
            }
        }
        Ok(())
    }

    pub fn get_calendars(&self) -> Vec<MobileCalendar> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let disabled_set: HashSet<String> = config.disabled_calendars.iter().cloned().collect();
        let mut result = Vec::new();
        let store = self.controller.store.blocking_lock();
        if let Ok(locals) = LocalCalendarRegistry::load(self.ctx.as_ref()) {
            for loc in locals {
                if loc.href == crate::storage::LOCAL_TRASH_HREF || loc.href == "local://recovery" {
                    if let Some(map) = store.calendars.get(&loc.href) {
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

        let order = config.collection_order.clone();
        result.sort_by(|a, b| {
            crate::model::compare_calendars(&a.href, &a.name, &b.href, &b.name, &order)
        });

        result
    }

    pub fn get_ongoing_tasks(&self) -> Vec<MobileTask> {
        let store = self.controller.store.blocking_lock();
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let parent_uids = store.get_all_parent_uids();
        let mut results = Vec::new();
        for (href, map) in &store.calendars {
            if href == crate::storage::LOCAL_TRASH_HREF || href == "local://recovery" {
                continue;
            }
            for t in map.values() {
                if t.status == crate::model::TaskStatus::InProcess {
                    let mut cloned = t.clone();
                    populate_transient(&mut cloned, &store, &config.tag_aliases, &parent_uids);
                    results.push(task_to_mobile(&cloned, &store));
                }
            }
        }
        results
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

    pub fn toggle_all_calendars(&self, show_all: bool) -> Result<(), MobileError> {
        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        if show_all {
            config.hidden_calendars.clear();
            if config.default_calendar.as_deref() != Some(crate::storage::LOCAL_TRASH_HREF) {
                config
                    .hidden_calendars
                    .push(crate::storage::LOCAL_TRASH_HREF.to_string());
            }
        } else {
            let cals = self.get_calendars();
            for cal in cals {
                if config.default_calendar.as_ref() != Some(&cal.href)
                    && !config.hidden_calendars.contains(&cal.href)
                {
                    config.hidden_calendars.push(cal.href);
                }
            }
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
                if task.status.is_done() || task.status == crate::model::TaskStatus::InProcess {
                    continue;
                }
                if task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                    || task.calendar_href == "local://recovery"
                {
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
                    let mut check_implicit = |dt: DateTime<Utc>| {
                        if !task.has_alarm_at(dt) {
                            check_ts(dt.timestamp(), &mut global_earliest);
                        }
                    };
                    if let Some(due) = &task.due {
                        let dt = match due {
                            DateType::Specific(t) => *t,
                            DateType::AllDay(d) => {
                                crate::model::item::safe_local_to_utc(*d, default_time)
                            }
                            DateType::Month(y, m) => {
                                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                            DateType::Year(y) => {
                                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                        };
                        check_implicit(dt);
                    }
                    if let Some(start) = &task.dtstart {
                        let dt = match start {
                            DateType::Specific(t) => *t,
                            DateType::AllDay(d) => {
                                crate::model::item::safe_local_to_utc(*d, default_time)
                            }
                            DateType::Month(y, m) => {
                                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                            DateType::Year(y) => {
                                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                        };
                        check_implicit(dt);
                    }
                }
            }
        }
        global_earliest
    }

    pub fn get_firing_alarms(&self) -> Vec<MobileAlarmInfo> {
        let mut firing_entries = Vec::new();
        {
            let cached = self.alarm_index_cache.blocking_lock();
            if let Some(ref index) = *cached {
                firing_entries = index.get_firing_alarms();
            }
        }

        if !firing_entries.is_empty() {
            let store = self.controller.store.blocking_lock();
            return firing_entries
                .into_iter()
                .filter(|e| {
                    if let Some(task) = store.get_task_ref(&e.task_uid) {
                        !task.status.is_done()
                            && task.calendar_href != crate::storage::LOCAL_TRASH_HREF
                            && task.calendar_href != "local://recovery"
                    } else {
                        false
                    }
                })
                .map(|e| MobileAlarmInfo {
                    task_uid: e.task_uid,
                    alarm_uid: e.alarm_uid,
                    title: e.task_title,
                    body: e
                        .description
                        .unwrap_or_else(|| rust_i18n::t!("reminder").to_string()),
                })
                .collect();
        }

        let index = AlarmIndex::load(self.ctx.as_ref());
        if !index.is_empty() {
            let firing_from_disk = index.get_firing_alarms();
            if !firing_from_disk.is_empty() {
                *self.alarm_index_cache.blocking_lock() = Some(index);
                let store = self.controller.store.blocking_lock();
                return firing_from_disk
                    .into_iter()
                    .filter(|e| {
                        if let Some(task) = store.get_task_ref(&e.task_uid) {
                            !task.status.is_done()
                                && task.calendar_href != crate::storage::LOCAL_TRASH_HREF
                                && task.calendar_href != "local://recovery"
                        } else {
                            false
                        }
                    })
                    .map(|e| MobileAlarmInfo {
                        task_uid: e.task_uid,
                        alarm_uid: e.alarm_uid,
                        title: e.task_title,
                        body: e
                            .description
                            .unwrap_or_else(|| rust_i18n::t!("reminder").to_string()),
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
                if task.status.is_done() || task.status == crate::model::TaskStatus::InProcess {
                    continue;
                }
                if task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                    || task.calendar_href == "local://recovery"
                {
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
                            body: alarm
                                .description
                                .clone()
                                .unwrap_or_else(|| rust_i18n::t!("reminder").to_string()),
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
                            DateType::AllDay(d) => {
                                crate::model::item::safe_local_to_utc(*d, default_time)
                            }
                            DateType::Month(y, m) => {
                                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                            DateType::Year(y) => {
                                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                        };
                        let alarm_due_now = rust_i18n::t!("alarm_due_now");
                        check_implicit(dt, alarm_due_now.as_ref(), "due");
                    }
                    if let Some(start) = &task.dtstart {
                        let dt = match start {
                            DateType::Specific(t) => *t,
                            DateType::AllDay(d) => {
                                crate::model::item::safe_local_to_utc(*d, default_time)
                            }
                            DateType::Month(y, m) => {
                                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                            DateType::Year(y) => {
                                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                crate::model::item::safe_local_to_utc(d, default_time)
                            }
                        };
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
        let tags_str = tags.join(",");
        let proper_tags = crate::model::parser::parse_alias_values(&tags_str);
        crate::model::validate_alias_integrity(&key, &proper_tags, &c.tag_aliases)
            .map_err(MobileError::from)?;
        c.tag_aliases.insert(key.clone(), proper_tags.clone());
        c.save(self.ctx.as_ref()).map_err(MobileError::from)?;
        let mut store = self.controller.store.lock().await;
        let modified = store.apply_alias_retroactively(&key, &proper_tags);
        drop(store);
        if !modified.is_empty() {
            for t in modified {
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
            return Err(MobileError::from(
                rust_i18n::t!("error_cannot_depend_on_self").to_string(),
            ));
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
        let mut err_msg = None;
        let res = self
            .apply_store_mutation(&child_uid, |store, id| {
                match store.set_parent(id, parent_uid) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        err_msg = Some(e);
                        None
                    }
                }
            })
            .await;

        if let Some(e) = err_msg {
            return Err(MobileError::from(e));
        }
        res
    }

    pub async fn add_related_to(
        &self,
        task_uid: String,
        related_uid: String,
    ) -> Result<(), MobileError> {
        if task_uid == related_uid {
            return Err(MobileError::from(
                rust_i18n::t!("error_cannot_relate_to_self").to_string(),
            ));
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

    pub async fn sync_journal(&self) -> Result<bool, MobileError> {
        let (_warns, synced, _config_changed) = self
            .controller
            .sync_and_update_store()
            .await
            .map_err(MobileError::from)?;
        Ok(synced
            .iter()
            .any(|t| t.summary.ends_with("(Conflict Copy)")))
    }

    pub async fn sync(&self) -> Result<String, MobileError> {
        let config = Config::load_with_credentials(self.ctx.as_ref()).map_err(MobileError::from)?;
        self.apply_connection(config).await
    }

    pub async fn connect(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
    ) -> Result<String, MobileError> {
        let mut config = load_mobile_config_with_credentials(self.ctx.as_ref());
        config.url = url;
        apply_mobile_credentials_update(&mut config, &user, &pass);
        config.allow_insecure_certs = insecure;
        self.apply_connection(config).await
    }

    pub async fn get_all_tags(&self) -> Vec<MobileTag> {
        Vec::new()
    }

    pub async fn get_all_locations(&self) -> Vec<MobileLocation> {
        Vec::new()
    }

    pub async fn get_task_by_uid(&self, uid: String) -> Option<MobileTask> {
        let store = self.controller.store.lock().await;
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let parent_uids = store.get_all_parent_uids();

        if let Some(task) = store.get_task_ref(&uid) {
            let mut cloned = task.clone();
            populate_transient(&mut cloned, &store, &config.tag_aliases, &parent_uids);
            Some(task_to_mobile(&cloned, &store))
        } else {
            None
        }
    }

    pub async fn get_view_tasks(&self, options: MobileFilterOptions) -> MobileViewData {
        let store = self.controller.store.lock().await;
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);

        let expanded_set: HashSet<String> = options.expanded_groups.into_iter().collect();
        let expanded_tags_set: HashSet<String> = options.expanded_tags.into_iter().collect();
        let expanded_locations_set: HashSet<String> =
            options.expanded_locations.into_iter().collect();

        let session = self.session.lock().await;
        let search_collapsed_set: HashSet<String> =
            session.search_collapsed_tasks.iter().cloned().collect();
        let focused_task_uid = session.focused_task_uid.clone();
        drop(session);

        let cutoff_date = config
            .sort_cutoff_days
            .map(|d| Utc::now() + chrono::Duration::days(d as i64));
        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &options.filter_tags.into_iter().collect(),
            selected_locations: &options.filter_locations.into_iter().collect(),
            match_all_categories: options.match_all_categories,
            search_term: &options.search_query,
            hide_completed_global: config.hide_completed,
            hide_fully_completed_tags: config.hide_fully_completed_tags,
            hide_aliases_in_sidebar: config.hide_aliases_in_sidebar,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
            sort_standard_by_priority: config.sort_standard_by_priority,
            sort_preset: config.sort_preset,
            expanded_done_groups: &expanded_set,
            expanded_tags: &expanded_tags_set,
            expanded_locations: &expanded_locations_set,
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
            tag_aliases: &config.tag_aliases,
            search_collapsed_tasks: &search_collapsed_set,
            focused_task_uid: focused_task_uid.as_deref(),
        });

        let mut last_calendar_href = String::new();
        let tasks = filtered
            .items
            .into_iter()
            .filter_map(|item| {
                if let crate::store::TaskListItem::Task(t) = item {
                    let mt = task_to_mobile(&t, &store);
                    last_calendar_href = mt.calendar_href.clone();
                    Some(mt)
                } else if let crate::store::TaskListItem::ExpandGroup(p_uid, depth) = item {
                    let mut vt = MobileTask::empty_virtual("expand", &p_uid, depth as u32);
                    vt.calendar_href = if p_uid.is_empty() {
                        last_calendar_href.clone()
                    } else if let Some(p) = store.get_task_ref(&p_uid) {
                        p.calendar_href.clone()
                    } else {
                        last_calendar_href.clone()
                    };
                    Some(vt)
                } else if let crate::store::TaskListItem::CollapseGroup(p_uid, depth) = item {
                    let mut vt = MobileTask::empty_virtual("collapse", &p_uid, depth as u32);
                    vt.calendar_href = if p_uid.is_empty() {
                        last_calendar_href.clone()
                    } else if let Some(p) = store.get_task_ref(&p_uid) {
                        p.calendar_href.clone()
                    } else {
                        last_calendar_href.clone()
                    };
                    Some(vt)
                } else {
                    None
                }
            })
            .collect();

        let tags = filtered
            .categories
            .into_iter()
            .map(|item| MobileTag {
                name: item.full_key.clone(),
                display_name: item.display_name,
                count: item.count,
                depth: item.depth,
                has_children: item.has_children,
                is_expanded: item.is_expanded,
                is_uncategorized: item.full_key == UNCATEGORIZED_ID,
            })
            .collect();

        let locations = filtered
            .locations
            .into_iter()
            .map(|item| MobileLocation {
                name: item.full_key.clone(),
                display_name: item.display_name,
                count: item.count,
                depth: item.depth,
                has_children: item.has_children,
                is_expanded: item.is_expanded,
            })
            .collect();

        let mut evaluated_goals = Vec::new();
        for (key, goal) in &config.goals {
            let progress = store.calculate_goal_progress(key, goal);
            let (progress_str, target_str) = if goal.goal_type == crate::config::GoalType::Duration
            {
                crate::model::parser::format_goal_duration(progress, goal.target)
            } else {
                (progress.to_string(), goal.target.to_string())
            };
            let pct = if goal.target > 0 {
                (progress as f32 / goal.target as f32).min(1.0)
            } else {
                0.0
            };
            let history = store.calculate_goal_history(key, goal, 7);

            evaluated_goals.push(MobileGoalProgress {
                key: key.clone(),
                progress_str,
                target_str: target_str.clone(),
                period_str: goal.format_target_display(&target_str),
                pct,
                history,
            });
        }

        if config.show_task_goals_in_sidebar {
            let _now = chrono::Utc::now();
            let mut task_goals = Vec::new();
            for (href, map) in store.calendars.iter() {
                if hidden.contains(href)
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
                        let progress =
                            store.calculate_goal_progress(&format!("task:{}", t.uid), goal);
                        let (progress_str, target_str) =
                            if goal.goal_type == crate::config::GoalType::Duration {
                                crate::model::parser::format_goal_duration(progress, goal.target)
                            } else {
                                (progress.to_string(), goal.target.to_string())
                            };
                        let pct = if goal.target > 0 {
                            (progress as f32 / goal.target as f32).min(1.0)
                        } else {
                            0.0
                        };
                        let history =
                            store.calculate_goal_history(&format!("task:{}", t.uid), goal, 7);
                        task_goals.push(MobileGoalProgress {
                            key: format!("task:{}", t.uid), // special prefix for UI to know it's a task jump
                            progress_str,
                            target_str: target_str.clone(),
                            period_str: format!(
                                "{} - {}",
                                t.summary,
                                goal.format_target_display(&target_str)
                            ),
                            pct,
                            history,
                        });
                    }
                }
            }
            // Sort task goals alphabetically by the period_str (which now contains the summary)
            task_goals.sort_by(|a, b| a.period_str.cmp(&b.period_str));
            evaluated_goals.extend(task_goals);
        }

        MobileViewData {
            tasks,
            tags,
            locations,
            goals: evaluated_goals,
            focused_task_uid,
        }
    }

    pub async fn dispatch(&self, intent: crate::model::AppIntent) -> Result<(), MobileError> {
        let mut session = self.session.lock().await;
        let mut store = self.controller.store.lock().await;
        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();

        session.apply_session_intent(&intent);

        let mut config_to_save = config.clone();
        config_to_save.expanded_tags = session.expanded_tags.clone();
        config_to_save.expanded_locations = session.expanded_locations.clone();
        let _ = config_to_save.save(self.ctx.as_ref());

        let actions = store.apply_task_intent(&intent, &config);

        drop(store);
        drop(session);

        if !actions.is_empty() {
            // Await disk persistence synchronously so the app doesn't suspend
            // before the user's modifications are safely queued to disk.
            let _ = self.controller.persist_changes(actions).await;
        }

        let store_arc = self.controller.store.clone();
        let alarm_cache = self.alarm_index_cache.clone();
        let ctx_clone = self.ctx.clone();
        tokio::spawn(async move {
            let index = {
                let s = store_arc.lock().await;
                crate::alarm_index::AlarmIndex::rebuild_from_tasks(
                    &s.calendars,
                    config.auto_reminders,
                    &config.default_reminder_time,
                )
            }; // Lock is dropped here
            let _ = index.save(ctx_clone.as_ref());
            *alarm_cache.lock().await = Some(index);
        });

        Ok(())
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
            .sort_cutoff_days
            .map(|d| Utc::now() + chrono::Duration::days(d as i64));
        let filter_res = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &filter_tags.into_iter().collect(),
            selected_locations: &filter_locations.into_iter().collect(),
            match_all_categories: false,
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            hide_fully_completed_tags: config.hide_fully_completed_tags,
            hide_aliases_in_sidebar: config.hide_aliases_in_sidebar,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
            sort_standard_by_priority: config.sort_standard_by_priority,
            sort_preset: config.sort_preset,
            expanded_done_groups: &HashSet::new(),
            expanded_tags: &HashSet::new(),
            expanded_locations: &HashSet::new(),
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
            tag_aliases: &config.tag_aliases,
            search_collapsed_tasks: &HashSet::new(),
            focused_task_uid: None,
        });
        let filtered: Vec<crate::model::Task> = filter_res
            .items
            .iter()
            .filter_map(|item| {
                if let crate::store::TaskListItem::Task(t) = item {
                    Some((**t).clone())
                } else {
                    None
                }
            })
            .collect();
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
        let (clean_input_1, new_goals) = crate::model::extract_inline_goals(&input);
        let (clean_input, new_aliases) = crate::model::extract_inline_aliases(&clean_input_1);

        let config_changed = !new_goals.is_empty() || !new_aliases.is_empty();

        if !new_goals.is_empty() {
            config.goals.extend(new_goals);
        }

        if !new_aliases.is_empty() {
            for (k, v) in &new_aliases {
                crate::model::validate_alias_integrity(k, v, &config.tag_aliases)
                    .map_err(MobileError::from)?;
            }
            config.tag_aliases.extend(new_aliases.clone());

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
        }

        if config_changed {
            let old_config = Config::load(self.ctx.as_ref()).unwrap_or_default();
            config.update_sync_timestamp_if_changed(&old_config);
            config.save(self.ctx.as_ref()).map_err(MobileError::from)?;

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

        let store = self.controller.store.lock().await;
        if let Err(e) = store.resolve_dependencies(&mut task) {
            return Err(MobileError::from(e));
        }
        drop(store);

        if task.summary.trim().is_empty() {
            return Ok("".to_string());
        }
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
        self.rebuild_alarm_index().await;
        #[cfg(target_os = "android")]
        log::debug!("Alarm index rebuilt. Returning uid: {}", uid);
        Ok(uid)
    }

    pub async fn add_task_with_description(
        &self,
        input: String,
        description: String,
    ) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        log::debug!("add_task_with_description: '{}'", input);

        let mut config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let (clean_input_1, new_goals) = crate::model::extract_inline_goals(&input);
        let (clean_input, new_aliases) = crate::model::extract_inline_aliases(&clean_input_1);

        let config_changed = !new_goals.is_empty() || !new_aliases.is_empty();

        if !new_goals.is_empty() {
            config.goals.extend(new_goals);
        }

        if !new_aliases.is_empty() {
            for (k, v) in &new_aliases {
                crate::model::validate_alias_integrity(k, v, &config.tag_aliases)
                    .map_err(MobileError::from)?;
            }
            config.tag_aliases.extend(new_aliases.clone());

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
        }

        if config_changed {
            let old_config = Config::load(self.ctx.as_ref()).unwrap_or_default();
            config.update_sync_timestamp_if_changed(&old_config);
            config.save(self.ctx.as_ref()).map_err(MobileError::from)?;

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

        let def_time =
            chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

        let (cleaned_desc, extracted_subtasks) =
            crate::model::extractor::extract_markdown_tasks(&description);

        let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);

        let store = self.controller.store.lock().await;
        if let Err(e) = store.resolve_dependencies(&mut task) {
            return Err(MobileError::from(e));
        }
        drop(store);

        if task.summary.trim().is_empty() && cleaned_desc.is_empty() {
            return Ok("".to_string());
        }
        if !cleaned_desc.is_empty() {
            if task.description.is_empty() {
                task.description = cleaned_desc;
            } else {
                task.description.push_str(&format!("\n\n{}", cleaned_desc));
            }
        }
        task.calendar_href = config
            .default_calendar
            .clone()
            .unwrap_or(crate::storage::LOCAL_CALENDAR_HREF.to_string());

        let parent_uid = self
            .controller
            .create_task(task)
            .await
            .map_err(MobileError::from)?;

        for ext in extracted_subtasks {
            let mut sub = Task::new(&ext.raw_text, &config.tag_aliases, def_time);
            sub.uid = ext.uid.clone();

            let store = self.controller.store.lock().await;
            if let Err(e) = store.resolve_dependencies(&mut sub) {
                return Err(MobileError::from(e));
            }
            drop(store);

            if !ext.description.is_empty() {
                if sub.description.is_empty() {
                    sub.description = ext.description;
                } else {
                    sub.description
                        .push_str(&format!("\n\n{}", ext.description));
                }
            }

            let smart_status = sub.status;
            sub.status = ext.status;
            match ext.status {
                crate::model::TaskStatus::Completed => {
                    if sub.completion_date().is_none() {
                        sub.set_completion_date(Some(chrono::Utc::now()));
                    }
                }
                crate::model::TaskStatus::Cancelled => {
                    if sub.completion_date().is_none() {
                        sub.set_completion_date(Some(chrono::Utc::now()));
                    }
                }
                crate::model::TaskStatus::InProcess => {
                    if sub.last_started_at.is_none() {
                        sub.last_started_at = Some(chrono::Utc::now().timestamp());
                    }
                }
                crate::model::TaskStatus::NeedsAction => {
                    if smart_status == crate::model::TaskStatus::Completed {
                        sub.status = crate::model::TaskStatus::Completed;
                    }
                }
            }

            sub.parent_uid = Some(ext.parent_uid.unwrap_or(parent_uid.clone()));
            sub.dependencies = ext.dependencies;
            sub.calendar_href = config
                .default_calendar
                .clone()
                .unwrap_or(crate::storage::LOCAL_CALENDAR_HREF.to_string());

            self.controller
                .create_task(sub)
                .await
                .map_err(MobileError::from)?;
        }

        #[cfg(target_os = "android")]
        log::debug!("Rebuilding alarm index after adding {}", parent_uid);

        self.rebuild_alarm_index().await;

        Ok(parent_uid)
    }

    pub async fn change_priority(&self, uid: String, delta: i8) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::ChangePriority { uid, delta })
            .await?;
        Ok(())
    }

    pub async fn set_status_process(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::StartTask { uid })
            .await?;
        Ok(())
    }

    pub async fn set_status_cancelled(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::CancelTask { uid })
            .await?;
        Ok(())
    }

    pub async fn pause_task(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::PauseTask { uid })
            .await?;
        Ok(())
    }
    pub async fn stop_task(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::StopTask { uid })
            .await?;
        Ok(())
    }
    pub async fn start_task(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::StartTask { uid })
            .await?;
        Ok(())
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
                task.sequence += 1;
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
        let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
        let def_time =
            chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

        let (clean_desc, extracted) = crate::model::extractor::extract_markdown_tasks(&description);

        let mut store = self.controller.store.lock().await;
        let mut actions = Vec::new();
        let parent_href = if let Some((task, _)) = store.get_task_mut(&uid) {
            task.description = clean_desc;
            task.sequence += 1;
            let href = task.calendar_href.clone();
            actions.push(crate::journal::Action::Update(task.clone()));
            href
        } else {
            return Ok(());
        };

        for ext in extracted {
            let mut sub = crate::model::Task::new(&ext.raw_text, &config.tag_aliases, def_time);
            sub.uid = ext.uid;
            if !ext.description.is_empty() {
                if sub.description.is_empty() {
                    sub.description = ext.description;
                } else {
                    sub.description
                        .push_str(&format!("\n\n{}", ext.description));
                }
            }

            let smart_status = sub.status;
            sub.status = ext.status;
            match ext.status {
                crate::model::TaskStatus::Completed => {
                    if sub.completion_date().is_none() {
                        sub.set_completion_date(Some(chrono::Utc::now()));
                    }
                }
                crate::model::TaskStatus::Cancelled => {
                    if sub.completion_date().is_none() {
                        sub.set_completion_date(Some(chrono::Utc::now()));
                    }
                }
                crate::model::TaskStatus::InProcess => {
                    if sub.last_started_at.is_none() {
                        sub.last_started_at = Some(chrono::Utc::now().timestamp());
                    }
                }
                crate::model::TaskStatus::NeedsAction => {
                    if smart_status == crate::model::TaskStatus::Completed {
                        sub.status = crate::model::TaskStatus::Completed;
                    }
                }
            }

            sub.parent_uid = Some(ext.parent_uid.unwrap_or(uid.clone()));
            sub.dependencies = ext.dependencies;
            sub.calendar_href = parent_href.clone();
            if let Some(pc) = ext.percent_complete {
                sub.percent_complete = Some(pc);
            }

            store.add_task(sub.clone());
            actions.push(crate::journal::Action::Create(sub));
        }

        drop(store);
        if !actions.is_empty() {
            self.controller
                .persist_changes(actions)
                .await
                .map_err(MobileError::from)?;
            self.rebuild_alarm_index().await;
        }
        Ok(())
    }

    pub async fn toggle_task(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::ToggleTask { uid })
            .await?;
        Ok(())
    }

    pub async fn toggle_task_shift(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::ToggleTaskShift { uid })
            .await?;
        Ok(())
    }

    pub async fn move_task(&self, uid: String, new_cal_href: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::MoveTask {
            uid,
            target_href: new_cal_href,
        })
        .await?;
        Ok(())
    }

    pub async fn delete_task(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::DeleteTask { uid })
            .await?;
        Ok(())
    }

    pub async fn duplicate_task_tree(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::DuplicateTaskTree { uid })
            .await?;
        Ok(())
    }

    pub async fn delete_task_tree(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::DeleteTaskTree { uid })
            .await?;
        Ok(())
    }

    pub async fn toggle_pin(&self, uid: String) -> Result<(), MobileError> {
        self.dispatch(crate::model::AppIntent::TogglePin { uid })
            .await?;
        Ok(())
    }

    pub async fn sync_task_tree_from_markdown(
        &self,
        uid: String,
        markdown: String,
    ) -> Result<(), MobileError> {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let def_time =
            chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

        let mut cals = crate::cache::Cache::load_calendars(self.ctx.as_ref()).unwrap_or_default();
        if let Ok(locals) = crate::storage::LocalCalendarRegistry::load(self.ctx.as_ref()) {
            cals.extend(locals);
        }

        let mut store = self.controller.store.lock().await;

        match store.sync_tree_from_markdown(
            &uid,
            &markdown,
            &config.tag_aliases,
            def_time,
            config.trash_retention_days,
            &cals,
        ) {
            Ok(actions) => {
                drop(store);
                if !actions.is_empty() {
                    self.controller
                        .persist_changes(actions)
                        .await
                        .map_err(MobileError::from)?;
                    self.rebuild_alarm_index().await;
                }
                Ok(())
            }
            Err(e) => Err(MobileError::from(e)),
        }
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
            .ok_or(MobileError::from(
                rust_i18n::t!("error_client_not_connected").to_string(),
            ))?
            .clone();
        let tasks = LocalStorage::load_for_href(self.ctx.as_ref(), &source_href)
            .map_err(|e| MobileError::from(e.to_string()))?;
        if tasks.is_empty() {
            return Ok(rust_i18n::t!("status_no_tasks_to_migrate").to_string());
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

    pub async fn create_remote_calendar(
        &self,
        name: String,
        color: Option<String>,
    ) -> Result<String, MobileError> {
        let client = self
            .controller
            .client
            .lock()
            .await
            .clone()
            .ok_or_else(|| MobileError::from("Offline"))?;
        let href = client
            .create_calendar(&name, color.as_deref())
            .await
            .map_err(|e| MobileError::from(e.to_string()))?;

        // Optimistic update to prevent Android UI jitter
        if let Ok(mut cals) = crate::cache::Cache::load_calendars(self.ctx.as_ref()) {
            cals.push(crate::model::CalendarListEntry {
                name,
                href: href.clone(),
                color,
            });
            let _ = crate::cache::Cache::save_calendars(self.ctx.as_ref(), &cals);
        }
        Ok(href)
    }

    pub async fn update_remote_calendar(
        &self,
        href: String,
        name: String,
        color: Option<String>,
    ) -> Result<(), MobileError> {
        let client = self
            .controller
            .client
            .lock()
            .await
            .clone()
            .ok_or_else(|| MobileError::from("Offline"))?;
        client
            .update_calendar(&href, &name, color.as_deref())
            .await
            .map_err(|e| MobileError::from(e.to_string()))?;

        // Optimistic update to prevent Android UI jitter
        if let Ok(mut cals) = crate::cache::Cache::load_calendars(self.ctx.as_ref())
            && let Some(c) = cals.iter_mut().find(|c| c.href == href)
        {
            c.name = name;
            c.color = color;
            let _ = crate::cache::Cache::save_calendars(self.ctx.as_ref(), &cals);
        }
        Ok(())
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
            Err(MobileError::from(
                rust_i18n::t!("error_no_calendar_available").to_string(),
            ))
        }
    }

    pub async fn delete_local_calendar(&self, href: String) -> Result<(), MobileError> {
        if href == LOCAL_CALENDAR_HREF {
            return Err(MobileError::from(
                rust_i18n::t!("error_cannot_delete_default_calendar").to_string(),
            ));
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
            drop(store);
            self.rebuild_alarm_index().await;
            Ok(())
        } else {
            Err(MobileError::from(
                rust_i18n::t!("error_no_calendar_available").to_string(),
            ))
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
        // apply_store_mutation already rebuilds the alarm index
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
        // apply_store_mutation already rebuilds the alarm index
        #[cfg(target_os = "android")]
        log::debug!("Dismiss successful");
        Ok(())
    }

    pub async fn get_next_global_alarm_time(&self) -> Option<i64> {
        let store = self.controller.store.lock().await;
        let mut earliest: Option<i64> = None;
        for map in store.calendars.values() {
            for task in map.values() {
                if task.status.is_done()
                    || task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                    || task.calendar_href == "local://recovery"
                {
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
                .ok_or(MobileError::from(rust_i18n::t!("offline").to_string()))?
                .clone()
        };

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
                .ok_or(MobileError::from(rust_i18n::t!("offline").to_string()))?
                .clone()
        };

        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let count = client
            .sync_multiple_companion_events(&all_tasks, true, config.delete_events_on_completion)
            .await
            .unwrap_or(0);
        Ok(count as u32)
    }

    pub async fn should_keep_notification(
        &self,
        task_uid: String,
        notif_type: String,
        alarm_uid: Option<String>,
    ) -> bool {
        let store = self.controller.store.lock().await;
        let task = match store.get_task_ref(&task_uid) {
            Some(t) => t,
            None => return false,
        };

        if task.status.is_done()
            || task.calendar_href == crate::storage::LOCAL_TRASH_HREF
            || task.calendar_href == "local://recovery"
        {
            return false;
        }

        if notif_type == "ongoing" {
            let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
            if !config.show_ongoing_notifications {
                return false;
            }
            return task.status == crate::model::TaskStatus::InProcess;
        }

        if notif_type == "alarm" && task.status == crate::model::TaskStatus::InProcess {
            return false;
        }

        if notif_type == "alarm"
            && let Some(a_uid) = alarm_uid
        {
            if let Some(alarm) = task.alarms.iter().find(|a| a.uid == a_uid) {
                if alarm.acknowledged.is_some() {
                    return false;
                }
                let now = chrono::Utc::now();
                let trigger_dt = match alarm.trigger {
                    crate::model::AlarmTrigger::Absolute(dt) => dt,
                    crate::model::AlarmTrigger::Relative(mins) => {
                        let anchor = if let Some(crate::model::DateType::Specific(d)) = task.due {
                            d
                        } else if let Some(crate::model::DateType::Specific(s)) = task.dtstart {
                            s
                        } else {
                            return false;
                        };
                        anchor + chrono::Duration::minutes(mins as i64)
                    }
                };
                if trigger_dt > now + chrono::Duration::minutes(5) {
                    return false;
                }
            } else if a_uid.starts_with("implicit_") {
                let parts: Vec<&str> = a_uid.split('|').collect();
                if parts.len() >= 2 {
                    let type_key_with_colon = parts[0];
                    let expected_ts = parts[1];

                    let config = crate::config::Config::load(self.ctx.as_ref()).unwrap_or_default();
                    let default_time =
                        chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
                            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());

                    let mut current_ts = None;
                    if type_key_with_colon == "implicit_due:" {
                        if let Some(due) = &task.due {
                            let dt = match due {
                                crate::model::DateType::Specific(t) => *t,
                                crate::model::DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(chrono::Local)
                                    .unwrap()
                                    .with_timezone(&chrono::Utc),
                                crate::model::DateType::Month(y, m) => {
                                    let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                    d.and_time(default_time)
                                        .and_local_timezone(chrono::Local)
                                        .unwrap()
                                        .with_timezone(&chrono::Utc)
                                }
                                crate::model::DateType::Year(y) => {
                                    let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                    d.and_time(default_time)
                                        .and_local_timezone(chrono::Local)
                                        .unwrap()
                                        .with_timezone(&chrono::Utc)
                                }
                            };
                            current_ts = Some(dt.to_rfc3339());
                        }
                    } else if type_key_with_colon == "implicit_start:"
                        && let Some(start) = &task.dtstart
                    {
                        let dt = match start {
                            crate::model::DateType::Specific(t) => *t,
                            crate::model::DateType::AllDay(d) => d
                                .and_time(default_time)
                                .and_local_timezone(chrono::Local)
                                .unwrap()
                                .with_timezone(&chrono::Utc),
                            crate::model::DateType::Month(y, m) => {
                                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                d.and_time(default_time)
                                    .and_local_timezone(chrono::Local)
                                    .unwrap()
                                    .with_timezone(&chrono::Utc)
                            }
                            crate::model::DateType::Year(y) => {
                                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                d.and_time(default_time)
                                    .and_local_timezone(chrono::Local)
                                    .unwrap()
                                    .with_timezone(&chrono::Utc)
                            }
                        };
                        current_ts = Some(dt.to_rfc3339());
                    }

                    if current_ts.as_deref() != Some(expected_ts) {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl CfaitMobile {
    async fn apply_store_mutation<F>(&self, uid: &str, mutator: F) -> Result<(), MobileError>
    where
        F: FnOnce(&mut TaskStore, &str) -> Option<Task>,
    {
        let mut store = self.controller.store.lock().await;
        let task_to_save = mutator(&mut store, uid).ok_or(MobileError::from(
            rust_i18n::t!("error_task_not_found").to_string(),
        ))?;
        drop(store);

        self.controller
            .update_task(task_to_save)
            .await
            .map_err(MobileError::from)?;

        self.rebuild_alarm_index().await;

        Ok(())
    }

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
                drop(store);
                self.rebuild_alarm_index().await;
                if let Some(w) = warning {
                    return Err(MobileError::from(w));
                } else {
                    return Err(MobileError::from(e));
                }
            }
        }
        drop(store);

        let _ = self.controller.sync_and_update_store().await;

        self.rebuild_alarm_index().await;
        Ok(warning.unwrap_or_else(|| rust_i18n::t!("status_connected").to_string()))
    }

    async fn rebuild_alarm_index(&self) {
        let config = Config::load(self.ctx.as_ref()).unwrap_or_default();
        let index = {
            let store = self.controller.store.lock().await;
            AlarmIndex::rebuild_from_tasks(
                &store.calendars,
                config.auto_reminders,
                &config.default_reminder_time,
            )
        };
        match index.save(self.ctx.as_ref()) {
            Ok(_) => {
                #[cfg(target_os = "android")]
                log::debug!("Alarm index rebuilt with {} alarms", index.len());
                *self.alarm_index_cache.lock().await = Some(index);
            }
            Err(e) => {
                #[cfg(target_os = "android")]
                log::warn!("Failed to save alarm index: {}", e);
                #[cfg(not(target_os = "android"))]
                let _ = e;
            }
        }
    }

    fn create_debug_export_internal(&self) -> Result<String, MobileError> {
        #[cfg(target_os = "android")]
        {
            log::logger().flush();

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
                                use std::io::Write;
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
                                use std::io::Read;
                                f.read_to_end(&mut buffer)
                                    .map_err(|e| MobileError::from(e.to_string()))?;
                                use std::io::Write;
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
                rust_i18n::t!("debug_export_android_only").to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_mobile_credentials_update;
    use crate::config::Config;

    #[test]
    fn preserves_existing_password_when_android_ui_leaves_password_blank() {
        let mut config = Config {
            username: "alice".to_string(),
            password: "secret".to_string(),
            ..Config::default()
        };

        apply_mobile_credentials_update(&mut config, "alice", "");

        assert_eq!(config.username, "alice");
        assert_eq!(config.password, "secret");
    }

    #[test]
    fn clears_password_when_username_changes_without_new_password() {
        let mut config = Config {
            username: "alice".to_string(),
            password: "secret".to_string(),
            ..Config::default()
        };

        apply_mobile_credentials_update(&mut config, "bob", "");

        assert_eq!(config.username, "bob");
        assert!(config.password.is_empty());
    }

    #[test]
    fn replaces_password_when_user_enters_a_new_one() {
        let mut config = Config {
            username: "alice".to_string(),
            password: "secret".to_string(),
            ..Config::default()
        };

        apply_mobile_credentials_update(&mut config, "alice", "new-secret");

        assert_eq!(config.username, "alice");
        assert_eq!(config.password, "new-secret");
    }
}
