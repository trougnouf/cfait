// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/recurrence.rs
use crate::model::item::{Alarm, AlarmTrigger, DateType, RawProperty, Task, TaskStatus};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rrule::RRuleSet;
use std::collections::HashSet; // Import HashSet for deduplication
use std::str::FromStr;
use uuid::Uuid;

pub struct RecurrenceEngine;

impl RecurrenceEngine {
    /// Calculates the next occurrence of a task based on its RRULE.
    /// Returns a new Task instance for the next occurrence, or None if finished.
    pub fn next_occurrence(task: &Task) -> Option<Task> {
        Self::next_occurrence_after(task, Local::now())
    }

    /// The local-naive seed date for the recurrence engine: `DTSTART` if
    /// present, else `DUE`. AllDay seeds use midnight, or the time of a
    /// specific sibling date field when one exists.
    fn seed_local_naive(task: &Task) -> Option<NaiveDateTime> {
        let seed = task.dtstart.as_ref().or(task.due.as_ref())?;
        Some(match seed {
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
            DateType::Month(y, m) => NaiveDate::from_ymd_opt(*y, *m, 1)?.and_hms_opt(0, 0, 0)?,
            DateType::Year(y) => NaiveDate::from_ymd_opt(*y, 1, 1)?.and_hms_opt(0, 0, 0)?,
        })
    }

