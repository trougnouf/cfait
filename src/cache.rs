// Caching mechanism for storing remote tasks locally.
//
// ⚠️ VERSION BUMP REQUIRED:
// Changes to Task struct or its nested types (Alarm, DateType, etc.) require
// incrementing CACHE_VERSION below to invalidate stale caches.
use crate::context::{AppContext, default_shared_context};
use crate::model::{CalendarListEntry, Task};
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
    // Internal variant that is explicitly passed a context. New/modern call sites should use these.
    fn get_calendars_path_with_ctx(ctx: &dyn AppContext) -> Option<PathBuf> {
        ctx.get_cache_dir().ok().map(|p| p.join("calendars.json"))
    }

    fn get_path_with_ctx(ctx: &dyn AppContext, key: &str) -> Option<PathBuf> {
        ctx.get_cache_dir().ok().map(|dir| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            let filename = format!("tasks_{:x}.json", hasher.finish());
            dir.join(filename)
        })
    }

    pub fn save_with_ctx(
        ctx: &dyn AppContext,
        key: &str,
        tasks: &[Task],
        sync_token: Option<String>,
    ) -> Result<()> {
        if let Some(path) = Self::get_path_with_ctx(ctx, key) {
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

    pub fn load_with_ctx(ctx: &dyn AppContext, key: &str) -> Result<(Vec<Task>, Option<String>)> {
        if let Some(path) = Self::get_path_with_ctx(ctx, key)
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

    pub fn save_calendars_with_ctx(ctx: &dyn AppContext, cals: &[CalendarListEntry]) -> Result<()> {
        if let Some(path) = Self::get_calendars_path_with_ctx(ctx) {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string_pretty(cals)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn load_calendars_with_ctx(ctx: &dyn AppContext) -> Result<Vec<CalendarListEntry>> {
        if let Some(path) = Self::get_calendars_path_with_ctx(ctx)
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

    // ---------------------------------------------------------------------
    // Backward-compatible wrappers:
    // Existing call sites that haven't been migrated can continue calling
    // the original signatures (without an AppContext). These wrappers use the
    // default shared context internally and delegate to the new `_with_ctx`
    // implementations. When migrating files, prefer calling the `_with_ctx`
    // methods directly and passing an explicit `&impl AppContext`.
    // ---------------------------------------------------------------------

    pub fn save(key: &str, tasks: &[Task], sync_token: Option<String>) -> Result<()> {
        let ctx = default_shared_context();
        Self::save_with_ctx(ctx.as_ref(), key, tasks, sync_token)
    }

    pub fn load(key: &str) -> Result<(Vec<Task>, Option<String>)> {
        let ctx = default_shared_context();
        Self::load_with_ctx(ctx.as_ref(), key)
    }

    pub fn save_calendars(cals: &[CalendarListEntry]) -> Result<()> {
        let ctx = default_shared_context();
        Self::save_calendars_with_ctx(ctx.as_ref(), cals)
    }

    pub fn load_calendars() -> Result<Vec<CalendarListEntry>> {
        let ctx = default_shared_context();
        Self::load_calendars_with_ctx(ctx.as_ref())
    }
}
