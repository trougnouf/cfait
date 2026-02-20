// File: src/model/item.rs
/*
This file contains the core Task data model used across clients (TUI, GUI, Mobile).
It defines the in-memory representation of a Task and related helper types (Alarm,
DateType, VirtualState, etc.).

Notes / rationale:
- The Task struct is intentionally a compact, serde-friendly representation that maps
  closely to VTODO/ICS fields. Additional transient fields (sorting flags, derived
  attributes) are marked `#[serde(skip)]` because they are runtime-only.
- A small "virtual" task concept (VirtualState) is used by the UI layers to inject
  expand/collapse placeholder rows when truncating completed-subtask groups. These
  virtual tasks are not persisted and are only used for rendering.
- Time-tracking fields (time_spent_seconds, last_started_at, sessions) track
  lightweight local work sessions. The recycle/advance logic commits running time
  before creating history snapshots for recurring tasks.
- Many helpers in this module are thin wrappers delegating to adapter/recurrence
  modules so higher-level logic stays testable and encapsulated.
*/

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn default_uid() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarListEntry {
    pub name: String,
    pub href: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    NeedsAction,
    InProcess,
    Completed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawProperty {
    pub key: String,
    pub value: String,
    pub params: Vec<(String, String)>,
}

/// A minimal work-session record for time tracking.
/// start/end are Unix timestamps (seconds since epoch).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkSession {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum DateType {
    AllDay(NaiveDate),
    Specific(DateTime<Utc>),
}

impl DateType {
    pub fn to_date_naive(&self) -> NaiveDate {
        match self {
            DateType::AllDay(d) => *d,
            DateType::Specific(dt) => dt.with_timezone(&Local).date_naive(),
        }
    }

    /// For sorting/comparison we often need either the end-of-day sentinel for all-day
    /// items, or the actual timestamp for specific datetimes.
    pub fn to_comparison_time(&self) -> DateTime<Utc> {
        match self {
            DateType::AllDay(d) => d
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap()
                .with_timezone(&chrono::Utc),
            DateType::Specific(dt) => *dt,
        }
    }

    /// When computing "starts in the future" semantics we want the start-of-day for
    /// all-day items and the exact dt for specific datetimes.
    pub fn to_start_comparison_time(&self) -> DateTime<Utc> {
        match self {
            DateType::AllDay(d) => d
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap()
                .with_timezone(&chrono::Utc),
            DateType::Specific(dt) => *dt,
        }
    }

    pub fn format_smart(&self) -> String {
        use chrono::Timelike;
        match self {
            DateType::AllDay(d) => d.format("%Y-%m-%d").to_string(),
            DateType::Specific(dt) => {
                let local = dt.with_timezone(&Local);
                // Only render time when it's not a pure-midnight all-day sentinel.
                if local.hour() == 0 && local.minute() == 0 && local.second() == 0 {
                    local.format("%Y-%m-%d").to_string()
                } else {
                    local.format("%Y-%m-%d %H:%M").to_string()
                }
            }
        }
    }
}

