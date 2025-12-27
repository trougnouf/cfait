// File: ./src/system.rs
use crate::config::Config; // Import Config
use crate::model::{Alarm, AlarmTrigger, DateType, Task}; // Import DateType
use chrono::{Local, NaiveTime, Utc}; // Import Time helpers
use notify_rust::Notification;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

#[derive(Debug, Clone)]
pub enum AlarmMessage {
    Fire(String, String), // TaskUID, AlarmUID
}

/// Spawns the background alarm manager.
/// returns: Sender to update the task list.
pub fn spawn_alarm_actor(ui_sender: Option<mpsc::Sender<AlarmMessage>>) -> mpsc::Sender<Vec<Task>> {
    let (tx, mut rx) = mpsc::channel(10);

    // Load config once at startup
    let config = Config::load().unwrap_or_default();

    // Parse default time (e.g., "08:00")
    let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

    tokio::spawn(async move {
        let mut tasks: Vec<Task> = Vec::new();
        let mut fired_history: HashMap<String, i64> = HashMap::new();

        loop {
            let now = Utc::now();
            let mut next_wake_ts: Option<i64> = None;
            let mut active_alarm_keys = HashSet::new();

            for task in &tasks {
                if task.status.is_done() {
                    continue;
                }

                // Collect explicit alarms + Implicit alarms (if enabled and no explicit exist)
                let mut check_list = Vec::new();

                // 1. Explicit Alarms
                for alarm in &task.alarms {
                    // FIX: Removed `&& !alarm.is_snooze()`.
                    // Snoozed alarms are active alarms that need to fire.
                    if alarm.acknowledged.is_none() {
                        check_list.push((alarm.clone(), false));
                    }
                }

                // 2. Implicit Alarms (Auto-Reminders)
                // Only if enabled AND the task doesn't already have an alarm covering this specific moment.
                if config.auto_reminders {
                    // Check Due Date
                    if let Some(due) = &task.due {
                        let trigger_dt = match due {
                            DateType::Specific(dt) => *dt,
                            DateType::AllDay(d) => d
                                .and_time(default_time)
                                .and_local_timezone(Local)
                                .unwrap()
                                .with_timezone(&Utc),
                        };

                        // FIX: Don't fire if ANY alarm (even acknowledged/dismissed) exists for this exact time.
                        // This prevents firing on restart after dismissal.
                        if !task.has_alarm_at(trigger_dt) {
                            // Encode the timestamp into the UID so the UI knows exactly what time to write back
                            let ts_str = trigger_dt.to_rfc3339();
                            let implicit_alarm = Alarm {
                                uid: format!("implicit_due:|{}|{}", ts_str, task.uid),
                                action: "DISPLAY".to_string(),
                                trigger: AlarmTrigger::Absolute(trigger_dt),
                                description: Some("Due now".to_string()),
                                acknowledged: None,
                                related_to_uid: None,
                                relation_type: None,
                            };
                            check_list.push((implicit_alarm, true));
                        }
                    }

                    // Check Start Date (Same logic)
                    if let Some(start) = &task.dtstart {
                        let trigger_dt = match start {
                            DateType::Specific(dt) => *dt,
                            DateType::AllDay(d) => d
                                .and_time(default_time)
                                .and_local_timezone(Local)
                                .unwrap()
                                .with_timezone(&Utc),
                        };

                        if !task.has_alarm_at(trigger_dt) {
                            let ts_str = trigger_dt.to_rfc3339();
                            let implicit_alarm = Alarm {
                                uid: format!("implicit_start:|{}|{}", ts_str, task.uid),
                                action: "DISPLAY".to_string(),
                                trigger: AlarmTrigger::Absolute(trigger_dt),
                                description: Some("Task starting".to_string()),
                                acknowledged: None,
                                related_to_uid: None,
                                relation_type: None,
                            };
                            check_list.push((implicit_alarm, true));
                        }
                    }
                }

                // Process collected alarms
                for (alarm, is_implicit) in check_list {
                    // Use synthetic UID for implicit history key
                    let history_key = if is_implicit {
                        alarm.uid.clone()
                    } else {
                        format!("{}:{}", task.uid, alarm.uid)
                    };

                    active_alarm_keys.insert(history_key.clone());

                    let trigger_dt = match alarm.trigger {
                        AlarmTrigger::Absolute(dt) => dt,
                        AlarmTrigger::Relative(mins) => {
                            // ... existing relative logic ...
                            let anchor = if let Some(DateType::Specific(d)) = task.due {
                                d
                            } else if let Some(DateType::Specific(s)) = task.dtstart {
                                s
                            } else {
                                continue;
                            };
                            anchor + chrono::Duration::minutes(mins as i64)
                        }
                    };

                    let timestamp = trigger_dt.timestamp();

                    if timestamp <= now.timestamp() {
                        // Check history
                        // Grace period 24h
                        if (now.timestamp() - timestamp) < 86400
                            && !fired_history.contains_key(&history_key)
                        {
                            fired_history.insert(history_key.clone(), now.timestamp());

                            // Notify UI (For implicit, we send task_uid and a special marker or just empty alarm_uid if UI supports it,
                            // OR we create a real runtime alarm object to pass.
                            // Since AlarmMessage takes (TaskUID, AlarmUID), and the UI looks up the alarm in the Task struct,
                            // we have a small issue: Implicit alarms aren't in the Task struct.
                            //
                            // SOLUTION: We fire the OS notification, but we might skip the UI Modal for implicit
                            // OR we push this synthetic alarm into the task copy held by the actor?
                            // For now, let's just trigger the OS notification for implicit alarms.
                            // To support UI Modals for implicit, the Task model would need 'runtime_alarms'.

                            // 1. Notify UI (Skip for implicit to avoid lookup failure crash in UI)
                            if !is_implicit && let Some(ui_tx) = &ui_sender {
                                let _ = ui_tx
                                    .send(AlarmMessage::Fire(task.uid.clone(), alarm.uid.clone()))
                                    .await;
                            }

                            // 2. OS Notification
                            let summary = task.summary.clone();
                            let body = alarm
                                .description
                                .clone()
                                .unwrap_or_else(|| "Reminder".to_string());

                            std::thread::spawn(move || {
                                let _ = Notification::new()
                                    .summary(&summary)
                                    .body(&body)
                                    .appname("Cfait")
                                    .show();
                            });
                        }
                    } else {
                        // Future alarm
                        match next_wake_ts {
                            None => next_wake_ts = Some(timestamp),
                            Some(t) if timestamp < t => next_wake_ts = Some(timestamp),
                            _ => {}
                        }
                    }
                }
            }

            // ... rest of loop (cleanup history, wait logic) ...
            fired_history.retain(|k, _| active_alarm_keys.contains(k));

            // ... existing sleep logic ...
            if let Some(target_ts) = next_wake_ts {
                let seconds_until = target_ts - now.timestamp();
                let duration = Duration::from_secs(seconds_until.max(0) as u64);
                let deadline = Instant::now() + duration;

                tokio::select! {
                    _ = sleep_until(deadline) => {}
                    Some(new_list) = rx.recv() => { tasks = new_list; }
                }
            } else if let Some(new_list) = rx.recv().await {
                tasks = new_list;
            } else {
                break;
            }
        }
    });

    tx
}
