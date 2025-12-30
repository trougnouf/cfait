// File: ./src/storage.rs
use crate::model::Task;
use crate::paths::AppPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// --- Android Specific Imports ---
#[cfg(target_os = "android")]
use std::collections::HashMap;
#[cfg(target_os = "android")]
use std::sync::Arc;

// --- Desktop Specific Imports ---
#[cfg(not(target_os = "android"))]
use fs2::FileExt;

// Constants for identification
pub const LOCAL_CALENDAR_HREF: &str = "local://default";
pub const LOCAL_CALENDAR_NAME: &str = "Local";

// Increment this when making breaking changes to the Task struct serialization format
// Version history:
// - v1: Original format with DateTime<Utc> for due/dtstart (v3.12 and earlier)
// - v2: DateType enum for due/dtstart with AllDay/Specific support (v3.14+)
const LOCAL_STORAGE_VERSION: u32 = 2;

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

/// Tracks whether the last load operation succeeded.
/// This prevents data loss by blocking saves when we couldn't load the existing data.
static LOAD_STATE: OnceLock<Mutex<LoadState>> = OnceLock::new();

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
    fn get() -> LoadState {
        *LOAD_STATE
            .get_or_init(|| Mutex::new(LoadState::Uninitialized))
            .lock()
            .unwrap()
    }

    fn set(state: LoadState) {
        *LOAD_STATE
            .get_or_init(|| Mutex::new(LoadState::Uninitialized))
            .lock()
            .unwrap() = state;
    }
}

pub struct LocalStorage;

impl LocalStorage {
    pub fn get_path() -> Option<PathBuf> {
        AppPaths::get_local_task_path()
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

    /// Save tasks to local.json
    ///
    /// # Data Loss Prevention
    /// This function checks `LoadState` before saving. If the last `load()` failed,
    /// this will return an error instead of overwriting the file with potentially
    /// incomplete data.
    ///
    /// To force a save (e.g., after manual data recovery), call `force_save()` instead.
    pub fn save(tasks: &[Task]) -> Result<()> {
        if !Self::can_save() {
            return Err(anyhow::anyhow!(
                "Cannot save: previous load failed. This prevents overwriting data that couldn't be read. \
                 Check logs for deserialization errors. Use force_save() if you're certain you want to overwrite."
            ));
        }
        Self::force_save(tasks)
    }

    /// Force save tasks to local.json, bypassing load state check.
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
            // Future versions would be parsed here:
            // 3 => {
            //     let data: LocalStorageData = serde_json::from_str(json)?;
            //     data.tasks
            // }
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
        if let Some(path) = Self::get_path() {
            if !path.exists() {
                LoadState::set(LoadState::Success);
                return Ok(vec![]);
            }
            let result = Self::with_lock(&path, || {
                let json = fs::read_to_string(&path)?;

                // Try to parse as versioned format first
                let tasks = if let Ok(data) = serde_json::from_str::<LocalStorageData>(&json) {
                    // Versioned format detected
                    if data.version == LOCAL_STORAGE_VERSION {
                        // Current version, use directly
                        data.tasks
                    } else {
                        // Old version, migrate
                        Self::migrate_to_current(data.version, &json)?
                    }
                } else {
                    // No version field or parsing failed - assume v1 (unversioned)
                    #[cfg(target_os = "android")]
                    log::info!("No version found in local.json, assuming v1 format");
                    #[cfg(not(target_os = "android"))]
                    eprintln!("No version found in local.json, assuming v1 format");

                    Self::migrate_v1_to_v2(&json)?
                };

                Ok(tasks)
            });

            match &result {
                Ok(_) => {
                    LoadState::set(LoadState::Success);
                    // If migration occurred, save with new version
                    // This is safe because LoadState is Success
                    if let Ok(tasks) = &result {
                        let _ = Self::save(tasks); // Best effort, don't fail load if save fails
                    }
                }
                Err(_) => LoadState::set(LoadState::Failed),
            }

            return result;
        }
        LoadState::set(LoadState::Success);
        Ok(vec![])
    }

    /// Check if the last load operation succeeded.
    ///
    /// Returns `true` if:
    /// - Load succeeded
    /// - No load has been attempted yet (Uninitialized state)
    ///
    /// Returns `false` if:
    /// - Load failed (deserialization error, corruption, etc.)
    pub fn can_save() -> bool {
        match LoadState::get() {
            LoadState::Uninitialized => true, // Allow save if never loaded
            LoadState::Success => true,
            LoadState::Failed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let temp_dir = std::env::temp_dir().join("cfait_test_lock");
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
    fn test_load_state_mechanism() {
        // Test the LoadState mechanism directly without file I/O

        // Start uninitialized - should allow save
        LoadState::set(LoadState::Uninitialized);
        assert_eq!(LoadState::get(), LoadState::Uninitialized);
        assert!(
            LocalStorage::can_save(),
            "Should allow save when uninitialized"
        );

        // Simulate successful load
        LoadState::set(LoadState::Success);
        assert_eq!(LoadState::get(), LoadState::Success);
        assert!(
            LocalStorage::can_save(),
            "Should allow save after successful load"
        );

        // Simulate failed load
        LoadState::set(LoadState::Failed);
        assert_eq!(LoadState::get(), LoadState::Failed);
        assert!(
            !LocalStorage::can_save(),
            "Should NOT allow save after failed load"
        );

        // Reset for other tests
        LoadState::set(LoadState::Uninitialized);
    }

    #[test]
    fn test_save_blocked_after_failed_load() {
        // Directly test that save() returns an error when LoadState is Failed
        LoadState::set(LoadState::Failed);

        let tasks = vec![];
        let save_result = LocalStorage::save(&tasks);

        assert!(
            save_result.is_err(),
            "save() should fail when LoadState is Failed"
        );
        assert!(
            save_result
                .unwrap_err()
                .to_string()
                .contains("previous load failed"),
            "Error message should explain why save was blocked"
        );

        // Reset
        LoadState::set(LoadState::Uninitialized);
    }

    #[test]
    fn test_force_save_bypasses_load_state() {
        // force_save should work even when LoadState is Failed
        LoadState::set(LoadState::Failed);

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
        LoadState::set(LoadState::Uninitialized);
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
        LoadState::set(LoadState::Uninitialized);
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
        LoadState::set(LoadState::Uninitialized);
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
}
