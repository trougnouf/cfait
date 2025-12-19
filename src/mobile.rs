// File: src/mobile.rs
use crate::cache::Cache;
use crate::client::RustyClient;
use crate::config::Config;
use crate::model::Task;
use crate::paths::AppPaths;
use crate::storage::{LOCAL_CALENDAR_HREF, LOCAL_CALENDAR_NAME, LocalStorage};
use crate::store::{FilterOptions, TaskStore, UNCATEGORIZED_ID};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os = "android")]
use android_logger::Config as LogConfig;
#[cfg(target_os = "android")]
use log::LevelFilter;

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    Generic(String),
}
impl From<String> for MobileError {
    fn from(e: String) -> Self {
        Self::Generic(e)
    }
}
impl From<&str> for MobileError {
    fn from(e: &str) -> Self {
        Self::Generic(e.to_string())
    }
}
impl From<anyhow::Error> for MobileError {
    fn from(e: anyhow::Error) -> Self {
        Self::Generic(e.to_string())
    }
}
impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MobileError::Generic(s) => s,
            }
        )
    }
}
impl std::error::Error for MobileError {}

// --- DTOs ---

#[derive(uniffi::Record)]
pub struct MobileTask {
    pub uid: String,
    pub summary: String,
    pub description: String,
    pub is_done: bool,
    pub priority: u8,
    pub due_date_iso: Option<String>,
    pub start_date_iso: Option<String>,
    pub duration_mins: Option<u32>,
    pub calendar_href: String,
    pub categories: Vec<String>,
    pub is_recurring: bool,
    pub parent_uid: Option<String>,
    pub smart_string: String,
    pub depth: u32,
    pub is_blocked: bool,
    pub status_string: String,
    pub blocked_by_names: Vec<String>,
    pub blocked_by_uids: Vec<String>,
    // New field to expose paused state nicely
    pub is_paused: bool,
}

#[derive(uniffi::Record)]
pub struct MobileCalendar {
    pub name: String,
    pub href: String,
    pub color: Option<String>,
    pub is_visible: bool,
    pub is_local: bool,
    pub is_disabled: bool,
}

#[derive(uniffi::Record)]
pub struct MobileTag {
    pub name: String,
    pub count: u32,
    pub is_uncategorized: bool,
}

#[derive(uniffi::Record)]
pub struct MobileConfig {
    pub url: String,
    pub username: String,
    pub default_calendar: Option<String>,
    pub allow_insecure: bool,
    pub hide_completed: bool,
    pub tag_aliases: HashMap<String, Vec<String>>,
    pub disabled_calendars: Vec<String>,
}

fn task_to_mobile(t: &Task, store: &TaskStore) -> MobileTask {
    let smart = t.to_smart_string();
    let status_str = format!("{:?}", t.status);
    let is_blocked = store.is_blocked(t);
    let blocked_by_names = t
        .dependencies
        .iter()
        .filter_map(|uid| store.get_summary(uid))
        .collect();

    MobileTask {
        uid: t.uid.clone(),
        summary: t.summary.clone(),
        description: t.description.clone(),
        is_done: t.status.is_done(),
        priority: t.priority,
        due_date_iso: t.due.map(|d| d.to_rfc3339()),
        start_date_iso: t.dtstart.map(|d| d.to_rfc3339()),
        duration_mins: t.estimated_duration,
        calendar_href: t.calendar_href.clone(),
        categories: t.categories.clone(),
        is_recurring: t.rrule.is_some(),
        parent_uid: t.parent_uid.clone(),
        smart_string: smart,
        depth: t.depth as u32,
        is_blocked,
        status_string: status_str,
        blocked_by_names,
        blocked_by_uids: t.dependencies.clone(),
        is_paused: t.is_paused(),
    }
}

// --- MAIN OBJECT ---

#[derive(uniffi::Object)]
pub struct CfaitMobile {
    client: Arc<Mutex<Option<RustyClient>>>,
    store: Arc<Mutex<TaskStore>>,
}

// ============================================================================
// PUBLIC API (Exported to Kotlin/Swift)
// ============================================================================