impl PartialOrd for DateType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DateType {
    fn cmp(&self, other: &Self) -> Ordering {
        let d1 = self.to_date_naive();
        let d2 = other.to_date_naive();
        match d1.cmp(&d2) {
            Ordering::Equal => match (self, other) {
                // If both are specific times, compare those times
                (DateType::Specific(t1), DateType::Specific(t2)) => t1.cmp(t2),
                // Prefer specific timestamps (they are "earlier" in ordering than an all-day)
                (DateType::Specific(_), DateType::AllDay(_)) => Ordering::Less,
                (DateType::AllDay(_), DateType::Specific(_)) => Ordering::Greater,
                (DateType::AllDay(_), DateType::AllDay(_)) => Ordering::Equal,
            },
            ord => ord,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AlarmTrigger {
    Relative(i32),           // relative minutes offset (negative for before)
    Absolute(DateTime<Utc>), // explicit timestamp
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Alarm {
    #[serde(default = "default_uid")]
    pub uid: String,
    pub action: String,
    pub trigger: AlarmTrigger,
    pub description: Option<String>,
    pub acknowledged: Option<DateTime<Utc>>,
    pub related_to_uid: Option<String>,
    pub relation_type: Option<String>,
}

impl Alarm {
    pub fn new_relative(minutes_before: u32) -> Self {
        Self {
            uid: default_uid(),
            action: "DISPLAY".to_string(),
            trigger: AlarmTrigger::Relative(-(minutes_before as i32)),
            description: None,
            acknowledged: None,
            related_to_uid: None,
            relation_type: None,
        }
    }

    pub fn new_absolute(dt: DateTime<Utc>) -> Self {
        Self {
            uid: default_uid(),
            action: "DISPLAY".to_string(),
            trigger: AlarmTrigger::Absolute(dt),
            description: None,
            acknowledged: None,
            related_to_uid: None,
            relation_type: None,
        }
    }

    pub fn is_snooze(&self) -> bool {
        self.relation_type.as_deref() == Some("SNOOZE")
    }
}

fn deserialize_date_option<'de, D>(deserializer: D) -> Result<Option<DateType>, D::Error>
where
    D: Deserializer<'de>,
{
    // Backwards-compatible deserializer: accept either the new DateType form or legacy RFC datetimes.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DateTypeOrLegacy {
        New(DateType),
        Legacy(DateTime<Utc>),
    }

    let v: Option<DateTypeOrLegacy> = Option::deserialize(deserializer)?;
    match v {
        Some(DateTypeOrLegacy::New(d)) => Ok(Some(d)),
        Some(DateTypeOrLegacy::Legacy(d)) => {
            // Convert legacy DateTime to AllDay when it's a pure midnight, otherwise Specific.
            let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
            if d.time() == midnight {
                Ok(Some(DateType::AllDay(d.date_naive())))
            } else {
                Ok(Some(DateType::Specific(d)))
            }
        }
        None => Ok(None),
    }
}

/// Virtual state used to represent placeholder/virtual rows in flattened lists.
/// These are not persisted and exist solely for UI presentation (expand/collapse).
#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub enum VirtualState {
    #[default]
    None,
    Expand(String),
    Collapse(String),
}

/// Primary in-memory Task model. Fields map closely to VTODO/ICS semantics.
/// Transient/display fields (is_blocked, sort_rank...) are skipped during serialization.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub status: TaskStatus,
    pub estimated_duration: Option<u32>,
    #[serde(default)]
    pub estimated_duration_max: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_date_option")]
    pub due: Option<DateType>,
    #[serde(default, deserialize_with = "deserialize_date_option")]
    pub dtstart: Option<DateType>,
    #[serde(default)]
    pub alarms: Vec<Alarm>,
    #[serde(default)]
    pub exdates: Vec<DateType>,
    pub priority: u8,
    pub percent_complete: Option<u8>,
    pub parent_uid: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub related_to: Vec<String>,
    pub etag: String,
    pub href: String,
    pub calendar_href: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub depth: usize,
    pub rrule: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub geo: Option<String>,

    // Time-tracking fields:
    // - `time_spent_seconds` accumulates committed seconds of work for this task.
    // - `last_started_at` is an optional unix timestamp when the timer was last started.
    // - `sessions` contains history of committed WorkSession entries.
    #[serde(default)]
    pub time_spent_seconds: u64,
    #[serde(default)]
    pub last_started_at: Option<i64>,
    #[serde(default)]
    pub sessions: Vec<WorkSession>,

    #[serde(default)]
    pub unmapped_properties: Vec<RawProperty>,
    #[serde(default)]
    pub sequence: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_alarms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_components: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_event: Option<bool>,

    // Transient UI/runtime fields (not serialized)
    #[serde(skip)]
    pub is_blocked: bool,
    #[serde(skip)]
    pub is_implicitly_blocked: bool,
    #[serde(skip)]
    pub sort_rank: u8,
    #[serde(skip)]
    pub effective_priority: u8,
    #[serde(skip)]
    pub effective_due: Option<DateType>,
    #[serde(skip)]
    pub effective_dtstart: Option<DateType>,
    #[serde(skip)]
    pub virtual_state: VirtualState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    pub rank: u8,
    pub prio: u8,
    pub due: Option<DateType>,
    pub start: Option<DateType>,
}

/// Comparison helper for sort policies. The ordering decision tree is centralized here
/// to make ranking behavior deterministic and easy to test.
pub fn compare_sortkeys(a: &SortKey, b: &SortKey, default_prio: u8) -> Ordering {
    if a.rank != b.rank {
        return a.rank.cmp(&b.rank);
    }
    let norm_prio = |p: u8| if p == 0 { default_prio } else { p };
    let compare_dates = |d1: &Option<DateType>, d2: &Option<DateType>| -> Ordering {
        match (d1, d2) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    };
    match a.rank {
        1 => norm_prio(a.prio)
            .cmp(&norm_prio(b.prio))
            .then_with(|| compare_dates(&a.due, &b.due)),
        2..=4 => compare_dates(&a.due, &b.due).then(norm_prio(a.prio).cmp(&norm_prio(b.prio))),
        5 | 6 => norm_prio(a.prio)
            .cmp(&norm_prio(b.prio))
            .then_with(|| compare_dates(&a.due, &b.due)),
        7 => {
            let s1 = a
                .start
                .as_ref()
                .map(|d: &DateType| d.to_start_comparison_time());
            let s2 = b
                .start
                .as_ref()
                .map(|d: &DateType| d.to_start_comparison_time());
            s1.cmp(&s2).then(norm_prio(a.prio).cmp(&norm_prio(b.prio)))
        }
        _ => norm_prio(a.prio)
            .cmp(&norm_prio(b.prio))
            .then_with(|| compare_dates(&a.due, &b.due)),
    }
}

// Helper context used by hierarchy organization routines.
// Bundles the children map, result vector and other parameters so recursive helpers
// have a concise signature.
struct HierarchyContext<'a> {
    children_map: &'a HashMap<String, Vec<Task>>,
    result: &'a mut Vec<Task>,
    visited_uids: &'a mut HashSet<String>,
    expanded_groups: &'a HashSet<String>,
    max_done_subtasks: usize,
}

