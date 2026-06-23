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
use std::sync::OnceLock;
use tokio::sync::mpsc;

pub static KEYRING_WARNING: OnceLock<Option<String>> = OnceLock::new();
use tokio::time::{Duration, Instant, sleep_until};

#[derive(Debug, Clone)]
pub enum AlarmMessage {
    Fire(String, String), // TaskUID, AlarmUID
    FocusTask(String),    // TaskUID
    TriggerSync,          // Tells the UI to initiate a background sync
}

// New enum to control the actor
#[derive(Debug, Clone)]
pub enum SystemEvent {
    UpdateTasks(Vec<Task>),
    EnableAlarms,
}

/// Reconfigures the global maximum log level.
/// This can be called after init_logging to change the log level at runtime.
pub fn set_log_level(level: log::LevelFilter) {
    log::set_max_level(level);
}

/// Initializes logging for the application.
///
/// Args:
///   ctx: Application context for determining cache directory
///   enable_stderr: Whether to enable stderr logging (safe for CLI/GUI, unsafe for interactive TUI)
///   level: The log level to use for both file and terminal logging
pub fn init_logging(ctx: &dyn AppContext, enable_stderr: bool, level: Option<log::LevelFilter>) {
    let cache_dir = ctx.get_cache_dir().unwrap_or_else(|_| std::env::temp_dir());
    let log_path = cache_dir.join("cfait.log");
    let old_log_path = cache_dir.join("cfait.old.log");

    // ROTATE LOGS: Preserve the previous session's log in case of a crash
    if log_path.exists() {
        let _ = std::fs::rename(&log_path, &old_log_path);
    }

    // Use Warn as default if no level specified (matches Config default)
    let level = level.unwrap_or(log::LevelFilter::Warn);

    // Silence noisy third-party crates (like iced logging raw icon pixels)
    // Mute the noisy UI crates, but DO NOT mute rustls (needed for TLS debugging)
    let log_config = simplelog::ConfigBuilder::new()
        .add_filter_ignore_str("iced_winit")
        .add_filter_ignore_str("zbus")
        .build();

    // File logger: creates a fresh cfait.log for this session
    let file_logger = WriteLogger::new(
        level,
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
                .with_max_level(level)
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
        log::set_max_level(level);
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
                level,
                log_config.clone(),
                TerminalMode::Stderr,
                ColorChoice::Auto,
            );
            loggers.push(term_logger);
        }

        let _ = CombinedLogger::init(loggers);
        log::set_max_level(level);
        log::info!("Cfait logging initialized. Log file at: {:?}", log_path);
    }
}

pub fn init_keyring() {
    use keyring_core::set_default_store;

    #[cfg(target_os = "windows")]
    {
        if let Ok(store) = windows_native_keyring_store::Store::new() {
            set_default_store(store);
            log::info!("Initialized Windows Credential Manager.");
        }
        let _ = KEYRING_WARNING.set(None);
    }

    #[cfg(target_os = "macos")]
    {
        // MacOS apps not running in a sandbox without an Apple provisioning profile must use the "keychain" store
        // rather than the iOS-style "protected" store.
        if let Ok(store) = apple_native_keyring_store::keychain::Store::new() {
            set_default_store(store);
            log::info!("Initialized macOS Keychain.");
        }
        let _ = KEYRING_WARNING.set(None);
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Probe if we have D-Bus / Portal access
        let oo7_works = block_on_async(async { get_keyring().await.is_ok() });

        // 2. Set the store dynamically based on the environment
        if oo7_works {
            set_default_store(Oo7Store::new());
            log::info!("Initialized Linux Secret Portal (oo7 wrapper).");
            let _ = KEYRING_WARNING.set(None);
        } else if let Ok(store) = linux_keyutils_keyring_store::Store::new() {
            set_default_store(store);
            log::warn!("Initialized Linux Keyutils (headless fallback).");
            let _ = KEYRING_WARNING.set(Some(
                "Warning: Using memory-only kernel keyring. Passwords will be lost on reboot. Install a Secret Service provider (e.g. gnome-keyring, kwallet) to save credentials permanently.".to_string(),
            ));
        } else {
            log::warn!("Failed to initialize any Linux keyring backend.");
            let _ = KEYRING_WARNING.set(Some(
                "Warning: No keyring backend available. Passwords cannot be saved securely."
                    .to_string(),
            ));
        }
    }

    #[cfg(target_os = "android")]
    {
        if let Ok(store) = android_native_keyring_store::Store::new() {
            set_default_store(store);
            log::info!("Initialized Android Native Keystore.");
        }
        let _ = KEYRING_WARNING.set(None);
    }
}

// --- LINUX OO7 WRAPPER & ASYNC HELPER ---