#[uniffi::export(async_runtime = "tokio")]
impl CfaitMobile {
    #[uniffi::constructor]
    pub fn new(android_files_dir: String) -> Self {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            LogConfig::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("CfaitRust"),
        );
        AppPaths::init_android_path(android_files_dir);
        Self {
            client: Arc::new(Mutex::new(None)),
            store: Arc::new(Mutex::new(TaskStore::new())),
        }
    }

    pub fn has_unsynced_changes(&self) -> bool {
        !crate::journal::Journal::load().is_empty()
    }

    pub fn get_config(&self) -> MobileConfig {
        let c = Config::load().unwrap_or_default();
        MobileConfig {
            url: c.url,
            username: c.username,
            default_calendar: c.default_calendar,
            allow_insecure: c.allow_insecure_certs,
            hide_completed: c.hide_completed,
            tag_aliases: c.tag_aliases,
            disabled_calendars: c.disabled_calendars,
        }
    }

    pub fn save_config(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
        hide_completed: bool,
        disabled_calendars: Vec<String>,
    ) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.url = url;
        c.username = user;
        if !pass.is_empty() {
            c.password = pass;
        }
        c.allow_insecure_certs = insecure;
        c.hide_completed = hide_completed;
        c.disabled_calendars = disabled_calendars;
        c.save().map_err(MobileError::from)
    }

    // --- Relationship Management ---

    pub async fn add_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        if task_uid == blocker_uid {
            return Err(MobileError::from("Cannot depend on self"));
        }
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.add_dependency(id, blocker_uid)
        })
        .await
    }

    pub async fn remove_dependency(
        &self,
        task_uid: String,
        blocker_uid: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(task_uid, |store: &mut TaskStore, id: &str| {
            store.remove_dependency(id, &blocker_uid)
        })
        .await
    }

    pub async fn set_parent(
        &self,
        child_uid: String,
        parent_uid: Option<String>,
    ) -> Result<(), MobileError> {
        if let Some(p) = &parent_uid
            && *p == child_uid
        {
            return Err(MobileError::from("Cannot be child of self"));
        }
        self.apply_store_mutation(child_uid, |store: &mut TaskStore, id: &str| {
            store.set_parent(id, parent_uid)
        })
        .await
    }

    // --- Calendar Management ---

    pub fn isolate_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        let all_cals = self.get_calendars();
        let mut new_hidden = vec![];
        for cal in all_cals {
            if cal.href != href {
                new_hidden.push(cal.href.clone());
            }
        }
        config.hidden_calendars = new_hidden;
        config.default_calendar = Some(href);
        config.save().map_err(MobileError::from)
    }

    // --- Existing Methods ---

    pub async fn add_alias(&self, key: String, tags: Vec<String>) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.tag_aliases.insert(key.clone(), tags.clone());
        c.save().map_err(MobileError::from)?;

        let mut store = self.store.lock().await;
        let modified = store.apply_alias_retroactively(&key, &tags);
        drop(store);

        if !modified.is_empty() {
            let client_guard = self.client.lock().await;
            if let Some(client) = &*client_guard {
                for mut t in modified {
                    let _ = client.update_task(&mut t).await;
                }
            } else {
                for t in modified {
                    let _ = crate::journal::Journal::push(crate::journal::Action::Update(t));
                }
            }
        }
        Ok(())
    }

    pub fn remove_alias(&self, key: String) -> Result<(), MobileError> {
        let mut c = Config::load().unwrap_or_default();
        c.tag_aliases.remove(&key);
        c.save().map_err(MobileError::from)
    }

    pub fn set_default_calendar(&self, href: String) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        config.default_calendar = Some(href);
        config.save().map_err(MobileError::from)
    }

    pub fn set_calendar_visibility(&self, href: String, visible: bool) -> Result<(), MobileError> {
        let mut config = Config::load().unwrap_or_default();
        if visible {
            config.hidden_calendars.retain(|h| h != &href);
        } else if !config.hidden_calendars.contains(&href) {
            config.hidden_calendars.push(href);
        }
        config.save().map_err(MobileError::from)
    }

    pub fn load_from_cache(&self) {
        let mut store = self.store.blocking_lock();
        store.clear();

        let journal = crate::journal::Journal::load();

        if let Ok(mut local) = LocalStorage::load() {
            // Apply Journal to Local
            for action in &journal.queue {
                match action {
                    crate::journal::Action::Create(t) | crate::journal::Action::Update(t) => {
                        if t.calendar_href == LOCAL_CALENDAR_HREF {
                            if let Some(pos) = local.iter().position(|x| x.uid == t.uid) {
                                local[pos] = t.clone();
                            } else {
                                local.push(t.clone());
                            }
                        }
                    }
                    crate::journal::Action::Delete(t) => {
                        if t.calendar_href == LOCAL_CALENDAR_HREF {
                            local.retain(|x| x.uid != t.uid);
                        }
                    }
                    crate::journal::Action::Move(t, new_href) => {
                        if t.calendar_href == LOCAL_CALENDAR_HREF {
                            local.retain(|x| x.uid != t.uid);
                        } else if new_href == LOCAL_CALENDAR_HREF {
                            let mut mt = t.clone();
                            mt.calendar_href = new_href.clone();
                            local.push(mt);
                        }
                    }
                }
            }
            store.insert(LOCAL_CALENDAR_HREF.to_string(), local);
        }

        if let Ok(cals) = Cache::load_calendars() {
            for cal in cals {
                if cal.href == LOCAL_CALENDAR_HREF {
                    continue;
                }
                if let Ok((mut tasks, _)) = Cache::load(&cal.href) {
                    // Apply Journal to Cache
                    for action in &journal.queue {
                        match action {
                            crate::journal::Action::Create(t)
                            | crate::journal::Action::Update(t) => {
                                if t.calendar_href == cal.href {
                                    if let Some(pos) = tasks.iter().position(|x| x.uid == t.uid) {
                                        tasks[pos] = t.clone();
                                    } else {
                                        tasks.push(t.clone());
                                    }
                                }
                            }
                            crate::journal::Action::Delete(t) => {
                                if t.calendar_href == cal.href {
                                    tasks.retain(|x| x.uid != t.uid);
                                }
                            }
                            crate::journal::Action::Move(t, new_href) => {
                                if t.calendar_href == cal.href {
                                    tasks.retain(|x| x.uid != t.uid);
                                } else if new_href == &cal.href {
                                    let mut mt = t.clone();
                                    mt.calendar_href = new_href.clone();
                                    tasks.push(mt);
                                }
                            }
                        }
                    }
                    store.insert(cal.href, tasks);
                }
            }
        }
    }

    pub async fn sync(&self) -> Result<String, MobileError> {
        let config = Config::load().map_err(MobileError::from)?;
        self.apply_connection(config).await
    }

    pub async fn connect(
        &self,
        url: String,
        user: String,
        pass: String,
        insecure: bool,
    ) -> Result<String, MobileError> {
        let mut config = Config::load().unwrap_or_default();
        config.url = url;
        config.username = user;
        if !pass.is_empty() {
            config.password = pass;
        }
        config.allow_insecure_certs = insecure;
        self.apply_connection(config).await
    }

    // --- Getters ---

    pub fn get_calendars(&self) -> Vec<MobileCalendar> {
        let config = Config::load().unwrap_or_default();
        let disabled_set: HashSet<String> = config.disabled_calendars.iter().cloned().collect();
        let mut result = Vec::new();
        let local_href = LOCAL_CALENDAR_HREF.to_string();
        result.push(MobileCalendar {
            name: LOCAL_CALENDAR_NAME.to_string(),
            href: local_href.clone(),
            color: None,
            is_visible: !config.hidden_calendars.contains(&local_href),
            is_local: true,
            is_disabled: false,
        });
        if let Ok(cals) = crate::cache::Cache::load_calendars() {
            for c in cals {
                if c.href == LOCAL_CALENDAR_HREF {
                    continue;
                }
                result.push(MobileCalendar {
                    name: c.name,
                    href: c.href.clone(),
                    color: c.color,
                    is_visible: !config.hidden_calendars.contains(&c.href),
                    is_local: false,
                    is_disabled: disabled_set.contains(&c.href),
                });
            }
        }
        result
    }

    pub async fn get_all_tags(&self) -> Vec<MobileTag> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let empty_includes = HashSet::new();
        let mut hidden_cals: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden_cals.extend(config.disabled_calendars);
        store
            .get_all_categories(
                config.hide_completed,
                config.hide_fully_completed_tags,
                &empty_includes,
                &hidden_cals,
            )
            .into_iter()
            .map(|(name, count)| MobileTag {
                name: name.clone(),
                count: count as u32,
                is_uncategorized: name == UNCATEGORIZED_ID,
            })
            .collect()
    }

    pub async fn get_view_tasks(
        &self,
        filter_tag: Option<String>,
        search_query: String,
    ) -> Vec<MobileTask> {
        let store = self.store.lock().await;
        let config = Config::load().unwrap_or_default();
        let mut selected_categories = HashSet::new();
        if let Some(tag) = filter_tag {
            selected_categories.insert(tag);
        }
        let mut hidden: HashSet<String> = config.hidden_calendars.into_iter().collect();
        hidden.extend(config.disabled_calendars);
        let cutoff_date = config
            .sort_cutoff_months
            .map(|months| chrono::Utc::now() + chrono::Duration::days(months as i64 * 30));
        let filtered = store.filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &hidden,
            selected_categories: &selected_categories,
            match_all_categories: false,
            search_term: &search_query,
            hide_completed_global: config.hide_completed,
            cutoff_date,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
        });
        filtered
            .into_iter()
            .map(|t| task_to_mobile(&t, &store))
            .collect()
    }

    // --- Task Actions ---

    pub async fn yank_task(&self, _uid: String) -> Result<(), MobileError> {
        Ok(())
    }

    pub async fn add_task_smart(&self, input: String) -> Result<String, MobileError> {
        let aliases = Config::load().unwrap_or_default().tag_aliases;
        let mut task = Task::new(&input, &aliases);
        let config = Config::load().unwrap_or_default();
        let target_href = config
            .default_calendar
            .clone()
            .unwrap_or(LOCAL_CALENDAR_HREF.to_string());
        task.calendar_href = target_href.clone();

        // 1. OPTIMISTIC UPDATE: Add to store immediately
        self.store.lock().await.add_task(task.clone());

        let guard = self.client.lock().await;
        if let Some(client) = &*guard {
            // 2. Network Call
            match client.create_task(&mut task).await {
                Ok(_) => {
                    // 3. SUCCESS: Update store with new ETag/Href from server
                    self.store.lock().await.update_or_add_task(task.clone());
                }
                Err(e) => {
                    // 4. FAILURE: Remove the optimistic task so it doesn't get stuck?
                    // Or keep it for offline sync later?
                    // For now, let's leave it (it behaves like offline) but log error
                    // Ideally, we would push to Journal here if network fails.
                    return Err(MobileError::from(e));
                }
            }
        } else {
            // Offline fallback
            if task.calendar_href == LOCAL_CALENDAR_HREF {
                let mut all = LocalStorage::load().unwrap_or_default();
                all.push(task.clone());
                LocalStorage::save(&all).map_err(MobileError::from)?;
            } else {
                crate::journal::Journal::push(crate::journal::Action::Create(task.clone()))
                    .map_err(MobileError::from)?;
            }
            // Update store again to be sure (though step 1 covered it)
            self.store.lock().await.add_task(task.clone());
        }

        Ok(task.uid)
    }

    pub async fn change_priority(&self, uid: String, delta: i8) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.change_priority(id, delta)
        })
        .await
    }

    pub async fn set_status_process(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status(id, crate::model::TaskStatus::InProcess)
        })
        .await
    }

    pub async fn set_status_cancelled(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status(id, crate::model::TaskStatus::Cancelled)
        })
        .await
    }

    // --- NEW: Specific Mobile Actions for Pause/Stop ---

    pub async fn pause_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| store.pause_task(id))
            .await
    }

    pub async fn stop_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| store.stop_task(id))
            .await
    }

    pub async fn start_task(&self, uid: String) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |store: &mut TaskStore, id: &str| {
            store.set_status_in_process(id)
        })
        .await
    }

    // ---------------------------------------------------

    pub async fn update_task_smart(
        &self,
        uid: String,
        smart_input: String,
    ) -> Result<(), MobileError> {
        let aliases = Config::load().unwrap_or_default().tag_aliases;
        self.apply_store_mutation(uid, |t: &mut TaskStore, id: &str| {
            if let Some((task, _)) = t.get_task_mut(id) {
                task.apply_smart_input(&smart_input, &aliases);
                Some(task.clone())
            } else {
                None
            }
        })
        .await
    }

    pub async fn update_task_description(
        &self,
        uid: String,
        description: String,
    ) -> Result<(), MobileError> {
        self.apply_store_mutation(uid, |t: &mut TaskStore, id: &str| {
            if let Some((task, _)) = t.get_task_mut(id) {
                task.description = description;
                Some(task.clone())
            } else {
                None
            }
        })
        .await
    }

    pub async fn toggle_task(&self, uid: String) -> Result<(), MobileError> {
        // 1. Logic via Store
        let mut store = self.store.lock().await;
        let (task, _) = store
            .get_task_mut(&uid)
            .ok_or(MobileError::from("Task not found"))?;

        if task.status.is_done() {
            task.status = crate::model::TaskStatus::NeedsAction;
            task.percent_complete = None; // <--- FIX: Reset progress
        } else {
            task.status = crate::model::TaskStatus::Completed;
            task.percent_complete = Some(100); // <--- FIX: Ensure 100%
        }

        let mut task_for_net = task.clone();
        drop(store);

        // 2. Client Operation
        let client_guard = self.client.lock().await;

        if let Some(client) = &*client_guard {
            let (_, next_task_opt, _) = client
                .toggle_task(&mut task_for_net)
                .await
                .map_err(MobileError::from)?;
            if let Some(next_task) = next_task_opt {
                let mut store = self.store.lock().await;
                store.update_or_add_task(next_task);
            }
        } else {
            // Offline
            if task_for_net.calendar_href == LOCAL_CALENDAR_HREF {
                let mut local = LocalStorage::load().unwrap_or_default();
                if let Some(idx) = local.iter().position(|t| t.uid == task_for_net.uid) {
                    local[idx] = task_for_net;
                    LocalStorage::save(&local).map_err(MobileError::from)?;
                }
            } else {
                crate::journal::Journal::push(crate::journal::Action::Update(task_for_net))
                    .map_err(MobileError::from)?;
            }
        }
        Ok(())
    }

    pub async fn move_task(&self, uid: String, new_cal_href: String) -> Result<(), MobileError> {
        let client_guard = self.client.lock().await;
        let mut store = self.store.lock().await;

        let updated_task = store
            .move_task(&uid, new_cal_href.clone())
            .ok_or(MobileError::from("Task not found"))?;

        drop(store);

        if let Some(client) = &*client_guard {
            client
                .move_task(&updated_task, &new_cal_href)
                .await
                .map_err(MobileError::from)?;
        } else if new_cal_href != LOCAL_CALENDAR_HREF {
            crate::journal::Journal::push(crate::journal::Action::Move(updated_task, new_cal_href))
                .map_err(MobileError::from)?;
        }
        Ok(())
    }

    pub async fn delete_task(&self, uid: String) -> Result<(), MobileError> {
        let mut store = self.store.lock().await;
        let (task, href) = store
            .delete_task(&uid)
            .ok_or(MobileError::from("Task not found"))?;
        drop(store);

        let client_guard = self.client.lock().await;
        if let Some(client) = &*client_guard {
            client.delete_task(&task).await.map_err(MobileError::from)?;
        } else if href != LOCAL_CALENDAR_HREF {
            crate::journal::Journal::push(crate::journal::Action::Delete(task))
                .map_err(MobileError::from)?;
        }
        Ok(())
    }

    pub async fn migrate_local_to(
        &self,
        target_calendar_href: String,
    ) -> Result<String, MobileError> {
        let client_guard = self.client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or(MobileError::from("Client not connected"))?;

        let local_tasks = LocalStorage::load().map_err(|e| MobileError::from(e.to_string()))?;
        if local_tasks.is_empty() {
            return Ok("No local tasks to migrate.".to_string());
        }

        let count = client
            .migrate_tasks(local_tasks, &target_calendar_href)
            .await
            .map_err(MobileError::from)?;

        Ok(format!("Successfully migrated {} tasks.", count))
    }
}