    /// Calculates the next occurrence of a task strictly after `now`, based on
    /// its RRULE. Returns a new Task instance for the next occurrence, or None
    /// if finished. `next_occurrence` is the same call with `now = Local::now()`.
    pub fn next_occurrence_after(task: &Task, now: DateTime<Local>) -> Option<Task> {
        let rule_str = task.rrule.as_ref()?;
        let seed_date_type = task.dtstart.as_ref().or(task.due.as_ref())?;
        let seed_local_naive = Self::seed_local_naive(task)?;

        // Construct RRULE string using Floating Time (No 'Z' suffix)
        let dtstart_str = seed_local_naive.format("%Y%m%dT%H%M%S").to_string();

        let clean_rule = rule_str.trim();
        let mut final_rule_part = if clean_rule.to_uppercase().starts_with("RRULE:") {
            clean_rule[6..].to_string()
        } else {
            clean_rule.to_string()
        };

        // Ensure UNTIL is strictly floating time (No 'Z') to match our local-naive DTSTART.
        // Rebuild the RRULE string cleanly to avoid leaving dangling TZID parameters.
        let parts: Vec<&str> = final_rule_part.split(';').collect();
        let mut new_parts = Vec::new();

        for part in parts {
            if let Some(until_val) = part.strip_prefix("UNTIL=") {
                if until_val.len() == 8 && !until_val.contains('T') {
                    new_parts.push(format!("UNTIL={}T235959", until_val));
                } else if let Some(stripped) = until_val.strip_suffix('Z') {
                    new_parts.push(format!("UNTIL={}", stripped));
                } else {
                    new_parts.push(part.to_string());
                }
            } else {
                new_parts.push(part.to_string());
            }
        }
        final_rule_part = new_parts.join(";");

        let rrule_string = format!("DTSTART:{}\nRRULE:{}\n", dtstart_str, final_rule_part);

        if let Ok(rrule_set) = RRuleSet::from_str(&rrule_string) {
            let comparison_now_local = match seed_date_type {
                DateType::AllDay(_) => now.date_naive().and_hms_opt(0, 0, 0).unwrap(),
                DateType::Specific(_) => now.naive_local(),
                DateType::Month(_, _) => now.date_naive().and_hms_opt(0, 0, 0).unwrap(),
                DateType::Year(_) => now.date_naive().and_hms_opt(0, 0, 0).unwrap(),
            };

            let search_floor_local = std::cmp::max(comparison_now_local, seed_local_naive);

            // Deduplicate and localize EXDATEs for manual filtering to completely
            // bypass the rrule crate's notoriously strict validation which crashes on
            // timezone/DST mismatches or past exceptions.
            let mut exclusion_dates = HashSet::new();
            if !task.exdates.is_empty() {
                for ex in &task.exdates {
                    exclusion_dates.insert(ex.to_date_naive());
                }
            }

            let rrule_next_naive = rrule_set
                .into_iter()
                .map(|d| d.naive_local())
                .find(|d| *d > search_floor_local && !exclusion_dates.contains(&d.date()));

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

                next_task.exdates.sort();
                next_task.exdates.dedup();

                // Keep all EXDATEs to preserve cancellation history for calendar event synchronization
                // This allows other clients to see which instances were cancelled

                // Calculate duration gap in Naive time to prevent DST shifts from stretching the gap
                let duration_naive = if let Some(old_due) = &task.due {
                    let due_naive = match old_due {
                        DateType::Specific(dt) => dt.with_timezone(&Local).naive_local(),
                        _ => old_due.to_date_naive().and_hms_opt(0, 0, 0).unwrap(),
                    };
                    due_naive - seed_local_naive
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

    /// The due date to display and schedule for a task.
    ///
    /// A recurring task whose stored due date is already in the past is
    /// projected forward to its next RRULE occurrence, so it surfaces as an
    /// upcoming item rather than a stale overdue one. Non-recurring tasks, and
    /// recurring tasks whose stored due date is still in the future, return
    /// their stored due date unchanged.
    ///
    /// This is a display-only projection: the stored `due` is never mutated,
    /// so the recycling model and calendar sync are preserved.
    pub fn projected_due(task: &Task, now: DateTime<Utc>) -> Option<DateType> {
        let due = task.due.as_ref()?;
        if task.rrule.is_none() || due.to_comparison_time() >= now {
            return Some(due.clone());
        }
        Self::next_occurrence(task)
            .and_then(|next| next.due)
            .or_else(|| Some(due.clone()))
    }

    /// Returns true if `dt` is a due moment of the task — the moment its
    /// "Due now" reminder is supposed to fire.
    ///
    /// This validates a ringing alarm. A projected occurrence's reminder
    /// fires at a moment when `projected_due` has already advanced to the
    /// following occurrence (the projection only ever looks strictly ahead of
    /// now), so comparing the ringing alarm against the current projection
    /// would kill it the instant it rings. Instead, `dt` is accepted when it
    /// matches the stored due date, or — for recurring tasks — when it is the
    /// due moment of any `RRULE` occurrence, with EXDATEs and UNTIL respected
    /// exactly as in `next_occurrence_after`.
    pub fn is_due_moment(task: &Task, dt: DateTime<Utc>, default_time: NaiveTime) -> bool {
        let Some(due) = task.due.as_ref() else {
            return false;
        };
        if due.to_utc_with_default_time(default_time) == dt {
            return true;
        }
        if task.rrule.is_none() {
            return false;
        }
        let Some(seed_local_naive) = Self::seed_local_naive(task) else {
            return false;
        };
        let dt_naive = dt.with_timezone(&Local).naive_local();
        // Reconstruct the occurrence the reminder was projected onto.
        let candidate = match due {
            // Specific due: the reminder sits `due - seed` after the occurrence.
            DateType::Specific(d) => {
                let due_naive = d.with_timezone(&Local).naive_local();
                dt_naive - (due_naive - seed_local_naive)
            }
            // AllDay/Month/Year due: the reminder fires at `default_time` on
            // the due's date, where the occurrence sits at the seed's
            // wall-clock time.
            _ => dt_naive.date().and_time(seed_local_naive.time()),
        };
        if candidate <= seed_local_naive {
            return false;
        }
        // The first occurrence strictly after `candidate - 1s` must be exactly
        // `candidate` (the 1s backoff stays sound even for SECONDLY rules).
        let Some(probe) = (candidate - chrono::Duration::seconds(1))
            .and_local_timezone(Local)
            .earliest()
        else {
            return false;
        };
        let Some(next) = Self::next_occurrence_after(task, probe) else {
            return false;
        };
        next.due
            .as_ref()
            .map(|d| d.to_utc_with_default_time(default_time))
            == Some(dt)
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
            let sequence = task.sequence;
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

            // Increment from the original sequence so the recycled task stays
            // "newer" than every previous version (stale-read protection,
            // cross-calendar dedup, and sync conflict keys all compare it).
            task.sequence = sequence + 1;
            return true;
        }
        false
    }
}
