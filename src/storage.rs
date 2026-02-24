/* File: cfait/src/storage.rs
 *
 * Manages local file storage for tasks and calendars.
 *
 * Refactored to require an explicit `AppContext` for all filesystem operations.
 * This removes hidden global state and makes the module testable and re-entrant.
 */
use crate::context::AppContext;
use crate::model::{CalendarListEntry, IcsAdapter, Task};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[cfg(not(target_os = "android"))]
use fs2::FileExt;
#[cfg(target_os = "android")]
use std::sync::Arc;

pub const LOCAL_CALENDAR_HREF: &str = "local://default";
pub const LOCAL_CALENDAR_NAME: &str = "Local";
pub const LOCAL_TRASH_HREF: &str = "local://trash";
pub const LOCAL_REGISTRY_FILENAME: &str = "local_calendars.json";
const LOCAL_STORAGE_VERSION: u32 = 4;

#[derive(Serialize, Deserialize)]
struct LocalStorageData {
    #[serde(default)]
    version: u32,
    tasks: Vec<Task>,
}

#[cfg(target_os = "android")]
static ANDROID_FILE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
static LOAD_STATE_MAP: OnceLock<Mutex<HashMap<String, LoadState>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadState {
    Uninitialized,
    Success,
    Failed,
}

impl LoadState {
    fn get(href: &str) -> LoadState {
        let map = LOAD_STATE_MAP.get_or_init(|| Mutex::new(HashMap::new()));
        *map.lock()
            .unwrap()
            .get(href)
            .unwrap_or(&LoadState::Uninitialized)
    }

    fn set(href: &str, state: LoadState) {
        let map = LOAD_STATE_MAP.get_or_init(|| Mutex::new(HashMap::new()));
        map.lock().unwrap().insert(href.to_string(), state);
    }
}

pub struct LocalCalendarRegistry;

impl LocalCalendarRegistry {
    fn get_path(ctx: &dyn AppContext) -> Option<PathBuf> {
        ctx.get_data_dir()
            .ok()
            .map(|p| p.join(LOCAL_REGISTRY_FILENAME))
    }

    /// Load all local calendars from the registry using an explicit context.
    pub fn load(ctx: &dyn AppContext) -> Result<Vec<CalendarListEntry>> {
        let mut cals = vec![];

        let default_cal = CalendarListEntry {
            name: LOCAL_CALENDAR_NAME.to_string(),
            href: LOCAL_CALENDAR_HREF.to_string(),
            color: None,
        };

        if let Some(path) = Self::get_path(ctx)
            && path.exists()
            && let Ok(content) = LocalStorage::with_lock(&path, || Ok(fs::read_to_string(&path)?))
            && let Ok(registry) = serde_json::from_str::<Vec<CalendarListEntry>>(&content)
        {
            cals = registry;
        }

        if !cals.iter().any(|c| c.href == LOCAL_CALENDAR_HREF) {
            cals.insert(0, default_cal);
        }

        Ok(cals)
    }