impl Task {
    /// Return the explicit COMPLETED date parsed from unmapped properties, if present.
    pub fn completion_date(&self) -> Option<DateTime<Utc>> {
        self.unmapped_properties
            .iter()
            .find(|p| p.key == "COMPLETED")
            .and_then(|p| {
                let v = p.value.trim();
                // Try several common datetime variants for resilience against different ICS sources.
                if v.contains('T') {
                    NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%SZ")
                        .ok()
                        .map(|ndt| Utc.from_utc_datetime(&ndt))
                        .or_else(|| {
                            NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S")
                                .ok()
                                .map(|ndt| Utc.from_utc_datetime(&ndt))
                        })
                        .or_else(|| {
                            DateTime::parse_from_rfc3339(v)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        })
                } else {
                    // Date-only value (all-day): interpret as midnight (UTC conversion).
                    NaiveDate::parse_from_str(v, "%Y%m%d")
                        .ok()
                        .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                        .map(|ndt| Utc.from_utc_datetime(&ndt))
                }
            })
    }

    /// Set (or clear) the COMPLETED date property and ensure status aligns.
    pub fn set_completion_date(&mut self, dt: Option<DateTime<Utc>>) {
        // Remove existing COMPLETED prop
        self.unmapped_properties.retain(|p| p.key != "COMPLETED");

        if let Some(date) = dt {
            // If we're setting a completion date, ensure task is in a done state.
            if !self.status.is_done() {
                self.status = TaskStatus::Completed;
            }

            let val = date.format("%Y%m%dT%H%M%SZ").to_string();
            self.unmapped_properties.push(RawProperty {
                key: "COMPLETED".to_string(),
                value: val,
                params: vec![],
            });
        }
    }

    /// Construct a new task from smart-syntax input. This is a thin constructor that
    /// initializes fields and delegates parsing to the smart-input parser.
    pub fn new(
        input: &str,
        aliases: &HashMap<String, Vec<String>>,
        default_reminder_time: Option<NaiveTime>,
    ) -> Self {
        let mut task = Self {
            uid: Uuid::new_v4().to_string(),
            summary: String::new(),
            description: String::new(),
            status: TaskStatus::NeedsAction,
            estimated_duration: None,
            estimated_duration_max: None,
            due: None,
            dtstart: None,
            alarms: Vec::new(),
            exdates: Vec::new(),
            priority: 0,
            percent_complete: None,
            parent_uid: None,
            dependencies: Vec::new(),
            related_to: Vec::new(),
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: Vec::new(),
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            time_spent_seconds: 0,
            last_started_at: None,
            sessions: Vec::new(),
            unmapped_properties: Vec::new(),
            sequence: 0,
            raw_alarms: Vec::new(),
            raw_components: Vec::new(),
            create_event: None,
            is_blocked: false,
            is_implicitly_blocked: false,
            sort_rank: 0,
            effective_priority: 0,
            effective_due: None,
            effective_dtstart: None,
            virtual_state: VirtualState::None,
        };
        task.apply_smart_input(input, aliases, default_reminder_time);
        task
    }

    pub fn apply_smart_input(
        &mut self,
        input: &str,
        aliases: &HashMap<String, Vec<String>>,
        default_reminder_time: Option<NaiveTime>,
    ) {
        // Delegate to parser module to keep the model focused on state.
        super::parser::apply_smart_input(self, input, aliases, default_reminder_time);
    }

