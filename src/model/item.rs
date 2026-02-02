// File: ./src/model/item.rs
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
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
                // FIX 1: Only format time if it's not midnight (or near midnight for safety)
                if local.hour() == 0 && local.minute() == 0 && local.second() == 0 {
                    local.format("%Y-%m-%d").to_string()
                } else {
                    // FIX 2 (CRUCIAL): Format UTC time component using a fixed pattern to prevent local conversion corruption
                    // The smart input parser *must* receive the UTC time so it re-parses correctly relative to UTC.
                    // This is a trade-off: The local time is shown, but the parser reads the correct, uncorrupted UTC-equivalent time string.
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
                (DateType::Specific(t1), DateType::Specific(t2)) => t1.cmp(t2),
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
    Relative(i32),
    Absolute(DateTime<Utc>),
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
    #[serde(skip)]
    pub is_blocked: bool,
    #[serde(skip)]
    pub sort_rank: u8,
    #[serde(skip)]
    pub effective_priority: u8,
    #[serde(skip)]
    pub effective_due: Option<DateType>,
    #[serde(skip)]
    pub effective_dtstart: Option<DateType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    pub rank: u8,
    pub prio: u8,
    pub due: Option<DateType>,
    pub start: Option<DateType>,
}

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
        5 => norm_prio(a.prio)
            .cmp(&norm_prio(b.prio))
            .then_with(|| compare_dates(&a.due, &b.due)),
        6 => {
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
    pub fn completion_date(&self) -> Option<DateTime<Utc>> {
        self.unmapped_properties
            .iter()
            .find(|p| p.key == "COMPLETED")
            .and_then(|p| {
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

    pub fn apply_smart_input(
        &mut self,
        input: &str,
        aliases: &HashMap<String, Vec<String>>,
        default_reminder_time: Option<NaiveTime>,
    ) {
        super::parser::apply_smart_input(self, input, aliases, default_reminder_time);
    }

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

        if let Some(start) = &self.dtstart {
            let start_time = start.to_start_comparison_time();
            let grace_threshold = now + chrono::Duration::days(start_grace_period_days as i64);
            if start_time > grace_threshold && !self.has_recent_acknowledged_alarm() {
                return 6;
            }
        }

        if !self.is_blocked {
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

        5
    }

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
        if self.sort_rank == 7 && other.sort_rank == 7 {
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

    pub fn compare_with_cutoff(
        &self,
        other: &Self,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
        default_priority: u8,
        start_grace_period_days: u32,
    ) -> Ordering {
        let rank_self =
            self.calculate_base_rank(cutoff, urgent_days, urgent_prio, start_grace_period_days);
        let rank_other =
            other.calculate_base_rank(cutoff, urgent_days, urgent_prio, start_grace_period_days);
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

    pub fn organize_hierarchy(mut tasks: Vec<Task>, default_priority: u8) -> Vec<Task> {
        let present_uids: std::collections::HashSet<String> =
            tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        tasks.sort_by(|a, b| a.compare_for_sort(b, default_priority));

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

        if result.len() < total_tasks {
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

    pub fn dismiss_implicit_alarm(&mut self, trigger_dt: DateTime<Utc>, description: String) {
        if self.has_alarm_at(trigger_dt) {
            return;
        }

        let mut alarm = Alarm::new_absolute(trigger_dt);
        alarm.description = Some(description);
        alarm.acknowledged = Some(Utc::now());
        self.alarms.push(alarm);
    }

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

    // --- REFACTORED: Use shared model logic ---
    pub fn resolve_visual_attributes(
        &self,
        parent_tags: &std::collections::HashSet<String>,
        parent_location: &Option<String>,
        aliases: &HashMap<String, Vec<String>>,
    ) -> (Vec<String>, Option<String>) {
        use crate::model::parser::strip_quotes;
        let mut hidden_tags = parent_tags.clone();
        let mut hidden_location = parent_location.clone();

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

        // Calculate Visible Tags (All - Hidden)
        let mut visible_tags = Vec::new();
        for cat in &self.categories {
            if !hidden_tags.contains(cat) {
                visible_tags.push(cat.clone());
            }
        }
        visible_tags.sort(); // Ensure stable order for UI

        // Calculate Visible Location
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
}

// Backward-compatible wrappers so existing call-sites/tests continue to work.
// These delegate to the new modules/traits (`IcsAdapter`, `RecurrenceEngine`, `TaskDisplay`)
// introduced during the refactor. Keeping these thin wrappers preserves the old public
// API surface for tests and external consumers.
impl Task {
    /// Parse a VCALENDAR/ICS string into a Task (compat wrapper).
    pub fn from_ics(
        raw_ics: &str,
        etag: String,
        href: String,
        calendar_href: String,
    ) -> Result<Task, String> {
        crate::model::IcsAdapter::from_ics(raw_ics, etag, href, calendar_href)
    }

    /// Serialize this Task into a full VCALENDAR string (compat wrapper).
    pub fn to_ics(&self) -> String {
        crate::model::IcsAdapter::to_ics(self)
    }

    /// Produce a companion event ICS for this Task if applicable (compat wrapper).
    pub fn to_event_ics(&self) -> Option<(String, String)> {
        crate::model::IcsAdapter::to_event_ics(self)
    }

    /// Advance this recurring task to the next occurrence in-place (compat wrapper).
    pub fn advance_recurrence(&mut self) -> bool {
        crate::model::RecurrenceEngine::advance(self)
    }

    // --- Display-related helpers delegated to TaskDisplay trait implementation ---

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

// New centralized recycle method for tasks.
// This moves the Snapshot + Recycle behavior into the model so all clients share the same semantics.
impl Task {
    /// Applies a terminal status (Completed or Cancelled).
    ///
    /// If recurring:
    /// 1. Creates a snapshot of the current state with the target status (History).
    /// 2. Advances the current task to the next date and resets status to NeedsAction (Recycled).
    ///
    /// Returns: (History_Snapshot_or_Updated_Task, Optional_Recycled_Task)
    /// If not recurring, returns (Updated_Task, None).
    pub fn recycle(&self, target_status: TaskStatus) -> (Task, Option<Task>) {
        // If the task is already in the target state, "toggle" it off to NeedsAction
        if self.status == target_status && target_status.is_done() {
            let mut updated = self.clone();
            updated.status = TaskStatus::NeedsAction;
            updated.percent_complete = None;
            updated.unmapped_properties.retain(|p| p.key != "COMPLETED");
            return (updated, None);
        }

        // Only recycle if it has an RRULE and we are finishing it (Done or Cancelled)
        if self.rrule.is_some() && target_status.is_done() {
            // 1. Create History Snapshot
            let mut history = self.clone();
            history.uid = Uuid::new_v4().to_string(); // New distinct UID
            history.href = String::new(); // Clear href (it's a new resource)
            history.etag = String::new();
            history.status = target_status;
            history.rrule = None; // History does not recur
            history.alarms.clear(); // History does not ring
            history.create_event = None; // Don't sync history to calendar
            history.related_to.push(self.uid.clone()); // Link history to parent

            // Set COMPLETED date (or CANCELLED)
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

            // 2. Advance Main Task
            let mut next_task = self.clone();

            // FIX: Restore Cancellation Logic.
            // If we are cancelling, we must add the current date to exdates on the recurring task
            // so the recurrence engine knows to skip this instance if it hasn't passed yet,
            // and to keep a record of the exception.
            if target_status == TaskStatus::Cancelled
                && let Some(current_date) = next_task
                    .dtstart
                    .as_ref()
                    .or(next_task.due.as_ref())
                    .cloned()
            {
                next_task.exdates.push(current_date);
            }

            // This advances dates and resets status to NeedsAction
            let advanced = crate::model::RecurrenceEngine::advance(&mut next_task);

            if advanced {
                return (history, Some(next_task));
            }
            // If advance failed (e.g. passed UNTIL date), fall through to non-recurring behavior
        }

        // Non-recurring (or finished recurring) behavior: Just update in place
        let mut updated = self.clone();
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
            // Un-completing
            updated.percent_complete = None;
            updated.unmapped_properties.retain(|p| p.key != "COMPLETED");
        }

        (updated, None)
    }
}
