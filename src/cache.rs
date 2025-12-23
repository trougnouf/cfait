// File: src/cache.rs
use crate::model::{CalendarListEntry, Task};
use crate::paths::AppPaths;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

// Increment this whenever the Task struct changes (e.g., new fields like location)
const CACHE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CalendarCache {
    // If this field is missing in the JSON (old cache), it defaults to 0.
    #[serde(default)]
    version: u32,
    sync_token: Option<String>,
    tasks: Vec<Task>,
}

pub struct Cache;

impl Cache {
    fn get_calendars_path() -> Option<PathBuf> {
        AppPaths::get_cache_dir()
            .ok()
            .map(|p| p.join("calendars.json"))
    }

    fn get_path(key: &str) -> Option<PathBuf> {
        AppPaths::get_cache_dir().ok().map(|dir| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            let filename = format!("tasks_{:x}.json", hasher.finish());
            dir.join(filename)
        })
    }

    pub fn save(key: &str, tasks: &[Task], sync_token: Option<String>) -> Result<()> {
        if let Some(path) = Self::get_path(key) {
            LocalStorage::with_lock(&path, || {
                let data = CalendarCache {
                    version: CACHE_VERSION, // Write the current version (1)
                    sync_token: sync_token.clone(),
                    tasks: tasks.to_vec(),
                };
                let json = serde_json::to_string_pretty(&data)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn load(key: &str) -> Result<(Vec<Task>, Option<String>)> {
        if let Some(path) = Self::get_path(key)
            && path.exists()
        {
            return LocalStorage::with_lock(&path, || {
                let json = fs::read_to_string(&path)?;
                if let Ok(cache) = serde_json::from_str::<CalendarCache>(&json) {
                    // VERSION CHECK:
                    // If the file is from v0.3.7, 'version' will be 0 (default).
                    // Since CACHE_VERSION is 1, this check fails, and we fall through.
                    if cache.version == CACHE_VERSION {
                        return Ok((cache.tasks, cache.sync_token));
                    }
                }

                // Fallback for older cache format (just array) - kept for safety,
                // but effectively these will also be invalidated by the version logic above
                // or simply treated as empty/invalid.
                if let Ok(_tasks) = serde_json::from_str::<Vec<Task>>(&json) {
                    // Logic decision: Do we accept raw arrays?
                    // Since they definitely don't have the version field,
                    // we should probably ignore them to enforce the new schema.
                    // Returning empty forces a re-sync.
                    return Ok((vec![], None));
                }

                // If version mismatch or parse error, return empty to trigger full sync
                Ok((vec![], None))
            });
        }
        Ok((vec![], None))
    }

    // ... save_calendars and load_calendars remain unchanged ...
    pub fn save_calendars(cals: &[CalendarListEntry]) -> Result<()> {
        if let Some(path) = Self::get_calendars_path() {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string_pretty(cals)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn load_calendars() -> Result<Vec<CalendarListEntry>> {
        if let Some(path) = Self::get_calendars_path()
            && path.exists()
        {
            return LocalStorage::with_lock(&path, || {
                let json = fs::read_to_string(&path)?;
                let cals: Vec<CalendarListEntry> = serde_json::from_str(&json)?;
                Ok(cals)
            });
        }
        Ok(vec![])
    }
}
