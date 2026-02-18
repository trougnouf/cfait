use crate::context::AppContext;
use crate::model::{CalendarListEntry, Task};
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

const CACHE_VERSION: u32 = 6;

#[derive(Serialize, Deserialize)]
struct CalendarCache {
    #[serde(default)]
    version: u32,
    sync_token: Option<String>,
    tasks: Vec<Task>,
}

pub struct Cache;

impl Cache {
    fn get_calendars_path(ctx: &dyn AppContext) -> Option<PathBuf> {
        ctx.get_cache_dir().ok().map(|p| p.join("calendars.json"))
    }

    fn get_path(ctx: &dyn AppContext, key: &str) -> Option<PathBuf> {
        ctx.get_cache_dir().ok().map(|dir| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            let filename = format!("tasks_{:x}.json", hasher.finish());
            dir.join(filename)
        })
    }

    pub fn save(
        ctx: &dyn AppContext,
        key: &str,
        tasks: &[Task],
        sync_token: Option<String>,
    ) -> Result<()> {
        if let Some(path) = Self::get_path(ctx, key) {
            LocalStorage::with_lock(&path, || {
                let data = CalendarCache {
                    version: CACHE_VERSION,
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

    pub fn load(ctx: &dyn AppContext, key: &str) -> Result<(Vec<Task>, Option<String>)> {
        if let Some(path) = Self::get_path(ctx, key)
            && path.exists()
        {
            return LocalStorage::with_lock(&path, || {
                let json = fs::read_to_string(&path)?;
                if let Ok(cache) = serde_json::from_str::<CalendarCache>(&json)
                    && cache.version == CACHE_VERSION
                {
                    return Ok((cache.tasks, cache.sync_token));
                }
                Ok((vec![], None))
            });
        }
        Ok((vec![], None))
    }

    pub fn save_calendars(ctx: &dyn AppContext, cals: &[CalendarListEntry]) -> Result<()> {
        if let Some(path) = Self::get_calendars_path(ctx) {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string_pretty(cals)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn load_calendars(ctx: &dyn AppContext) -> Result<Vec<CalendarListEntry>> {
        if let Some(path) = Self::get_calendars_path(ctx)
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
