// File: ./src/model/item.rs
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
    /// AllDay -> End of day (23:59:59). Specific -> Exact time.
    pub fn to_comparison_time(&self) -> DateTime<Utc> {
        match self {
            DateType::AllDay(d) => d.and_hms_opt(23, 59, 59).unwrap().and_utc(),
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

    pub priority: u8,
    pub percent_complete: Option<u8>,
    pub parent_uid: Option<String>,
    pub dependencies: Vec<String>,
    pub related_to: Vec<String>,
    pub etag: String,
    pub href: String,
    pub calendar_href: String,
    pub categories: Vec<String>,
    pub depth: usize,
    pub rrule: Option<String>,

    pub location: Option<String>,
    pub url: Option<String>,
    pub geo: Option<String>,
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
    ) -> Ordering {
        let now = Utc::now();

        let is_urgent = |t: &Task| -> bool {
            if t.status.is_done() {
                return false;
            }
            let is_high_prio = t.priority > 0 && t.priority <= urgent_prio;
            let is_due_soon = if let Some(due) = &t.due {
                due.to_comparison_time() <= now + chrono::Duration::days(urgent_days as i64)
            } else {
                false
            };
            is_high_prio || is_due_soon
        };

        let s1_urgent = is_urgent(self);
        let s2_urgent = is_urgent(other);

        let s1_active = self.status == TaskStatus::InProcess;
        let s2_active = other.status == TaskStatus::InProcess;
        let s1_done = self.status.is_done();
        let s2_done = other.status.is_done();

        let s1_future = self
            .dtstart
            .as_ref()
            .map(|d| d.to_comparison_time() > now)
            .unwrap_or(false);
        let s2_future = other
            .dtstart
            .as_ref()
            .map(|d| d.to_comparison_time() > now)
            .unwrap_or(false);

        let is_in_window = |t: &Task| -> bool {
            match (&t.due, cutoff) {
                (Some(d), Some(limit)) => d.to_comparison_time() <= limit,
                (Some(_), None) => true,
                (None, _) => false,
            }
        };
        let s1_in = is_in_window(self);
        let s2_in = is_in_window(other);

        let p1 = if self.priority == 0 { 5 } else { self.priority };
        let p2 = if other.priority == 0 {
            5
        } else {
            other.priority
        };

        s2_urgent
            .cmp(&s1_urgent)
            .then(s2_active.cmp(&s1_active))
            .then(s1_done.cmp(&s2_done))
            .then(s1_future.cmp(&s2_future))
            .then(s2_in.cmp(&s1_in))
            .then(p1.cmp(&p2))
            .then_with(|| match (&self.due, &other.due) {
                (Some(d1), Some(d2)) => d1.cmp(d2),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then(other.is_paused().cmp(&self.is_paused()))
            .then(self.summary.cmp(&other.summary))
    }

    pub fn organize_hierarchy(
        mut tasks: Vec<Task>,
        cutoff: Option<DateTime<Utc>>,
        urgent_days: u32,
        urgent_prio: u8,
    ) -> Vec<Task> {
        let present_uids: std::collections::HashSet<String> =
            tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        tasks.sort_by(|a, b| a.compare_with_cutoff(b, cutoff, urgent_days, urgent_prio));

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

        // New task should preserve its DateType variants
        assert!(matches!(tasks[1].due, Some(DateType::AllDay(_))));
        assert!(matches!(tasks[1].dtstart, Some(DateType::Specific(_))));
    }

    #[test]
    fn test_real_world_upgrade_scenario() {
        // Simulate a real upgrade from v3.12 to v3.14
        // This test writes a v3.12 format file, then reads it back using v3.14 deserializer
        use std::fs;
        use std::io::Write;

        let temp_dir = std::env::temp_dir().join("cfait_test_upgrade_scenario");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("local_v312.json");

        // Simulate v3.12 local.json file with multiple tasks
        let v312_content = r#"[
  {
    "uid": "legacy-task-1",
    "summary": "Buy groceries",
    "description": "Milk, eggs, bread",
    "status": "NeedsAction",
    "estimated_duration": 30,
    "due": "2024-02-15T18:00:00Z",
    "dtstart": null,
    "alarms": [],
    "priority": 5,
    "percent_complete": null,
    "parent_uid": null,
    "dependencies": [],
    "related_to": [],
    "etag": "abc123",
    "href": "task-1.ics",
    "calendar_href": "local://default",
    "categories": ["personal", "shopping"],
    "depth": 0,
    "rrule": null,
    "location": null,
    "url": null,
    "geo": null,
    "unmapped_properties": [],
    "sequence": 0
  },
  {
    "uid": "legacy-task-2",
    "summary": "Dentist appointment",
    "description": "",
    "status": "NeedsAction",
    "estimated_duration": null,
    "due": "2024-03-10T14:30:00Z",
    "dtstart": "2024-03-10T14:30:00Z",
    "alarms": [],
    "priority": 0,
    "percent_complete": null,
    "parent_uid": null,
    "dependencies": [],
    "related_to": [],
    "etag": "def456",
    "href": "task-2.ics",
    "calendar_href": "local://default",
    "categories": [],
    "depth": 0,
    "rrule": null,
    "location": "Dr. Smith's Office",
    "url": null,
    "geo": null,
    "unmapped_properties": [],
    "sequence": 0
  },
  {
    "uid": "legacy-task-3",
    "summary": "Completed task",
    "description": "This was already done",
    "status": "Completed",
    "estimated_duration": null,
    "due": null,
    "dtstart": null,
    "alarms": [],
    "priority": 0,
    "percent_complete": 100,
    "parent_uid": null,
    "dependencies": [],
    "related_to": [],
    "etag": "ghi789",
    "href": "task-3.ics",
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

        // Write v3.12 format file
        let mut file = fs::File::create(&file_path).expect("Failed to create test file");
        file.write_all(v312_content.as_bytes())
            .expect("Failed to write test file");
        drop(file);

        // Now read it back using v3.14 deserializer (this simulates the upgrade)
        let json = fs::read_to_string(&file_path).expect("Failed to read test file");
        let tasks: Vec<Task> = serde_json::from_str(&json)
            .expect("Failed to deserialize v3.12 format - upgrade simulation failed!");

        // Verify all tasks were loaded successfully
        assert_eq!(tasks.len(), 3, "All three tasks should be loaded");

        // Verify first task (with due date)
        assert_eq!(tasks[0].summary, "Buy groceries");
        assert!(matches!(tasks[0].due, Some(DateType::Specific(_))));
        assert!(tasks[0].dtstart.is_none());
        assert_eq!(tasks[0].estimated_duration, Some(30));
        assert_eq!(tasks[0].categories, vec!["personal", "shopping"]);

        // Verify second task (with both due and dtstart)
        assert_eq!(tasks[1].summary, "Dentist appointment");
        assert!(matches!(tasks[1].due, Some(DateType::Specific(_))));
        assert!(matches!(tasks[1].dtstart, Some(DateType::Specific(_))));
        assert_eq!(tasks[1].location, Some("Dr. Smith's Office".to_string()));

        // Verify third task (completed, no dates)
        assert_eq!(tasks[2].summary, "Completed task");
        assert_eq!(tasks[2].status, TaskStatus::Completed);
        assert!(tasks[2].due.is_none());
        assert!(tasks[2].dtstart.is_none());
        assert_eq!(tasks[2].percent_complete, Some(100));

        // Now save it back in v3.14 format (simulate normal operation after upgrade)
        let v314_json = serde_json::to_string_pretty(&tasks).expect("Failed to serialize");
        let v314_path = temp_dir.join("local_v314.json");
        fs::write(&v314_path, v314_json).expect("Failed to write v3.14 format");

        // Verify the new format can still be read
        let reloaded_json = fs::read_to_string(&v314_path).expect("Failed to read v3.14 file");
        let reloaded_tasks: Vec<Task> =
            serde_json::from_str(&reloaded_json).expect("Failed to deserialize v3.14 format");

        assert_eq!(
            reloaded_tasks.len(),
            3,
            "All tasks should survive the round-trip"
        );
        assert_eq!(reloaded_tasks[0].summary, "Buy groceries");
        assert_eq!(reloaded_tasks[1].summary, "Dentist appointment");
        assert_eq!(reloaded_tasks[2].summary, "Completed task");

        // Verify that the new format uses the DateType enum structure
        assert!(
            reloaded_json.contains(r#""type":"#),
            "v3.14 format should contain DateType enum structure"
        );

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }
}