    /// Calculate a compact base rank used by the multi-stage sort algorithm.
    /// The numeric rank selects a priority class; lower is more urgent.
    /// The mapping balances urgency, start-time grace, blocking and completion.
    ///
    /// The `effectively_blocked` argument signals whether the task is blocked
    /// either explicitly (is_blocked) or implicitly (inherited from ancestors).
    pub fn calculate_base_rank(
        &self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        start_grace_period_days: u32,
        effectively_blocked: bool,
    ) -> u8 {
        // Trash items are bottom-most
        if self.calendar_href == "local://trash" {
            return 9;
        }

        // Completed items go below active ones
        if self.status.is_done() {
            return 8;
        }

        let now = Utc::now();

        // Tasks that start significantly in the future are deferred in ranking.
        if let Some(start) = &self.dtstart {
            let start_time = start.to_start_comparison_time();
            let grace_threshold = now + chrono::Duration::days(start_grace_period_days as i64);
            if start_time > grace_threshold && !self.has_recent_acknowledged_alarm() {
                return 7;
            }
        }

        // Effectively blocked tasks are deprioritized below normal tasks.
        if effectively_blocked {
            return 6;
        }

        // Urgency buckets based on explicit priority and near due date.
        if self.priority > 0 && self.priority <= urgent_prio {
            return 1;
        }
        if let Some(due) = &self.due
            && due.to_comparison_time() <= now + chrono::Duration::days(urgent_days as i64)
        {
            return 2;
        }
        if self.status == TaskStatus::InProcess {
            return 3;
        }

        if let Some(due) = &self.due {
            if let Some(limit) = cutoff {
                if due.to_comparison_time() <= limit {
                    return 4;
                }
            } else {
                return 4;
            }
        }

        // Default rank (low priority)
        5
    }

    /// Compare component values used by the propagation algorithm.
    /// This is a thin wrapper used to evaluate child/parent contributions when
    /// computing propagation-selected tasks for multi-column UIs (e.g. randomness).
    #[allow(clippy::too_many_arguments)]
    pub fn compare_components(
        rank_a: u8,
        prio_a: u8,
        due_a: &Option<DateType>,
        start_a: &Option<DateType>,
        rank_b: u8,
        prio_b: u8,
        due_b: &Option<DateType>,
        start_b: &Option<DateType>,
        default_prio: u8,
    ) -> Ordering {
        let a = SortKey {
            rank: rank_a,
            prio: prio_a,
            due: due_a.clone(),
            start: start_a.clone(),
        };
        let b = SortKey {
            rank: rank_b,
            prio: prio_b,
            due: due_b.clone(),
            start: start_b.clone(),
        };
        compare_sortkeys(&a, &b, default_prio)
    }

    /// Compare two tasks using their effective fields (used by top-level sorting).
    pub fn compare_for_sort(&self, other: &Self, default_priority: u8) -> Ordering {
        // Stable ordering for trash and completed groups uses completion date desc.
        if self.sort_rank == 9 && other.sort_rank == 9 {
            return other
                .completion_date()
                .cmp(&self.completion_date())
                .then_with(|| self.summary.cmp(&other.summary));
        }

        if self.sort_rank == 8 && other.sort_rank == 8 {
            return other
                .completion_date()
                .cmp(&self.completion_date())
                .then_with(|| self.summary.cmp(&other.summary));
        }

        let a = SortKey {
            rank: self.sort_rank,
            prio: self.effective_priority,
            due: self.effective_due.clone(),
            start: self.effective_dtstart.clone(),
        };
        let b = SortKey {
            rank: other.sort_rank,
            prio: other.effective_priority,
            due: other.effective_due.clone(),
            start: other.effective_dtstart.clone(),
        };
        compare_sortkeys(&a, &b, default_priority).then_with(|| self.summary.cmp(&other.summary))
    }

    /// Compare taking into account cutoff and other global settings.
    pub fn compare_with_cutoff(
        &self,
        other: &Self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
        start_grace_period_days: u32,
    ) -> Ordering {
        let eff_blocked_self = self.is_blocked || self.is_implicitly_blocked;
        let eff_blocked_other = other.is_blocked || other.is_implicitly_blocked;

        let rank_self = self.calculate_base_rank(
            cutoff,
            urgent_days,
            urgent_prio,
            start_grace_period_days,
            eff_blocked_self,
        );
        let rank_other = other.calculate_base_rank(
            cutoff,
            urgent_days,
            urgent_prio,
            start_grace_period_days,
            eff_blocked_other,
        );
        let a = SortKey {
            rank: rank_self,
            prio: self.priority,
            due: self.due.clone(),
            start: self.dtstart.clone(),
        };
        let b = SortKey {
            rank: rank_other,
            prio: other.priority,
            due: other.due.clone(),
            start: other.dtstart.clone(),
        };
        compare_sortkeys(&a, &b, default_priority).then_with(|| self.summary.cmp(&other.summary))
    }