    /// Save all local calendars to the registry using an explicit context.
    pub fn save(ctx: &dyn AppContext, calendars: &[CalendarListEntry]) -> Result<()> {
        if let Some(path) = Self::get_path(ctx) {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string_pretty(calendars)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Ensures the "Trash" calendar exists in the registry.
    /// Returns true if it was created, false if it already existed.
    pub fn ensure_trash_calendar_exists(ctx: &dyn AppContext) -> Result<bool> {
        let mut locals = Self::load(ctx)?;
        if !locals.iter().any(|c| c.href == LOCAL_TRASH_HREF) {
            locals.push(CalendarListEntry {
                name: "Trash".to_string(),
                href: LOCAL_TRASH_HREF.to_string(),
                // Use a distinctive color (Gray)
                color: Some("#808080".to_string()),
            });
            Self::save(ctx, &locals)?;
            return Ok(true);
        }
        Ok(false)
    }
}

pub struct LocalStorage;

impl LocalStorage {
    pub fn get_path_for_href(ctx: &dyn AppContext, href: &str) -> Option<PathBuf> {
        if href == LOCAL_CALENDAR_HREF {
            return ctx.get_local_task_path();
        } else if href.starts_with("local://") {
            let id = href.trim_start_matches("local://");
            let safe_id: String = id
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect();
            return ctx
                .get_data_dir()
                .ok()
                .map(|p| p.join(format!("local_{}.json", safe_id)));
        }
        None
    }

    /// Imports tasks from an ICS string and merges them into the specified calendar.
    /// Returns the number of tasks successfully imported.
    pub fn import_from_ics(
        ctx: &dyn AppContext,
        calendar_href: &str,
        ics_content: &str,
    ) -> Result<usize> {
        let mut imported_tasks = Vec::new();
        // Normalize line endings to \r\n for consistent parsing
        let normalized_content = ics_content.replace("\r\n", "\n").replace('\n', "\r\n");

        // Split by VTODO blocks and parse each
        let parts: Vec<&str> = normalized_content.split("BEGIN:VTODO").collect();

        for component in parts.iter().skip(1) {
            if !component.contains("END:VTODO") {
                continue;
            }

            // Extract just the VTODO content (everything up to and including END:VTODO)
            let vtodo_end = match component.find("END:VTODO") {
                Some(pos) => pos + "END:VTODO".len(),
                None => continue,
            };
            let vtodo_content = &component[..vtodo_end];

            // Reconstruct a valid VTODO block
            let vtodo = format!("BEGIN:VTODO{}", vtodo_content);

            // Always wrap in a proper VCALENDAR for parsing
            let full_ics = format!(
                "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//cfait//cfait//EN\r\n{}\r\nEND:VCALENDAR",
                vtodo
            );
            if let Ok(task) = IcsAdapter::from_ics(
                &full_ics,
                String::new(),
                format!("{}.ics", uuid::Uuid::new_v4()),
                calendar_href.to_string(),
            ) {
                imported_tasks.push(task);
            }
        }

        if imported_tasks.is_empty() {
            anyhow::bail!("No valid tasks found in ICS file");
        }

        let mut existing_tasks = Self::load_for_href(ctx, calendar_href)?;
        let count = imported_tasks.len();

        // Upsert tasks: replace existing ones with the same UID, append new ones
        for imported in imported_tasks {
            if let Some(idx) = existing_tasks.iter().position(|t| t.uid == imported.uid) {
                existing_tasks[idx] = imported;
            } else {
                existing_tasks.push(imported);
            }
        }

        Self::save_for_href(ctx, calendar_href, &existing_tasks)?;

        Ok(count)
    }

    pub fn to_ics_string(tasks: &[Task]) -> String {
        let mut output =
            String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Cfait//Export//EN\r\n");
        for task in tasks {
            let full_ics = IcsAdapter::to_ics(task);
            if let Some(start) = full_ics.find("BEGIN:VTODO")
                && let Some(end_idx) = full_ics.rfind("END:VTODO")
            {
                let vtodo = &full_ics[start..end_idx + 9];
                output.push_str(vtodo);
                output.push_str("\r\n");
            }
        }
        output.push_str("END:VCALENDAR");
        output
    }

    #[cfg(not(target_os = "android"))]
    fn get_lock_path(file_path: &Path) -> PathBuf {
        let mut lock_path = file_path.to_path_buf();
        if let Some(ext) = lock_path.extension() {
            let mut new_ext = ext.to_os_string();
            new_ext.push(".lock");
            lock_path.set_extension(new_ext);
        } else {
            lock_path.set_extension("lock");
        }
        lock_path
    }

    #[cfg(not(target_os = "android"))]
    pub fn with_lock<F, T>(file_path: &Path, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let lock_path = Self::get_lock_path(file_path);
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        file.lock_exclusive()?;
        let result = f();
        file.unlock()?;
        result
    }

    #[cfg(target_os = "android")]
    pub fn with_lock<F, T>(file_path: &Path, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let map_mutex = ANDROID_FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let key = file_path.canonicalize().unwrap_or(file_path.to_path_buf());
        let file_mutex = {
            let mut map = map_mutex.lock().unwrap();
            map.entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = file_mutex.lock().unwrap();
        f()
    }

    pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
        let path = path.as_ref();
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, contents)?;
        fs::rename(tmp_path, path)?;
        Ok(())
    }

    /// Load tasks from a specific file path (Internal)
    fn load_from_path(path: &Path, href: &str) -> Result<Vec<Task>> {
        if !path.exists() {
            LoadState::set(href, LoadState::Success);
            return Ok(vec![]);
        }
        let result = Self::with_lock(path, || {
            let json = fs::read_to_string(path)?;
            let (mut tasks, mut needs_save) =
                if let Ok(data) = serde_json::from_str::<LocalStorageData>(&json) {
                    if data.version == LOCAL_STORAGE_VERSION {
                        (data.tasks, false)
                    } else {
                        (Self::migrate_to_current(data.version, &json)?, true)
                    }
                } else {
                    (Self::migrate_v1_to_v2(&json)?, true)
                };

            // DEDUPLICATION FIX (v0.4.10)
            // Remove duplicate tasks by UID, keeping the last occurrence (matching TaskStore behavior).
            // This fixes data corruption from previous versions where imports appended duplicates.
            let len_before = tasks.len();
            let mut uid_to_index = HashMap::new();

            // Iterate to find indices of the *last* occurrence of each UID
            for (i, t) in tasks.iter().enumerate() {
                uid_to_index.insert(t.uid.clone(), i);
            }

            if uid_to_index.len() < len_before {
                let mut indices: Vec<usize> = uid_to_index.into_values().collect();
                indices.sort_unstable();

                let mut deduped = Vec::with_capacity(indices.len());
                for i in indices {
                    deduped.push(tasks[i].clone());
                }
                tasks = deduped;
                needs_save = true;

                #[cfg(target_os = "android")]
                log::info!(
                    "Repaired {} duplicate tasks in {}",
                    len_before - tasks.len(),
                    href
                );
                #[cfg(not(target_os = "android"))]
                eprintln!(
                    "Repaired {} duplicate tasks in {}",
                    len_before - tasks.len(),
                    href
                );
            }

            if needs_save {
                let data = LocalStorageData {
                    version: LOCAL_STORAGE_VERSION,
                    tasks: tasks.clone(),
                };
                let upgraded_json = serde_json::to_string_pretty(&data)?;
                Self::atomic_write(path, upgraded_json)?;
            }

            Ok(tasks)
        });
        match &result {
            Ok(_) => LoadState::set(href, LoadState::Success),
            Err(_) => LoadState::set(href, LoadState::Failed),
        }
        result
    }

    fn save_to_path(path: &Path, href: &str, tasks: &[Task]) -> Result<()> {
        if !Self::can_save_href(href) {
            return Err(anyhow::anyhow!(
                "Cannot save {}: previous load failed.",
                href
            ));
        }
        Self::with_lock(path, || {
            let data = LocalStorageData {
                version: LOCAL_STORAGE_VERSION,
                tasks: tasks.to_vec(),
            };
            let json = serde_json::to_string_pretty(&data)?;
            Self::atomic_write(path, json)?;
            Ok(())
        })
    }

    pub fn load_for_href(ctx: &dyn AppContext, href: &str) -> Result<Vec<Task>> {
        if let Some(path) = Self::get_path_for_href(ctx, href) {
            Self::load_from_path(&path, href)
        } else {
            Ok(vec![])
        }
    }

    pub fn save_for_href(ctx: &dyn AppContext, href: &str, tasks: &[Task]) -> Result<()> {
        if let Some(path) = Self::get_path_for_href(ctx, href) {
            Self::save_to_path(&path, href, tasks)
        } else {
            Err(anyhow::anyhow!("Invalid local href: {}", href))
        }
    }

    pub fn can_save_href(href: &str) -> bool {
        match LoadState::get(href) {
            LoadState::Uninitialized | LoadState::Success => true,
            LoadState::Failed => false,
        }
    }

    fn migrate_v1_to_v2(json: &str) -> Result<Vec<Task>> {
        serde_json::from_str::<Vec<Task>>(json)
            .map_err(|e| anyhow::anyhow!("Failed to migrate v1 to v2: {}", e))
    }

    fn migrate_to_current(old_version: u32, json: &str) -> Result<Vec<Task>> {
        if old_version > LOCAL_STORAGE_VERSION {
            return Err(anyhow::anyhow!("Local storage version too new"));
        }
        let tasks = match old_version {
            0 | 1 => Self::migrate_v1_to_v2(json)?,
            2 | 3 => {
                let data: LocalStorageData = serde_json::from_str(json)?;
                data.tasks
            }
            _ => return Err(anyhow::anyhow!("Unknown version {}", old_version)),
        };
        Ok(tasks)
    }
}

#[cfg(not(target_os = "android"))]
pub struct DaemonLock {
    _file: std::fs::File,
}

#[cfg(not(target_os = "android"))]
impl DaemonLock {
    /// Acquired by UI instances. Multiple UIs can hold this shared lock simultaneously.
    /// If the daemon is currently syncing, this blocks briefly until the daemon finishes.
    pub fn acquire_shared(ctx: &dyn AppContext) -> Result<Self> {
        let path = ctx.get_data_dir()?.join("daemon.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        file.lock_shared()?;
        Ok(Self { _file: file })
    }

    /// Acquired by the background daemon.
    /// Returns None immediately if ANY UI instance is currently holding a shared lock.
    pub fn try_acquire_exclusive(ctx: &dyn AppContext) -> Result<Option<Self>> {
        let path = ctx.get_data_dir()?.join("daemon.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        match file.try_lock_exclusive() {
            Ok(_) => Ok(Some(Self { _file: file })),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::PermissionDenied => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to acquire daemon lock: {}", e)),
        }
    }
}

#[cfg(test)]
#[cfg(not(target_os = "android"))]
mod lock_tests {
    use super::*;
    use crate::context::TestContext;

    #[test]
    fn test_daemon_locks_shared_vs_exclusive() {
        let ctx = TestContext::new();

        // 1. Multiple shared locks are allowed (TUI & GUI open simultaneously)
        let shared1 = DaemonLock::acquire_shared(&ctx).unwrap();
        let shared2 = DaemonLock::acquire_shared(&ctx).unwrap();

        // 2. Exclusive lock should fail while shared locks are held (Daemon yields)
        let excl1 = DaemonLock::try_acquire_exclusive(&ctx).unwrap();
        assert!(excl1.is_none(), "Exclusive lock should fail when shared locks exist");

        // 3. Drop all UI shared locks
        drop(shared1);
        drop(shared2);

        // 4. Exclusive lock should now succeed (Daemon syncs)
        let excl2 = DaemonLock::try_acquire_exclusive(&ctx).unwrap();
        assert!(excl2.is_some(), "Exclusive lock should succeed when no shared locks exist");
    }
}
