// SPDX-License-Identifier: GPL-3.0-or-later
use crate::context::AppContext;
use crate::model::{CalendarListEntry, Task};
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

const CACHE_VERSION: u32 = 12;

/// Metadata for the settings-sync feature: where the settings live on the
/// server and which calendars were probed for custom-property support.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SettingsMeta {
    /// Calendar href where the settings property is currently stored.
    /// `None` means the settings are not (yet) stored as a property, i.e.
    /// the app is in fallback (VEVENT/VTODO object) mode.
    #[serde(default)]
    pub property_href: Option<String>,
    /// Calendar href -> whether the server accepted custom properties.
    #[serde(default)]
    pub probed: HashMap<String, bool>,
    /// Calendar href -> unix seconds of the last probe for that calendar.
    #[serde(default)]
    pub probed_at: HashMap<String, i64>,
}

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

    fn get_settings_meta_path(ctx: &dyn AppContext) -> Option<PathBuf> {
        ctx.get_cache_dir().ok().map(|p| p.join("settings_meta.json"))
    }

    pub fn load_settings_meta(ctx: &dyn AppContext) -> SettingsMeta {
        if let Some(path) = Self::get_settings_meta_path(ctx)
            && path.exists()
        {
            if let Ok(json) = fs::read_to_string(&path)
                && let Ok(meta) = serde_json::from_str::<SettingsMeta>(&json)
            {
                return meta;
            }
        }
        SettingsMeta::default()
    }

    pub fn save_settings_meta(ctx: &dyn AppContext, meta: &SettingsMeta) -> Result<()> {
        if let Some(path) = Self::get_settings_meta_path(ctx) {
            LocalStorage::with_lock(&path, || {
                let json = serde_json::to_string(meta)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
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
                let json = serde_json::to_string(&data)?;
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
                let json = serde_json::to_string(cals)?;
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

    /// Load the full disk state (calendars + tasks for every calendar, with the
    /// journal applied) in one pass. Used by the TUI/GUI to pick up changes made
    /// by other cfait instances without hitting the network.
    pub fn load_all_disk_state(
        ctx: &dyn AppContext,
        local_mode_enabled: bool,
    ) -> (Vec<CalendarListEntry>, Vec<(String, Vec<Task>)>) {
        let mut cached_cals = Self::load_calendars(ctx).unwrap_or_default();
        if local_mode_enabled && let Ok(locals) = crate::storage::LocalCalendarRegistry::load(ctx) {
            for loc in locals {
                if !cached_cals.iter().any(|c| c.href == loc.href) {
                    cached_cals.push(loc);
                }
            }
        }
        if !local_mode_enabled {
            cached_cals.retain(|c| !c.href.starts_with("local://"));
        }

        let mut cached_tasks = Vec::new();
        for cal in &cached_cals {
            if cal.href.starts_with("local://") {
                if let Ok(mut tasks) = crate::storage::LocalStorage::load_for_href(ctx, &cal.href) {
                    crate::journal::Journal::apply_to_tasks(ctx, &mut tasks, &cal.href);
                    cached_tasks.push((cal.href.clone(), tasks));
                }
            } else if let Ok((mut tasks, _)) = Self::load(ctx, &cal.href) {
                crate::journal::Journal::apply_to_tasks(ctx, &mut tasks, &cal.href);
                cached_tasks.push((cal.href.clone(), tasks));
            }
        }

        (cached_cals, cached_tasks)
    }
}