    /// Build a flattened, display-ordered list that respects parent/child hierarchy
    /// and injects "virtual" expand/collapse rows when completed-subtask groups are truncated.
    /// This function keeps complexity manageable by:
    ///  - Sorting first using stable compare_for_sort
    ///  - Partitioning roots vs children and then performing a deterministic traversal
    pub fn organize_hierarchy(
        mut tasks: Vec<Task>,
        default_priority: u8,
        expanded_groups: &HashSet<String>,
        max_done_roots: usize,
        max_done_subtasks: usize,
    ) -> Vec<Task> {
        let present_uids: HashSet<String> = tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        // Sort by the canonical comparator before building hierarchy
        tasks.sort_by(|a, b| a.compare_for_sort(b, default_priority));

        for mut task in tasks {
            let is_orphan = match &task.parent_uid {
                Some(p_uid) => !present_uids.contains(p_uid),
                None => true,
            };

            if is_orphan {
                if task.parent_uid.is_some() {
                    task.depth = 0; // orphaned child promoted to root
                }
                roots.push(task);
            } else {
                let p_uid = task.parent_uid.as_ref().unwrap().clone();
                children_map.entry(p_uid).or_default().push(task);
            }
        }

        let mut result = Vec::new();
        let mut visited_uids = HashSet::new();

        fn process_group(
            raw_group: Vec<Task>,
            parent_uid: String,
            limit: usize,
            is_root: bool,
            context: &mut HierarchyContext,
            depth: usize,
        ) {
            // Partition into active and done to allow truncation of the done-group
            let (active, done): (Vec<Task>, Vec<Task>) =
                raw_group.into_iter().partition(|t| !t.status.is_done());

            // Append active tasks always
            for task in active {
                Task::append_task_and_children(&task, context, depth);
            }

            if done.is_empty() {
                return;
            }

            // Effective key used to index expansion state: "" for roots
            let effective_key = if is_root {
                "".to_string()
            } else {
                parent_uid.clone()
            };
            let is_expanded = context.expanded_groups.contains(&effective_key);

            if is_expanded {
                // Show all done tasks followed by a Collapse virtual row
                for task in done {
                    Task::append_task_and_children(&task, context, depth);
                }
                let mut collapse = Task::new("Collapse", &HashMap::new(), None);
                collapse.uid = format!("virtual-collapse-{}", effective_key);
                collapse.virtual_state = VirtualState::Collapse(effective_key);
                collapse.depth = depth;
                collapse.parent_uid = if is_root { None } else { Some(parent_uid) };
                context.result.push(collapse);
            } else if done.len() > limit {
                // Show a small sample and then an Expand row
                let count_to_show = limit.saturating_sub(1);
                let mut iter = done.into_iter();

                for _ in 0..count_to_show {
                    if let Some(task) = iter.next() {
                        Task::append_task_and_children(&task, context, depth);
                    }
                }

                let mut expand = Task::new("Expand", &HashMap::new(), None);
                expand.uid = format!("virtual-expand-{}", effective_key);
                expand.virtual_state = VirtualState::Expand(effective_key);
                expand.depth = depth;
                expand.parent_uid = if is_root { None } else { Some(parent_uid) };
                context.result.push(expand);
            } else {
                // Under the limit: show all done tasks
                for task in done {
                    Task::append_task_and_children(&task, context, depth);
                }
            }
        }

        let mut context = HierarchyContext {
            children_map: &children_map,
            result: &mut result,
            visited_uids: &mut visited_uids,
            expanded_groups,
            max_done_subtasks,
        };

        process_group(roots, "".to_string(), max_done_roots, true, &mut context, 0);

        result
    }

    /// Append `task` and recursively its children to `context.result`. This helper
    /// respects expansion state for child groups and injects virtual expand/collapse
    /// placeholders when needed.
    fn append_task_and_children(task: &Task, context: &mut HierarchyContext, depth: usize) {
        if context.visited_uids.contains(&task.uid) {
            return;
        }
        context.visited_uids.insert(task.uid.clone());

        let mut t = task.clone();
        t.depth = depth;
        context.result.push(t);

        if let Some(children) = context.children_map.get(&task.uid) {
            let (active, done): (Vec<Task>, Vec<Task>) =
                children.iter().cloned().partition(|t| !t.status.is_done());

            for child in active {
                Self::append_task_and_children(&child, context, depth + 1);
            }

            if !done.is_empty() {
                let is_expanded = context.expanded_groups.contains(&task.uid);
                if is_expanded {
                    for child in done {
                        Self::append_task_and_children(&child, context, depth + 1);
                    }
                    let mut collapse = Task::new("Collapse", &HashMap::new(), None);
                    collapse.uid = format!("virtual-collapse-{}", task.uid);
                    collapse.virtual_state = VirtualState::Collapse(task.uid.clone());
                    collapse.depth = depth + 1;
                    collapse.parent_uid = Some(task.uid.clone());
                    context.result.push(collapse);
                } else if done.len() > context.max_done_subtasks {
                    let show = context.max_done_subtasks.saturating_sub(1);
                    let mut iter = done.into_iter();
                    for _ in 0..show {
                        if let Some(c) = iter.next() {
                            Self::append_task_and_children(&c, context, depth + 1);
                        }
                    }
                    let mut expand = Task::new("Expand", &HashMap::new(), None);
                    expand.uid = format!("virtual-expand-{}", task.uid);
                    expand.virtual_state = VirtualState::Expand(task.uid.clone());
                    expand.depth = depth + 1;
                    expand.parent_uid = Some(task.uid.clone());
                    context.result.push(expand);
                } else {
                    for child in done {
                        Self::append_task_and_children(&child, context, depth + 1);
                    }
                }
            }
        }
    }

