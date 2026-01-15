// Caching mechanism for storing remote tasks locally.
//
// ⚠️ VERSION BUMP REQUIRED:
// Changes to Task struct or its nested types (Alarm, DateType, etc.) require
// incrementing CACHE_VERSION below to invalidate stale caches.
use crate::model::{CalendarListEntry, Task};
use crate::paths::AppPaths;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

// Increment this whenever the Task struct changes (e.g., new fields like create_event) to invalidate old caches
const CACHE_VERSION: u32 = 5; // last update: added estimated_duration_max field

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
                // Try parsing the new versioned format first
                if let Ok(cache) = serde_json::from_str::<CalendarCache>(&json) {
                    // If versions match, use the cached data
                    if cache.version == CACHE_VERSION {
                        return Ok((cache.tasks, cache.sync_token));
                    }
                }
                // If version mismatch or any parsing error occurs, treat cache as invalid
                // to force a full re-sync.
                Ok((vec![], None))
            });
        }
        Ok((vec![], None))
    }

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