#[cfg(target_os = "linux")]
async fn get_keyring() -> Result<std::sync::Arc<oo7::Keyring>, oo7::Error> {
    // Cache the DBus connection so we only negotiate with the Secret Portal once per launch
    static KEYRING: tokio::sync::OnceCell<std::sync::Arc<oo7::Keyring>> =
        tokio::sync::OnceCell::const_new();
    let keyring = KEYRING
        .get_or_try_init(|| async {
            Ok::<std::sync::Arc<oo7::Keyring>, oo7::Error>(std::sync::Arc::new(
                oo7::Keyring::new().await?,
            ))
        })
        .await?;
    Ok(keyring.clone())
}

#[cfg(target_os = "linux")]
/// Safely runs an async block whether we are currently inside a Tokio runtime or not.
fn block_on_async<F: std::future::Future>(future: F) -> F::Output
where
    F::Output: Send,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We are inside a Tokio runtime (e.g., Iced UI or TUI), safely block in place
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        // No runtime exists yet (e.g., CLI command), spin up a temporary one
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }
}

#[cfg(target_os = "linux")]
struct Oo7Store;

#[cfg(target_os = "linux")]
impl Oo7Store {
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Oo7Store)
    }
}

#[cfg(target_os = "linux")]
impl keyring_core::api::CredentialStoreApi for Oo7Store {
    fn vendor(&self) -> String {
        "oo7 Secret Portal store".to_string()
    }

    fn id(&self) -> String {
        "oo7-portal".to_string()
    }

    fn build(
        &self,
        service: &str,
        user: &str,
        _modifiers: Option<&std::collections::HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        // Create the credential backend for this specific entry
        let cred = Oo7Cred::new(service.to_string(), user.to_string());
        // Return a keyring Entry wrapped around our custom credential API
        Ok(keyring_core::Entry::new_with_credential(cred))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(target_os = "linux")]
struct Oo7Cred {
    service: String,
    user: String,
}

#[cfg(target_os = "linux")]
impl Oo7Cred {
    fn new(service: String, user: String) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Oo7Cred { service, user })
    }
}