    /// Dismiss an alarm by uid. Supports both explicit and implicit alarm ids.
    /// implicit alarms use the form "implicit|<rfc3339>" so we can dismiss/snooze them
    /// without needing a stored Alarm object.
    pub fn handle_dismiss(&mut self, alarm_uid: &str) -> bool {
        if alarm_uid.starts_with("implicit_") {
            let parts: Vec<&str> = alarm_uid.split('|').collect();
            if parts.len() >= 2
                && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(parts[1])
            {
                let desc = if alarm_uid.contains("due") {
                    "Due now"
                } else {
                    "Starting"
                };
                self.dismiss_implicit_alarm(dt.with_timezone(&chrono::Utc), desc.to_string());
                return true;
            }
            return false;
        }
        self.dismiss_alarm(alarm_uid)
    }

    pub fn handle_snooze(&mut self, alarm_uid: &str, mins: u32) -> bool {
        if alarm_uid.starts_with("implicit_") {
            let parts: Vec<&str> = alarm_uid.split('|').collect();
            if parts.len() >= 2
                && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(parts[1])
            {
                let desc = if alarm_uid.contains("due") {
                    "Due now"
                } else {
                    "Starting"
                };
                self.snooze_implicit_alarm(dt.with_timezone(&chrono::Utc), desc.to_string(), mins);
                return true;
            }
            return false;
        }
        self.snooze_alarm(alarm_uid, mins)
    }

    pub fn dismiss_alarm(&mut self, alarm_uid: &str) -> bool {
        if let Some(alarm) = self.alarms.iter_mut().find(|a| a.uid == alarm_uid) {
            alarm.acknowledged = Some(Utc::now());
            return true;
        }
        false
    }

    pub fn snooze_alarm(&mut self, alarm_uid: &str, minutes: u32) -> bool {
        let now = Utc::now();
        let mut new_alarm_opt = None;

        if let Some(parent_alarm) = self.alarms.iter_mut().find(|a| a.uid == alarm_uid) {
            parent_alarm.acknowledged = Some(now);

            let trigger_time = now + chrono::Duration::minutes(minutes as i64);
            let mut snooze = Alarm::new_absolute(trigger_time);

            let root_uid = if parent_alarm.is_snooze() {
                parent_alarm
                    .related_to_uid
                    .clone()
                    .unwrap_or(parent_alarm.uid.clone())
            } else {
                parent_alarm.uid.clone()
            };

            snooze.related_to_uid = Some(root_uid);
            snooze.relation_type = Some("SNOOZE".to_string());
            snooze.description = Some(format!("Snoozed for {}m", minutes));
            snooze.action = parent_alarm.action.clone();

            new_alarm_opt = Some(snooze);
        }

        self.alarms.retain(|a| {
            if a.uid == alarm_uid && a.is_snooze() {
                return false;
            }
            true
        });

        if let Some(new_alarm) = new_alarm_opt {
            self.alarms.push(new_alarm);
            return true;
        }

        false
    }

    pub fn next_trigger_timestamp(&self) -> Option<i64> {
        let now = Utc::now();
        let mut earliest: Option<i64> = None;

        for alarm in &self.alarms {
            if alarm.acknowledged.is_some() {
                continue;
            }

            let trigger_dt = match alarm.trigger {
                AlarmTrigger::Absolute(dt) => dt,
                AlarmTrigger::Relative(mins) => {
                    // Relative triggers are anchored to due or dtstart (prefer due)
                    let anchor = if let Some(DateType::Specific(d)) = self.due {
                        d
                    } else if let Some(DateType::Specific(s)) = self.dtstart {
                        s
                    } else {
                        continue;
                    };
                    anchor + chrono::Duration::minutes(mins as i64)
                }
            };

            if trigger_dt > now || (now - trigger_dt).num_hours() < 24 {
                let ts = trigger_dt.timestamp();
                match earliest {
                    Some(e) if ts < e => earliest = Some(ts),
                    None => earliest = Some(ts),
                    _ => {}
                }
            }
        }
        earliest
    }

    pub fn has_alarm_at(&self, dt: DateTime<Utc>) -> bool {
        self.alarms.iter().any(|a| match a.trigger {
            AlarmTrigger::Absolute(t) => t == dt,
            _ => false,
        })
    }

