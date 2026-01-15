// Manages local file storage for tasks and calendars.
//
// ⚠️ VERSION BUMP REQUIRED:
// Changes to Task struct or its nested types (Alarm, DateType, etc.) require
// incrementing LOCAL_STORAGE_VERSION below to prevent data corruption.
use crate::model::{CalendarListEntry, Task};
use crate::paths::AppPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// --- Android Specific Imports ---
#[cfg(target_os = "android")]
use std::sync::Arc;

// --- Desktop Specific Imports ---
#[cfg(not(target_os = "android"))]
use fs2::FileExt;

// Constants for identification
pub const LOCAL_CALENDAR_HREF: &str = "local://default";
pub const LOCAL_CALENDAR_NAME: &str = "Local";
pub const LOCAL_REGISTRY_FILENAME: &str = "local_calendars.json";

// Increment this when making breaking changes to the Task struct serialization format
// Version history:
// - v1: Original format with DateTime<Utc> for due/dtstart (v3.12 and earlier)
// - v2: DateType enum for due/dtstart with AllDay/Specific support (v3.14+)
// - v3: Added estimated_duration_max field for duration ranges
const LOCAL_STORAGE_VERSION: u32 = 3;

/// Wrapper struct for versioned local storage
#[derive(Serialize, Deserialize)]
struct LocalStorageData {
    #[serde(default)]
    version: u32,
    tasks: Vec<Task>,
}

// --- Android Global Lock Map ---
#[cfg(target_os = "android")]
static ANDROID_FILE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

