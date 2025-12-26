// File: ./src/system.rs
use crate::model::Task;
use chrono::Utc;
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

    tokio::spawn(async move {
        let mut tasks: Vec<Task> = Vec::new();
        // Track fired alarms to prevent spamming if the UI doesn't acknowledge immediately
        // (Uid -> Timestamp of fire)
        let mut fired_history: HashMap<String, i64> = HashMap::new();

        loop {
            let now = Utc::now();
            let mut next_wake_ts: Option<i64> = None;

            // Clean up history for alarms that no longer exist
            let mut active_alarm_keys = HashSet::new();

            for task in &tasks {
                if task.status.is_done() {
                    continue;
                }

                for alarm in &task.alarms {
                    // Skip if snoozed or acknowledged
                    if alarm.acknowledged.is_some() || alarm.is_snooze() {
                        continue;
                    }

                    let history_key = format!("{}:{}", task.uid, alarm.uid);
                    active_alarm_keys.insert(history_key.clone());

                    // Calculate trigger time
                    let trigger_dt = match alarm.trigger {
                        crate::model::AlarmTrigger::Absolute(dt) => dt,
                        crate::model::AlarmTrigger::Relative(mins) => {
                            // Find anchor (Due or Start)
                            let anchor = if let Some(crate::model::DateType::Specific(d)) = task.due
                            {
                                d
                            } else if let Some(crate::model::DateType::Specific(s)) = task.dtstart {
                                s
                            } else {
                                continue;
                            };
                            anchor + chrono::Duration::minutes(mins as i64)
                        }
                    };

                    let timestamp = trigger_dt.timestamp();

                    // Check if it should fire
                    if timestamp <= now.timestamp() {
                        // Grace period: ignore alarms older than 24h (stale)
                        // Fire only if not in history
                        if (now.timestamp() - timestamp) < 86400
                            && !fired_history.contains_key(&history_key)
                        {
                            // --- FIRE ---
                            fired_history.insert(history_key.clone(), now.timestamp());

                            // 1. Notify UI
                            if let Some(ui_tx) = &ui_sender {
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
                        // Future alarm: Update next_wake
                        match next_wake_ts {
                            None => next_wake_ts = Some(timestamp),
                            Some(t) if timestamp < t => next_wake_ts = Some(timestamp),
                            _ => {}
                        }
                    }
                }
            }

            // Cleanup history
            fired_history.retain(|k, _| active_alarm_keys.contains(k));

            // Wait Logic
            if let Some(target_ts) = next_wake_ts {
                let seconds_until = target_ts - now.timestamp();
                // Ensure we don't pass a negative duration if calculation drifted slightly
                let duration = Duration::from_secs(seconds_until.max(0) as u64);

                // convert duration to Instant for sleep_until
                let deadline = Instant::now() + duration;

                tokio::select! {
                    _ = sleep_until(deadline) => {
                        // Woke up for alarm -> Loop recycles, finds the alarm <= now, and fires it
                    }
                    Some(new_list) = rx.recv() => {
                        // List changed -> Loop recycles, recalculates everything immediately
                        tasks = new_list;
                    }
                }
            } else {
                // No future alarms? Just wait for updates.
                if let Some(new_list) = rx.recv().await {
                    tasks = new_list;
                } else {
                    // Channel closed, exit actor
                    break;
                }
            }
        }
    });

    tx
}