    pub fn has_recent_acknowledged_alarm(&self) -> bool {
        self.alarms.iter().any(|alarm| alarm.acknowledged.is_some())
    }

    /// Add an implicit dismissed alarm record so UI can show it as acknowledged.
    pub fn dismiss_implicit_alarm(&mut self, trigger_dt: DateTime<Utc>, description: String) {
        if self.has_alarm_at(trigger_dt) {
            return;
        }

        let mut alarm = Alarm::new_absolute(trigger_dt);
        alarm.description = Some(description);
        alarm.acknowledged = Some(Utc::now());
        self.alarms.push(alarm);
    }

    /// Create a parent + snooze implicit chain for snoozed times.
    pub fn snooze_implicit_alarm(
        &mut self,
        trigger_dt: DateTime<Utc>,
        description: String,
        snooze_mins: u32,
    ) {
        let mut parent = Alarm::new_absolute(trigger_dt);
        parent.description = Some(description);
        parent.acknowledged = Some(Utc::now());

        let parent_uid = parent.uid.clone();
        self.alarms.push(parent);

        let now = Utc::now();
        let next_trigger = now + chrono::Duration::minutes(snooze_mins as i64);

        let mut snooze = Alarm::new_absolute(next_trigger);
        snooze.related_to_uid = Some(parent_uid);
        snooze.relation_type = Some("SNOOZE".to_string());
        snooze.description = Some(format!("Snoozed for {}m", snooze_mins));

        self.alarms.push(snooze);
    }

    /// Resolve visible tags and location given parent/alias expansions.
    /// Returns (visible_tags, visible_location).
    pub fn resolve_visual_attributes(
        &self,
        parent_tags: &HashSet<String>,
        parent_location: &Option<String>,
        aliases: &HashMap<String, Vec<String>>,
    ) -> (Vec<String>, Option<String>) {
        use crate::model::parser::strip_quotes;
        let mut hidden_tags = parent_tags.clone();
        let mut hidden_location = parent_location.clone();

        // Expand alias directives (e.g. if a category expands into hidden targets)
        let mut process_expansions = |targets: &Vec<String>| {
            for target in targets {
                if let Some(val) = target.strip_prefix('#') {
                    hidden_tags.insert(strip_quotes(val));
                } else if let Some(val) = target.strip_prefix("@@") {
                    hidden_location = Some(strip_quotes(val));
                } else if target.to_lowercase().starts_with("loc:") {
                    hidden_location = Some(strip_quotes(&target[4..]));
                }
            }
        };

        for cat in &self.categories {
            if let Some(targets) = aliases.get(cat) {
                process_expansions(targets);
            }
            let mut search = cat.as_str();
            while let Some(idx) = search.rfind(':') {
                search = &search[..idx];
                if let Some(targets) = aliases.get(search) {
                    process_expansions(targets);
                }
            }
        }

        if let Some(loc) = &self.location {
            let key = format!("@@{}", loc);
            if let Some(targets) = aliases.get(&key) {
                process_expansions(targets);
            }
            let mut search = key.as_str();
            while let Some(idx) = search.rfind(':') {
                if idx < 2 {
                    break;
                }
                search = &search[..idx];
                if let Some(targets) = aliases.get(search) {
                    process_expansions(targets);
                }
            }
        }

        let mut visible_tags = Vec::new();
        for cat in &self.categories {
            if !hidden_tags.contains(cat) {
                visible_tags.push(cat.clone());
            }
        }
        visible_tags.sort();

        let visible_location = if let Some(loc) = &self.location {
            if hidden_location.as_ref() != Some(loc) {
                Some(loc.clone())
            } else {
                None
            }
        } else {
            None
        };

        (visible_tags, visible_location)
    }

