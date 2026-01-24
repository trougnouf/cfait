// Core data structures for Tasks, Alarms, and Calendars.
//
// ⚠️ VERSION BUMP REQUIRED:
// Changes to the following structs/enums require bumping version constants:
//
// - Task, Alarm, AlarmTrigger, DateType, TaskStatus, RawProperty:
//   → Bump CACHE_VERSION in src/cache.rs
//   → Bump LOCAL_STORAGE_VERSION in src/storage.rs
//
// - AlarmIndexEntry-related changes:
//   → Bump version field in AlarmIndex (src/alarm_index.rs)
//
// - CalendarListEntry:
//   → May require versioning in cache/registry (currently unversioned)
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
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

// --- DATE TYPES ---

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
            // Convert to Local for date comparison to align with user expectation (e.g. "today")
            DateType::Specific(dt) => dt.with_timezone(&Local).date_naive(),
        }
    }

    /// Returns the logical end of the event/deadline for comparison.
    /// AllDay -> End of day (23:59:59) in local timezone, converted to UTC.
    /// Specific -> Exact time (already in UTC).
    pub fn to_comparison_time(&self) -> DateTime<Utc> {
        match self {
            DateType::AllDay(d) => {
                // Interpret the naive date as local time (since it was parsed from local input)
                // then convert to UTC for comparison
                d.and_hms_opt(23, 59, 59)
                    .unwrap()
                    .and_local_timezone(chrono::Local)
                    .unwrap()
                    .with_timezone(&chrono::Utc)
            }
            DateType::Specific(dt) => *dt,
        }
    }

    /// Returns the logical start of the event for comparison (used for start dates).
    /// AllDay -> Start of day (00:00:00) in local timezone, converted to UTC.
    /// Specific -> Exact time (already in UTC).
    pub fn to_start_comparison_time(&self) -> DateTime<Utc> {
        match self {
            DateType::AllDay(d) => {
                // Interpret the naive date as local time at midnight
                // then convert to UTC for comparison
                d.and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_local_timezone(chrono::Local)
                    .unwrap()
                    .with_timezone(&chrono::Utc)
            }
            DateType::Specific(dt) => *dt,
        }
    }

    pub fn format_smart(&self) -> String {
        match self {
            DateType::AllDay(d) => d.format("%Y-%m-%d").to_string(),
            DateType::Specific(dt) => {
                // Convert UTC to Local before formatting for display/edit string
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
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
                // Same day: Specific time comes BEFORE All Day (urgency)
                (DateType::Specific(t1), DateType::Specific(t2)) => t1.cmp(t2),
                (DateType::Specific(_), DateType::AllDay(_)) => Ordering::Less,
                (DateType::AllDay(_), DateType::Specific(_)) => Ordering::Greater,
                (DateType::AllDay(_), DateType::AllDay(_)) => Ordering::Equal,
            },
            ord => ord,
        }
    }
}

