// File: src/model/item.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

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
pub struct Task {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub status: TaskStatus,
    pub estimated_duration: Option<u32>,
    pub due: Option<DateTime<Utc>>,
    pub dtstart: Option<DateTime<Utc>>,
    pub priority: u8,
    pub percent_complete: Option<u8>,
    pub parent_uid: Option<String>,
    pub dependencies: Vec<String>,
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
}

impl Task {
    pub fn new(input: &str, aliases: &HashMap<String, Vec<String>>) -> Self {
        let mut task = Self {
            uid: Uuid::new_v4().to_string(),
            summary: String::new(),
            description: String::new(),
            status: TaskStatus::NeedsAction,
            estimated_duration: None,
            due: None,
            dtstart: None,
            priority: 0,
            percent_complete: None,
            parent_uid: None,
            dependencies: Vec::new(),
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
        };
        // The apply_smart_input implementation lives in the parser module.
        task.apply_smart_input(input, aliases);
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

    pub fn compare_with_cutoff(&self, other: &Self, cutoff: Option<DateTime<Utc>>) -> Ordering {
        let s1_active = self.status == TaskStatus::InProcess;
        let s2_active = other.status == TaskStatus::InProcess;
        let s1_done = self.status.is_done();
        let s2_done = other.status.is_done();
        let now = Utc::now();
        let s1_future = self.dtstart.map(|d| d > now).unwrap_or(false);
        let s2_future = other.dtstart.map(|d| d > now).unwrap_or(false);

        let is_in_window = |t: &Task| -> bool {
            match (t.due, cutoff) {
                (Some(d), Some(limit)) => d <= limit,
                (Some(_), None) => true,
                (None, _) => false,
            }
        };
        let s1_in = is_in_window(self);
        let s2_in = is_in_window(other);

        let p1 = if self.priority == 0 { 5 } else { self.priority };
        let p2 = if other.priority == 0 { 5 } else { other.priority };

        s2_active
            .cmp(&s1_active)
            .then(s1_done.cmp(&s2_done))
            .then(s1_future.cmp(&s2_future))
            .then(s2_in.cmp(&s1_in))
            .then(p1.cmp(&p2))
            .then_with(|| match (self.due, other.due) {
                (Some(d1), Some(d2)) => d1.cmp(&d2),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            })
            .then(other.is_paused().cmp(&self.is_paused()))
            .then(self.summary.cmp(&other.summary))
    }

    pub fn organize_hierarchy(mut tasks: Vec<Task>, cutoff: Option<DateTime<Utc>>) -> Vec<Task> {
        let present_uids: HashSet<String> = tasks.iter().map(|t| t.uid.clone()).collect();
        let mut children_map: HashMap<String, Vec<Task>> = HashMap::new();
        let mut roots: Vec<Task> = Vec::new();

        tasks.sort_by(|a, b| a.compare_with_cutoff(b, cutoff));

        for mut task in tasks.clone() {
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
        let mut visited_uids = HashSet::new();

        for root in roots {
            Self::append_task_and_children(&root, &mut result, &children_map, 0, &mut visited_uids);
        }

        if result.len() < tasks.len() {
            for mut task in tasks {
                if !visited_uids.contains(&task.uid) {
                    task.depth = 0;
                    result.push(task);
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
        visited: &mut HashSet<String>,
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
}