    /// Recycle a recurring task when it is completed/cancelled.
    ///
    /// Behavior:
    ///  - Commit any running timer to `time_spent_seconds` and close the session.
    ///  - If the task is already in the target done state and target is done, toggle back.
    ///  - If an RRULE exists and target is done:
    ///      * create a history snapshot (new UID) representing the completed instance,
    ///      * advance the master recurring task to the next occurrence and reset timing,
    ///      * return (history, Some(next_task)) on success.
    ///  - Otherwise produce an updated in-place task reflecting the new status.
    ///
    /// Returns (primary, optional_secondary) where `primary` is what should be inserted/seen
    /// immediately in the UI (history or updated task) and `secondary` is the next-instance.
    pub fn recycle(&self, target_status: TaskStatus) -> (Task, Option<Task>) {
        // 0. Commit time tracking: finalize any running timer before creating history.
        let mut base_task = self.clone();
        if let Some(start_ts) = base_task.last_started_at {
            let now = Utc::now().timestamp();
            if now > start_ts {
                base_task.time_spent_seconds = base_task
                    .time_spent_seconds
                    .saturating_add((now - start_ts) as u64);
            }
            base_task.last_started_at = None;
        }

        // If already in target done state, toggle to NeedsAction (undo).
        if base_task.status == target_status && target_status.is_done() {
            let mut updated = base_task.clone();
            updated.status = TaskStatus::NeedsAction;
            updated.percent_complete = None;
            updated.unmapped_properties.retain(|p| p.key != "COMPLETED");
            return (updated, None);
        }

        // Only perform full recycle/advance for recurring tasks when completing.
        if base_task.rrule.is_some() && target_status.is_done() {
            // 1. Create a history snapshot (represents the completed instance)
            let mut history = base_task.clone();
            history.uid = Uuid::new_v4().to_string();
            history.href = String::new();
            history.etag = String::new();
            history.status = target_status;
            history.rrule = None; // History is a non-recurring snapshot
            history.alarms.clear(); // History does not ring
            history.create_event = None; // Do not attempt to create a calendar event for history
            history.related_to.push(base_task.uid.clone()); // Link history back to master

            // Stamp COMPLETED (or CANCELLED) date for the history item.
            let now_str = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            history.unmapped_properties.retain(|p| p.key != "COMPLETED");
            history.unmapped_properties.push(RawProperty {
                key: "COMPLETED".to_string(),
                value: now_str,
                params: vec![],
            });

            if target_status == TaskStatus::Completed {
                history.percent_complete = Some(100);
            }

            // 2. Advance the main recurring item to the next occurrence and reset timing.
            let mut next_task = base_task.clone();

            // Reset time-tracking for next occurrence.
            next_task.time_spent_seconds = 0;
            next_task.last_started_at = None;

            if target_status == TaskStatus::Cancelled
                && let Some(current_date) = next_task
                    .dtstart
                    .as_ref()
                    .or(next_task.due.as_ref())
                    .cloned()
            {
                // If cancelling, add the current instance date to EXDATEs so the instance is skipped.
                next_task.exdates.push(current_date);
            }

            let advanced = crate::model::RecurrenceEngine::advance(&mut next_task);

            if advanced {
                return (history, Some(next_task));
            }
        }

        // Non-recurring or failed-to-advance: update in place.
        let mut updated = base_task.clone();
        updated.status = target_status;
        if target_status.is_done() {
            let now_str = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
            updated.unmapped_properties.retain(|p| p.key != "COMPLETED");
            updated.unmapped_properties.push(RawProperty {
                key: "COMPLETED".to_string(),
                value: now_str,
                params: vec![],
            });
            if target_status == TaskStatus::Completed {
                updated.percent_complete = Some(100);
            }
        } else {
            updated.percent_complete = None;
            updated.unmapped_properties.retain(|p| p.key != "COMPLETED");
        }

        (updated, None)
    }
}

// --- Backwards-compatible convenience wrappers delegating to adapters ---
// These small wrappers keep external call sites simple during the refactor.

impl Task {
    /// Parse a VCALENDAR/ICS string into a Task (delegates to IcsAdapter).
    pub fn from_ics(
        raw_ics: &str,
        etag: String,
        href: String,
        calendar_href: String,
    ) -> Result<Task, String> {
        crate::model::IcsAdapter::from_ics(raw_ics, etag, href, calendar_href)
    }

    /// Serialize this Task into VCALENDAR string.
    pub fn to_ics(&self) -> String {
        crate::model::IcsAdapter::to_ics(self)
    }

    /// Produce companion VEVENT ICS files for this Task if applicable.
    /// Returns a vector of (suffix, ics_body) tuples where each tuple represents
    /// a separate .ics file (e.g., `evt-<uid>-start.ics`, `evt-<uid>-due.ics`).
    pub fn to_event_ics(&self) -> Vec<(String, String)> {
        crate::model::IcsAdapter::to_event_ics(self)
    }

    /// Advance recurrence in-place (delegates to RecurrenceEngine).
    pub fn advance_recurrence(&mut self) -> bool {
        crate::model::RecurrenceEngine::advance(self)
    }

    // Display-related helpers delegated to TaskDisplay trait implementation.
    pub fn to_smart_string(&self) -> String {
        crate::model::TaskDisplay::to_smart_string(self)
    }

    pub fn format_duration_short(&self) -> String {
        crate::model::TaskDisplay::format_duration_short(self)
    }

    pub fn checkbox_symbol(&self) -> &'static str {
        crate::model::TaskDisplay::checkbox_symbol(self)
    }

    pub fn is_paused(&self) -> bool {
        crate::model::TaskDisplay::is_paused(self)
    }
}
