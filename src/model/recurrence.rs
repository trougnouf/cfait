// File: ./src/model/recurrence.rs
use crate::model::item::{Alarm, DateType, RawProperty, Task, TaskStatus};
use chrono::{Local, Utc};
use rrule::RRuleSet;
use std::collections::HashSet; // Import HashSet for deduplication
use std::str::FromStr;
use uuid::Uuid;

pub struct RecurrenceEngine;

impl RecurrenceEngine {
    /// Calculates the next occurrence of a task based on its RRULE.
    /// Returns a new Task instance for the next occurrence, or None if finished.
    pub fn next_occurrence(task: &Task) -> Option<Task> {
        let rule_str = task.rrule.as_ref()?;
        let seed_date_type = task.dtstart.as_ref().or(task.due.as_ref())?;

        // Normalize seed to UTC DateTime for calculation
        // For AllDay, prefer to preserve an available specific time component (if present
        // elsewhere on the task) to avoid losing the user's intended time when computing
        // recurrences. Fallback to UTC midnight for pure AllDay seeds.
        let seed_dt_utc = match seed_date_type {
            DateType::AllDay(d) => {
                // Try to find a specific time on the task (prefer dtstart then due).
                // If either field has a specific DateTime, use its time component with the AllDay date.
                // We use UTC time components directly to ensure stability across timezones.
                let specific_time_opt = match (task.dtstart.as_ref(), task.due.as_ref()) {
                    (Some(DateType::Specific(dt)), _) => Some(dt.time()),
                    (_, Some(DateType::Specific(dt))) => Some(dt.time()),
                    _ => None,
                };

                if let Some(time) = specific_time_opt {
                    // Combine the AllDay date with the discovered UTC time component.
                    d.and_time(time).and_utc()
                } else {
                    // No specific time found: treat as pure AllDay at UTC midnight.
                    d.and_hms_opt(0, 0, 0).unwrap().and_utc()
                }
            }
            DateType::Specific(dt) => *dt,
        };

        // Construct RRULE string using strict UTC format for calculation stability.
        let dtstart_str = seed_dt_utc.format("%Y%m%dT%H%M%SZ").to_string();

        // FIX 1: Sanitize rule string.
        // If the stored rule already starts with "RRULE:", strip it to avoid double prefixing.
        let clean_rule = rule_str.trim();
        let mut final_rule_part = if clean_rule.to_uppercase().starts_with("RRULE:") {
            clean_rule[6..].to_string()
        } else {
            clean_rule.to_string()
        };

        // FIX 4 (CRITICAL): Normalize UNTIL to DateTime if DTSTART is DateTime.
        // The rrule crate (and RFC 5545) requires UNTIL to match the type of DTSTART.
        // Since we force DTSTART to be a UTC DateTime above, we MUST ensure UNTIL is also
        // a UTC DateTime. If parser provided "UNTIL=20261231", we must convert it to
        // "UNTIL=20261231T235959Z".
        if let Some(idx) = final_rule_part.find("UNTIL=") {
            let until_val_start = idx + 6;
            // Find end of UNTIL value (semicolon or end of string)
            let until_val_end = final_rule_part[until_val_start..]
                .find(';')
                .map(|i| until_val_start + i)
                .unwrap_or(final_rule_part.len());

            let until_val = &final_rule_part[until_val_start..until_val_end];

            // If UNTIL is Date-only (8 chars, no 'T'), upgrade it to End-of-Day UTC
            if until_val.len() == 8 && !until_val.contains('T') {
                let new_until = format!("{}T235959Z", until_val);
                final_rule_part.replace_range(until_val_start..until_val_end, &new_until);
            }
        }

        let mut rrule_string = format!("DTSTART:{}\nRRULE:{}\n", dtstart_str, final_rule_part);

        // FIX 2: Deduplicate EXDATEs.
        // The user's file had multiple identical exdates. We use a HashSet to ensure uniqueness
        // before feeding them into the RRuleSet parser.
        if !task.exdates.is_empty() {
            let mut seen_exdates = HashSet::new();
            for ex in &task.exdates {
                let ex_utc = match ex {
                    DateType::AllDay(d) => d.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                    DateType::Specific(dt) => *dt,
                };
                let ex_str = ex_utc.format("%Y%m%dT%H%M%SZ").to_string();
                if seen_exdates.insert(ex_str.clone()) {
                    rrule_string.push_str(&format!("EXDATE:{}\n", ex_str));
                }
            }
        }

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            // FIX: Determine "Now" based on the task type.
            // For AllDay tasks, we must respect the user's Local date.
            // If it is Jan 27 8PM Local, it is still Jan 27. Even if it is Jan 28 1AM UTC.
            let comparison_now = match seed_date_type {
                DateType::AllDay(_) => {
                    // Get Local date, set to midnight, convert to UTC to match rrule domain
                    Local::now()
                        .date_naive()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                }
                DateType::Specific(_) => Utc::now(),
            };

            let search_floor = std::cmp::max(comparison_now, seed_dt_utc);

            let next_occurrence = rrule_set
                .into_iter()
                .find(|d| d.to_utc() > search_floor)
                .map(|d| d.to_utc());

            if let Some(next_start) = next_occurrence {
                let mut next_task = task.clone();
                next_task.uid = Uuid::new_v4().to_string();
                next_task.href = String::new();
                next_task.etag = String::new();
                next_task.status = TaskStatus::NeedsAction;
                next_task.percent_complete = None;
                next_task.dependencies.clear();
                next_task.sequence = 0;

                // Remove COMPLETED property from unmapped properties as this is a fresh occurrence
                next_task
                    .unmapped_properties
                    .retain(|p| p.key != "COMPLETED");

                // Clear Alarms if they are snooze/stateful (keep user defined ones)
                next_task
                    .alarms
                    .retain(|a: &Alarm| !a.is_snooze() && a.acknowledged.is_none());

                // FIX 3: Ensure exdates in the *new* task are clean (deduplicated)
                // We keep the exdates in the new task so history is preserved, but we might as well clean them up.
                next_task.exdates.sort_by(|a, b| a.partial_cmp(b).unwrap());
                next_task.exdates.dedup();

                let duration = if let Some(old_due) = &task.due {
                    match old_due {
                        DateType::AllDay(d) => {
                            let due_utc = d.and_hms_opt(0, 0, 0).unwrap().and_utc();
                            due_utc - seed_dt_utc
                        }
                        DateType::Specific(dt) => *dt - seed_dt_utc,
                    }
                } else {
                    chrono::Duration::zero()
                };

                // Apply next date maintaining DateType flavor
                if let Some(old_start) = &task.dtstart {
                    next_task.dtstart = match old_start {
                        DateType::AllDay(_) => Some(DateType::AllDay(next_start.date_naive())),
                        DateType::Specific(_) => Some(DateType::Specific(next_start)),
                    };
                }

                if let Some(old_due) = &task.due {
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

    /// Advances the task to the next recurrence instance IN PLACE.
    /// This preserves UID, HREF, and ETAG, satisfying "Recycling" logic.
    pub fn advance(task: &mut Task) -> bool {
        if let Some(next) = Self::next_occurrence(task) {
            // Capture identity
            let uid = task.uid.clone();
            let href = task.href.clone();
            let etag = task.etag.clone();
            let calendar_href = task.calendar_href.clone();
            let created_at = task
                .unmapped_properties
                .iter()
                .find(|p| p.key == "CREATED")
                .map(|p| p.value.clone());

            *task = next;

            // Restore identity
            task.uid = uid;
            task.href = href;
            task.etag = etag;
            task.calendar_href = calendar_href;

            if let Some(created_val) = created_at {
                task.unmapped_properties.retain(|p| p.key != "CREATED");
                task.unmapped_properties.push(RawProperty {
                    key: "CREATED".to_string(),
                    value: created_val,
                    params: vec![],
                });
            }

            task.sequence += 1;
            return true;
        }
        false
    }
}
