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
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime, Utc};
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
                // FIX: Convert UTC to Local before formatting for display/edit string
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
        // Convert legacy DateTime<Utc> to DateType::Specific for backward compatibility
        Some(DateTypeOrLegacy::Legacy(d)) => Ok(Some(DateType::Specific(d))),
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
/// - v0.4.2: Fixed backward compatibility by adding #[serde(default)] to above fields
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub status: TaskStatus,
    pub estimated_duration: Option<u32>,

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
}

impl Task {
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
        };
        task.apply_smart_input(input, aliases, default_reminder_time);
        task
    }

    pub fn format_duration_short(&self) -> String {
        if let Some(mins) = self.estimated_duration {
            if mins >= 525600 {
                format!("[~{}y]", mins / 525600)
            } else if mins >= 43200 {
                format!("[~{}mo]", mins / 43200)
            } else if mins >= 10080 {
                format!("[~{}w]", mins / 10080)
            } else if mins >= 1440 {
                format!("[~{}d]", mins / 1440)
            } else if mins >= 60 {
                format!("[~{}h]", mins / 60)
            } else {
                format!("[~{}m]", mins)
            }
        } else {
            String::new()
        }
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

    pub fn compare_with_cutoff(
        &self,
        other: &Self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
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
            if let Some(start) = &t.dtstart
                && start.to_start_comparison_time() > now
            {
                return 6;
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
        match rank_self {
            1 => {
                // Urgent: Priority -> Due -> Name
                normalize_prio(self.priority)
                    .cmp(&normalize_prio(other.priority))
                    .then_with(|| compare_dates(&self.due, &other.due))
            }
            2 => {
                // Due Soon: Due -> Priority -> Name
                compare_dates(&self.due, &other.due)
                    .then(normalize_prio(self.priority).cmp(&normalize_prio(other.priority)))
            }
            3 => {
                // Started: Due -> Priority -> Name
                compare_dates(&self.due, &other.due)
                    .then(normalize_prio(self.priority).cmp(&normalize_prio(other.priority)))
            }
            4 => {
                // Inside Cutoff: Due -> Priority -> Name
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
                // Compare start dates for future tasks
                let s1 = self.dtstart.as_ref().map(|d| d.to_start_comparison_time());
                let s2 = other.dtstart.as_ref().map(|d| d.to_start_comparison_time());
                s1.cmp(&s2)
                    .then(normalize_prio(self.priority).cmp(&normalize_prio(other.priority)))
            }
            _ => {
                // Done/Other: Priority -> Due -> Name
                normalize_prio(self.priority)
                    .cmp(&normalize_prio(other.priority))
                    .then_with(|| compare_dates(&self.due, &other.due))
            }
        }
        .then_with(|| self.summary.cmp(&other.summary))
    }

    pub fn organize_hierarchy(
        mut tasks: Vec<Task>,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
    ) -> Vec<Task> {
        let present_uids: std::collections::HashSet<String> =
            tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        tasks.sort_by(|a, b| a.compare_with_cutoff(b, cutoff, urgent_days, urgent_prio, default_priority));

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

        if let Some(mins) = self.estimated_duration {
            if mins > 0 && mins % 525600 == 0 {
                s.push_str(&format!(" ~{}y", mins / 525600));
            } else if mins > 0 && mins % 43200 == 0 {
                s.push_str(&format!(" ~{}mo", mins / 43200));
            } else if mins > 0 && mins % 10080 == 0 {
                s.push_str(&format!(" ~{}w", mins / 10080));
            } else if mins > 0 && mins % 1440 == 0 {
                s.push_str(&format!(" ~{}d", mins / 1440));
            } else if mins > 0 && mins % 60 == 0 {
                s.push_str(&format!(" ~{}h", mins / 60));
            } else {
                s.push_str(&format!(" ~{}m", mins));
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

    #[test]
    fn test_datetype_backward_compatibility_new_format() {
        // Test that new v3.14 format (DateType enum) still works
        let new_json = r#"{
            "uid": "test-uid",
            "summary": "Test Task",
            "description": "",
            "status": "NeedsAction",
            "estimated_duration": null,
            "due": {"type": "Specific", "value": "2024-01-15T14:30:00Z"},
            "dtstart": {"type": "AllDay", "value": "2024-01-10"},
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

        let task: Task = serde_json::from_str(new_json).expect("Failed to deserialize new format");

        // Verify the DateType enum variants
        assert!(matches!(task.due, Some(DateType::Specific(_))));
        assert!(matches!(task.dtstart, Some(DateType::AllDay(_))));

        if let Some(DateType::Specific(dt)) = task.due {
            assert_eq!(dt.to_rfc3339(), "2024-01-15T14:30:00+00:00");
        }
        if let Some(DateType::AllDay(date)) = task.dtstart {
            assert_eq!(date.to_string(), "2024-01-10");
        }
    }

    #[test]
    fn test_datetype_backward_compatibility_null_values() {
        // Test that null values still work
        let null_json = r#"{
            "uid": "test-uid",
            "summary": "Test Task",
            "description": "",
            "status": "NeedsAction",
            "estimated_duration": null,
            "due": null,
            "dtstart": null,
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
            serde_json::from_str(null_json).expect("Failed to deserialize null values");

        assert!(task.due.is_none());
        assert!(task.dtstart.is_none());
    }

    #[test]
    fn test_datetype_backward_compatibility_mixed_formats() {
        // Test mixing old and new formats in a task list (as would happen during migration)
        let mixed_json = r#"[
            {
                "uid": "old-task",
                "summary": "Old Task",
                "description": "",
                "status": "NeedsAction",
                "estimated_duration": null,
                "due": "2024-01-15T14:30:00Z",
                "dtstart": null,
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
            },
            {
                "uid": "new-task",
                "summary": "New Task",
                "description": "",
                "status": "NeedsAction",
                "estimated_duration": null,
                "due": {"type": "AllDay", "value": "2024-01-20"},
                "dtstart": {"type": "Specific", "value": "2024-01-18T10:00:00Z"},
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
            }
        ]"#;

        let tasks: Vec<Task> =
            serde_json::from_str(mixed_json).expect("Failed to deserialize mixed formats");

        assert_eq!(tasks.len(), 2);

        // Old task should have legacy format converted to Specific
        assert!(matches!(tasks[0].due, Some(DateType::Specific(_))));
        assert!(tasks[0].dtstart.is_none());

        // New task should have DateType variants preserved
        assert!(matches!(tasks[1].due, Some(DateType::AllDay(_))));
        assert!(matches!(tasks[1].dtstart, Some(DateType::Specific(_))));
    }

    #[test]
    fn test_real_world_upgrade_scenario() {
        // Simulates a real upgrade: local.json written by v3.12, read by v3.14
        // User has tasks with DateTime<Utc> strings, we migrate them to DateType on load
        let v312_local_json = r#"{
            "tasks": [
                {
                    "uid": "task-1",
                    "summary": "Finish report",
                    "description": "Important deadline",
                    "status": "NeedsAction",
                    "estimated_duration": 120,
                    "due": "2024-02-01T17:00:00Z",
                    "dtstart": null,
                    "alarms": [],
                    "priority": 1,
                    "percent_complete": null,
                    "parent_uid": null,
                    "dependencies": [],
                    "related_to": [],
                    "etag": "abc123",
                    "href": "local://task-1",
                    "calendar_href": "local://default",
                    "categories": ["work"],
                    "depth": 0,
                    "rrule": null,
                    "location": null,
                    "url": null,
                    "geo": null,
                    "unmapped_properties": [],
                    "sequence": 1
                },
                {
                    "uid": "task-2",
                    "summary": "Dentist appointment",
                    "description": "",
                    "status": "NeedsAction",
                    "estimated_duration": null,
                    "due": "2024-02-05T10:30:00Z",
                    "dtstart": "2024-02-05T09:00:00Z",
                    "alarms": [],
                    "priority": 0,
                    "percent_complete": null,
                    "parent_uid": null,
                    "dependencies": [],
                    "related_to": [],
                    "etag": "def456",
                    "href": "local://task-2",
                    "calendar_href": "local://default",
                    "categories": ["health"],
                    "depth": 0,
                    "rrule": null,
                    "location": "Main Street Dental",
                    "url": null,
                    "geo": null,
                    "unmapped_properties": [],
                    "sequence": 0
                }
            ]
        }"#;

        #[derive(Deserialize)]
        struct LocalData {
            tasks: Vec<Task>,
        }

        let data: LocalData =
            serde_json::from_str(v312_local_json).expect("Failed to deserialize v3.12 local.json");

        assert_eq!(data.tasks.len(), 2);

        // Task 1: due should be migrated to DateType::Specific
        let task1 = &data.tasks[0];
        assert_eq!(task1.summary, "Finish report");
        assert!(matches!(task1.due, Some(DateType::Specific(_))));
        if let Some(DateType::Specific(dt)) = task1.due {
            assert_eq!(dt.to_rfc3339(), "2024-02-01T17:00:00+00:00");
        }
        assert!(task1.dtstart.is_none());

        // Task 2: both due and dtstart should be migrated
        let task2 = &data.tasks[1];
        assert_eq!(task2.summary, "Dentist appointment");
        assert!(matches!(task2.due, Some(DateType::Specific(_))));
        assert!(matches!(task2.dtstart, Some(DateType::Specific(_))));
        if let Some(DateType::Specific(dt)) = task2.due {
            assert_eq!(dt.to_rfc3339(), "2024-02-05T10:30:00+00:00");
        }
        if let Some(DateType::Specific(dt)) = task2.dtstart {
            assert_eq!(dt.to_rfc3339(), "2024-02-05T09:00:00+00:00");
        }

        // Now test serialization - should write new format
        let serialized =
            serde_json::to_string_pretty(&data.tasks[0]).expect("Failed to serialize task");

        // The serialized format should be the new DateType format (with proper spacing)
        assert!(serialized.contains(r#""type": "Specific"#));
        assert!(serialized.contains(r#""value": "2024-02-01T17:00:00Z"#));

        // Test round-trip: serialize and deserialize again
        let task1_roundtrip: Task =
            serde_json::from_str(&serialized).expect("Failed to deserialize after round-trip");
        assert_eq!(task1_roundtrip.summary, "Finish report");
        assert!(matches!(task1_roundtrip.due, Some(DateType::Specific(_))));
    }

    // Test the new rank-based sorting
    #[test]
    fn test_sorting_rank1_urgent_priority() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let urgent_task = Task {
            uid: "urgent".to_string(),
            summary: "Urgent task".to_string(),
            priority: 1,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(5))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let normal_task = Task {
            uid: "normal".to_string(),
            summary: "Normal task".to_string(),
            priority: 3,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(2))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Urgent (rank 1) should come before normal (rank 4)
        assert_eq!(
            urgent_task.compare_with_cutoff(&normal_task, None, urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank2_due_soon() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let due_soon = Task {
            uid: "due_soon".to_string(),
            summary: "Due soon".to_string(),
            priority: 3,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::hours(12))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let due_later = Task {
            uid: "due_later".to_string(),
            summary: "Due later".to_string(),
            priority: 3,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(5))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Due soon (rank 2) should come before due later (rank 4)
        assert_eq!(
            due_soon.compare_with_cutoff(&due_later, None, urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank3_started() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let started = Task {
            uid: "started".to_string(),
            summary: "Started task".to_string(),
            priority: 5,
            status: TaskStatus::InProcess,
            due: Some(DateType::Specific(now + chrono::Duration::days(5))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let not_started = Task {
            uid: "not_started".to_string(),
            summary: "Not started".to_string(),
            priority: 3,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(3))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Started (rank 3) should come before normal (rank 4)
        assert_eq!(
            started.compare_with_cutoff(&not_started, None, urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank3_started_within_rank() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let started_due_soon = Task {
            uid: "started_soon".to_string(),
            summary: "Started due soon".to_string(),
            priority: 5,
            status: TaskStatus::InProcess,
            due: Some(DateType::Specific(now + chrono::Duration::days(3))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let started_due_later = Task {
            uid: "started_later".to_string(),
            summary: "Started due later".to_string(),
            priority: 3, // Higher priority, but should still be sorted by date
            status: TaskStatus::InProcess,
            due: Some(DateType::Specific(now + chrono::Duration::days(10))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Within rank 3, should sort by due date first (earlier before later)
        assert_eq!(
            started_due_soon.compare_with_cutoff(
                &started_due_later,
                None,
                urgent_days,
                urgent_prio,
                5
            ),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank4_sorted_by_due_date() {
        let now = Utc::now();
        let cutoff = now + chrono::Duration::days(30);
        let urgent_days = 1;
        let urgent_prio = 1;

        let due_earlier = Task {
            uid: "earlier".to_string(),
            summary: "Due earlier".to_string(),
            priority: 5,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(10))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let due_later = Task {
            uid: "later".to_string(),
            summary: "Due later".to_string(),
            priority: 3, // Higher priority, but should still be sorted by date in rank 4
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(20))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Both in rank 4, should be sorted by due date first
        assert_eq!(
            due_earlier.compare_with_cutoff(&due_later, Some(cutoff), urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank5_sorted_by_priority_then_name() {
        let now = Utc::now();
        let cutoff = now + chrono::Duration::days(30);
        let urgent_days = 1;
        let urgent_prio = 1;

        let high_prio = Task {
            uid: "high".to_string(),
            summary: "Z task".to_string(),
            priority: 3,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(60))), // Outside cutoff
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let low_prio = Task {
            uid: "low".to_string(),
            summary: "A task".to_string(),
            priority: 5,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(50))), // Outside cutoff
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Both in rank 5 (outside cutoff), should be sorted by priority first
        assert_eq!(
            high_prio.compare_with_cutoff(&low_prio, Some(cutoff), urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank6_future_start() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let future_start = Task {
            uid: "future".to_string(),
            summary: "Future task".to_string(),
            priority: 1, // Even urgent priority doesn't matter if start is in future
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(10))),
            dtstart: Some(DateType::Specific(now + chrono::Duration::days(5))),
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let normal_task = Task {
            uid: "normal".to_string(),
            summary: "Normal task".to_string(),
            priority: 5,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(15))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Future start (rank 6) should come after normal (rank 4)
        // Normal task (rank 4) should come before future start (rank 6)
        assert_eq!(
            normal_task.compare_with_cutoff(&future_start, None, urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }

    #[test]
    fn test_sorting_rank7_done_tasks() {
        let now = Utc::now();
        let urgent_days = 1;
        let urgent_prio = 1;

        let done_task = Task {
            uid: "done".to_string(),
            summary: "Done task".to_string(),
            priority: 1,
            status: TaskStatus::Completed,
            due: Some(DateType::Specific(now + chrono::Duration::days(1))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        let normal_task = Task {
            uid: "normal".to_string(),
            summary: "Normal task".to_string(),
            priority: 5,
            status: TaskStatus::NeedsAction,
            due: Some(DateType::Specific(now + chrono::Duration::days(30))),
            dtstart: None,
            alarms: vec![],
            exdates: vec![],
            description: String::new(),
            estimated_duration: None,
            percent_complete: None,
            parent_uid: None,
            dependencies: vec![],
            related_to: vec![],
            etag: String::new(),
            href: String::new(),
            calendar_href: String::new(),
            categories: vec![],
            depth: 0,
            rrule: None,
            location: None,
            url: None,
            geo: None,
            unmapped_properties: vec![],
            sequence: 0,
            raw_alarms: vec![],
            raw_components: vec![],
            create_event: None,
            is_blocked: false,
        };

        // Done (rank 7) should come after all others
        // Normal task (rank 4/5) should come before done task (rank 7)
        assert_eq!(
            normal_task.compare_with_cutoff(&done_task, None, urgent_days, urgent_prio, 5),
            Ordering::Less
        );
    }
}