#[cfg(target_os = "linux")]
impl keyring_core::api::CredentialApi for Oo7Cred {
    fn set_secret(&self, secret: &[u8]) -> keyring_core::Result<()> {
        block_on_async(async {
            let keyring = get_keyring().await.map_err(|e| {
                keyring_core::Error::PlatformFailure(format!("oo7 init: {}", e).into())
            })?;

            log::info!(
                "Waiting for Linux Secret Portal keyring to unlock... (check for OS prompts)"
            );
            let _ = keyring.unlock().await;
            log::info!(
                "Unlocked Linux Secret Portal keyring for {}/{}",
                self.service,
                self.user
            );

            keyring
                .create_item(
                    &format!("{} ({})", self.service, self.user),
                    &std::collections::HashMap::from([
                        ("service", self.service.as_str()),
                        ("user", self.user.as_str()),
                    ]),
                    secret,
                    true,
                )
                .await
                .map_err(|e| {
                    keyring_core::Error::PlatformFailure(format!("oo7 create: {}", e).into())
                })?;

            Ok(())
        })
    }

    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        block_on_async(async {
            let keyring = get_keyring().await.map_err(|e| {
                keyring_core::Error::PlatformFailure(format!("oo7 init: {}", e).into())
            })?;

            log::info!(
                "Waiting for Linux Secret Portal keyring to unlock... (check for OS prompts)"
            );
            let _ = keyring.unlock().await;
            log::info!(
                "Unlocked Linux Secret Portal keyring for {}/{}",
                self.service,
                self.user
            );

            let items = keyring
                .search_items(&std::collections::HashMap::from([
                    ("service", self.service.as_str()),
                    ("user", self.user.as_str()),
                ]))
                .await
                .map_err(|e| {
                    keyring_core::Error::PlatformFailure(format!("oo7 search: {}", e).into())
                })?;

            if let Some(item) = items.first() {
                let secret = item.secret().await.map_err(|e| {
                    keyring_core::Error::PlatformFailure(format!("oo7 secret: {}", e).into())
                })?;
                Ok(secret.as_bytes().to_vec())
            } else {
                Err(keyring_core::Error::NoEntry)
            }
        })
    }

    fn delete_credential(&self) -> keyring_core::Result<()> {
        block_on_async(async {
            let keyring = get_keyring().await.map_err(|e| {
                keyring_core::Error::PlatformFailure(format!("oo7 init: {}", e).into())
            })?;

            let _ = keyring.unlock().await;

            let items = keyring
                .search_items(&std::collections::HashMap::from([
                    ("service", self.service.as_str()),
                    ("user", self.user.as_str()),
                ]))
                .await
                .map_err(|e| {
                    keyring_core::Error::PlatformFailure(format!("oo7 search: {}", e).into())
                })?;

            for item in items {
                item.delete().await.map_err(|e| {
                    keyring_core::Error::PlatformFailure(format!("oo7 delete: {}", e).into())
                })?;
            }
            Ok(())
        })
    }

    fn get_credential(
        &self,
    ) -> keyring_core::Result<Option<std::sync::Arc<keyring_core::Credential>>> {
        // Return None to signal that we don't have a pre-cached full credential object.
        // The keyring crate will then fall back to calling `get_secret()`.
        Ok(None)
    }

    fn get_specifiers(&self) -> Option<(String, String)> {
        Some((self.service.clone(), self.user.clone()))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
        let mut last_sync_request = Instant::now() - Duration::from_secs(60);

        loop {
            let now = Utc::now();
            let mut next_wake_ts: Option<i64> = None;
            let mut active_alarm_keys = HashSet::new();
            let mut ready_to_fire = Vec::new();

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
                            if (now.timestamp() - timestamp) < 86400
                                && !fired_history.contains_key(&history_key)
                            {
                                ready_to_fire.push((
                                    task.clone(),
                                    alarm.clone(),
                                    is_implicit,
                                    history_key.clone(),
                                ));
                            }
                        } else {
                            match next_wake_ts {
                                None => next_wake_ts = Some(timestamp),
                                Some(t) if timestamp < t => next_wake_ts = Some(timestamp),
                                _ => {}
                            }
                        }
                    }
                }

                fired_history.retain(|k, _| active_alarm_keys.contains(k));
            }

            if !ready_to_fire.is_empty() {
                // Try to sync before firing
                if last_sync_request.elapsed() > Duration::from_secs(15) {
                    if let Some(ui_tx) = &ui_sender {
                        let _ = ui_tx.send(AlarmMessage::TriggerSync).await;
                    }
                    last_sync_request = Instant::now();

                    // Wait up to 3 seconds for an UpdateTasks event
                    let timeout_deadline = Instant::now() + Duration::from_secs(3);
                    tokio::select! {
                        msg = rx.recv() => {
                            match msg {
                                Some(SystemEvent::UpdateTasks(new_list)) => { tasks = new_list; }
                                Some(SystemEvent::EnableAlarms) => { alarms_enabled = true; }
                                None => break,
                            }
                            continue; // Re-evaluate ready_to_fire with updated tasks
                        }
                        _ = sleep_until(timeout_deadline) => {
                            // Timeout reached, proceed to fire
                        }
                    }
                }

                for (task, alarm, is_implicit, history_key) in ready_to_fire {
                    fired_history.insert(history_key.clone(), now.timestamp());

                    if !is_implicit && let Some(ui_tx) = &ui_sender {
                        let _ = ui_tx
                            .send(AlarmMessage::Fire(task.uid.clone(), alarm.uid.clone()))
                            .await;
                    }

                    let summary = task.summary.clone();
                    let body = alarm
                        .description
                        .clone()
                        .unwrap_or_else(|| rust_i18n::t!("reminder").to_string());

                    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
                    let ui_tx_clone = ui_sender.clone();
                    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
                    let task_uid_clone = task.uid.clone();

                    std::thread::spawn(move || {
                        let mut n = Notification::new();
                        n.summary(&summary)
                            .body(&body)
                            .appname("Cfait")
                            .action("default", "Open");

                        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
                        match n.show() {
                            Ok(handle) => {
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
                            Err(e) => log::error!("Failed to show system notification: {}", e),
                        }

                        #[cfg(any(
                            target_os = "windows",
                            target_os = "macos",
                            target_os = "android"
                        ))]
                        {
                            if let Err(e) = n.show() {
                                log::error!("Failed to show system notification: {}", e);
                            }
                        }
                    });
                }
                continue; // Re-evaluate after firing to get next_wake_ts correct
            }

            if let Some(target_ts) = next_wake_ts {
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

                    if seconds_until > 15 {
                        let sync_ts = target_ts - 15;
                        let duration =
                            Duration::from_secs((sync_ts - now.timestamp()).max(0) as u64);
                        let deadline = Instant::now() + duration;

                        tokio::select! {
                            _ = sleep_until(deadline) => {
                                if last_sync_request.elapsed() > Duration::from_secs(15) {
                                    if let Some(ui_tx) = &ui_sender {
                                        let _ = ui_tx.send(AlarmMessage::TriggerSync).await;
                                    }
                                    last_sync_request = Instant::now();
                                }
                            }
                            msg = rx.recv() => {
                                match msg {
                                    Some(SystemEvent::UpdateTasks(new_list)) => { tasks = new_list; }
                                    Some(SystemEvent::EnableAlarms) => { alarms_enabled = true; }
                                    None => break,
                                }
                            }
                        }
                    } else {
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
                }
            } else {
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
