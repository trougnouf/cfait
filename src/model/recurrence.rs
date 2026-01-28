// File: ./src/model/recurrence.rs
use crate::model::item::{Alarm, DateType, RawProperty, Task, TaskStatus};
use chrono::Utc;
use rrule::RRuleSet;
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
        // For AllDay, we force UTC Midnight (T000000Z) to ensure stable arithmetic
        let seed_dt_utc = match seed_date_type {
            DateType::AllDay(d) => d.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            DateType::Specific(dt) => *dt,
        };

        // Construct RRULE string using strict UTC format for calculation stability.
        let dtstart_str = seed_dt_utc.format("%Y%m%dT%H%M%SZ").to_string();
        let mut rrule_string = format!("DTSTART:{}\nRRULE:{}\n", dtstart_str, rule_str);

        // Add EXDATEs normalized to UTC matching DTSTART
        if !task.exdates.is_empty() {
            for ex in &task.exdates {
                let ex_utc = match ex {
                    DateType::AllDay(d) => d.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                    DateType::Specific(dt) => *dt,
                };
                rrule_string.push_str(&format!("EXDATE:{}\n", ex_utc.format("%Y%m%dT%H%M%SZ")));
            }
        }

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            let now = Utc::now();
            let search_floor = std::cmp::max(now, seed_dt_utc);

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

    pub fn advance_with_cancellation(task: &mut Task) -> bool {
        // Add current occurrence date to exdates BEFORE calculation
        let seed_dt = task.dtstart.as_ref().or(task.due.as_ref()).cloned();
        if let Some(current_date) = seed_dt {
            task.exdates.push(current_date);
        }

        if let Some(next) = Self::next_occurrence(task) {
            let uid = task.uid.clone();
            let href = task.href.clone();
            let etag = task.etag.clone();
            let calendar_href = task.calendar_href.clone();
            let old_seq = task.sequence;

            *task = next;

            task.uid = uid;
            task.href = href;
            task.etag = etag;
            task.calendar_href = calendar_href;
            task.sequence = old_seq + 1;

            return true;
        }
        false
    }
}
