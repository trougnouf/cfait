// File: src/model/adapter.rs
use crate::model::item::{Alarm, AlarmTrigger, DateType, RawProperty, Task, TaskStatus};
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
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
    "PERCENT-COMPLETE",
    "CATEGORIES",
    "RELATED-TO",
    "DTSTAMP",
    "CREATED",
    "LAST-MODIFIED",
    "SEQUENCE",
    "PRODID",
    "VERSION",
    "CALSCALE",
    "RECURRENCE-ID",
    "LOCATION",
    "URL",
    "GEO",
];

impl Task {
    pub fn respawn(&self) -> Option<Task> {
        let rule_str = self.rrule.as_ref()?;
        let seed_date_type = self.dtstart.as_ref().or(self.due.as_ref())?;

        let seed_dt_utc = match seed_date_type {
            DateType::AllDay(d) => d.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            DateType::Specific(dt) => *dt,
        };

        let dtstart_str = seed_dt_utc.format("%Y%m%dT%H%M%SZ").to_string();
        let rrule_string = format!("DTSTART:{}\nRRULE:{}", dtstart_str, rule_str);

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            let now = Utc::now();
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
                next_task.percent_complete = None;
                next_task.dependencies.clear();
                next_task.sequence = 0;

                // Clear Alarms if they are snooze/stateful (keep user defined ones)
                next_task
                    .alarms
                    .retain(|a: &Alarm| !a.is_snooze() && a.acknowledged.is_none());

                let duration = if let Some(old_due) = &self.due {
                    match old_due {
                        DateType::AllDay(_) => chrono::Duration::zero(), // All day preserves all day
                        DateType::Specific(dt) => *dt - seed_dt_utc,
                    }
                } else {
                    chrono::Duration::zero()
                };

                // Apply next date maintaining DateType flavor
                if let Some(old_start) = &self.dtstart {
                    next_task.dtstart = match old_start {
                        DateType::AllDay(_) => Some(DateType::AllDay(next_start.date_naive())),
                        DateType::Specific(_) => Some(DateType::Specific(next_start)),
                    };
                }

                if let Some(old_due) = &self.due {
                    let next_due_utc = next_start + duration;
                    next_task.due = match old_due {
                        DateType::AllDay(_) => Some(DateType::AllDay(next_due_utc.date_naive())),
                        DateType::Specific(_) => Some(DateType::Specific(next_due_utc)),
                    };
                }

                return Some(next_task);
            }
        }
        None
    }

    pub fn advance_recurrence(&mut self) -> bool {
        if let Some(next) = self.respawn() {
            *self = next;
            return true;
        }
        false
    }

    pub fn to_ics(&self) -> String {
        let mut todo = Todo::new();
        todo.add_property("UID", &self.uid);
        todo.summary(&self.summary);
        if !self.description.is_empty() {
            todo.description(&self.description);
        }
        todo.timestamp(Utc::now());
        todo.add_property("SEQUENCE", self.sequence.to_string());

        if let Some(loc) = &self.location {
            todo.add_property("LOCATION", loc);
        }
        if let Some(u) = &self.url {
            todo.add_property("URL", u);
        }
        if let Some(g) = &self.geo {
            let geo_val: String = g.replace(',', ";");
            todo.add_property("GEO", &geo_val);
        }

        match self.status {
            TaskStatus::NeedsAction => todo.status(TodoStatus::NeedsAction),
            TaskStatus::InProcess => todo.status(TodoStatus::InProcess),
            TaskStatus::Completed => todo.status(TodoStatus::Completed),
            TaskStatus::Cancelled => todo.status(TodoStatus::Cancelled),
        };
        if let Some(pc) = self.percent_complete {
            todo.percent_complete(pc);
        }

        if let Some(dt) = &self.dtstart {
            match dt {
                DateType::AllDay(d) => {
                    let mut p = icalendar::Property::new("DTSTART", d.format("%Y%m%d").to_string());
                    p.add_parameter("VALUE", "DATE");
                    todo.append_property(p);
                }
                DateType::Specific(t) => {
                    todo.add_property("DTSTART", t.format("%Y%m%dT%H%M%SZ").to_string());
                }
            }
        }

        if let Some(dt) = &self.due {
            match dt {
                DateType::AllDay(d) => {
                    let mut p = icalendar::Property::new("DUE", d.format("%Y%m%d").to_string());
                    p.add_parameter("VALUE", "DATE");
                    todo.append_property(p);
                }
                DateType::Specific(t) => {
                    todo.add_property("DUE", t.format("%Y%m%dT%H%M%SZ").to_string());
                }
            }
        }

        if let Some(mins) = self.estimated_duration {
            // Simplified duration format
            todo.add_property("DURATION", format!("PT{}M", mins));
        }
        if self.priority > 0 {
            todo.priority(self.priority.into());
        }
        if let Some(rrule) = &self.rrule {
            let rrule_str: String = rrule.as_str().into();
            todo.add_property("RRULE", &rrule_str);
        }

        if let Some(p_uid) = &self.parent_uid {
            let p_uid_str: String = p_uid.as_str().into();
            let prop = icalendar::Property::new("RELATED-TO", &p_uid_str);
            todo.append_property(prop);
        }
        for dep_uid in &self.dependencies {
            let mut prop = icalendar::Property::new("RELATED-TO", dep_uid);
            prop.add_parameter("RELTYPE", "DEPENDS-ON");
            todo.append_property(prop);
        }

        // Unmapped
        for raw in &self.unmapped_properties {
            let mut prop = icalendar::Property::new(&raw.key, &raw.value);
            for (k, v) in &raw.params {
                prop.add_parameter(k, v);
            }
            todo.append_property(prop);
        }

        let mut calendar = Calendar::new();
        calendar.push(todo);
        let mut ics = calendar.to_string();

        // Inject Categories manually (icalendar lib support varies)
        if !self.categories.is_empty() {
            let escaped_cats: Vec<String> = self
                .categories
                .iter()
                .map(|c: &String| c.replace(',', "\\,"))
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

        // --- INJECT VALARM (RFC 9074) ---
        // We use manual injection because the `icalendar` library's Alarm struct support
        // might not fully cover all RFC 9074 fields or custom properties easily via its builder.
        if !self.alarms.is_empty()
            && let Some(idx) = ics.rfind("END:VTODO") {
                let (start, end) = ics.split_at(idx);
                let mut buffer = String::with_capacity(ics.len() + 1024);
                buffer.push_str(start);

                for alarm in &self.alarms {
                    buffer.push_str("BEGIN:VALARM\r\n");
                    buffer.push_str(&format!("UID:{}\r\n", alarm.uid));
                    buffer.push_str(&format!("ACTION:{}\r\n", alarm.action));
                    if let Some(desc) = &alarm.description {
                        buffer.push_str(&format!("DESCRIPTION:{}\r\n", desc));
                    } else {
                        buffer.push_str("DESCRIPTION:Reminder\r\n");
                    }

                    match alarm.trigger {
                        AlarmTrigger::Relative(mins) => {
                            let sign = if mins < 0 { "-" } else { "" };
                            buffer.push_str(&format!("TRIGGER:{}PT{}M\r\n", sign, mins.abs()));
                        }
                        AlarmTrigger::Absolute(dt) => {
                            buffer.push_str(&format!(
                                "TRIGGER;VALUE=DATE-TIME:{}\r\n",
                                dt.format("%Y%m%dT%H%M%SZ")
                            ));
                        }
                    }

                    if let Some(ack) = alarm.acknowledged {
                        let ack_str: String = ack.format("%Y%m%dT%H%M%SZ").to_string();
                        buffer.push_str(&format!("ACKNOWLEDGED:{}\r\n", ack_str));
                    }

                    if let Some(rel) = &alarm.related_to_uid {
                        if let Some(rtype) = &alarm.relation_type {
                            buffer.push_str(&format!("RELATED-TO;RELTYPE={}:{}\r\n", rtype, rel));
                        } else {
                            buffer.push_str(&format!("RELATED-TO:{}\r\n", rel));
                        }
                    }

                    buffer.push_str("END:VALARM\r\n");
                }
                buffer.push_str(end);
                ics = buffer;
            }

        // Inject Raw Components
        if !self.raw_components.is_empty() {
            let extra_len: usize = self
                .raw_components
                .iter()
                .map(|s: &String| s.len() + 2)
                .sum();
            if let Some(idx) = ics.rfind("END:VCALENDAR") {
                let (start, end) = ics.split_at(idx);
                let mut buffer = String::with_capacity(ics.len() + extra_len);
                buffer.push_str(start);
                for raw in &self.raw_components {
                    buffer.push_str(raw);
                    if !raw.ends_with('\n') {
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
        let mut raw_components: Vec<String> = Vec::new();

        // icalendar::Calendar::components is Vec<CalendarComponent>
        for component in &calendar.components {
            match component {
                CalendarComponent::Todo(t) => {
                    // Check for RECURRENCE-ID (Exception) via properties map
                    // Inner properties are accessible via .properties() method on Todo (via Component trait)
                    if t.properties().contains_key("RECURRENCE-ID") {
                        raw_components.push(t.to_string());
                    } else if master_todo.is_none() {
                        master_todo = Some(t);
                    } else {
                        raw_components.push(t.to_string());
                    }
                }
                CalendarComponent::Event(e) => raw_components.push(e.to_string()),
                CalendarComponent::Venue(v) => raw_components.push(v.to_string()),
                CalendarComponent::Other(o) => raw_components.push(o.to_string()),
                _ => {}
            }
        }

        let todo = master_todo.ok_or("No Master VTODO found in ICS".to_string())?;

        // Helper to get property string value
        let get_prop = |key: &str| -> Option<String> {
            todo.properties().get(key).map(|p| p.value().to_string())
        };

        let uid = get_prop("UID").unwrap_or_default();
        let summary = get_prop("SUMMARY").unwrap_or_default();
        let description = get_prop("DESCRIPTION").unwrap_or_default();

        let status = if let Some(val) = get_prop("STATUS") {
            match val.trim().to_uppercase().as_str() {
                "COMPLETED" => TaskStatus::Completed,
                "IN-PROCESS" => TaskStatus::InProcess,
                "CANCELLED" => TaskStatus::Cancelled,
                _ => TaskStatus::NeedsAction,
            }
        } else {
            TaskStatus::NeedsAction
        };

        let priority = get_prop("PRIORITY")
            .and_then(|v| v.parse::<u8>().ok())
            .unwrap_or(0);
        let sequence = get_prop("SEQUENCE")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let percent_complete = get_prop("PERCENT-COMPLETE").and_then(|v| v.parse::<u8>().ok());

        let location = get_prop("LOCATION");
        let url = get_prop("URL");
        let geo = get_prop("GEO").map(|s| s.replace(';', ","));

        let parse_date_type = |prop: &icalendar::Property| -> Option<DateType> {
            let val = prop.value();
            // Check VALUE param
            let is_date = prop
                .params()
                .get("VALUE")
                .map(|v| v.value() == "DATE")
                .unwrap_or(false);
            if is_date || val.len() == 8 {
                NaiveDate::parse_from_str(val, "%Y%m%d")
                    .ok()
                    .map(DateType::AllDay)
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
                .map(|d| DateType::Specific(Utc.from_utc_datetime(&d)))
            }
        };

        let due = todo.properties().get("DUE").and_then(parse_date_type);
        let dtstart = todo.properties().get("DTSTART").and_then(parse_date_type);
        let rrule = get_prop("RRULE");

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
            .and_then(|p: &icalendar::Property| parse_dur(p.value()));

        if estimated_duration.is_none() {
            estimated_duration = todo
                .properties()
                .get("DURATION")
                .and_then(|p: &icalendar::Property| parse_dur(p.value()));
        }

        let mut categories = Vec::new();
        // Check for multi-value property CATEGORIES
        if let Some(multi_props) = todo.multi_properties().get("CATEGORIES") {
            for prop in multi_props {
                let parts: Vec<String> = prop
                    .value()
                    .split(',')
                    .map(|s: &str| s.trim().to_string())
                    .filter(|s: &String| !s.is_empty())
                    .collect();
                categories.extend(parts);
            }
        }
        // Also check single property if not multi
        if let Some(prop) = todo.properties().get("CATEGORIES") {
            let parts: Vec<String> = prop
                .value()
                .split(',')
                .map(|s: &str| s.trim().to_string())
                .filter(|s: &String| !s.is_empty())
                .collect();
            categories.extend(parts);
        }
        categories.sort();
        categories.dedup();

        // Relations
        let mut parent_uid = None;
        let mut dependencies = Vec::new();

        // Check single RELATED-TO
        if let Some(prop) = todo.properties().get("RELATED-TO") {
            let is_dep = prop
                .params()
                .get("RELTYPE")
                .map(|p| p.value() == "DEPENDS-ON")
                .unwrap_or(false);
            let val = prop.value().to_string();
            if is_dep {
                dependencies.push(val);
            } else {
                parent_uid = Some(val);
            }
        }
        // Check multi RELATED-TO
        if let Some(props) = todo.multi_properties().get("RELATED-TO") {
            for prop in props {
                let is_dep = prop
                    .params()
                    .get("RELTYPE")
                    .map(|p| p.value() == "DEPENDS-ON")
                    .unwrap_or(false);
                let val = prop.value().to_string();
                if is_dep {
                    if !dependencies.contains(&val) {
                        dependencies.push(val);
                    }
                } else {
                    parent_uid = Some(val);
                }
            }
        }

        // Unmapped
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

        // ALARMS Extraction from raw ICS text
        // Because accessing sub-components via icalendar crate structs is specific,
        // and we need to handle RFC 9074 fields which might be ignored by strict parsers,
        // we parse the alarm blocks manually from the raw string for robustness.
        let mut alarms = Vec::new();
        let mut in_alarm = false;
        let mut current_alarm_lines: Vec<String> = Vec::new();

        for line in raw_ics.lines() {
            let trim = line.trim();
            if trim == "BEGIN:VALARM" {
                in_alarm = true;
                continue;
            }
            if trim == "END:VALARM" {
                in_alarm = false;
                let mut alarm = Alarm {
                    uid: Uuid::new_v4().to_string(),
                    action: "DISPLAY".to_string(),
                    trigger: AlarmTrigger::Relative(0),
                    description: None,
                    acknowledged: None,
                    related_to_uid: None,
                    relation_type: None,
                };

                for l in &current_alarm_lines {
                    if let Some((key, val)) = l.split_once(':') {
                        let k_upper = key.split(';').next().unwrap_or(key).to_uppercase();
                        match k_upper.as_str() {
                            "UID" => alarm.uid = val.trim().to_string(),
                            "ACTION" => alarm.action = val.trim().to_string(),
                            "DESCRIPTION" => alarm.description = Some(val.trim().to_string()),
                            "TRIGGER" => {
                                if val.contains('T') && !val.contains('P') {
                                    if let Ok(dt) =
                                        NaiveDateTime::parse_from_str(val.trim(), "%Y%m%dT%H%M%SZ")
                                    {
                                        alarm.trigger =
                                            AlarmTrigger::Absolute(Utc.from_utc_datetime(&dt));
                                    }
                                } else {
                                    // Simplistic relative duration parser for -PT15M
                                    let v_trim = val.trim();
                                    let is_neg = v_trim.starts_with('-');
                                    // Find numeric part
                                    let numeric: String =
                                        v_trim.chars().filter(|c| c.is_numeric()).collect();
                                    if let Ok(mins) = numeric.parse::<i32>() {
                                        // Assume M if not specified, usually PT15M
                                        alarm.trigger = AlarmTrigger::Relative(if is_neg {
                                            -mins
                                        } else {
                                            mins
                                        });
                                    }
                                }
                            }
                            "ACKNOWLEDGED" => {
                                if let Ok(dt) =
                                    NaiveDateTime::parse_from_str(val.trim(), "%Y%m%dT%H%M%SZ")
                                {
                                    alarm.acknowledged = Some(Utc.from_utc_datetime(&dt));
                                }
                            }
                            "RELATED-TO" => {
                                alarm.related_to_uid = Some(val.trim().to_string());
                                if key.contains("RELTYPE=SNOOZE") {
                                    alarm.relation_type = Some("SNOOZE".to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                alarms.push(alarm);
                current_alarm_lines.clear();
                continue;
            }
            if in_alarm {
                current_alarm_lines.push(line.to_string());
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
            alarms,
            priority,
            percent_complete,
            parent_uid,
            dependencies,
            etag,
            href,
            calendar_href,
            categories,
            depth: 0,
            rrule,
            location,
            url,
            geo,
            unmapped_properties,
            sequence,
            raw_components,
            raw_alarms: Vec::new(),
        })
    }
}
