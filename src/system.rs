// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/system.rs
// Background system actor for handling alarms and notifications.
use crate::config::Config; // Import Config
use crate::context::{AppContext, StandardContext}; // Import AppContext trait
use crate::model::{Alarm, AlarmTrigger, DateType, Task}; // Import DateType
use chrono::{NaiveTime, Utc}; // Import Time helpers
use notify_rust::Notification;
use simplelog::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

#[derive(Debug, Clone)]
pub enum AlarmMessage {
    Fire(String, String), // TaskUID, AlarmUID
    FocusTask(String),    // TaskUID
}

// New enum to control the actor
#[derive(Debug, Clone)]
pub enum SystemEvent {
    UpdateTasks(Vec<Task>),
    EnableAlarms,
}

/// Initializes logging for the application.
///
/// Args:
///   ctx: Application context for determining cache directory
///   enable_stderr: Whether to enable stderr logging (safe for CLI/GUI, unsafe for interactive TUI)
pub fn init_logging(ctx: &dyn AppContext, enable_stderr: bool) {
    let cache_dir = ctx.get_cache_dir().unwrap_or_else(|_| std::env::temp_dir());
    let log_path = cache_dir.join("cfait.log");
    let old_log_path = cache_dir.join("cfait.old.log");

    // ROTATE LOGS: Preserve the previous session's log in case of a crash
    if log_path.exists() {
        let _ = std::fs::rename(&log_path, &old_log_path);
    }

    // Silence noisy third-party crates (like iced logging raw icon pixels)
    // Mute the noisy UI crates, but DO NOT mute rustls (needed for TLS debugging)
    let log_config = simplelog::ConfigBuilder::new()
        .add_filter_ignore_str("iced")
        .add_filter_ignore_str("winit")
        .add_filter_ignore_str("iced_winit")
        .add_filter_ignore_str("iced_wgpu")
        .add_filter_ignore_str("wgpu_core")
        .add_filter_ignore_str("wgpu_hal")
        .add_filter_ignore_str("calloop")
        .build();

    // File logger: creates a fresh cfait.log for this session
    let file_logger = WriteLogger::new(
        LevelFilter::Info, // Change to Debug if you want verbose file logs
        log_config.clone(),
        File::create(&log_path).expect("Failed to create log file"),
    );

    #[cfg(target_os = "android")]
    {
        // On Android, we create a custom logger that splits output between
        // the log file and Android's native Logcat.
        let _enable_stderr = enable_stderr;
        let android_logger = android_logger::AndroidLogger::new(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("CfaitRust"),
        );

        struct DualLogger {
            file: Box<dyn log::Log>,
            android: android_logger::AndroidLogger,
        }

        impl log::Log for DualLogger {
            fn enabled(&self, metadata: &log::Metadata) -> bool {
                self.file.enabled(metadata) || self.android.enabled(metadata)
            }

            fn log(&self, record: &log::Record) {
                if self.file.enabled(record.metadata()) {
                    self.file.log(record);
                }
                if self.android.enabled(record.metadata()) {
                    self.android.log(record);
                }
            }

            fn flush(&self) {
                self.file.flush();
                self.android.flush();
            }
        }

        let logger = DualLogger {
            file: file_logger,
            android: android_logger,
        };

        let _ = log::set_boxed_logger(Box::new(logger));
        log::set_max_level(log::LevelFilter::Debug);
        log::info!(
            "Cfait logging initialized on Android. Log file at: {:?}",
            log_path
        );
    }

    #[cfg(not(target_os = "android"))]
    {
        let mut loggers: Vec<Box<dyn SharedLogger>> = vec![file_logger];

        // Terminal logger: only enabled when safe to do so (GUI / CLI)
        if enable_stderr {
            let term_logger = TermLogger::new(
                LevelFilter::Warn,
                log_config.clone(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            );
            loggers.push(term_logger);
        }

        let _ = CombinedLogger::init(loggers);
        log::info!("Cfait logging initialized. Log file at: {:?}", log_path);
    }
}

/// Spawns the background alarm manager.
/// returns: Sender to update the task list or change state.
pub fn spawn_alarm_actor(
    ui_sender: Option<mpsc::Sender<AlarmMessage>>,
) -> mpsc::Sender<SystemEvent> {
    let (tx, mut rx) = mpsc::channel(100);

    // Load config once at startup using a fresh standard context (no global state)
    let ctx = StandardContext::new(None);
    let config = Config::load(&ctx).unwrap_or_default();

    // Parse default time (e.g., "08:00")
    let default_time = NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

    tokio::spawn(async move {
        let mut tasks: Vec<Task> = Vec::new();
        let mut fired_history: HashMap<String, i64> = HashMap::new();
        // Start muted
        let mut alarms_enabled = false;

        loop {
            let now = Utc::now();
            let mut next_wake_ts: Option<i64> = None;
            let mut active_alarm_keys = HashSet::new();

            // Only check for alarms if enabled
            if alarms_enabled {
                for task in &tasks {
                    if task.status.is_done() || task.status == crate::model::TaskStatus::InProcess {
                        continue;
                    }
                    if task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                        || task.calendar_href == "local://recovery"
                    {
                        continue;
                    }

                    // Collect explicit alarms + Implicit alarms (if enabled and no explicit exist)
                    let mut check_list = Vec::new();

                    // 1. Explicit Alarms
                    for alarm in &task.alarms {
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
                            let trigger_dt = due.to_utc_with_default_time(default_time);

                            // Don't fire if ANY alarm (even acknowledged/dismissed) exists for this exact time.
                            // This prevents firing on restart after dismissal.
                            if !task.has_alarm_at(trigger_dt) {
                                // Encode the timestamp into the UID so the UI knows exactly what time to write back
                                let ts_str = trigger_dt.to_rfc3339();
                                let implicit_alarm = Alarm {
                                    uid: format!("implicit_due:|{}|{}", ts_str, task.uid),
                                    action: "DISPLAY".to_string(),
                                    trigger: AlarmTrigger::Absolute(trigger_dt),
                                    description: Some(rust_i18n::t!("alarm_due_now").to_string()),
                                    acknowledged: None,
                                    related_to_uid: None,
                                    relation_type: None,
                                };
                                check_list.push((implicit_alarm, true));
                            }
                        }

                        // Check Start Date (Same logic)
                        if let Some(start) = &task.dtstart {
                            let trigger_dt = start.to_utc_with_default_time(default_time);

                            if !task.has_alarm_at(trigger_dt) {
                                let ts_str = trigger_dt.to_rfc3339();
                                let implicit_alarm = Alarm {
                                    uid: format!("implicit_start:|{}|{}", ts_str, task.uid),
                                    action: "DISPLAY".to_string(),
                                    trigger: AlarmTrigger::Absolute(trigger_dt),
                                    description: Some(
                                        rust_i18n::t!("alarm_task_starting").to_string(),
                                    ),
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

                                // 1. Notify UI (Skip for implicit to avoid lookup failure crash in UI)
                                if !is_implicit && let Some(ui_tx) = &ui_sender {
                                    let _ = ui_tx
                                        .send(AlarmMessage::Fire(
                                            task.uid.clone(),
                                            alarm.uid.clone(),
                                        ))
                                        .await;
                                }

                                // 2. OS Notification
                                let summary = task.summary.clone();
                                let body = alarm
                                    .description
                                    .clone()
                                    .unwrap_or_else(|| rust_i18n::t!("reminder").to_string());

                                #[cfg(all(
                                    unix,
                                    not(target_os = "macos"),
                                    not(target_os = "android")
                                ))]
                                let ui_tx_clone = ui_sender.clone();

                                #[cfg(all(
                                    unix,
                                    not(target_os = "macos"),
                                    not(target_os = "android")
                                ))]
                                let task_uid_clone = task.uid.clone();

                                std::thread::spawn(move || {
                                    let mut n = Notification::new();
                                    n.summary(&summary)
                                        .body(&body)
                                        .appname("Cfait")
                                        .action("default", "Open");

                                    // On Linux/BSD, we get a handle and can wait for actions.
                                    #[cfg(all(
                                        unix,
                                        not(target_os = "macos"),
                                        not(target_os = "android")
                                    ))]
                                    if let Ok(handle) = n.show() {
                                        handle.wait_for_action(move |action| {
                                            if action == "default"
                                                && let Some(tx) = &ui_tx_clone
                                            {
                                                let _ = tx.try_send(AlarmMessage::FocusTask(
                                                    task_uid_clone.clone(),
                                                ));
                                            }
                                        });
                                    }

                                    // On windows, macos, and android we can't wait for actions.
                                    // The notification will still appear, but clicking it will not
                                    // focus the app window via this code path.
                                    #[cfg(any(
                                        target_os = "windows",
                                        target_os = "macos",
                                        target_os = "android"
                                    ))]
                                    {
                                        let _ = n.show();
                                    }
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

                // Cleanup history
                fired_history.retain(|k, _| active_alarm_keys.contains(k));
            }

            // Wait logic
            if let Some(target_ts) = next_wake_ts {
                // If disabled, we don't sleep until a deadline, we just wait for a message
                // but if enabled, we sleep until next alarm OR message
                if !alarms_enabled {
                    match rx.recv().await {
                        Some(SystemEvent::UpdateTasks(new_list)) => {
                            tasks = new_list;
                        }
                        Some(SystemEvent::EnableAlarms) => {
                            alarms_enabled = true;
                        }
                        None => break,
                    }
                } else {
                    let seconds_until = target_ts - now.timestamp();
                    let duration = Duration::from_secs(seconds_until.max(0) as u64);
                    let deadline = Instant::now() + duration;

                    tokio::select! {
                        _ = sleep_until(deadline) => {}
                        msg = rx.recv() => {
                            match msg {
                                Some(SystemEvent::UpdateTasks(new_list)) => { tasks = new_list; }
                                Some(SystemEvent::EnableAlarms) => { alarms_enabled = true; }
                                None => break,
                            }
                        }
                    }
                }
            } else {
                // No future alarms or disabled, just wait for message
                match rx.recv().await {
                    Some(SystemEvent::UpdateTasks(new_list)) => {
                        tasks = new_list;
                    }
                    Some(SystemEvent::EnableAlarms) => {
                        alarms_enabled = true;
                    }
                    None => break,
                }
            }
        }
    });

    tx
}