// ============================================================================
// INTERNAL HELPERS
// ============================================================================

impl CfaitMobile {
    async fn apply_connection(&self, config: Config) -> Result<String, MobileError> {
        let (client, cals, _, _, warning) = RustyClient::connect_with_fallback(config)
            .await
            .map_err(MobileError::from)?;
        *self.client.lock().await = Some(client.clone());
        let mut store = self.store.lock().await;
        store.clear();
        if let Ok(local) = LocalStorage::load() {
            store.insert(LOCAL_CALENDAR_HREF.to_string(), local);
        }

        match client.get_all_tasks(&cals).await {
            Ok(results) => {
                for (href, tasks) in results {
                    store.insert(href, tasks);
                }
            }
            Err(e) => {
                for cal in &cals {
                    if cal.href != LOCAL_CALENDAR_HREF
                        && !store.calendars.contains_key(&cal.href)
                        && let Ok((cached, _)) = crate::cache::Cache::load(&cal.href)
                    {
                        store.insert(cal.href.clone(), cached);
                    }
                }
                if warning.is_none() {
                    return Err(MobileError::from(e));
                }
            }
        }
        Ok(warning.unwrap_or_else(|| "Connected".to_string()))
    }

    // Helper to abstract Store mutation -> Client/Journal logic
    async fn apply_store_mutation<F>(&self, uid: String, mutator: F) -> Result<(), MobileError>
    where
        F: FnOnce(&mut TaskStore, &str) -> Option<Task>,
    {
        let mut store = self.store.lock().await;
        // 1. Mutate and get the updated task
        let updated_task = mutator(&mut store, &uid)
            .ok_or(MobileError::from("Task not found or mutation failed"))?;

        // 2. OPTIMISTIC: Save to store immediately (The mutator usually modifies in-place,
        // but let's ensure persistence if needed or just drop the lock).
        // Since `mutator` takes `&mut TaskStore`, the map is ALREADY updated in memory!
        // We just need to clone the task for the network before dropping the lock.
        let mut task_for_net = updated_task.clone();
        drop(store);

        let client_guard = self.client.lock().await;

        if let Some(client) = &*client_guard {
            // 3. Network Call
            client
                .update_task(&mut task_for_net)
                .await
                .map_err(MobileError::from)?;

            // 4. Update Store with Server Response (ETag/Sequence updates)
            let mut store = self.store.lock().await;
            store.update_or_add_task(task_for_net);
        } else if task_for_net.calendar_href == LOCAL_CALENDAR_HREF {
            let mut local = LocalStorage::load().unwrap_or_default();
            if let Some(idx) = local.iter().position(|t| t.uid == task_for_net.uid) {
                local[idx] = task_for_net;
                LocalStorage::save(&local).map_err(MobileError::from)?;
            }
        } else {
            crate::journal::Journal::push(crate::journal::Action::Update(task_for_net))
                .map_err(MobileError::from)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::sync::Arc;
    use tokio::runtime::Runtime;

    #[test]
    fn test_android_concurrency_stress() {
        let rt = Runtime::new().unwrap();
        let temp_dir = env::temp_dir().join("cfait_mobile_stress");
        let _ = fs::create_dir_all(&temp_dir);
        let path_str = temp_dir.to_string_lossy().to_string();

        let api = Arc::new(CfaitMobile::new(path_str));

        rt.block_on(async {
            api.add_task_smart("Initial Task !1".to_string())
                .await
                .unwrap();
        });

        let api_write = api.clone();
        let write_handle = rt.spawn(async move {
            for i in 0..50 {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                let _ = api_write.add_task_smart(format!("Stress Task {}", i)).await;
                let tasks = api_write.get_view_tasks(None, "".to_string()).await;
                if let Some(t) = tasks.first() {
                    let _ = api_write.toggle_task(t.uid.clone()).await;
                }
            }
        });

        let api_read = api.clone();
        let read_handle = rt.spawn(async move {
            for _ in 0..50 {
                let tasks = api_read.get_view_tasks(None, "".to_string()).await;
                for t in tasks {
                    assert!(!t.uid.is_empty());
                }
                let _ = api_read.get_config();
            }
        });

        rt.block_on(async {
            let _ = tokio::join!(write_handle, read_handle);
        });

        rt.block_on(async {
            let tasks = api.get_view_tasks(None, "".to_string()).await;
            assert_eq!(
                tasks.len(),
                51,
                "Data loss detected during concurrent Mobile access"
            );
        });

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_mobile_unsynced_state_flag() {
        let temp_dir = env::temp_dir().join(format!("cfait_test_unsynced_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let config_dir = temp_dir.join("config");
        fs::create_dir_all(&config_dir).unwrap();

        fs::write(
            config_dir.join("config.toml"),
            r#"
            url = "http://mock.server"
            username = "user"
            password = "password"
            default_calendar = "http://mock.server/remote-cal/"
            "#,
        )
        .unwrap();

        let api = CfaitMobile::new(temp_dir.to_string_lossy().to_string());

        assert!(!api.has_unsynced_changes());

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            api.add_task_smart("Offline task for a remote calendar".to_string())
                .await
                .unwrap();
        });

        assert!(api.has_unsynced_changes());

        let _ = fs::remove_dir_all(temp_dir);
    }
}