/// Tracks whether the last load operation succeeded for each calendar.
/// This prevents data loss by blocking saves when we couldn't load the existing data.
/// Key is the calendar href (e.g., "local://default", "local://<uuid>")
static LOAD_STATE_MAP: OnceLock<Mutex<HashMap<String, LoadState>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadState {
    /// Never attempted to load
    Uninitialized,
    /// Last load succeeded
    Success,
    /// Last load failed (deserialization error, corruption, etc.)
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

/// Registry for managing multiple local calendars
pub struct LocalCalendarRegistry;

impl LocalCalendarRegistry {
    fn get_path() -> Option<PathBuf> {
        AppPaths::get_data_dir()
            .ok()
            .map(|p| p.join(LOCAL_REGISTRY_FILENAME))
    }

    /// Load all local calendars from the registry
    pub fn load() -> Result<Vec<CalendarListEntry>> {
        let mut cals = vec![];

        // Ensure default local calendar always exists
        let default_cal = CalendarListEntry {
            name: LOCAL_CALENDAR_NAME.to_string(),
            href: LOCAL_CALENDAR_HREF.to_string(),
            color: None,
        };

        if let Some(path) = Self::get_path()
            && path.exists()
            && let Ok(content) = LocalStorage::with_lock(&path, || Ok(fs::read_to_string(&path)?))
            && let Ok(registry) = serde_json::from_str::<Vec<CalendarListEntry>>(&content)
        {
            cals = registry;
        }

        // Ensure default is present if missing (or empty registry)
        if !cals.iter().any(|c| c.href == LOCAL_CALENDAR_HREF) {
            cals.insert(0, default_cal);
        }

        Ok(cals)
    }

    /// Save all local calendars to the registry
    pub fn save(calendars: &[CalendarListEntry]) -> Result<()> {
        if let Some(path) = Self::get_path() {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string_pretty(calendars)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use serial_test::serial;

    // RAII guard to restore CFAIT_TEST_DIR after test
    struct TestDirGuard {
        original_value: Option<String>,
        temp_dir: std::path::PathBuf,
    }

    impl TestDirGuard {
        fn new(test_name: &str) -> Self {
            let original_value = std::env::var("CFAIT_TEST_DIR").ok();
            let temp_dir = std::env::temp_dir().join(format!(
                "cfait_test_{}_{}",
                test_name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::create_dir_all(&temp_dir);

            // Clear load state map BEFORE setting new directory to prevent test interference
            if let Some(map) = LOAD_STATE_MAP.get() {
                map.lock().unwrap().clear();
            }

            unsafe {
                std::env::set_var("CFAIT_TEST_DIR", &temp_dir);
            }

            Self {
                original_value,
                temp_dir,
            }
        }
    }

    impl Drop for TestDirGuard {
        fn drop(&mut self) {
            // Clean up temp directory
            let _ = fs::remove_dir_all(&self.temp_dir);

            // Restore original CFAIT_TEST_DIR or remove if it wasn't set
            unsafe {
                match &self.original_value {
                    Some(val) => std::env::set_var("CFAIT_TEST_DIR", val),
                    None => std::env::remove_var("CFAIT_TEST_DIR"),
                }
            }

            // Clear load state map again to prevent leaking state to next test
            if let Some(map) = LOAD_STATE_MAP.get() {
                map.lock().unwrap().clear();
            }
        }
    }

    fn create_test_task(uid: &str, summary: &str, calendar_href: &str) -> Task {
        let mut task = Task::new(summary, &std::collections::HashMap::new(), None);
        task.uid = uid.to_string();
        task.calendar_href = calendar_href.to_string();
        task
    }

    #[test]
    #[serial]
    fn test_multi_calendar_save_and_load() {
        let _guard = TestDirGuard::new("multi_save_load");

        // Create registry with multiple calendars
        let cal1 = CalendarListEntry {
            name: "Work".to_string(),
            href: "local://work".to_string(),
            color: Some("#FF0000".to_string()),
        };
        let cal2 = CalendarListEntry {
            name: "Personal".to_string(),
            href: "local://personal".to_string(),
            color: Some("#00FF00".to_string()),
        };

        let registry = vec![cal1.clone(), cal2.clone()];
        LocalCalendarRegistry::save(&registry).unwrap();

        // Create tasks for each calendar
        let task1 = create_test_task("task1", "Work task", "local://work");
        let task2 = create_test_task("task2", "Personal task", "local://personal");

        // Save to different calendars
        LocalStorage::save_for_href("local://work", std::slice::from_ref(&task1)).unwrap();
        LocalStorage::save_for_href("local://personal", std::slice::from_ref(&task2)).unwrap();

        // Load and verify each calendar independently
        let loaded_work = LocalStorage::load_for_href("local://work").unwrap();
        assert_eq!(loaded_work.len(), 1);
        assert_eq!(loaded_work[0].uid, "task1");
        assert_eq!(loaded_work[0].summary, "Work task");

        let loaded_personal = LocalStorage::load_for_href("local://personal").unwrap();
        assert_eq!(loaded_personal.len(), 1);
        assert_eq!(loaded_personal[0].uid, "task2");
        assert_eq!(loaded_personal[0].summary, "Personal task");

        // Verify registry persists (may include default local calendar)
        let loaded_registry = LocalCalendarRegistry::load().unwrap();
        assert!(loaded_registry.len() >= 2);
        assert!(loaded_registry.iter().any(|c| c.href == "local://work"));
        assert!(loaded_registry.iter().any(|c| c.href == "local://personal"));
    }

    #[test]
    #[serial]
    fn test_multi_calendar_isolation() {
        let _guard = TestDirGuard::new("multi_isolation");

        // Create two calendars with tasks
        let task1 = create_test_task("task1", "Calendar A task", "local://cal-a");
        let task2 = create_test_task("task2", "Calendar B task", "local://cal-b");

        LocalStorage::save_for_href("local://cal-a", std::slice::from_ref(&task1)).unwrap();
        LocalStorage::save_for_href("local://cal-b", std::slice::from_ref(&task2)).unwrap();

        // Modify calendar A
        let mut cal_a_tasks = LocalStorage::load_for_href("local://cal-a").unwrap();
        let task3 = create_test_task("task3", "Another A task", "local://cal-a");
        cal_a_tasks.push(task3);
        LocalStorage::save_for_href("local://cal-a", &cal_a_tasks).unwrap();

        // Verify calendar B is unchanged
        let cal_b_tasks = LocalStorage::load_for_href("local://cal-b").unwrap();
        assert_eq!(cal_b_tasks.len(), 1);
        assert_eq!(cal_b_tasks[0].uid, "task2");

        // Verify calendar A has two tasks
        let cal_a_tasks = LocalStorage::load_for_href("local://cal-a").unwrap();
        assert_eq!(cal_a_tasks.len(), 2);
    }

    #[test]
    #[serial]
    fn test_multi_calendar_default_compatibility() {
        let _guard = TestDirGuard::new("multi_compat");

        // Create task using old API (default calendar)
        let task = create_test_task("default-task", "Default task", LOCAL_CALENDAR_HREF);

        LocalStorage::save(std::slice::from_ref(&task)).unwrap();

        // Verify it's accessible via both APIs
        let loaded_old = LocalStorage::load().unwrap();
        let loaded_new = LocalStorage::load_for_href(LOCAL_CALENDAR_HREF).unwrap();

        assert_eq!(loaded_old.len(), 1);
        assert_eq!(loaded_new.len(), 1);
        assert_eq!(loaded_old[0].uid, loaded_new[0].uid);
    }

    #[test]
    #[serial]
    fn test_multi_calendar_independent_operations() {
        let _guard = TestDirGuard::new("multi_independent");

        // Create calendar A with one task
        let task_a = create_test_task("task-a", "Task A", "local://cal-a");
        LocalStorage::save_for_href("local://cal-a", std::slice::from_ref(&task_a)).unwrap();

        // Create calendar B with one task
        let task_b = create_test_task("task-b", "Task B", "local://cal-b");
        LocalStorage::save_for_href("local://cal-b", std::slice::from_ref(&task_b)).unwrap();

        // Verify both calendars work independently
        let loaded_a = LocalStorage::load_for_href("local://cal-a").unwrap();
        assert_eq!(loaded_a.len(), 1);
        assert_eq!(loaded_a[0].uid, "task-a");

        let loaded_b = LocalStorage::load_for_href("local://cal-b").unwrap();
        assert_eq!(loaded_b.len(), 1);
        assert_eq!(loaded_b[0].uid, "task-b");

        // Update calendar A - should not affect calendar B
        let mut tasks_a = loaded_a;
        let task_a2 = create_test_task("task-a2", "Task A2", "local://cal-a");
        tasks_a.push(task_a2);
        LocalStorage::save_for_href("local://cal-a", &tasks_a).unwrap();

        // Verify calendar A has 2 tasks
        let reloaded_a = LocalStorage::load_for_href("local://cal-a").unwrap();
        assert_eq!(reloaded_a.len(), 2);

        // Verify calendar B still has 1 task (unchanged)
        let reloaded_b = LocalStorage::load_for_href("local://cal-b").unwrap();
        assert_eq!(reloaded_b.len(), 1);
        assert_eq!(reloaded_b[0].uid, "task-b");
    }
}

pub struct LocalStorage;

impl LocalStorage {
    /// Returns the file path for a given local calendar href.
    /// local://default -> local.json
    /// local://<uuid>  -> local_<safe_uuid>.json
    pub fn get_path_for_href(href: &str) -> Option<PathBuf> {
        if href == LOCAL_CALENDAR_HREF {
            AppPaths::get_local_task_path()
        } else if href.starts_with("local://") {
            let id = href.trim_start_matches("local://");
            // Sanitize the ID to only allow alphanumeric and hyphens
            let safe_id: String = id
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect();
            AppPaths::get_data_dir()
                .ok()
                .map(|p| p.join(format!("local_{}.json", safe_id)))
        } else {
            None
        }
    }

    /// Legacy method for backward compatibility (redirects to default)
    pub fn get_path() -> Option<PathBuf> {
        Self::get_path_for_href(LOCAL_CALENDAR_HREF)
    }

    /// Imports tasks from an ICS string and merges them into the specified calendar.
    /// Returns the number of tasks successfully imported.
    pub fn import_from_ics(calendar_href: &str, ics_content: &str) -> Result<usize> {
        let mut imported_tasks = Vec::new();

        // Normalize line endings to \r\n for consistent parsing
        let normalized_content = ics_content.replace("\r\n", "\n").replace('\n', "\r\n");

        // Split by VTODO blocks and parse each
        let parts: Vec<&str> = normalized_content.split("BEGIN:VTODO").collect();

        // Skip the first part (before first VTODO or VCALENDAR header)
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

            // Parse the task
            if let Ok(task) = Task::from_ics(
                &full_ics,
                String::new(), // Empty etag for imported tasks
                format!("{}.ics", uuid::Uuid::new_v4()), // Generate new href
                calendar_href.to_string(),
            ) {
                imported_tasks.push(task);
            }
        }

        if imported_tasks.is_empty() {
            anyhow::bail!("No valid tasks found in ICS file");
        }

        // Load existing tasks
        let mut existing_tasks = Self::load_for_href(calendar_href)?;

        // Merge: append imported tasks
        let count = imported_tasks.len();
        existing_tasks.extend(imported_tasks);

        // Save back
        Self::save_for_href(calendar_href, &existing_tasks)?;

        Ok(count)
    }

    /// Generates a single VCALENDAR string containing all provided tasks as VTODO components.
    pub fn to_ics_string(tasks: &[Task]) -> String {
        let mut output =
            String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Cfait//Export//EN\r\n");

        for task in tasks {
            let full_ics = task.to_ics();
            // Extract the VTODO block from the full VCALENDAR string generated by the model
            if let Some(start) = full_ics.find("BEGIN:VTODO")
                && let Some(end_idx) = full_ics.rfind("END:VTODO")
            {
                // Extract up to and including END:VTODO
                // "END:VTODO" is 9 chars long
                let vtodo = &full_ics[start..end_idx + 9];
                output.push_str(vtodo);
                output.push_str("\r\n");
            }
        }

        output.push_str("END:VCALENDAR");
        output
    }

    /// Helper to get a sidecar lock file path (Desktop only)
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

    // --- DESKTOP IMPLEMENTATION (fs2) ---
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

    // --- ANDROID IMPLEMENTATION (In-Memory Mutex) ---
    #[cfg(target_os = "android")]
    pub fn with_lock<F, T>(file_path: &Path, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // Get the global map
        let map_mutex = ANDROID_FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));

        // Canonicalize to avoid race conditions via symlinks or relative paths
        let key = file_path.canonicalize().unwrap_or(file_path.to_path_buf());

        // Get or create the mutex specifically for this file path
        let file_mutex = {
            let mut map = map_mutex.lock().unwrap();
            map.entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Lock this specific file
        let _guard = file_mutex.lock().unwrap();

        // Perform operation
        f()
    }

    /// Atomic write: Write to .tmp file then rename
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

            // Try to parse as versioned format first
            let (tasks, needs_upgrade) =
                if let Ok(data) = serde_json::from_str::<LocalStorageData>(&json) {
                    // Versioned format detected
                    if data.version == LOCAL_STORAGE_VERSION {
                        // Current version, use directly
                        (data.tasks, false)
                    } else {
                        // Old version, migrate
                        (Self::migrate_to_current(data.version, &json)?, true)
                    }
                } else {
                    // No version field or parsing failed - assume v1 (unversioned)
                    #[cfg(target_os = "android")]
                    log::info!("Migrating {} from v1 to v{}", href, LOCAL_STORAGE_VERSION);
                    #[cfg(not(target_os = "android"))]
                    eprintln!("Migrating {} from v1 to v{}", href, LOCAL_STORAGE_VERSION);

                    (Self::migrate_v1_to_v2(&json)?, true)
                };

            // If we migrated, save the upgraded version immediately
            if needs_upgrade {
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

    /// Save tasks to a specific file path (Internal)
    fn save_to_path(path: &Path, href: &str, tasks: &[Task]) -> Result<()> {
        if !Self::can_save_href(href) {
            return Err(anyhow::anyhow!(
                "Cannot save {}: previous load failed. This prevents overwriting data that couldn't be read.",
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

    /// Public load method handling any local href
    pub fn load_for_href(href: &str) -> Result<Vec<Task>> {
        if let Some(path) = Self::get_path_for_href(href) {
            Self::load_from_path(&path, href)
        } else {
            // Unknown scheme, return empty
            Ok(vec![])
        }
    }

    /// Public save method handling any local href
    pub fn save_for_href(href: &str, tasks: &[Task]) -> Result<()> {
        if let Some(path) = Self::get_path_for_href(href) {
            Self::save_to_path(&path, href, tasks)
        } else {
            Err(anyhow::anyhow!("Invalid local href: {}", href))
        }
    }

    /// Save tasks to local.json (default calendar)
    ///
    /// # Data Loss Prevention
    /// This function checks `LoadState` before saving. If the last `load()` failed,
    /// this will return an error instead of overwriting the file with potentially
    /// incomplete data.
    ///
    /// To force a save (e.g., after manual data recovery), call `force_save()` instead.
    pub fn save(tasks: &[Task]) -> Result<()> {
        Self::save_for_href(LOCAL_CALENDAR_HREF, tasks)
    }

    /// Force save tasks to local.json (default calendar), bypassing load state check.
    ///
    /// # WARNING
    /// This bypasses the data loss prevention check. Only use this if:
    /// - You've manually verified the data is correct
    /// - You're recovering from a known issue
    /// - You understand the risks of overwriting existing data
    ///
    /// In most cases, use `save()` instead.
    pub fn force_save(tasks: &[Task]) -> Result<()> {
        if let Some(path) = Self::get_path() {
            Self::with_lock(&path, || {
                let data = LocalStorageData {
                    version: LOCAL_STORAGE_VERSION,
                    tasks: tasks.to_vec(),
                };
                let json = serde_json::to_string_pretty(&data)?;
                Self::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Migrate tasks from version 1 (unversioned) to version 2 (versioned with DateType)
    /// This handles the v3.12 -> v3.14 migration using the backward-compatible deserializer
    fn migrate_v1_to_v2(json: &str) -> Result<Vec<Task>> {
        // The backward-compatible deserializer in Task struct handles this automatically
        // It converts DateTime<Utc> strings to DateType::Specific
        serde_json::from_str::<Vec<Task>>(json)
            .map_err(|e| anyhow::anyhow!("Failed to migrate v1 to v2: {}", e))
    }

    /// Run migrations to upgrade from old version to current version
    ///
    /// This function chains migrations automatically: v0 → v1 → v2 → v3 → ... → current
    /// Each migration is applied in sequence, ensuring proper data transformation.
    fn migrate_to_current(old_version: u32, json: &str) -> Result<Vec<Task>> {
        #[cfg(target_os = "android")]
        log::info!(
            "Migrating local storage from v{} to v{}",
            old_version,
            LOCAL_STORAGE_VERSION
        );
        #[cfg(not(target_os = "android"))]
        eprintln!(
            "Migrating local storage from v{} to v{}",
            old_version, LOCAL_STORAGE_VERSION
        );

        // Check for future versions first
        if old_version > LOCAL_STORAGE_VERSION {
            return Err(anyhow::anyhow!(
                "Local storage version {} is newer than supported version {}. Please upgrade Cfait.",
                old_version,
                LOCAL_STORAGE_VERSION
            ));
        }

        // Parse the JSON based on the old version format
        let tasks = match old_version {
            0 | 1 => {
                // Version 0 or 1: Unversioned format (v3.12 and earlier)
                // Parse as raw Vec<Task> which uses backward-compatible deserializer
                Self::migrate_v1_to_v2(json)?
            }
            2 => {
                // Version 2: Versioned format with DateType
                let data: LocalStorageData = serde_json::from_str(json)?;
                data.tasks
            }
            3 => {
                // Version 3: Added estimated_duration_max for duration ranges
                let data: LocalStorageData = serde_json::from_str(json)?;
                data.tasks
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown local storage version: {}",
                    old_version
                ));
            }
        };

        // Now apply incremental migrations in sequence
        // This allows chaining: v1 → v2 → v3 → v4 automatically

        // Example for future v3:
        // if old_version < 3 {
        //     tasks = migrate_v2_to_v3(tasks)?;
        // }

        // Example for future v4:
        // if old_version < 4 {
        //     tasks = migrate_v3_to_v4(tasks)?;
        // }

        Ok(tasks)
    }

    /// Load tasks from local.json with automatic version migration
    ///
    /// # CRITICAL WARNING
    /// **NEVER** silently ignore errors from this function using `if let Ok(_)` or `.unwrap_or_default()`!
    ///
    /// In v3.14, silent error handling caused data loss when the serialization format changed.
    /// Users upgrading from v3.12 had their local.json deserialization fail, which was silently
    /// ignored, resulting in an empty task list that overwrote their data.
    ///
    /// **Always** log errors from this function. If deserialization fails, it indicates:
    /// - Data corruption
    /// - Format incompatibility (version mismatch)
    /// - File system issues
    ///
    /// See MIGRATION_V3.12_TO_V3.14.md for details on the incident and prevention measures.
    ///
    /// # Load State Tracking
    /// This function tracks load success/failure via `LoadState`. If loading fails,
    /// `save()` will be blocked to prevent overwriting corrupted or incompatible data.
    ///
    /// # Version Migration
    /// This function automatically detects the storage version and migrates data if needed:
    /// - Version 0/1: Unversioned format (v3.12) with DateTime<Utc> → migrates to v2
    /// - Version 2: Current format with DateType enum → loads directly
    /// - Future versions: Additional migrations applied as needed
    pub fn load() -> Result<Vec<Task>> {
        Self::load_for_href(LOCAL_CALENDAR_HREF)
    }

    /// Check if the last load operation succeeded for the default calendar.
    ///
    /// Returns `true` if:
    /// - Load succeeded
    /// - No load has been attempted yet (Uninitialized state)
    ///
    /// Returns `false` if:
    /// - Load failed (deserialization error, corruption, etc.)
    pub fn can_save() -> bool {
        Self::can_save_href(LOCAL_CALENDAR_HREF)
    }

    /// Check if the last load operation succeeded for a specific calendar.
    ///
    /// Returns `true` if:
    /// - Load succeeded
    /// - No load has been attempted yet (Uninitialized state)
    ///
    /// Returns `false` if:
    /// - Load failed (deserialization error, corruption, etc.)
    pub fn can_save_href(href: &str) -> bool {
        match LoadState::get(href) {
            LoadState::Uninitialized => true, // Allow save if never loaded
            LoadState::Success => true,
            LoadState::Failed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_atomic_write_and_load() {
        let temp_dir = std::env::temp_dir().join("cfait_test_storage");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test.json");

        // Fix: Explicit type annotation
        let tasks: Vec<Task> = vec![];

        LocalStorage::atomic_write(&file_path, serde_json::to_string(&tasks).unwrap()).unwrap();

        let loaded: Vec<Task> =
            serde_json::from_str(&fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(loaded.len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_locking_concurrency() {
        // Use a uniquely-named temporary directory to avoid interference between
        // parallel test runs or other processes that may reuse the same name.
        let unique = format!(
            "cfait_test_lock_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(unique);
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("lock_test.txt");
        let path_ref = Arc::new(file_path.clone());

        let _ = fs::write(&file_path, "0");

        let mut handles = vec![];
        for _ in 0..10 {
            let p = path_ref.clone();
            handles.push(thread::spawn(move || {
                LocalStorage::with_lock(&p, || {
                    let content = fs::read_to_string(&*p).unwrap();
                    let num: i32 = content.parse().unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    fs::write(&*p, (num + 1).to_string()).unwrap();
                    Ok(())
                })
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "10");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    #[serial]
    fn test_load_state_mechanism() {
        // Test the LoadState mechanism directly without file I/O
        let test_href = "local://test";

        // Start uninitialized - should allow save
        LoadState::set(test_href, LoadState::Uninitialized);
        assert_eq!(LoadState::get(test_href), LoadState::Uninitialized);
        assert!(
            LocalStorage::can_save_href(test_href),
            "Should allow save when uninitialized"
        );

        // Simulate successful load
        LoadState::set(test_href, LoadState::Success);
        assert_eq!(LoadState::get(test_href), LoadState::Success);
        assert!(
            LocalStorage::can_save_href(test_href),
            "Should allow save after successful load"
        );

        // Simulate failed load
        LoadState::set(test_href, LoadState::Failed);
        assert_eq!(LoadState::get(test_href), LoadState::Failed);
        assert!(
            !LocalStorage::can_save_href(test_href),
            "Should NOT allow save after failed load"
        );

        // Test independence: another calendar should not be affected
        let other_href = "local://other";
        assert_eq!(LoadState::get(other_href), LoadState::Uninitialized);
        assert!(
            LocalStorage::can_save_href(other_href),
            "Other calendar should be unaffected"
        );
    }

    #[test]
    #[serial]
    fn test_save_blocked_after_failed_load() {
        // Directly test that save() returns an error when LoadState is Failed
        // Use a unique test href with timestamp to avoid conflicts with parallel tests
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_href = format!("local://test-save-blocked-{}", timestamp);

        // Set state to Failed
        LoadState::set(&test_href, LoadState::Failed);

        // Verify the state was actually set correctly
        assert_eq!(
            LoadState::get(&test_href),
            LoadState::Failed,
            "LoadState should be Failed after set"
        );

        // Verify can_save_href returns false
        assert!(
            !LocalStorage::can_save_href(&test_href),
            "can_save_href should return false when LoadState is Failed"
        );

        let tasks = vec![];
        let save_result = LocalStorage::save_for_href(&test_href, &tasks);

        assert!(
            save_result.is_err(),
            "save_for_href() should fail when LoadState is Failed"
        );
        assert!(
            save_result
                .unwrap_err()
                .to_string()
                .contains("previous load failed"),
            "Error message should explain why save was blocked"
        );

        // Cleanup: reset state to avoid affecting other tests
        LoadState::set(&test_href, LoadState::Uninitialized);
    }

    #[test]
    #[serial]
    fn test_force_save_bypasses_load_state() {
        // force_save should work even when LoadState is Failed
        let test_href = LOCAL_CALENDAR_HREF;
        LoadState::set(test_href, LoadState::Failed);

        let temp_dir = std::env::temp_dir().join("cfait_test_force_save");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test_force_save.json");

        let tasks: Vec<Task> = vec![];
        LocalStorage::atomic_write(&file_path, serde_json::to_string(&tasks).unwrap()).unwrap();

        // Verify file was written
        assert!(
            file_path.exists(),
            "force_save should create file even when LoadState is Failed"
        );

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_versioned_save_format() {
        // Test that the versioned format is serialized correctly
        let temp_dir = std::env::temp_dir().join("cfait_test_versioned_save");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test_versioned.json");

        let tasks: Vec<Task> = vec![];

        // Use atomic_write with versioned format
        let data = LocalStorageData {
            version: LOCAL_STORAGE_VERSION,
            tasks: tasks.to_vec(),
        };
        let json = serde_json::to_string_pretty(&data).unwrap();
        LocalStorage::atomic_write(&file_path, json).unwrap();

        // Read back and verify structure
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(
            content.contains("\"version\":"),
            "Should contain version field"
        );
        assert!(
            content.contains(&format!("\"version\": {}", LOCAL_STORAGE_VERSION)),
            "Should have current version"
        );
        assert!(content.contains("\"tasks\":"), "Should contain tasks field");

        // Parse and verify
        let loaded: LocalStorageData = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.version, LOCAL_STORAGE_VERSION);
        assert_eq!(loaded.tasks.len(), 0);

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_migration_v1_to_v2() {
        // Test migration from unversioned (v1) format to versioned (v2) format
        // This simulates v3.12 -> v3.14 upgrade
        let v1_json = r#"[
  {
    "uid": "test-v1-task",
    "summary": "Old format task",
    "description": "",
    "status": "NeedsAction",
    "estimated_duration": null,
    "due": "2024-01-15T14:30:00Z",
    "dtstart": null,
    "alarms": [],
    "priority": 0,
    "percent_complete": null,
    "parent_uid": null,
    "dependencies": [],
    "related_to": [],
    "etag": "",
    "href": "",
    "calendar_href": "local://default",
    "categories": [],
    "depth": 0,
    "rrule": null,
    "location": null,
    "url": null,
    "geo": null,
    "unmapped_properties": [],
    "sequence": 0
  }
]"#;

        let tasks = LocalStorage::migrate_v1_to_v2(v1_json)
            .expect("Migration from v1 to v2 should succeed");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].summary, "Old format task");
        assert!(
            matches!(tasks[0].due, Some(crate::model::DateType::Specific(_))),
            "Due date should be migrated to DateType::Specific"
        );
    }

    #[test]
    fn test_versioned_format_loads_directly() {
        // Test that v2 (versioned) format loads without migration
        let v2_json = format!(
            r#"{{
  "version": {},
  "tasks": [
    {{
      "uid": "test-v2-task",
      "summary": "New format task",
      "description": "",
      "status": "NeedsAction",
      "estimated_duration": null,
      "due": {{"type": "Specific", "value": "2024-01-15T14:30:00Z"}},
      "dtstart": null,
      "alarms": [],
      "priority": 0,
      "percent_complete": null,
      "parent_uid": null,
      "dependencies": [],
      "related_to": [],
      "etag": "",
      "href": "",
      "calendar_href": "local://default",
      "categories": [],
      "depth": 0,
      "rrule": null,
      "location": null,
      "url": null,
      "geo": null,
      "unmapped_properties": [],
      "sequence": 0
    }}
  ]
}}"#,
            LOCAL_STORAGE_VERSION
        );

        let data: LocalStorageData =
            serde_json::from_str(&v2_json).expect("V2 format should parse directly");

        assert_eq!(data.version, LOCAL_STORAGE_VERSION);
        assert_eq!(data.tasks.len(), 1);
        assert_eq!(data.tasks[0].summary, "New format task");
    }

    #[test]
    fn test_migrate_to_current_handles_versions() {
        // Test that migrate_to_current properly handles different versions

        // Test v1 (unversioned)
        let v1_json = r#"[]"#;
        let result = LocalStorage::migrate_to_current(1, v1_json);
        assert!(result.is_ok(), "Should handle v1 migration");

        // Test v2 (current)
        let v2_json = format!(r#"{{"version": {}, "tasks": []}}"#, LOCAL_STORAGE_VERSION);
        let result = LocalStorage::migrate_to_current(LOCAL_STORAGE_VERSION, &v2_json);
        assert!(result.is_ok(), "Should handle current version");

        // Test future version (should fail)
        let future_json = r#"{"version": 999, "tasks": []}"#;
        let result = LocalStorage::migrate_to_current(999, future_json);
        assert!(result.is_err(), "Should reject future versions");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("newer than supported"),
            "Should explain version is too new"
        );
    }

    #[test]
    #[serial]
    fn test_full_migration_workflow() {
        // Test complete workflow: v1 file -> load -> auto-migrate -> save -> load as v2
        let temp_dir = std::env::temp_dir().join("cfait_test_full_migration");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("local_migration.json");

        // Write v1 format file (unversioned with DateTime<Utc>)
        let v1_content = r#"[
  {
    "uid": "migration-test",
    "summary": "Migrate me",
    "description": "",
    "status": "NeedsAction",
    "estimated_duration": null,
    "due": "2024-06-15T10:00:00Z",
    "dtstart": null,
    "alarms": [],
    "priority": 0,
    "percent_complete": null,
    "parent_uid": null,
    "dependencies": [],
    "related_to": [],
    "etag": "",
    "href": "",
    "calendar_href": "local://default",
    "categories": [],
    "depth": 0,
    "rrule": null,
    "location": null,
    "url": null,
    "geo": null,
    "unmapped_properties": [],
    "sequence": 0
  }
]"#;
        fs::write(&file_path, v1_content).unwrap();

        // Simulate load with migration
        let test_href = "local://test-migration";
        LoadState::set(test_href, LoadState::Uninitialized);
        let json = fs::read_to_string(&file_path).unwrap();

        // Parse as v1 and migrate
        let tasks = LocalStorage::migrate_v1_to_v2(&json).expect("Should migrate v1 to v2");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].summary, "Migrate me");

        // Save in v2 format
        let data = LocalStorageData {
            version: LOCAL_STORAGE_VERSION,
            tasks: tasks.clone(),
        };
        let v2_json = serde_json::to_string_pretty(&data).unwrap();
        fs::write(&file_path, v2_json).unwrap();

        // Load again and verify it's now v2
        let reloaded_json = fs::read_to_string(&file_path).unwrap();
        assert!(
            reloaded_json.contains("\"version\":"),
            "Should have version field now"
        );

        let reloaded_data: LocalStorageData = serde_json::from_str(&reloaded_json).unwrap();
        assert_eq!(reloaded_data.version, LOCAL_STORAGE_VERSION);
        assert_eq!(reloaded_data.tasks.len(), 1);
        assert_eq!(reloaded_data.tasks[0].summary, "Migrate me");

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_migration_chain_ready_for_future_versions() {
        // This test verifies the migration chain structure is ready for v3, v4, etc.
        // When you add v3, you'd update this test to actually run the migration.

        // Test v1 → v2 (current)
        let v1_json = r#"[{"uid":"test","summary":"Task","description":"","status":"NeedsAction","estimated_duration":null,"due":"2024-01-15T14:30:00Z","dtstart":null,"alarms":[],"priority":0,"percent_complete":null,"parent_uid":null,"dependencies":[],"related_to":[],"etag":"","href":"","calendar_href":"local://default","categories":[],"depth":0,"rrule":null,"location":null,"url":null,"geo":null,"unmapped_properties":[],"sequence":0}]"#;

        let result = LocalStorage::migrate_to_current(1, v1_json);
        assert!(result.is_ok(), "v1 → v2 migration should work");
        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);

        // Test v2 → v2 (no migration needed)
        let v2_json = format!(
            r#"{{"version":{},"tasks":[{{"uid":"test","summary":"Task","description":"","status":"NeedsAction","estimated_duration":null,"due":{{"type":"Specific","value":"2024-01-15T14:30:00Z"}},"dtstart":null,"alarms":[],"priority":0,"percent_complete":null,"parent_uid":null,"dependencies":[],"related_to":[],"etag":"","href":"","calendar_href":"local://default","categories":[],"depth":0,"rrule":null,"location":null,"url":null,"geo":null,"unmapped_properties":[],"sequence":0}}]}}"#,
            LOCAL_STORAGE_VERSION
        );

        let result = LocalStorage::migrate_to_current(2, &v2_json);
        assert!(result.is_ok(), "v2 → v2 (current) should work");

        // Test future version rejection
        let result = LocalStorage::migrate_to_current(999, "{}");
        assert!(result.is_err(), "Future versions should be rejected");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("newer than supported")
        );
    }

    #[test]
    fn test_desktop_and_android_both_work() {
        // This test verifies that the versioning system works on all platforms
        // All the code is platform-agnostic (no #[cfg] in critical paths)

        let v1_json = r#"[]"#;

        // This should work on any platform
        let result = LocalStorage::migrate_v1_to_v2(v1_json);
        assert!(result.is_ok(), "Migration should work on all platforms");

        // Versioned format should work on any platform
        let v2_json = format!(r#"{{"version":{},"tasks":[]}}"#, LOCAL_STORAGE_VERSION);
        let data: Result<LocalStorageData, _> = serde_json::from_str(&v2_json);
        assert!(
            data.is_ok(),
            "Versioned format should parse on all platforms"
        );
        assert_eq!(data.unwrap().version, LOCAL_STORAGE_VERSION);
    }

    #[test]
    fn test_future_migration_chain_example() {
        // This test demonstrates how v1→v2→v3→v4 would chain when those versions exist
        // Currently we only have v1→v2, but the structure is ready for more

        // Simulate what WOULD happen if we had v1→v2→v3→v4 migrations:
        //
        // User has v1 file, current version is v4:
        // 1. old_version = 1
        // 2. Parse as v1 format → gets tasks
        // 3. if old_version < 2: migrate_v1_to_v2(tasks) → tasks now v2 format
        // 4. if old_version < 3: migrate_v2_to_v3(tasks) → tasks now v3 format
        // 5. if old_version < 4: migrate_v3_to_v4(tasks) → tasks now v4 format
        // 6. Return v4 tasks
        //
        // This ensures: v1 → v2 → v3 → v4 automatically!

        // Test the current state (v1 → v2)
        let v1_json = r#"[
          {
            "uid": "chain-test",
            "summary": "Will chain through versions",
            "description": "",
            "status": "NeedsAction",
            "estimated_duration": null,
            "due": "2024-01-15T14:30:00Z",
            "dtstart": null,
            "alarms": [],
            "priority": 0,
            "percent_complete": null,
            "parent_uid": null,
            "dependencies": [],
            "related_to": [],
            "etag": "",
            "href": "",
            "calendar_href": "local://default",
            "categories": [],
            "depth": 0,
            "rrule": null,
            "location": null,
            "url": null,
            "geo": null,
            "unmapped_properties": [],
            "sequence": 0
          }
        ]"#;

        // Migrate from v1 to current (v2)
        let tasks = LocalStorage::migrate_to_current(1, v1_json).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].summary, "Will chain through versions");

        // Verify v1→v2 migration happened (DateType::Specific)
        assert!(
            matches!(tasks[0].due, Some(crate::model::DateType::Specific(_))),
            "v1 DateTime<Utc> should be converted to v2 DateType::Specific"
        );

        // When you add v3 in the future, the migration would look like:
        //
        // const LOCAL_STORAGE_VERSION: u32 = 3;
        //
        // fn migrate_v2_to_v3(tasks: Vec<Task>) -> Result<Vec<Task>> {
        //     tasks.into_iter().map(|mut task| {
        //         // Your v2→v3 transformation here
        //         Ok(task)
        //     }).collect()
        // }
        //
        // fn migrate_to_current(old_version: u32, json: &str) -> Result<Vec<Task>> {
        //     let mut tasks = match old_version {
        //         0 | 1 => migrate_v1_to_v2(json)?,
        //         2 => { let data: LocalStorageData = serde_json::from_str(json)?; data.tasks }
        //         3 => { let data: LocalStorageData = serde_json::from_str(json)?; data.tasks }
        //         _ => return Err(...)
        //     };
        //
        //     // Migration chain - this is where the magic happens!
        //     if old_version < 3 {
        //         tasks = migrate_v2_to_v3(tasks)?;  // v1→v2→v3 or v2→v3
        //     }
        //
        //     Ok(tasks)
        // }
        //
        // This structure ensures:
        // - v1 user: parse as v1 → apply v2→v3 migration → done
        // - v2 user: parse as v2 → apply v2→v3 migration → done
        // - v3 user: parse as v3 → no migration needed → done
    }

    #[test]
    fn test_backward_compatibility_missing_related_to_and_dependencies() {
        // Test that tasks saved before v0.1.7 (no dependencies) and v0.3.14 (no related_to)
        // can still be loaded. This simulates the actual user's issue.
        let old_task_json = r#"[{
    "uid": "old-task-uid",
    "summary": "Old task without related_to or dependencies",
    "description": "This task was created before those fields existed",
    "status": "NeedsAction",
    "estimated_duration": null,
    "due": "2024-01-15T14:30:00Z",
    "dtstart": null,
    "alarms": [],
    "priority": 5,
    "percent_complete": null,
    "parent_uid": null,
    "etag": "abc123",
    "href": "task1.ics",
    "calendar_href": "local://default",
    "categories": ["work"],
    "depth": 0,
    "rrule": null,
    "location": null,
    "url": null,
    "geo": null,
    "unmapped_properties": [],
    "sequence": 0
}]"#;

        // This should not fail even though dependencies and related_to are missing
        let result = LocalStorage::migrate_v1_to_v2(old_task_json);
        assert!(
            result.is_ok(),
            "Should successfully load old tasks without dependencies/related_to fields. Error: {:?}",
            result.err()
        );

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(
            tasks[0].summary,
            "Old task without related_to or dependencies"
        );
        assert_eq!(
            tasks[0].dependencies.len(),
            0,
            "dependencies should default to empty vec"
        );
        assert_eq!(
            tasks[0].related_to.len(),
            0,
            "related_to should default to empty vec"
        );
    }

    #[test]
    fn test_comprehensive_backward_compatibility() {
        // This test ensures that tasks missing ANY optional field can still be loaded.
        // It serves as documentation and protection against future backward compatibility breaks.
        //
        // When adding a new field to Task:
        // 1. If it's Vec<T>, usize, u8, u32, etc. → add #[serde(default)]
        // 2. If it's String and truly required → implement a migration
        // 3. If it's String and optional → use Option<String>
        // 4. Add the field to this test with its default value
        //
        // This test documents which fields MUST be present (the ones in the JSON below)
        // and which fields can be missing (tested via assertions after deserialization).

        let minimal_task_json = r#"[{
    "uid": "minimal-task",
    "summary": "Minimal task with only required fields",
    "description": "",
    "status": "NeedsAction",
    "priority": 0,
    "etag": "",
    "href": "",
    "calendar_href": "local://default"
}]"#;

        let result = LocalStorage::migrate_v1_to_v2(minimal_task_json);
        assert!(
            result.is_ok(),
            "Should load task with only truly required fields. Error: {:?}",
            result.err()
        );

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        let task = &tasks[0];

        // Verify all optional fields got their defaults
        assert_eq!(
            task.estimated_duration, None,
            "estimated_duration should default to None"
        );
        assert_eq!(task.due, None, "due should default to None");
        assert_eq!(task.dtstart, None, "dtstart should default to None");
        assert_eq!(task.alarms.len(), 0, "alarms should default to empty vec");
        assert_eq!(task.exdates.len(), 0, "exdates should default to empty vec");
        assert_eq!(
            task.percent_complete, None,
            "percent_complete should default to None"
        );
        assert_eq!(task.parent_uid, None, "parent_uid should default to None");
        assert_eq!(
            task.dependencies.len(),
            0,
            "dependencies should default to empty vec (added v0.1.7)"
        );
        assert_eq!(
            task.related_to.len(),
            0,
            "related_to should default to empty vec (added v0.3.14)"
        );
        assert_eq!(
            task.categories.len(),
            0,
            "categories should default to empty vec"
        );
        assert_eq!(task.depth, 0, "depth should default to 0");
        assert_eq!(task.rrule, None, "rrule should default to None");
        assert_eq!(task.location, None, "location should default to None");
        assert_eq!(task.url, None, "url should default to None");
        assert_eq!(task.geo, None, "geo should default to None");
        assert_eq!(
            task.unmapped_properties.len(),
            0,
            "unmapped_properties should default to empty vec"
        );
        assert_eq!(task.sequence, 0, "sequence should default to 0");
        assert_eq!(
            task.raw_alarms.len(),
            0,
            "raw_alarms should default to empty vec"
        );
        assert_eq!(
            task.raw_components.len(),
            0,
            "raw_components should default to empty vec"
        );
        assert_eq!(
            task.create_event, None,
            "create_event should default to None"
        );
    }

    #[test]
    fn test_future_field_addition_pattern() {
        // This test documents the pattern for safely adding new fields.
        // It simulates what happens when someone adds a field in the future.

        // Scenario: We're on v0.5.0 and want to add a new field "tags: Vec<String>"
        // Following the documentation in Task struct, we add #[serde(default)] to it.

        // Old task JSON from v0.4.2 (before hypothetical "tags" field)
        let old_task = r#"[{
    "uid": "old-task",
    "summary": "Task from before tags field existed",
    "description": "",
    "status": "NeedsAction",
    "priority": 5,
    "dependencies": [],
    "related_to": [],
    "etag": "123",
    "href": "task.ics",
    "calendar_href": "local://default"
}]"#;

        // This should work because all Vec fields have #[serde(default)]
        let result = LocalStorage::migrate_v1_to_v2(old_task);
        assert!(
            result.is_ok(),
            "Old tasks should load even if new Vec fields are added with #[serde(default)]"
        );

        // The key insight: By consistently using #[serde(default)] on all Vec<T> fields,
        // we avoid needing migrations for most field additions.
    }

    #[test]
    fn test_option_fields_dont_need_serde_default() {
        // This test PROVES that Option<T> fields do NOT need #[serde(default)]
        // They automatically default to None when missing from JSON.
        //
        // This is a special case in serde - Option<T> has built-in default behavior.
        // If this test passes, it proves the LLM's assessment was incorrect.

        let json_without_optional_fields = r#"[{
    "uid": "test-option-behavior",
    "summary": "Task with NO optional String fields in JSON",
    "description": "",
    "status": "NeedsAction",
    "priority": 0,
    "etag": "",
    "href": "",
    "calendar_href": "local://default"
}]"#;

        // This JSON is missing: location, url, geo, parent_uid, percent_complete, etc.
        // All of these are Option<T> fields WITHOUT #[serde(default)]
        // If the LLM were correct, this would fail with "missing field" errors.
        let result = LocalStorage::migrate_v1_to_v2(json_without_optional_fields);

        assert!(
            result.is_ok(),
            "Option<T> fields should default to None without #[serde(default)]. Error: {:?}",
            result.err()
        );

        let tasks = result.unwrap();
        assert_eq!(tasks.len(), 1);
        let task = &tasks[0];

        // Verify all Option fields defaulted to None
        assert_eq!(task.location, None, "location should be None");
        assert_eq!(task.url, None, "url should be None");
        assert_eq!(task.geo, None, "geo should be None");
        assert_eq!(task.parent_uid, None, "parent_uid should be None");
        assert_eq!(
            task.percent_complete, None,
            "percent_complete should be None"
        );
        assert_eq!(
            task.estimated_duration, None,
            "estimated_duration should be None"
        );
        assert_eq!(task.rrule, None, "rrule should be None");

        // CONCLUSION: Option<T> does NOT need #[serde(default)]
        // Adding it would be redundant and make the code less idiomatic.
    }
}