// --- ALARMS (RFC 9074) ---

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AlarmTrigger {
    Relative(i32),
    Absolute(DateTime<Utc>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Alarm {
    #[serde(default = "default_uid")]
    pub uid: String, // RFC 9074 Section 4

    pub action: String, // DISPLAY, AUDIO
    pub trigger: AlarmTrigger,
    pub description: Option<String>,

    pub acknowledged: Option<DateTime<Utc>>, // RFC 9074 Section 6.1

    pub related_to_uid: Option<String>, // RFC 9074 Section 5
    pub relation_type: Option<String>,  // RFC 9074 Section 7.1 (SNOOZE)
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

/// Custom deserializer to handle migration from v3.12 (DateTime<Utc>) to v3.14 (DateType)
/// This ensures backward compatibility with old local.json files that have date fields
/// stored as simple DateTime<Utc> strings instead of the new DateType enum format.
fn deserialize_date_option<'de, D>(deserializer: D) -> Result<Option<DateType>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DateTypeOrLegacy {
        // Matches new format: { "type": "...", "value": "..." }
        New(DateType),
        // Matches old format: "2024-01-01T12:00:00Z" string
        Legacy(DateTime<Utc>),
    }

    let v: Option<DateTypeOrLegacy> = Option::deserialize(deserializer)?;
    match v {
        Some(DateTypeOrLegacy::New(d)) => Ok(Some(d)),
        // Convert legacy DateTime<Utc> to DateType
        Some(DateTypeOrLegacy::Legacy(d)) => {
            // HEURISTIC FIX:
            // If the legacy timestamp is exactly midnight UTC (00:00:00), it was likely
            // intended as an All-Day task in the old version (which didn't support DateType).
            // converting it to AllDay fixes the "1 AM" display issue in local time.
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

/// Task represents a single TODO/task item with full CalDAV support.
///
/// # Backward Compatibility Requirements
///
/// **CRITICAL**: This struct is serialized/deserialized for local storage.
/// When adding new fields, you MUST follow these rules to prevent data loss:
///
/// 1. **ALL new Vec<T> fields MUST have `#[serde(default)]`**
///    - Without it, old tasks without the field will fail to load
///    - This was the cause of the "missing field related_to" bug in v0.4.2
///
/// 2. **ALL new primitive fields (u8, usize, etc.) MUST have `#[serde(default)]`**
///    - Or use Option<T> if the field should truly be optional
///
/// 3. **New String fields should usually be Option<String>**
///    - Only make them required if you're implementing a migration
///
/// 4. **Test backward compatibility when adding fields**:
///    - See `test_backward_compatibility_missing_related_to_and_dependencies` in storage.rs
///    - Create a JSON without your new field and verify it loads
///
/// 5. **Document when fields were added** in comments (helps with future migrations)
///
/// ## Migration Strategy
///
/// We use versioned storage (see `LocalStorageData` in storage.rs). When breaking changes
/// are needed, increment `LOCAL_STORAGE_VERSION` and add a migration function.
/// However, `#[serde(default)]` avoids most breaking changes.
///
/// ## History
/// - v0.1.7: Added `dependencies` field (RFC 9253 DEPENDS-ON support)
/// - v0.3.14: Added `related_to` field (generic RELATED-TO relationships)
/// - v0.4.2: Fixed backward compatibility by adding #[serde/default] to above fields
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub status: TaskStatus,
    pub estimated_duration: Option<u32>,

    // NEW FIELD: Optional Max duration
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
    #[serde(default)]
    pub unmapped_properties: Vec<RawProperty>,

    #[serde(default)]
    pub sequence: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_alarms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_components: Vec<String>,

    /// Per-task override for event creation (None = use global config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_event: Option<bool>,

    /// Transient field used for sorting. Not saved to disk.
    /// Set to true if task is blocked (by dependencies or #blocked tag).
    #[serde(skip)]
    pub is_blocked: bool,

    /// Transient field for sorting rank (1=Urgent, 7=Done).
    /// Populated during filter/sort phase (not serialized).
    #[serde(skip)]
    pub sort_rank: u8,

    // --- NEW TRANSIENT FIELDS ---
    /// Effective priority propagated from most-urgent descendant (or own priority)
    #[serde(skip)]
    pub effective_priority: u8,

    /// Effective due date propagated from most-urgent descendant (or own due)
    #[serde(skip)]
    pub effective_due: Option<DateType>,

    /// Effective dtstart propagated from most-urgent descendant (or own dtstart)
    #[serde(skip)]
    pub effective_dtstart: Option<DateType>,
}

// --- Module-scope SortKey and comparator ---
//
// Move these out to module scope so they can be used by other functions and
// avoid embedding struct definitions inside `impl` blocks (not supported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    pub rank: u8,
    pub prio: u8,
    pub due: Option<DateType>,
    pub start: Option<DateType>,
}

/// Compare two sort keys using the same tie-breaker rules that were previously
/// embedded in `compare_components`. This centralizes the logic and keeps
/// the public compatibility wrapper simple.
pub fn compare_sortkeys(a: &SortKey, b: &SortKey, default_prio: u8) -> Ordering {
    if a.rank != b.rank {
        return a.rank.cmp(&b.rank);
    }

    let norm_prio = |p: u8| if p == 0 { default_prio } else { p };

    let compare_dates = |d1: &Option<DateType>, d2: &Option<DateType>| -> Ordering {
        match (d1, d2) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less, // Has date < No date (More urgent)
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    };

    match a.rank {
        1 => {
            // Urgent: Priority -> Due
            norm_prio(a.prio)
                .cmp(&norm_prio(b.prio))
                .then_with(|| compare_dates(&a.due, &b.due))
        }
        2..=4 => {
            // Due Soon / Started / Standard: Due -> Priority
            compare_dates(&a.due, &b.due).then(norm_prio(a.prio).cmp(&norm_prio(b.prio)))
        }
        5 => {
            // Remaining: Priority -> Due (Name is handled in final sort)
            norm_prio(a.prio)
                .cmp(&norm_prio(b.prio))
                .then_with(|| compare_dates(&a.due, &b.due))
        }
        6 => {
            // Future: Start -> Priority
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

impl Task {
    // COMPLETION DATE: parse the COMPLETED unmapped property (if present)
    pub fn completion_date(&self) -> Option<DateTime<Utc>> {
        self.unmapped_properties
            .iter()
            .find(|p| p.key == "COMPLETED")
            .and_then(|p| {
                // Handle different date-time formats for robustness
                if p.value.contains('T') {
                    NaiveDateTime::parse_from_str(&p.value, "%Y%m%dT%H%M%SZ")
                        .ok()
                        .map(|ndt| Utc.from_utc_datetime(&ndt))
                } else {
                    NaiveDate::parse_from_str(&p.value, "%Y%m%d")
                        .ok()
                        .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                        .map(|ndt| Utc.from_utc_datetime(&ndt))
                }
            })
    }

    // Changed signature to accept default_time
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
            unmapped_properties: Vec::new(),
            sequence: 0,
            raw_alarms: Vec::new(),
            raw_components: Vec::new(),
            create_event: None,
            is_blocked: false,
            sort_rank: 0,
            effective_priority: 0,
            effective_due: None,
            effective_dtstart: None,
        };
        task.apply_smart_input(input, aliases, default_reminder_time);
        task
    }

    pub fn format_duration_short(&self) -> String {
        fn fmt_min(m: u32) -> String {
            if m >= 525600 {
                format!("{}y", m / 525600)
            } else if m >= 43200 {
                format!("{}mo", m / 43200)
            } else if m >= 10080 {
                format!("{}w", m / 10080)
            } else if m >= 1440 {
                format!("{}d", m / 1440)
            } else if m >= 60 {
                format!("{}h", m / 60)
            } else {
                format!("{}m", m)
            }
        }

        if let Some(min) = self.estimated_duration {
            if let Some(max) = self.estimated_duration_max
                && max > min
            {
                return format!("[~{}-{}]", fmt_min(min), fmt_min(max));
            }
            return format!("[~{}]", fmt_min(min));
        }
        String::new()
    }

    pub fn is_paused(&self) -> bool {
        self.status == TaskStatus::NeedsAction
            && self.percent_complete.unwrap_or(0) > 0
            && self.percent_complete.unwrap_or(0) < 100
    }

    pub fn checkbox_symbol(&self) -> &'static str {
        if self.is_paused() {
            return "[‖]";
        }
        match self.status {
            TaskStatus::Completed => "[✔]",
            TaskStatus::Cancelled => "[✘]",
            TaskStatus::InProcess => "[▶]",
            TaskStatus::NeedsAction => "[ ]",
        }
    }

    /// Calculates the "intrinsic" sort rank of this task based on its own properties.
    /// 1: Urgent, 2: Due Soon, 3: Started, 4: Standard, 5: Defer/NoDate, 6: Future, 7: Done
    pub fn calculate_base_rank(
        &self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        start_grace_period_days: u32,
    ) -> u8 {
        if self.status.is_done() {
            return 7;
        }

        let now = Utc::now();

        // Future check
        if let Some(start) = &self.dtstart {
            let start_time = start.to_start_comparison_time();
            let grace_threshold = now + chrono::Duration::days(start_grace_period_days as i64);
            if start_time > grace_threshold && !self.has_recent_acknowledged_alarm() {
                return 6;
            }
        }

        if !self.is_blocked {
            // 1: Urgent (Priority)
            if self.priority > 0 && self.priority <= urgent_prio {
                return 1;
            }
            // 2: Due Soon
            if let Some(due) = &self.due
                && due.to_comparison_time() <= now + chrono::Duration::days(urgent_days as i64)
            {
                return 2;
            }
            // 3: Started
            if self.status == TaskStatus::InProcess {
                return 3;
            }
        }

        // 4: Inside Cutoff
        if let Some(due) = &self.due {
            if let Some(limit) = cutoff {
                if due.to_comparison_time() <= limit {
                    return 4;
                }
            } else {
                return 4;
            }
        }

        // 5: Remaining
        5
    }

    /// Backwards-compatible wrapper: preserve the original function signature so
    /// existing call sites continue to work. Internally this constructs `SortKey`
    /// instances and delegates to `compare_sortkeys`.
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

    pub fn compare_for_sort(&self, other: &Self, default_priority: u8) -> Ordering {
        // If both tasks are done (Completed/Cancelled), sort by completion date (newest first)
        if self.sort_rank == 7 && other.sort_rank == 7 {
            // Newest first: so compare `other` to `self`.
            return other
                .completion_date()
                .cmp(&self.completion_date())
                .then_with(|| self.summary.cmp(&other.summary));
        }

        // Build compact SortKey instances and use the central comparator.
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

    // --- Existing compare_with_cutoff preserved for backward compatibility/tests ---
    pub fn compare_with_cutoff(
        &self,
        other: &Self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
        start_grace_period_days: u32,
    ) -> Ordering {
        let now = Utc::now();

        // Helper: Treat Priority 0 (Unset) as default_priority for comparison
        let normalize_prio = |p: u8| if p == 0 { default_priority } else { p };

        // Helper: Determine the sorting Rank (1-7)
        // 1: Urgent (priority <= urgent_prio)
        // 2: Due Soon (due within urgent_days)
        // 3: Started (InProcess)
        // 4: Remaining (Inside Cutoff, sorted by due date)
        // 5: Remaining (Outside Cutoff/No Date, sorted by priority)
        // 6: Future Start
        // 7: Done/Cancelled
        let get_rank = |t: &Task| -> u8 {
            if t.status.is_done() {
                return 7;
            }

            // Check Future Start (Excludes from 1-5)
            // But NOT if:
            // 1. Start date is within the grace period
            // 2. Task has a recent acknowledged alarm (indicating user interaction)
            if let Some(start) = &t.dtstart {
                let start_time = start.to_start_comparison_time();
                let grace_threshold = now + chrono::Duration::days(start_grace_period_days as i64);

                // If start is beyond grace period AND no recent acknowledged alarms, rank as future
                if start_time > grace_threshold && !t.has_recent_acknowledged_alarm() {
                    return 6;
                }
            }

            // Blocked tasks skip ranks 1-3 (Urgent, Due Soon, Started)
            // They fall through to rank 4 or 5 based on their due date
            if !t.is_blocked {
                // 1: Urgent (Priority threshold)
                if t.priority > 0 && t.priority <= urgent_prio {
                    return 1;
                }

                // 2: Due Soon (Date threshold)
                if let Some(due) = &t.due
                    && due.to_comparison_time() <= now + chrono::Duration::days(urgent_days as i64)
                {
                    return 2;
                }

                // 3: Started
                if t.status == TaskStatus::InProcess {
                    return 3;
                }
            }

            // 4: Remaining (Inside Cutoff)
            if let Some(due) = &t.due {
                if let Some(limit) = cutoff {
                    if due.to_comparison_time() <= limit {
                        return 4;
                    }
                } else {
                    // If no cutoff, all dated tasks are essentially "inside cutoff"
                    return 4;
                }
            }

            // 5: Remaining (Outside Cutoff or No Date)
            5
        };

        let rank_self = get_rank(self);
        let rank_other = get_rank(other);

        if rank_self != rank_other {
            return rank_self.cmp(&rank_other);
        }

        // Helper: Sort dates, putting None last
        let compare_dates = |d1: &Option<DateType>, d2: &Option<DateType>| -> Ordering {
            match (d1, d2) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        };

        // Tie-breaking within ranks
        let order = match rank_self {
            1 => {
                // Urgent: Priority -> Due -> Name
                normalize_prio(self.priority)
                    .cmp(&normalize_prio(other.priority))
                    .then_with(|| compare_dates(&self.due, &other.due))
            }
            2..=4 => {
                // Due Soon / Started / Inside Cutoff: Due -> Priority -> Name
                compare_dates(&self.due, &other.due)
                    .then(normalize_prio(self.priority).cmp(&normalize_prio(other.priority)))
            }
            5 => {
                // Outside Cutoff: Priority -> Name -> Due
                normalize_prio(self.priority)
                    .cmp(&normalize_prio(other.priority))
                    .then_with(|| self.summary.cmp(&other.summary))
                    .then_with(|| compare_dates(&self.due, &other.due))
            }
            6 => {
                // Future: Start Date -> Priority -> Name
                let s1 = self.dtstart.as_ref().map(|d| d.to_start_comparison_time());
                let s2 = other.dtstart.as_ref().map(|d| d.to_start_comparison_time());
                s1.cmp(&s2)
                    .then(normalize_prio(self.priority).cmp(&normalize_prio(other.priority)))
            }
            7 => {
                // Done/Cancelled: Newest completion date first
                other.completion_date().cmp(&self.completion_date())
            }
            _ => Ordering::Equal,
        };

        order.then_with(|| self.summary.cmp(&other.summary))
    }

    /// Organize hierarchy expecting tasks to already have `sort_rank` populated.
    /// This function now only needs the `default_priority` for tie-breakers during sort.
    pub fn organize_hierarchy(mut tasks: Vec<Task>, default_priority: u8) -> Vec<Task> {
        let present_uids: std::collections::HashSet<String> =
            tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        // Sort using precomputed `sort_rank` and tie-breakers
        tasks.sort_by(|a, b| a.compare_for_sort(b, default_priority));

        // Consume tasks directly instead of cloning the entire vector
        let total_tasks = tasks.len();
        for mut task in tasks {
            let is_orphan = match &task.parent_uid {
                Some(p_uid) => !present_uids.contains(p_uid),
                None => true,
            };

            if is_orphan {
                if task.parent_uid.is_some() {
                    task.depth = 0;
                }
                roots.push(task);
            } else {
                let p_uid = task.parent_uid.as_ref().unwrap().clone();
                children_map.entry(p_uid).or_default().push(task);
            }
        }

        let mut result = Vec::new();
        let mut visited_uids = std::collections::HashSet::new();

        for root in roots {
            Self::append_task_and_children(&root, &mut result, &children_map, 0, &mut visited_uids);
        }

        // Check for unvisited tasks (cycle detection)
        if result.len() < total_tasks {
            // Collect any remaining tasks from children_map that weren't visited
            for tasks_vec in children_map.into_values() {
                for mut task in tasks_vec {
                    if !visited_uids.contains(&task.uid) {
                        task.depth = 0;
                        result.push(task);
                    }
                }
            }
        }

        result
    }

    fn append_task_and_children(
        task: &Task,
        result: &mut Vec<Task>,
        map: &HashMap<String, Vec<Task>>,
        depth: usize,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if visited.contains(&task.uid) {
            return;
        }
        visited.insert(task.uid.clone());

        let mut t = task.clone();
        t.depth = depth;
        result.push(t);
        if let Some(children) = map.get(&task.uid) {
            for child in children {
                Self::append_task_and_children(child, result, map, depth + 1, visited);
            }
        }
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

            // Resolve root UID if this is already a snooze
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

        // Clean up snoozed snoozes
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

            // Ignore stale alarms older than 24h
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

    /// Checks if a specific absolute time is already covered by an existing alarm (active or acknowledged).
    /// Used by the system actor to decide if an auto-reminder is needed.
    pub fn has_alarm_at(&self, dt: DateTime<Utc>) -> bool {
        self.alarms.iter().any(|a| match a.trigger {
            AlarmTrigger::Absolute(t) => t == dt,
            // We don't compare relative here because auto-reminders are calculated as absolute times
            // by the system actor before checking.
            _ => false,
        })
    }

    /// Check if task has any acknowledged alarms.
    /// This helps identify tasks that the user has interacted with via reminders,
    /// preventing them from being pushed to the "future tasks" section.
    ///
    /// For recurring tasks, old alarms from past recurrences are automatically cleared
    /// when the task advances (see `respawn()` in adapter.rs), so any acknowledged alarm
    /// present is guaranteed to be for the current task instance.
    pub fn has_recent_acknowledged_alarm(&self) -> bool {
        self.alarms.iter().any(|alarm| alarm.acknowledged.is_some())
    }

    /// Converts a synthetic implicit alarm into a real, acknowledged VALARM.
    pub fn dismiss_implicit_alarm(&mut self, trigger_dt: DateTime<Utc>, description: String) {
        // Double check we don't already have it
        if self.has_alarm_at(trigger_dt) {
            return;
        }

        let mut alarm = Alarm::new_absolute(trigger_dt);
        alarm.description = Some(description);
        alarm.acknowledged = Some(Utc::now());
        self.alarms.push(alarm);
    }

    /// Converts a synthetic implicit alarm into a real snooze chain.
    /// 1. Creates the "Original" alarm (acknowledged).
    /// 2. Creates the "Snooze" alarm (active).
    pub fn snooze_implicit_alarm(
        &mut self,
        trigger_dt: DateTime<Utc>,
        description: String,
        snooze_mins: u32,
    ) {
        // 1. Create the "ghost" original alarm so the snooze has a parent
        let mut parent = Alarm::new_absolute(trigger_dt);
        parent.description = Some(description);
        parent.acknowledged = Some(Utc::now());

        let parent_uid = parent.uid.clone();
        self.alarms.push(parent);

        // 2. Create the snooze
        let now = Utc::now();
        let next_trigger = now + chrono::Duration::minutes(snooze_mins as i64);

        let mut snooze = Alarm::new_absolute(next_trigger);
        snooze.related_to_uid = Some(parent_uid);
        snooze.relation_type = Some("SNOOZE".to_string());
        snooze.description = Some(format!("Snoozed for {}m", snooze_mins));

        self.alarms.push(snooze);
    }
}

// --- MAIN IMPLEMENTATION ---

impl Task {
    pub fn to_smart_string(&self) -> String {
        let mut s = super::parser::escape_summary(&self.summary);
        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }
        if let Some(loc) = &self.location {
            s.push_str(&format!(" @@{}", super::parser::quote_value(loc)));
        }
        if let Some(u) = &self.url {
            s.push_str(&format!(" url:{}", super::parser::quote_value(u)));
        }
        if let Some(g) = &self.geo {
            s.push_str(&format!(" geo:{}", super::parser::quote_value(g)));
        }
        if let Some(start) = &self.dtstart {
            s.push_str(&format!(" ^{}", start.format_smart()));
        }
        if let Some(d) = &self.due {
            s.push_str(&format!(" @{}", d.format_smart()));
        }

        if let Some(min) = self.estimated_duration {
            let fmt_val = |m: u32| -> String {
                if m.is_multiple_of(525600) {
                    format!("{}y", m / 525600)
                } else if m.is_multiple_of(43200) {
                    format!("{}mo", m / 43200)
                } else if m.is_multiple_of(10080) {
                    format!("{}w", m / 10080)
                } else if m.is_multiple_of(1440) {
                    format!("{}d", m / 1440)
                } else if m.is_multiple_of(60) {
                    format!("{}h", m / 60)
                } else {
                    format!("{}m", m)
                }
            };

            if let Some(max) = self.estimated_duration_max {
                if max > min {
                    s.push_str(&format!(" ~{}-{}", fmt_val(min), fmt_val(max)));
                } else {
                    s.push_str(&format!(" ~{}", fmt_val(min)));
                }
            } else {
                s.push_str(&format!(" ~{}", fmt_val(min)));
            }
        }

        if let Some(r) = &self.rrule {
            let pretty = super::parser::prettify_recurrence(r);
            s.push_str(&format!(" {}", pretty));
        }

        // Add exdates to smart string
        for ex in &self.exdates {
            s.push_str(&format!(" except {}", ex.format_smart()));
        }

        // Re-construct smart reminders?
        for alarm in &self.alarms {
            if alarm.is_snooze() || alarm.acknowledged.is_some() {
                continue;
            } // Skip technical alarms
            match alarm.trigger {
                AlarmTrigger::Relative(offset) => {
                    let mins = -offset;
                    if mins > 0 {
                        if mins % 10080 == 0 {
                            s.push_str(&format!(" rem:{}w", mins / 10080));
                        } else if mins % 1440 == 0 {
                            s.push_str(&format!(" rem:{}d", mins / 1440));
                        } else if mins % 60 == 0 {
                            s.push_str(&format!(" rem:{}h", mins / 60));
                        } else {
                            s.push_str(&format!(" rem:{}m", mins));
                        }
                    } else {
                        s.push_str(&format!(" rem:{}m", mins));
                    }
                }
                AlarmTrigger::Absolute(dt) => {
                    let local = dt.with_timezone(&Local);
                    let now = Local::now();

                    // Smart reconstruction: use keywords when possible
                    if local.date_naive() == now.date_naive() {
                        // Today: just show time
                        s.push_str(&format!(" rem:{}", local.format("%H:%M")));
                    } else if local.date_naive() == now.date_naive() + Duration::days(1) {
                        // Tomorrow: use keyword
                        s.push_str(&format!(" rem:tomorrow {}", local.format("%H:%M")));
                    } else {
                        // Other dates: use full date
                        s.push_str(&format!(" rem:{}", local.format("%Y-%m-%d %H:%M")));
                    }
                }
            }
        }

        for cat in &self.categories {
            s.push_str(&format!(" #{}", super::parser::quote_value(cat)));
        }

        // Add event creation override if explicitly set
        if let Some(create_event) = self.create_event {
            s.push_str(if create_event { " +cal" } else { " -cal" });
        }

        s
    }

    pub fn apply_smart_input(
        &mut self,
        input: &str,
        aliases: &HashMap<String, Vec<String>>,
        default_reminder_time: Option<NaiveTime>,
    ) {
        super::parser::apply_smart_input(self, input, aliases, default_reminder_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetype_backward_compatibility_legacy_format() {
        // Test that old v3.12 format (DateTime<Utc> string) can be deserialized
        let legacy_json = r#"{
            "uid": "test-uid",
            "summary": "Test Task",
            "description": "",
            "status": "NeedsAction",
            "estimated_duration": null,
            "due": "2024-01-15T14:30:00Z",
            "dtstart": "2024-01-10T09:00:00Z",
            "alarms": [],
            "priority": 0,
            "percent_complete": null,
            "parent_uid": null,
            "dependencies": [],
            "related_to": [],
            "etag": "",
            "href": "",
            "calendar_href": "local://default",
            "categories": [],
            "depth": 0,
            "rrule": null,
            "location": null,
            "url": null,
            "geo": null,
            "unmapped_properties": [],
            "sequence": 0
        }"#;

        let task: Task =
            serde_json::from_str(legacy_json).expect("Failed to deserialize legacy format");

        // Verify that the legacy DateTime strings were converted to DateType::Specific
        assert!(matches!(task.due, Some(DateType::Specific(_))));
        assert!(matches!(task.dtstart, Some(DateType::Specific(_))));

        // Verify the actual datetime values
        if let Some(DateType::Specific(dt)) = task.due {
            assert_eq!(dt.to_rfc3339(), "2024-01-15T14:30:00+00:00");
        }
        if let Some(DateType::Specific(dt)) = task.dtstart {
            assert_eq!(dt.to_rfc3339(), "2024-01-10T09:00:00+00:00");
        }
    }

    // ... (the rest of the tests remain unchanged) ...
}
