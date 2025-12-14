// File: src/model/adapter.rs
use crate::model::item::{RawProperty, Task, TaskStatus};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use icalendar::{Calendar, CalendarComponent, Component, Todo, TodoStatus};
use rrule::RRuleSet;
use std::str::FromStr;
use uuid::Uuid;

const HANDLED_KEYS: &[&str] = &[
    "UID",
    "SUMMARY",
    "DESCRIPTION",
    "STATUS",
    "PRIORITY",
    "DUE",
    "DTSTART",
    "RRULE",
    "DURATION",
    "X-ESTIMATED-DURATION",
    "CATEGORIES",
    "RELATED-TO",
    "DTSTAMP",
    "CREATED",
    "LAST-MODIFIED",
    "SEQUENCE",
    "PRODID",
    "VERSION",
    "CALSCALE",
];

impl Task {
    pub fn respawn(&self) -> Option<Task> {
        let rule_str = self.rrule.as_ref()?;
        let seed_date = self.dtstart.or(self.due)?;

        let dtstart_str = seed_date.format("%Y%m%dT%H%M%SZ").to_string();
        let rrule_string = format!("DTSTART:{}\nRRULE:{}", dtstart_str, rule_str);

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            let now = Utc::now();
            // Find first occurrence AFTER now
            let next_occurrence = rrule_set
                .into_iter()
                .find(|d| d.to_utc() > now)
                .map(|d| d.to_utc());

            if let Some(next_start) = next_occurrence {
                let mut next_task = self.clone();
                next_task.uid = Uuid::new_v4().to_string();
                next_task.href = String::new();
                next_task.etag = String::new();
                next_task.status = TaskStatus::NeedsAction;
                next_task.dependencies.clear();
                next_task.sequence = 0; // Reset sequence for new task

                let duration = if let Some(old_due) = self.due {
                    old_due - seed_date
                } else {
                    chrono::Duration::zero()
                };

                if self.dtstart.is_some() {
                    next_task.dtstart = Some(next_start);
                }

                if self.due.is_some() {
                    next_task.due = Some(next_start + duration);
                }

                return Some(next_task);
            }
        }
        None
    }

    pub fn to_ics(&self) -> String {
        let mut todo = Todo::new();
        todo.uid(&self.uid);
        todo.summary(&self.summary);
        if !self.description.is_empty() {
            todo.description(&self.description);
        }
        todo.timestamp(Utc::now());

        // Sequence
        todo.add_property("SEQUENCE", self.sequence.to_string());

        match self.status {
            TaskStatus::NeedsAction => todo.status(TodoStatus::NeedsAction),
            TaskStatus::InProcess => todo.status(TodoStatus::InProcess),
            TaskStatus::Completed => todo.status(TodoStatus::Completed),
            TaskStatus::Cancelled => todo.status(TodoStatus::Cancelled),
        };

        fn format_iso_duration(mins: u32) -> String {
            if mins.is_multiple_of(24 * 60) {
                format!("P{}D", mins / (24 * 60))
            } else if mins.is_multiple_of(60) {
                format!("PT{}H", mins / 60)
            } else {
                format!("PT{}M", mins)
            }
        }

        if let Some(dt) = self.dtstart {
            let formatted = dt.format("%Y%m%dT%H%M%SZ").to_string();
            todo.add_property("DTSTART", &formatted);
        }

        if let Some(dt) = self.due {
            let formatted = dt.format("%Y%m%dT%H%M%SZ").to_string();
            todo.add_property("DUE", &formatted);
            if let Some(mins) = self.estimated_duration {
                let val = format_iso_duration(mins);
                todo.add_property("X-ESTIMATED-DURATION", &val);
            }
        } else if let Some(mins) = self.estimated_duration {
            let val = format_iso_duration(mins);
            todo.add_property("DURATION", &val);
        }
        if self.priority > 0 {
            todo.priority(self.priority.into());
        }
        if let Some(rrule) = &self.rrule {
            todo.add_property("RRULE", rrule.as_str());
        }

        // --- HIERARCHY & DEPENDENCIES ---
        if let Some(p_uid) = &self.parent_uid {
            let prop = icalendar::Property::new("RELATED-TO", p_uid.as_str());
            todo.append_multi_property(prop);
        }

        for dep_uid in &self.dependencies {
            let mut prop = icalendar::Property::new("RELATED-TO", dep_uid);
            prop.add_parameter("RELTYPE", "DEPENDS-ON");
            todo.append_multi_property(prop);
        }

        // --- WRITE BACK UNMAPPED PROPERTIES ---
        for raw in &self.unmapped_properties {
            let mut prop = icalendar::Property::new(&raw.key, &raw.value);
            for (k, v) in &raw.params {
                prop.add_parameter(k, v);
            }
            todo.append_multi_property(prop);
        }

        let mut calendar = Calendar::new();
        calendar.push(todo);
        let mut ics = calendar.to_string();

        // 1. Manual injection of CATEGORIES
        if !self.categories.is_empty() {
            let escaped_cats: Vec<String> = self
                .categories
                .iter()
                .map(|c| c.replace(',', "\\,"))
                .collect();
            let cat_line = format!("CATEGORIES:{}", escaped_cats.join(","));

            if let Some(idx) = ics.rfind("END:VTODO") {
                let (start, end) = ics.split_at(idx);
                let mut buffer = String::with_capacity(ics.len() + cat_line.len() + 2);
                buffer.push_str(start);
                buffer.push_str(&cat_line);
                buffer.push_str("\r\n");
                buffer.push_str(end);
                ics = buffer;
            }
        }

        // 2. Inject ALARMS
        if !self.raw_alarms.is_empty()
            && let Some(idx) = ics.rfind("END:VTODO") {
                let (start, end) = ics.split_at(idx);
                let mut buffer = String::with_capacity(ics.len() + 500);
                buffer.push_str(start);
                for alarm in &self.raw_alarms {
                    buffer.push_str(alarm);
                    if !alarm.ends_with('\n') {
                        buffer.push('\n');
                    }
                }
                buffer.push_str(end);
                ics = buffer;
            }

        // 3. Inject Raw Components (Exceptions, Timezones, etc.)
        if !self.raw_components.is_empty() {
            let trimmed = ics.trim_end();
            if let Some(idx) = trimmed.rfind("END:VCALENDAR") {
                let (start, end) = trimmed.split_at(idx);

                let extra_len: usize = self.raw_components.iter().map(|s| s.len() + 2).sum();
                let mut buffer = String::with_capacity(trimmed.len() + extra_len);

                buffer.push_str(start);
                for raw in &self.raw_components {
                    buffer.push_str(raw);
                    if !raw.ends_with("\r\n") && !raw.ends_with('\n') {
                        buffer.push_str("\r\n");
                    }
                }
                buffer.push_str(end);
                ics = buffer;
            }
        }

        ics
    }

    pub fn from_ics(
        raw_ics: &str,
        etag: String,
        href: String,
        calendar_href: String,
    ) -> Result<Self, String> {
        let calendar: Calendar = raw_ics.parse().map_err(|e| format!("Parse: {}", e))?;

        let mut master_todo: Option<&Todo> = None;
        let mut raw_components: Vec<String> = Vec::with_capacity(calendar.components.len());

        for component in &calendar.components {
            match component {
                CalendarComponent::Todo(t) => {
                    let is_exception = t.properties().contains_key("RECURRENCE-ID");

                    if is_exception {
                        raw_components.push(t.to_string());
                    } else if master_todo.is_none() {
                        master_todo = Some(t);
                    } else {
                        raw_components.push(t.to_string());
                    }
                }
                CalendarComponent::Event(e) => raw_components.push(e.to_string()),
                CalendarComponent::Venue(v) => raw_components.push(v.to_string()),
                _ => {}
            }
        }

        let todo = match master_todo {
            Some(t) => t,
            None => return Err("No Master VTODO found in ICS".to_string()),
        };

        let summary = todo.get_summary().unwrap_or("No Title").to_string();
        let description = todo.get_description().unwrap_or("").to_string();
        let uid = todo.get_uid().unwrap_or_default().to_string();

        let status = if let Some(prop) = todo.properties().get("STATUS") {
            match prop.value().trim().to_uppercase().as_str() {
                "COMPLETED" => TaskStatus::Completed,
                "IN-PROCESS" => TaskStatus::InProcess,
                "CANCELLED" => TaskStatus::Cancelled,
                _ => TaskStatus::NeedsAction,
            }
        } else {
            TaskStatus::NeedsAction
        };
        let priority = todo
            .properties()
            .get("PRIORITY")
            .and_then(|p| p.value().parse::<u8>().ok())
            .unwrap_or(0);

        let sequence = todo
            .properties()
            .get("SEQUENCE")
            .and_then(|p| p.value().parse::<u32>().ok())
            .unwrap_or(0);

        let parse_date_prop = |val: &str| -> Option<DateTime<Utc>> {
            if val.len() == 8 {
                NaiveDate::parse_from_str(val, "%Y%m%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|d| d.and_utc())
            } else {
                NaiveDateTime::parse_from_str(
                    val,
                    if val.ends_with('Z') {
                        "%Y%m%dT%H%M%SZ"
                    } else {
                        "%Y%m%dT%H%M%S"
                    },
                )
                .ok()
                .map(|d| Utc.from_utc_datetime(&d))
            }
        };

        let due = todo.properties().get("DUE").and_then(|p| {
            let val = p.value();
            if val.len() == 8 {
                NaiveDate::parse_from_str(val, "%Y%m%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(23, 59, 59))
                    .map(|d| d.and_utc())
            } else {
                parse_date_prop(val)
            }
        });

        let dtstart = todo
            .properties()
            .get("DTSTART")
            .and_then(|p| parse_date_prop(p.value()));

        let rrule = todo
            .properties()
            .get("RRULE")
            .map(|p| p.value().to_string());

        let parse_dur = |val: &str| -> Option<u32> {
            let mut minutes = 0;
            let mut num_buf = String::new();
            let mut in_time = false;
            for c in val.chars() {
                if c == 'T' {
                    in_time = true;
                } else if c.is_numeric() {
                    num_buf.push(c);
                } else if !num_buf.is_empty() {
                    let n = num_buf.parse::<u32>().unwrap_or(0);
                    match c {
                        'D' => minutes += n * 24 * 60,
                        'H' => {
                            if in_time {
                                minutes += n * 60
                            }
                        }
                        'M' => {
                            if in_time {
                                minutes += n
                            }
                        }
                        'W' => minutes += n * 7 * 24 * 60,
                        _ => {}
                    }
                    num_buf.clear();
                }
            }
            if minutes > 0 { Some(minutes) } else { None }
        };

        let mut estimated_duration = todo
            .properties()
            .get("X-ESTIMATED-DURATION")
            .and_then(|p| parse_dur(p.value()));

        if estimated_duration.is_none() {
            estimated_duration = todo
                .properties()
                .get("DURATION")
                .and_then(|p| parse_dur(p.value()));
        }

        let mut categories = Vec::new();
        if let Some(multi_props) = todo.multi_properties().get("CATEGORIES") {
            for prop in multi_props {
                let parts: Vec<String> = prop
                    .value()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                categories.extend(parts);
            }
        }
        if let Some(prop) = todo.properties().get("CATEGORIES") {
            let parts: Vec<String> = prop
                .value()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            categories.extend(parts);
        }
        categories.sort();
        categories.dedup();

        // --- OPTIMIZED RELATION EXTRACTION (MANUAL PARSE) ---
        let (parent_uid, dependencies) = parse_related_to_manually(raw_ics);

        // --- CAPTURE UNMAPPED PROPERTIES ---
        let mut unmapped_properties = Vec::new();

        let to_raw = |prop: &icalendar::Property| -> RawProperty {
            let mut params = Vec::new();
            for (k, param) in prop.params().iter() {
                params.push((k.clone(), param.value().to_string()));
            }
            if !params.is_empty() {
                params.sort_unstable();
            }

            RawProperty {
                key: prop.key().to_string(),
                value: prop.value().to_string(),
                params,
            }
        };

        // Use case-insensitive check for handled keys
        for (key, prop) in todo.properties() {
            if !HANDLED_KEYS.contains(&key.to_uppercase().as_str()) {
                unmapped_properties.push(to_raw(prop));
            }
        }
        for (key, props) in todo.multi_properties() {
            if !HANDLED_KEYS.contains(&key.to_uppercase().as_str()) {
                for prop in props {
                    unmapped_properties.push(to_raw(prop));
                }
            }
        }

        if !unmapped_properties.is_empty() {
            unmapped_properties
                .sort_unstable_by(|a, b| a.key.cmp(&b.key).then(a.value.cmp(&b.value)));
        }

        // --- CAPTURE ALARMS ---
        let mut raw_alarms = Vec::new();
        let mut in_alarm = false;
        let mut current_alarm = String::new();

        for line in raw_ics.lines() {
            let trim = line.trim();
            if trim == "BEGIN:VALARM" {
                in_alarm = true;
                current_alarm.clear();
                current_alarm.push_str(line);
                current_alarm.push('\r');
                current_alarm.push('\n');
                continue;
            }
            if trim == "END:VALARM" {
                if in_alarm {
                    current_alarm.push_str(line);
                    current_alarm.push('\r');
                    current_alarm.push('\n');
                    raw_alarms.push(current_alarm.clone());
                    in_alarm = false;
                }
                continue;
            }
            if in_alarm {
                current_alarm.push_str(line);
                current_alarm.push('\r');
                current_alarm.push('\n');
            }
        }

        Ok(Task {
            uid,
            summary,
            description,
            status,
            estimated_duration,
            due,
            dtstart,
            priority,
            parent_uid,
            dependencies,
            etag,
            href,
            calendar_href,
            categories,
            depth: 0,
            rrule,
            unmapped_properties,
            sequence,
            raw_alarms,
            raw_components,
        })
    }
}

fn parse_related_to_manually(raw_ics: &str) -> (Option<String>, Vec<String>) {
    let mut parent = None;
    let mut deps = Vec::new();
    let mut current_line = String::new();

    let process_line = |line: &str, p: &mut Option<String>, d: &mut Vec<String>| {
        if line.to_uppercase().starts_with("RELATED-TO")
            && let Some((params_part, value)) = line.split_once(':')
        {
            let params_upper = params_part.to_uppercase();
            let is_dependency = params_upper.contains("RELTYPE=DEPENDS-ON");
            let val = value.trim().to_string();
            if is_dependency {
                if !d.contains(&val) {
                    d.push(val);
                }
            } else {
                *p = Some(val);
            }
        }
    };

    for raw_line in raw_ics.lines() {
        if raw_line.starts_with(' ') || raw_line.starts_with('\t') {
            current_line.push_str(raw_line.trim_start());
        } else {
            if !current_line.is_empty() {
                process_line(&current_line, &mut parent, &mut deps);
            }
            current_line = raw_line.to_string();
        }
    }
    if !current_line.is_empty() {
        process_line(&current_line, &mut parent, &mut deps);
    }

    (parent, deps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relationships_parsing_duplicate_protection() {
        let ics = "BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VTODO
UID:child-task-uid
SUMMARY:Child Task
RELATED-TO:parent-task-uid
RELATED-TO;RELTYPE=DEPENDS-ON:blocker-task-uid
END:VTODO
END:VCALENDAR";

        let task = Task::from_ics(
            ics,
            "etag".to_string(),
            "/href".to_string(),
            "/cal/".to_string(),
        )
        .expect("Failed to parse ICS");

        assert_eq!(task.parent_uid, Some("parent-task-uid".to_string()));
        assert_eq!(task.dependencies, vec!["blocker-task-uid".to_string()]);
    }

    #[test]
    fn test_alarm_preservation() {
        let ics = "BEGIN:VCALENDAR
BEGIN:VTODO
UID:alarm-test
SUMMARY:Wake Up
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:Wake up
TRIGGER:-PT10M
END:VALARM
END:VTODO
END:VCALENDAR";

        let task = Task::from_ics(
            ics,
            "etag".to_string(),
            "/href".to_string(),
            "/cal/".to_string(),
        )
        .expect("Parse failed");

        assert_eq!(task.raw_alarms.len(), 1);
        let output = task.to_ics();
        assert!(output.contains("BEGIN:VALARM"));
        assert!(output.contains("TRIGGER:-PT10M"));
    }

    #[test]
    fn test_sequence_preservation() {
        let ics = "BEGIN:VCALENDAR
BEGIN:VTODO
UID:seq-test
SEQUENCE:5
END:VTODO
END:VCALENDAR";
        let task = Task::from_ics(ics, "".to_string(), "".to_string(), "".to_string()).unwrap();
        assert_eq!(task.sequence, 5);
        let output = task.to_ics();
        assert!(output.contains("SEQUENCE:5"));
    }
}
