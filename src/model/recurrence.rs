// File: ./src/model/recurrence.rs
use crate::model::item::{Alarm, AlarmTrigger, DateType, RawProperty, Task, TaskStatus};
use chrono::{Datelike, Local, NaiveDate};
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

        // 1. Establish the Local Naive seed for the recurrence engine.
        // For AllDay tasks, we use midnight (or a specific time if one exists on the other date field).
        let seed_local_naive = match seed_date_type {
            DateType::AllDay(d) => {
                let local_time = match (task.dtstart.as_ref(), task.due.as_ref()) {
                    (Some(DateType::Specific(dt)), _) => Some(dt.with_timezone(&Local).time()),
                    (_, Some(DateType::Specific(dt))) => Some(dt.with_timezone(&Local).time()),
                    _ => None,
                };
                if let Some(time) = local_time {
                    d.and_time(time)
                } else {
                    d.and_hms_opt(0, 0, 0).unwrap()
                }
            }
            DateType::Specific(dt) => dt.with_timezone(&Local).naive_local(),
            DateType::Month(y, m) => {
                let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                d.and_hms_opt(0, 0, 0).unwrap()
            }
            DateType::Year(y) => {
                let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                d.and_hms_opt(0, 0, 0).unwrap()
            }
        };

        // Construct RRULE string using Floating Time (No 'Z' suffix)
        let dtstart_str = seed_local_naive.format("%Y%m%dT%H%M%S").to_string();

        let clean_rule = rule_str.trim();
        let mut final_rule_part = if clean_rule.to_uppercase().starts_with("RRULE:") {
            clean_rule[6..].to_string()
        } else {
            clean_rule.to_string()
        };

        // Ensure UNTIL is also stripped of 'Z' so it matches the floating DTSTART
        if let Some(idx) = final_rule_part.find("UNTIL=") {
            let until_val_start = idx + 6;
            let until_val_end = final_rule_part[until_val_start..]
                .find(';')
                .map(|i| until_val_start + i)
                .unwrap_or(final_rule_part.len());

            let until_val = final_rule_part[until_val_start..until_val_end].to_string();

            if until_val.len() == 8 && !until_val.contains('T') {
                let new_until = format!("{}T235959", until_val); // NO 'Z'
                final_rule_part.replace_range(until_val_start..until_val_end, &new_until);
            } else if until_val.ends_with('Z') {
                let new_until = &until_val[..until_val.len() - 1]; // Strip the 'Z'
                final_rule_part.replace_range(until_val_start..until_val_end, new_until);
            }
        }

        let mut rrule_string = format!("DTSTART:{}\nRRULE:{}\n", dtstart_str, final_rule_part);

        // Deduplicate and localize EXDATEs
        if !task.exdates.is_empty() {
            let mut seen_exdates = HashSet::new();
            for ex in &task.exdates {
                let ex_local_naive = match ex {
                    // Ensure AllDay EXDATEs match the exact time rrule uses to generate instances
                    DateType::AllDay(d) => d.and_time(seed_local_naive.time()),
                    DateType::Specific(dt) => dt.with_timezone(&Local).naive_local(),
                    DateType::Month(y, m) => {
                        let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                        d.and_time(seed_local_naive.time())
                    }
                    DateType::Year(y) => {
                        let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                        d.and_time(seed_local_naive.time())
                    }
                };
                let ex_str = ex_local_naive.format("%Y%m%dT%H%M%S").to_string(); // NO 'Z'

                if seen_exdates.insert(ex_str.clone()) {
                    rrule_string.push_str(&format!("EXDATE:{}\n", ex_str));
                }
            }
        }

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            let comparison_now_local = match seed_date_type {
                DateType::AllDay(_) => Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap(),
                DateType::Specific(_) => Local::now().naive_local(),
                DateType::Month(_, _) => Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap(),
                DateType::Year(_) => Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap(),
            };

            let search_floor_local = std::cmp::max(comparison_now_local, seed_local_naive);

            let rrule_next_naive = rrule_set
                .into_iter()
                .map(|d| d.naive_local())
                .find(|d| *d > search_floor_local);

            let is_simple_monthly =
                final_rule_part.contains("FREQ=MONTHLY") && !final_rule_part.contains("BY");
            let is_simple_yearly =
                final_rule_part.contains("FREQ=YEARLY") && !final_rule_part.contains("BY");

            let next_occurrence_naive = if let Some(rn) = rrule_next_naive {
                if is_simple_monthly || is_simple_yearly {
                    let interval = if let Some(idx) = final_rule_part.find("INTERVAL=") {
                        let end = final_rule_part[idx..]
                            .find(';')
                            .map(|i| idx + i)
                            .unwrap_or(final_rule_part.len());
                        final_rule_part[idx + 9..end].parse::<u32>().unwrap_or(1)
                    } else {
                        1
                    };

                    let expected_months = if is_simple_yearly {
                        interval * 12
                    } else {
                        interval
                    };

                    if let Some(expected_date) =
                        seed_local_naive.checked_add_months(chrono::Months::new(expected_months))
                    {
                        if rn > expected_date && expected_date > search_floor_local {
                            Some(expected_date)
                        } else {
                            Some(rn)
                        }
                    } else {
                        Some(rn)
                    }
                } else {
                    Some(rn)
                }
            } else {
                None
            };

            if let Some(naive_next) = next_occurrence_naive {
                // Convert back to Absolute UTC for safe storage
                let next_start =
                    crate::model::item::safe_local_to_utc(naive_next.date(), naive_next.time());

                let mut next_task = task.clone();
                next_task.uid = Uuid::new_v4().to_string();
                next_task.href = String::new();
                next_task.etag = String::new();
                next_task.status = TaskStatus::NeedsAction;
                next_task.percent_complete = None;
                next_task.dependencies.clear();
                next_task.sequence = 0;

                next_task
                    .unmapped_properties
                    .retain(|p| p.key != "COMPLETED");

                next_task
                    .alarms
                    .retain(|a: &Alarm| !a.is_snooze() && a.acknowledged.is_none());

                // Advance explicit absolute alarms along with the task
                // By applying the delta in Local Naive time, we preserve the wall-clock hour
                // perfectly across Daylight Saving Time boundaries!
                let naive_delta = naive_next - seed_local_naive;

                for alarm in &mut next_task.alarms {
                    if let AlarmTrigger::Absolute(ref mut dt) = alarm.trigger {
                        let alarm_local = dt.with_timezone(&Local).naive_local();
                        let new_alarm_local = alarm_local + naive_delta;
                        *dt = crate::model::item::safe_local_to_utc(
                            new_alarm_local.date(),
                            new_alarm_local.time(),
                        );
                    }
                }

                next_task.exdates.sort_by(|a, b| a.partial_cmp(b).unwrap());
                next_task.exdates.dedup();

                // Calculate duration gap in Naive time to prevent DST shifts from stretching the gap
                let duration_naive = if let Some(old_due) = &task.due {
                    match old_due {
                        DateType::AllDay(d) => {
                            let due_naive = d.and_hms_opt(0, 0, 0).unwrap();
                            due_naive - seed_local_naive
                        }
                        DateType::Specific(dt) => {
                            let due_naive = dt.with_timezone(&Local).naive_local();
                            due_naive - seed_local_naive
                        }
                        DateType::Month(y, m) => {
                            let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                            let due_naive = d.and_hms_opt(0, 0, 0).unwrap();
                            due_naive - seed_local_naive
                        }
                        DateType::Year(y) => {
                            let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                            let due_naive = d.and_hms_opt(0, 0, 0).unwrap();
                            due_naive - seed_local_naive
                        }
                    }
                } else {
                    chrono::Duration::zero()
                };

                if let Some(old_start) = &task.dtstart {
                    next_task.dtstart = match old_start {
                        DateType::AllDay(_) => Some(DateType::AllDay(naive_next.date())),
                        DateType::Specific(_) => Some(DateType::Specific(next_start)),
                        DateType::Month(_, _) => {
                            Some(DateType::Month(naive_next.year(), naive_next.month()))
                        }
                        DateType::Year(_) => Some(DateType::Year(naive_next.year())),
                    };
                }

                if let Some(old_due) = &task.due {
                    let next_due_naive = naive_next + duration_naive;
                    let next_due_utc = crate::model::item::safe_local_to_utc(
                        next_due_naive.date(),
                        next_due_naive.time(),
                    );
                    next_task.due = match old_due {
                        DateType::AllDay(_) => Some(DateType::AllDay(next_due_naive.date())),
                        DateType::Specific(_) => Some(DateType::Specific(next_due_utc)),
                        DateType::Month(_, _) => Some(DateType::Month(
                            next_due_naive.year(),
                            next_due_naive.month(),
                        )),
                        DateType::Year(_) => Some(DateType::Year(next_due_naive.year())),
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
