// File: ./src/model/adapter.rs
use crate::model::item::{Alarm, AlarmTrigger, DateType, RawProperty, Task, TaskStatus};
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use icalendar::{Calendar, CalendarComponent, Component, Event, Todo, TodoStatus};
use uuid::Uuid;

/// List of property keys we explicitly handle when mapping to/from ICS.
/// Time-tracking properties `X-TIME-SPENT` and `X-LAST-START` are included here
/// so they are not treated as unmapped custom properties.
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
    "LAST-MODIFIED",
    "SEQUENCE",
    "PRODID",
    "VERSION",
    "CALSCALE",
    "RECURRENCE-ID",
    "LOCATION",
    "URL",
    "GEO",
    "X-CFAIT-CREATE-EVENT",
    // Time-tracking properties we explicitly understand
    "X-TIME-SPENT",
    "X-LAST-START",
    "EXDATE",
];

pub struct IcsAdapter;

impl IcsAdapter {
    pub fn to_ics(task: &Task) -> String {
        let mut todo = Todo::new();
        todo.add_property("UID", &task.uid);
        todo.summary(&task.summary);
        if !task.description.is_empty() {
            todo.description(&task.description);
        }
        todo.timestamp(Utc::now());
        todo.add_property("SEQUENCE", task.sequence.to_string());

        if let Some(loc) = &task.location {
            todo.add_property("LOCATION", loc);
        }
        if let Some(u) = &task.url {
            todo.add_property("URL", u);
        }
        if let Some(g) = &task.geo {
            // ICS GEO uses ';' separator sometimes - we stored as "lat,lon"
            let geo_val: String = g.replace(',', ";");
            todo.add_property("GEO", &geo_val);
        }

        match task.status {
            TaskStatus::NeedsAction => todo.status(TodoStatus::NeedsAction),
            TaskStatus::InProcess => todo.status(TodoStatus::InProcess),
            TaskStatus::Completed => todo.status(TodoStatus::Completed),
            TaskStatus::Cancelled => todo.status(TodoStatus::Cancelled),
        };
        if let Some(pc) = task.percent_complete {
            todo.percent_complete(pc);
        }

        if let Some(dt) = &task.dtstart {
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

        if let Some(dt) = &task.due {
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

        if let Some(mins) = task.estimated_duration {
            let is_all_day_start = task
                .dtstart
                .as_ref()
                .map(|dt| matches!(dt, DateType::AllDay(_)))
                .unwrap_or(false);

            if task.due.is_some() || is_all_day_start {
                todo.add_property("X-ESTIMATED-DURATION", format!("PT{}M", mins));
            } else {
                todo.add_property("DURATION", format!("PT{}M", mins));
            }

            if let Some(max) = task.estimated_duration_max {
                todo.add_property("X-CFAIT-ESTIMATED-DURATION-MAX", format!("PT{}M", max));
            }
        }

        if task.priority > 0 {
            let prio = if task.priority > 9 { 9 } else { task.priority };
            todo.priority(prio.into());
        }
        if let Some(rrule) = &task.rrule {
            let rrule_str: String = rrule.as_str().into();
            todo.add_property("RRULE", &rrule_str);
        }

        for ex in &task.exdates {
            match ex {
                DateType::AllDay(d) => {
                    let mut p = icalendar::Property::new("EXDATE", d.format("%Y%m%d").to_string());
                    p.add_parameter("VALUE", "DATE");
                    todo.append_multi_property(p);
                }
                DateType::Specific(dt) => {
                    todo.append_multi_property(icalendar::Property::new(
                        "EXDATE",
                        dt.format("%Y%m%dT%H%M%SZ").to_string(),
                    ));
                }
            }
        }

        if let Some(p_uid) = &task.parent_uid {
            let p_uid_str: String = p_uid.as_str().into();
            let prop = icalendar::Property::new("RELATED-TO", &p_uid_str);
            todo.append_multi_property(prop);
        }
        for dep_uid in &task.dependencies {
            let mut prop = icalendar::Property::new("RELATED-TO", dep_uid);
            prop.add_parameter("RELTYPE", "DEPENDS-ON");
            todo.append_multi_property(prop);
        }
        for related_uid in &task.related_to {
            let mut prop = icalendar::Property::new("RELATED-TO", related_uid);
            prop.add_parameter("RELTYPE", "SIBLING");
            todo.append_multi_property(prop);
        }

        if let Some(create_event) = task.create_event {
            todo.add_property(
                "X-CFAIT-CREATE-EVENT",
                if create_event { "TRUE" } else { "FALSE" },
            );
        }

        // Emit time-tracking properties (if present)
        if task.time_spent_seconds > 0 {
            todo.add_property("X-TIME-SPENT", task.time_spent_seconds.to_string());
        }
        if let Some(ts) = task.last_started_at {
            todo.add_property("X-LAST-START", ts.to_string());
        }

        for raw in &task.unmapped_properties {
            let mut prop = icalendar::Property::new(&raw.key, &raw.value);
            for (k, v) in &raw.params {
                prop.add_parameter(k, v);
            }
            todo.append_property(prop);
        }

        let mut calendar = Calendar::new();
        calendar.push(todo);
        let mut ics = calendar.to_string();

        if !task.categories.is_empty() {
            let escaped_cats: Vec<String> = task
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

        if !task.alarms.is_empty()
            && let Some(idx) = ics.rfind("END:VTODO")
        {
            let (start, end) = ics.split_at(idx);
            let mut buffer = String::with_capacity(ics.len() + 1024);
            buffer.push_str(start);

            for alarm in &task.alarms {
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

        if !task.raw_components.is_empty() {
            let extra_len: usize = task
                .raw_components
                .iter()
                .map(|s: &String| s.len() + 2)
                .sum();
            if let Some(idx) = ics.rfind("END:VCALENDAR") {
                let (start, end) = ics.split_at(idx);
                let mut buffer = String::with_capacity(ics.len() + extra_len);
                buffer.push_str(start);
                for raw in &task.raw_components {
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
    ) -> Result<Task, String> {
        let calendar: Calendar = raw_ics.parse().map_err(|e| format!("Parse: {}", e))?;

        let mut master_todo: Option<&Todo> = None;
        let mut raw_components: Vec<String> = Vec::new();

        for component in &calendar.components {
            match component {
                CalendarComponent::Todo(t) => {
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
            .map(|p| if p > 9 { 9 } else { p })
            .unwrap_or(0);
        let sequence = get_prop("SEQUENCE")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let percent_complete = get_prop("PERCENT-COMPLETE").and_then(|v| v.parse::<u8>().ok());

        let location = get_prop("LOCATION");
        let url = get_prop("URL");
        let geo = get_prop("GEO").map(|s| s.replace(';', ","));

        let create_event =
            get_prop("X-CFAIT-CREATE-EVENT").and_then(|v| match v.trim().to_uppercase().as_str() {
                "TRUE" | "1" | "YES" => Some(true),
                "FALSE" | "0" | "NO" => Some(false),
                _ => None,
            });

        let parse_date_type = |prop: &icalendar::Property| -> Option<DateType> {
            let val = prop.value();
            let is_date = prop
                .params()
                .get("VALUE")
                .map(|v| v.value() == "DATE")
                .unwrap_or(false);

            if is_date || val.len() == 8 {
                NaiveDate::parse_from_str(val, "%Y%m%d")
                    .ok()
                    .map(DateType::AllDay)
            } else if val.ends_with('Z') {
                NaiveDateTime::parse_from_str(val, "%Y%m%dT%H%M%SZ")
                    .ok()
                    .map(|d| DateType::Specific(Utc.from_utc_datetime(&d)))
            } else {
                NaiveDateTime::parse_from_str(val, "%Y%m%dT%H%M%S")
                    .ok()
                    .map(|d| {
                        let dt = chrono::Local
                            .from_local_datetime(&d)
                            .earliest()
                            .unwrap_or_else(|| {
                                Utc.from_utc_datetime(&d).with_timezone(&chrono::Local)
                            });
                        DateType::Specific(dt.with_timezone(&Utc))
                    })
            }
        };

        let due = todo.properties().get("DUE").and_then(parse_date_type);
        let dtstart = todo.properties().get("DTSTART").and_then(parse_date_type);
        let rrule = get_prop("RRULE");

        let mut exdates = Vec::new();
        if let Some(multi_props) = todo.multi_properties().get("EXDATE") {
            for prop in multi_props {
                let is_date = prop
                    .params()
                    .get("VALUE")
                    .map(|v| v.value() == "DATE")
                    .unwrap_or(false);
                let val_str = prop.value();
                for part in val_str.split(',') {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }

                    if is_date || part.len() == 8 {
                        if let Ok(d) = NaiveDate::parse_from_str(part, "%Y%m%d") {
                            exdates.push(DateType::AllDay(d));
                        }
                    } else if part.ends_with('Z') {
                        if let Ok(dt) = NaiveDateTime::parse_from_str(part, "%Y%m%dT%H%M%SZ") {
                            exdates.push(DateType::Specific(Utc.from_utc_datetime(&dt)));
                        }
                    } else if let Ok(dt) = NaiveDateTime::parse_from_str(part, "%Y%m%dT%H%M%S") {
                        let local = chrono::Local
                            .from_local_datetime(&dt)
                            .earliest()
                            .unwrap_or_else(|| {
                                Utc.from_utc_datetime(&dt).with_timezone(&chrono::Local)
                            });
                        exdates.push(DateType::Specific(local.with_timezone(&Utc)));
                    }
                }
            }
        }

        // Helper to parse iCal durations like PT1H30M or P1DT etc. Return total minutes.
        let parse_ics_duration = |val: &str| -> i32 {
            // Very small state machine to parse ISO 8601 durations (subset).
            // We'll accept patterns like PnW, PnDTnHnM, PTnHnM, etc.
            let mut minutes: i32 = 0;
            let mut num_buf = String::new();
            let mut in_time = false;
            for c in val.chars() {
                match c {
                    'P' | 'p' => {}
                    'T' | 't' => {
                        in_time = true;
                    }
                    d if d.is_ascii_digit() => {
                        num_buf.push(d);
                    }
                    unit if !num_buf.is_empty() => {
                        if let Ok(n) = num_buf.parse::<i32>() {
                            match unit {
                                'W' => minutes += n * 7 * 24 * 60,
                                'D' => minutes += n * 24 * 60,
                                'H' if in_time => minutes += n * 60,
                                'M' if in_time => minutes += n,
                                _ => {}
                            }
                        }
                        num_buf.clear();
                    }
                    _ => {}
                }
            }
            minutes
        };

        let parse_dur = |val: &str| -> Option<u32> {
            let mut minutes = 0u32;
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

        let estimated_duration_max = todo
            .properties()
            .get("X-CFAIT-ESTIMATED-DURATION-MAX")
            .and_then(|p: &icalendar::Property| parse_dur(p.value()));

        let mut categories = Vec::new();
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

        let mut parent_uid = None;
        let mut dependencies = Vec::new();
        let mut related_to = Vec::new();

        let unfolded = icalendar::parser::unfold(raw_ics);
        let mut in_vtodo = false;
        let mut in_valarm = false;

        for line in unfolded.lines() {
            let line = line.trim();
            if line == "BEGIN:VTODO" {
                in_vtodo = true;
                continue;
            }
            if line == "END:VTODO" {
                in_vtodo = false;
                continue;
            }
            if line == "BEGIN:VALARM" {
                in_valarm = true;
                continue;
            }
            if line == "END:VALARM" {
                in_valarm = false;
                continue;
            }

            if in_vtodo
                && !in_valarm
                && line.starts_with("RELATED-TO")
                && let Some((raw_key, val)) = line.split_once(':')
            {
                let parts: Vec<&str> = raw_key.split(';').collect();
                let mut is_dep = false;
                let mut is_sibling = false;
                for param in parts.iter().skip(1) {
                    if param.contains("RELTYPE") {
                        if param.contains("DEPENDS-ON") {
                            is_dep = true;
                        } else if param.contains("SIBLING") {
                            is_sibling = true;
                        }
                    }
                }

                let value = val.trim().to_string();
                if is_dep {
                    if !dependencies.contains(&value) {
                        dependencies.push(value);
                    }
                } else if is_sibling {
                    if !related_to.contains(&value) {
                        related_to.push(value);
                    }
                } else {
                    parent_uid = Some(value);
                }
            }
        }

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
                                    let v_trim = val.trim();
                                    let is_neg = v_trim.starts_with('-');
                                    let abs_mins = parse_ics_duration(if is_neg {
                                        &v_trim[1..]
                                    } else {
                                        v_trim
                                    });
                                    alarm.trigger = AlarmTrigger::Relative(if is_neg {
                                        -abs_mins
                                    } else {
                                        abs_mins
                                    });
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

        // Parse time-tracking fields from properties
        let time_spent_seconds = get_prop("X-TIME-SPENT")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let last_started_at = get_prop("X-LAST-START").and_then(|v| v.parse::<i64>().ok());

        Ok(Task {
            uid,
            summary,
            description,
            status,
            estimated_duration,
            estimated_duration_max,
            due,
            dtstart,
            alarms,
            exdates,
            priority,
            percent_complete,
            parent_uid,
            dependencies,
            related_to,
            etag,
            href,
            calendar_href,
            categories,
            depth: 0,
            rrule,
            location,
            url,
            geo,
            time_spent_seconds,
            last_started_at,
            unmapped_properties,
            sequence,
            raw_alarms: Vec::new(),
            raw_components,
            create_event,
            is_blocked: false,
            sort_rank: 0,
            effective_priority: 0,
            effective_due: None,
            effective_dtstart: None,
            virtual_state: crate::model::VirtualState::None,
        })
    }

    pub fn to_event_ics(task: &Task) -> Option<(String, String)> {
        if task.status == TaskStatus::Completed {
            let completed_at = task
                .unmapped_properties
                .iter()
                .find(|p| p.key == "COMPLETED")
                .and_then(|p| NaiveDateTime::parse_from_str(&p.value, "%Y%m%dT%H%M%SZ").ok())
                .map(|ndt| Utc.from_utc_datetime(&ndt))
                .unwrap_or_else(Utc::now);

            let duration_mins = task.estimated_duration.unwrap_or(60) as i64;
            let start_at = completed_at - chrono::Duration::minutes(duration_mins);

            let event_uid = format!("evt-{}", task.uid);
            let mut event = Event::new();
            event.add_property("UID", &event_uid);
            event.summary(&task.summary);
            event.timestamp(Utc::now());

            event.add_property("STATUS", "CONFIRMED");

            let mut event_desc = String::new();
            if !task.description.is_empty() {
                event_desc.push_str(&task.description);
                event_desc.push_str("\n\n");
            }
            event_desc.push_str("✓ Task Completed\n");
            event_desc.push_str("This event marks when the task was checked off.\n");
            event.description(&event_desc);

            if let Some(loc) = &task.location {
                event.add_property("LOCATION", loc);
            }
            if let Some(url) = &task.url {
                event.add_property("URL", url);
            }

            event.add_property("DTSTART", start_at.format("%Y%m%dT%H%M%SZ").to_string());
            event.add_property("DTEND", completed_at.format("%Y%m%dT%H%M%SZ").to_string());

            let mut calendar = Calendar::new();
            calendar.push(event);
            return Some((event_uid, calendar.to_string()));
        }

        if task.due.is_none() && task.dtstart.is_none() {
            return None;
        }

        let event_uid = format!("evt-{}", task.uid);
        let mut event = Event::new();
        event.add_property("UID", &event_uid);
        event.summary(&task.summary);
        event.timestamp(Utc::now());

        let mut event_desc = String::new();
        if !task.description.is_empty() {
            event_desc.push_str(&task.description);
            event_desc.push_str("\n\n");
        }
        event_desc.push_str("⚠️ This event was automatically created by Cfait from a task.\n");
        event_desc.push_str("It will be automatically updated/overwritten when the task changes, and it might get deleted when the task is completed or canceled.\n");
        event_desc.push_str("Any changes made directly to this event will be lost.\n");
        event.description(&event_desc);
        if let Some(loc) = &task.location {
            event.add_property("LOCATION", loc);
        }
        if let Some(url) = &task.url {
            event.add_property("URL", url);
        }

        let (start, end) = match (&task.dtstart, &task.due) {
            (Some(s), Some(d)) => {
                let span_days = match (s, d) {
                    (DateType::AllDay(start_date), DateType::AllDay(end_date)) => {
                        (*end_date - *start_date).num_days()
                    }
                    (DateType::Specific(start_dt), DateType::Specific(end_dt)) => {
                        (*end_dt - *start_dt).num_days()
                    }
                    (DateType::AllDay(start_date), DateType::Specific(end_dt)) => {
                        (end_dt.date_naive() - *start_date).num_days()
                    }
                    (DateType::Specific(start_dt), DateType::AllDay(end_date)) => {
                        (*end_date - start_dt.date_naive()).num_days()
                    }
                };

                if span_days > 7 {
                    match d {
                        DateType::AllDay(date) => {
                            (DateType::AllDay(*date), DateType::AllDay(*date))
                        }
                        DateType::Specific(dt) => (
                            DateType::Specific(*dt - chrono::Duration::hours(1)),
                            DateType::Specific(*dt),
                        ),
                    }
                } else {
                    (s.clone(), d.clone())
                }
            }

            (Some(s), None) => {
                let duration_mins = task.estimated_duration.unwrap_or(60) as i64;
                match s {
                    DateType::AllDay(d) => (DateType::AllDay(*d), DateType::AllDay(*d)),
                    DateType::Specific(dt) => (
                        DateType::Specific(*dt),
                        DateType::Specific(*dt + chrono::Duration::minutes(duration_mins)),
                    ),
                }
            }

            (None, Some(d)) => {
                let duration_mins = task.estimated_duration.unwrap_or(60) as i64;
                match d {
                    DateType::AllDay(date) => (DateType::AllDay(*date), DateType::AllDay(*date)),
                    DateType::Specific(dt) => (
                        DateType::Specific(*dt - chrono::Duration::minutes(duration_mins)),
                        DateType::Specific(*dt),
                    ),
                }
            }

            (None, None) => return None,
        };

        match start {
            DateType::AllDay(d) => {
                let mut p = icalendar::Property::new("DTSTART", d.format("%Y%m%d").to_string());
                p.add_parameter("VALUE", "DATE");
                event.append_property(p);
            }
            DateType::Specific(t) => {
                event.add_property("DTSTART", t.format("%Y%m%dT%H%M%SZ").to_string());
            }
        }

        match end {
            DateType::AllDay(d) => {
                let next_day = d + chrono::Duration::days(1);
                let mut p =
                    icalendar::Property::new("DTEND", next_day.format("%Y%m%d").to_string());
                p.add_parameter("VALUE", "DATE");
                event.append_property(p);
            }
            DateType::Specific(t) => {
                event.add_property("DTEND", t.format("%Y%m%dT%H%M%SZ").to_string());
            }
        }

        let status_str = match task.status {
            TaskStatus::Cancelled => "CANCELLED",
            TaskStatus::Completed => "CONFIRMED",
            _ => "CONFIRMED",
        };
        event.add_property("STATUS", status_str);

        let mut calendar = Calendar::new();
        calendar.push(event);

        Some((event_uid, calendar.to_string()))
    }
}
