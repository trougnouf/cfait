/* cfait/src/store.rs
 *
 * Optimized in-memory store for tasks.
 *
 * Changes:
 * - `calendars` map: HashMap<CalendarHref, HashMap<Uid, Task>> for O(1) UID lookups.
 * - Filter pipeline operates on &Task references and only clones final results.
 * - get_task / get_task_mut use direct map lookups.
 *
 * Context injection:
 * - TaskStore now stores an `Arc<dyn AppContext>` to perform file IO without
 *   relying on global state.
 *
 * NOTE: This file includes a small behavioral change: `set_status` and
 * `toggle_task` now automatically reset completed children (set them to
 * `NeedsAction`, clear percent_complete and remove `COMPLETED` unmapped
 * property) when a recurring parent task is completed and a next-instance
 * (secondary) is created. The function returns the list of reset children
 * so callers can persist/sync them.
 */

use crate::cache::Cache;
use crate::context::AppContext;
use crate::model::{Task, TaskStatus};
use crate::storage::LocalStorage;
use chrono::{DateTime, Utc};
use fastrand;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const UNCATEGORIZED_ID: &str = ":::uncategorized:::";

pub struct FilterResult {
    pub tasks: Vec<Task>,
    pub categories: Vec<(String, usize)>,
    pub locations: Vec<(String, usize)>,
}

/// Select an index from `tasks` at random weighted by priority.
/// Tasks with priority 0 use the provided `default_priority`.
/// Lower numeric priority indicates higher importance; we invert
/// to produce weights where priority 1 -> weight 9, priority 9 -> weight 1.
///
/// Filters for "is:ready" criteria:
/// - Must not be done (Completed/Cancelled)
/// - Must not be blocked (is_blocked)
/// - Must not have a future start date
///
/// Returns `None` for an empty slice or if no tasks qualify.
pub fn select_weighted_random_index(tasks: &[Task], default_priority: u8) -> Option<usize> {
    if tasks.is_empty() {
        return None;
    }

    let now = Utc::now();

    let weights: Vec<u32> = tasks
        .iter()
        .map(|t| {
            // 1. Must be active (not done/cancelled)
            if t.status.is_done() {
                return 0;
            }

            // 2. Must not be blocked (is:ready logic)
            // Note: is_blocked is a transient field populated by store.filter()
            if t.is_blocked {
                return 0;
            }

            // 3. Must not start in the future (is:ready logic)
            if let Some(start) = &t.dtstart
                && start.to_start_comparison_time() > now
            {
                return 0;
            }

            // Use the passed default_priority instead of any hardcoded value
            let p = if t.priority == 0 {
                default_priority
            } else {
                t.priority
            };
            // Invert priority so 1 is high weight (range 1..=9 -> weight 9..=1)
            (10u32).saturating_sub(p as u32)
        })
        .collect();

    let total_weight: u32 = weights.iter().sum();
    if total_weight == 0 {
        return None;
    }

    let mut rng = fastrand::Rng::new();
    let mut choice = rng.u32(0..total_weight);

    for (i, w) in weights.iter().enumerate() {
        if *w == 0 {
            continue;
        }
        if choice < *w {
            return Some(i);
        }
        choice -= *w;
    }

    None
}

#[derive(Debug, Clone)]
pub struct TaskStore {
    // OPTIMIZATION: Changed inner type from Vec<Task> to HashMap<Uid, Task> for O(1) lookups
    pub calendars: HashMap<String, HashMap<String, Task>>,
    pub index: HashMap<String, String>, // UID -> CalendarHref mapping
    pub related_from_index: HashMap<String, Vec<String>>,
    // Maps a Task UID -> List of tasks that are blocked BY this task
    pub blocking_index: HashMap<String, Vec<String>>,
    // Context for filesystem operations and test isolation
    pub ctx: Arc<dyn AppContext>,
}

pub struct FilterOptions<'a> {
    pub active_cal_href: Option<&'a str>,
    pub hidden_calendars: &'a std::collections::HashSet<String>,
    pub selected_categories: &'a HashSet<String>,
    pub selected_locations: &'a HashSet<String>,
    pub match_all_categories: bool,
    pub search_term: &'a str,
    pub hide_completed_global: bool,
    pub hide_fully_completed_tags: bool,
    pub cutoff_date: Option<DateTime<Utc>>,
    pub min_duration: Option<u32>,
    pub max_duration: Option<u32>,
    pub include_unset_duration: bool,
    pub urgent_days: u32,
    pub urgent_prio: u8,
    pub default_priority: u8,
    pub start_grace_period_days: u32,
    pub expanded_done_groups: &'a HashSet<String>,
    pub max_done_roots: usize,
    pub max_done_subtasks: usize,
}

impl TaskStore {
    /// Creates a new TaskStore with an explicit AppContext.
    pub fn new(ctx: Arc<dyn AppContext>) -> Self {
        Self {
            calendars: HashMap::new(),
            index: HashMap::new(),
            related_from_index: HashMap::new(),
            blocking_index: HashMap::new(),
            ctx,
        }
    }

    /// Efficiently checks if there are any tasks in the store across all calendars.
    /// Used to differentiate between "App is empty" and "Filters matched nothing".
    pub fn has_any_tasks(&self) -> bool {
        self.calendars.values().any(|map| !map.is_empty())
    }

    pub fn insert(&mut self, calendar_href: String, tasks: Vec<Task>) {
        let mut map = HashMap::new();
        for task in tasks {
            self.index.insert(task.uid.clone(), calendar_href.clone());
            map.insert(task.uid.clone(), task);
        }
        self.calendars.insert(calendar_href, map);
        self.rebuild_relation_index();
    }

    pub fn add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());

        self.calendars
            .entry(href)
            .or_default()
            .insert(task.uid.clone(), task);

        self.rebuild_relation_index();
    }

    pub fn update_or_add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());
        let cal_map = self.calendars.entry(href.clone()).or_default();

        cal_map.insert(task.uid.clone(), task);

        // Convert Map values back to Vec for persistence (legacy compatibility)
        let list: Vec<Task> = cal_map.values().cloned().collect();

        if href.starts_with("local://") {
            let _ = LocalStorage::save_for_href(self.ctx.as_ref(), &href, &list);
        } else {
            let (_, token) = Cache::load(self.ctx.as_ref(), &href).unwrap_or((vec![], None));
            let _ = Cache::save(self.ctx.as_ref(), &href, &list, token);
        }

        self.rebuild_relation_index();
    }

    pub fn clear(&mut self) {
        self.calendars.clear();
        self.index.clear();
        self.related_from_index.clear();
        self.blocking_index.clear();
    }

    pub fn remove(&mut self, calendar_href: &str) {
        if let Some(tasks_map) = self.calendars.remove(calendar_href) {
            for uid in tasks_map.keys() {
                self.index.remove(uid);
            }
        }
        self.rebuild_relation_index();
    }

    // OPTIMIZATION: O(1) Lookup
    pub fn get_task_mut(&mut self, uid: &str) -> Option<(&mut Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(map) = self.calendars.get_mut(&href)
            && let Some(task) = map.get_mut(uid)
        {
            return Some((task, href));
        }
        // Inconsistent state fix
        self.index.remove(uid);
        None
    }

    // OPTIMIZATION: O(1) Lookup
    pub fn get_task_ref(&self, uid: &str) -> Option<&Task> {
        let href = self.index.get(uid)?;
        self.calendars.get(href).and_then(|map| map.get(uid))
    }

    /// Toggle the task status (Completed <-> NeedsAction).
    /// CHANGED: returns the optional secondary task (next instance for recurrences)
    /// and a Vec of any children that were auto-reset as part of a recurring completion.
    pub fn toggle_task(&mut self, uid: &str) -> Option<(Task, Option<Task>, Vec<Task>)> {
        let current_status = self.get_task_ref(uid)?.status;
        let next_status = if current_status == TaskStatus::Completed {
            TaskStatus::NeedsAction
        } else {
            TaskStatus::Completed
        };
        self.set_status(uid, next_status)
    }

    /// Set status for a given task uid.
    /// CHANGED: Signature returns a Vec<Task> containing any child tasks that were auto-reset.
    pub fn set_status(
        &mut self,
        uid: &str,
        status: TaskStatus,
    ) -> Option<(Task, Option<Task>, Vec<Task>)> {
        // 1. Get copy of task to modify
        let (task_ref, _href) = self.get_task_mut(uid)?;
        let task_copy = task_ref.clone();

        // Check if this is a recurring completion that requires child reset
        let should_reset_children = task_copy.rrule.is_some() && status.is_done();

        // 2. Perform logic (recycle or simple update) via model-level helper
        // NOTE: `recycle` is a model-level helper that returns (primary, secondary)
        // where `primary` is the history/updated task and `secondary` is the next instance.
        let (primary, secondary) = task_copy.recycle(status);

        // 3. Save Primary (This is either the history item OR the simple updated task)
        self.update_or_add_task(primary.clone());

        // 4. Save Secondary (This is the Recycled/Next instance if it exists)
        if let Some(sec) = &secondary {
            self.update_or_add_task(sec.clone());
        }

        // 5. Reset ALL Descendants if applicable
        let mut reset_children: Vec<Task> = Vec::new();

        if should_reset_children && secondary.is_some() {
            // Build adjacency map for full hierarchy traversal (Parent -> Vec<Children>)
            // We scan all calendars because children might technically live in different lists (though rare)
            let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
            for map in self.calendars.values() {
                for t in map.values() {
                    if let Some(p) = &t.parent_uid {
                        adjacency.entry(p.clone()).or_default().push(t.uid.clone());
                    }
                }
            }

            // BFS Traversal to find all descendants
            let mut queue = vec![uid.to_string()];
            let mut descendants = HashSet::new();

            while let Some(parent) = queue.pop() {
                if let Some(children) = adjacency.get(&parent) {
                    for child_uid in children {
                        if descendants.insert(child_uid.clone()) {
                            queue.push(child_uid.clone());
                        }
                    }
                }
            }

            // Reset found descendants
            for child_uid in descendants {
                if let Some((child, _)) = self.get_task_mut(&child_uid) {
                    // Only modify if it's actually done
                    if child.status.is_done() {
                        child.status = TaskStatus::NeedsAction;
                        child.percent_complete = None;
                        child
                            .unmapped_properties
                            .retain(|p| p.key.to_uppercase() != "COMPLETED");

                        let child_copy = child.clone();
                        self.update_or_add_task(child_copy.clone());
                        reset_children.push(child_copy);
                    }
                }
            }
        }

        Some((primary, secondary, reset_children))
    }

    pub fn set_status_in_process(&mut self, uid: &str) -> Vec<Task> {
        let mut updated = Vec::new();
        let mut current_uid = uid.to_string();
        let now = Utc::now().timestamp();

        loop {
            if let Some((task, _)) = self.get_task_mut(&current_uid) {
                let mut changed = false;
                if task.status != TaskStatus::InProcess {
                    task.status = TaskStatus::InProcess;
                    changed = true;
                }
                if task.last_started_at.is_none() {
                    task.last_started_at = Some(now);
                    changed = true;
                }
                if changed {
                    updated.push(task.clone());
                }
                if let Some(p) = task.parent_uid.clone() {
                    current_uid = p;
                    continue;
                }
            }
            break;
        }
        updated
    }

    pub fn pause_task(&mut self, uid: &str) -> Vec<Task> {
        let mut updated = Vec::new();
        let now = Utc::now().timestamp();
        let mut queue = vec![uid.to_string()];
        let mut visited = HashSet::new();

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        for map in self.calendars.values() {
            for t in map.values() {
                if let Some(p) = &t.parent_uid {
                    adjacency.entry(p.clone()).or_default().push(t.uid.clone());
                }
            }
        }

        while let Some(current_uid) = queue.pop() {
            if !visited.insert(current_uid.clone()) {
                continue;
            }

            if let Some((task, _)) = self.get_task_mut(&current_uid) {
                let mut changed = false;
                if task.status == TaskStatus::InProcess {
                    task.status = TaskStatus::NeedsAction;
                    let current_pc = task.percent_complete.unwrap_or(0);
                    if current_pc == 0 {
                        task.percent_complete = Some(50);
                    }
                    changed = true;
                }

                if let Some(start) = task.last_started_at {
                    if now > start {
                        let duration = (now - start) as u64;
                        task.time_spent_seconds = task.time_spent_seconds.saturating_add(duration);
                        if duration > 60 {
                            task.sessions
                                .push(crate::model::item::WorkSession { start, end: now });
                        }
                    }
                    task.last_started_at = None;
                    changed = true;
                }

                if changed {
                    updated.push(task.clone());
                }

                if let Some(children) = adjacency.get(&current_uid) {
                    queue.extend(children.clone());
                }
            }
        }
        updated
    }

    pub fn stop_task(&mut self, uid: &str) -> Vec<Task> {
        let mut updated = Vec::new();
        let now = Utc::now().timestamp();
        let mut queue = vec![uid.to_string()];
        let mut visited = HashSet::new();

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        for map in self.calendars.values() {
            for t in map.values() {
                if let Some(p) = &t.parent_uid {
                    adjacency.entry(p.clone()).or_default().push(t.uid.clone());
                }
            }
        }

        while let Some(current_uid) = queue.pop() {
            if !visited.insert(current_uid.clone()) {
                continue;
            }

            if let Some((task, _)) = self.get_task_mut(&current_uid) {
                let mut changed = false;
                if task.status != TaskStatus::NeedsAction || task.percent_complete.is_some() {
                    task.status = TaskStatus::NeedsAction;
                    task.percent_complete = None;
                    changed = true;
                }

                if let Some(start) = task.last_started_at {
                    if now > start {
                        let duration = (now - start) as u64;
                        task.time_spent_seconds = task.time_spent_seconds.saturating_add(duration);
                        if duration > 60 {
                            task.sessions
                                .push(crate::model::item::WorkSession { start, end: now });
                        }
                    }
                    task.last_started_at = None;
                    changed = true;
                }

                if changed {
                    updated.push(task.clone());
                }

                if let Some(children) = adjacency.get(&current_uid) {
                    queue.extend(children.clone());
                }
            }
        }
        updated
    }

    pub fn change_priority(&mut self, uid: &str, delta: i8, default_priority: u8) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            // If priority is unset, use the default and then apply the delta in one atomic step.
            // Delta > 0 means increase importance (decrease numeric value).
            // Delta < 0 means decrease importance (increase numeric value).
            let mut p = task.priority as i16;
            if p == 0 {
                p = default_priority as i16;
            }

            if delta > 0 {
                // apply positive delta, clamp to 1
                p = (p - delta as i16).max(1);
            } else if delta < 0 {
                // apply negative delta (subtracting a negative adds), clamp to 9
                p = (p - delta as i16).min(9);
            }

            task.priority = p as u8;
            return Some(task.clone());
        }
        None
    }

    // OPTIMIZATION: O(1) Delete
    pub fn delete_task(&mut self, uid: &str) -> Option<(Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(map) = self.calendars.get_mut(&href)
            && let Some(task) = map.remove(uid)
        {
            self.index.remove(uid);

            // Re-serialize for storage
            let list: Vec<Task> = map.values().cloned().collect();

            if href.starts_with("local://") {
                let _ = LocalStorage::save_for_href(self.ctx.as_ref(), &href, &list);
            } else {
                let (_, token) = Cache::load(self.ctx.as_ref(), &href).unwrap_or((vec![], None));
                let _ = Cache::save(self.ctx.as_ref(), &href, &list, token);
            }
            self.rebuild_relation_index();
            return Some((task, href));
        }
        None
    }

    // ... [Other setters remain unchanged and rely on get_task_mut] ...

    pub fn set_parent(&mut self, child_uid: &str, parent_uid: Option<String>) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(child_uid) {
            task.parent_uid = parent_uid;
            return Some(task.clone());
        }
        None
    }

    pub fn add_dependency(&mut self, task_uid: &str, dep_uid: String) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && !task.dependencies.contains(&dep_uid)
        {
            task.dependencies.push(dep_uid.clone());
            // Clone the modified task so we can return it after releasing the borrow
            let task_clone = task.clone();
            // Release the mutable borrow on `task` before mutating other fields on `self`
            // Update reverse blocking index: dep_uid (the dependency) is blocking task_uid
            self.blocking_index
                .entry(dep_uid)
                .or_default()
                .push(task_uid.to_string());
            return Some(task_clone);
        }
        None
    }

    pub fn remove_dependency(&mut self, task_uid: &str, dep_uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && let Some(pos) = task.dependencies.iter().position(|d| d == dep_uid)
        {
            task.dependencies.remove(pos);
            // Clone the modified task so we can return it after releasing the borrow
            let task_clone = task.clone();
            // Release the mutable borrow on `task` before mutating other fields on `self`
            // Update reverse blocking index: remove this task from the dep_uid entry
            if let Some(list) = self.blocking_index.get_mut(dep_uid) {
                list.retain(|u| u != task_uid);
                if list.is_empty() {
                    self.blocking_index.remove(dep_uid);
                }
            }
            return Some(task_clone);
        }
        None
    }

    pub fn add_related_to(&mut self, task_uid: &str, related_uid: String) -> Option<Task> {
        let result = if let Some((task, _)) = self.get_task_mut(task_uid)
            && !task.related_to.contains(&related_uid)
        {
            task.related_to.push(related_uid.clone());
            Some(task.clone())
        } else {
            None
        };
        if result.is_some() {
            self.related_from_index
                .entry(related_uid)
                .or_default()
                .push(task_uid.to_string());
        }
        result
    }

    pub fn remove_related_to(&mut self, task_uid: &str, related_uid: &str) -> Option<Task> {
        let result = if let Some((task, _)) = self.get_task_mut(task_uid)
            && let Some(pos) = task.related_to.iter().position(|r| r == related_uid)
        {
            task.related_to.remove(pos);
            Some(task.clone())
        } else {
            None
        };
        if result.is_some()
            && let Some(sources) = self.related_from_index.get_mut(related_uid)
        {
            sources.retain(|uid| uid != task_uid);
            if sources.is_empty() {
                self.related_from_index.remove(related_uid);
            }
        }
        result
    }

    pub fn move_task(&mut self, uid: &str, target_href: String) -> Option<(Task, Task)> {
        if let Some((mut task, old_href)) = self.delete_task(uid) {
            if old_href == target_href {
                self.add_task(task);
                return None;
            }
            let original = task.clone();
            task.calendar_href = target_href.clone();
            self.add_task(task.clone());

            // Persist target calendar
            if let Some(target_map) = self.calendars.get(&target_href) {
                let target_list: Vec<Task> = target_map.values().cloned().collect();

                if target_href.starts_with("local://") {
                    let _ =
                        LocalStorage::save_for_href(self.ctx.as_ref(), &target_href, &target_list);
                } else {
                    let (_, token) =
                        Cache::load(self.ctx.as_ref(), &target_href).unwrap_or((vec![], None));
                    let _ = Cache::save(self.ctx.as_ref(), &target_href, &target_list, token);
                }
            }

            return Some((original, task));
        }
        None
    }

    pub fn apply_alias_retroactively(
        &mut self,
        alias_key: &str,
        raw_values: &[String],
    ) -> Vec<Task> {
        let mut uids_to_update: Vec<String> = Vec::new();
        let is_location_alias = alias_key.starts_with("@@");
        let (clean_key, alias_prefix) = if is_location_alias {
            let clean = alias_key.trim_start_matches("@@");
            (clean, format!("{}:", clean))
        } else {
            (alias_key, format!("{}:", alias_key))
        };

        // Iterate map values
        for map in self.calendars.values() {
            for task in map.values() {
                let has_alias_or_child = if is_location_alias {
                    if let Some(loc) = &task.location {
                        loc == clean_key || loc.starts_with(&alias_prefix)
                    } else {
                        false
                    }
                } else {
                    task.categories
                        .iter()
                        .any(|cat| cat == clean_key || cat.starts_with(&alias_prefix))
                };

                if has_alias_or_child {
                    // (Check if update needed logic - simplified for brevity, same as before)
                    let mut needs_update = false;
                    for val in raw_values {
                        // ... same change detection logic ...
                        if let Some(tag) = val.strip_prefix('#') {
                            let clean = crate::model::parser::strip_quotes(tag);
                            if !task.categories.contains(&clean) {
                                needs_update = true;
                                break;
                            }
                        } else if let Some(loc) = val.strip_prefix("@@") {
                            let clean = crate::model::parser::strip_quotes(loc);
                            if task.location.as_ref() != Some(&clean) {
                                needs_update = true;
                                break;
                            }
                        } else if let Some(prio) = val.strip_prefix('!')
                            && let Ok(p) = prio.parse::<u8>()
                            && task.priority != p
                        {
                            needs_update = true;
                            break;
                        }
                    }
                    if needs_update {
                        uids_to_update.push(task.uid.clone());
                    }
                }
            }
        }

        if uids_to_update.is_empty() {
            return Vec::new();
        }

        let mut modified_tasks = Vec::new();
        for uid in uids_to_update {
            if let Some((task, _)) = self.get_task_mut(&uid) {
                // Apply changes logic (same as before)
                for val in raw_values {
                    // ... same mutation logic ...
                    if let Some(tag) = val.strip_prefix('#') {
                        let clean = crate::model::parser::strip_quotes(tag);
                        if !task.categories.contains(&clean) {
                            task.categories.push(clean);
                        }
                    } else if let Some(loc) = val.strip_prefix("@@") {
                        task.location = Some(crate::model::parser::strip_quotes(loc));
                    } else if let Some(prio) = val.strip_prefix('!')
                        && let Ok(p) = prio.parse::<u8>()
                    {
                        task.priority = p.min(9);
                    }
                    // ... etc ...
                }
                task.categories.sort();
                task.categories.dedup();
                modified_tasks.push(task.clone());
            }
        }
        modified_tasks
    }

    pub fn get_summary(&self, uid: &str) -> Option<String> {
        // O(1) Lookup
        let href = self.index.get(uid)?;
        self.calendars
            .get(href)
            .and_then(|m| m.get(uid))
            .map(|t| t.summary.clone())
    }

    pub fn is_task_done(&self, uid: &str) -> Option<bool> {
        let href = self.index.get(uid)?;
        self.calendars
            .get(href)
            .and_then(|m| m.get(uid))
            .map(|t| t.status.is_done())
    }

    pub fn get_task_status(&self, uid: &str) -> Option<bool> {
        self.is_task_done(uid)
    }

    pub fn is_blocked(&self, task: &Task) -> bool {
        if task.categories.contains(&"blocked".to_string()) {
            return true;
        }
        if task.dependencies.is_empty() {
            return false;
        }
        for dep_uid in &task.dependencies {
            if let Some(is_done) = self.is_task_done(dep_uid)
                && !is_done
            {
                return true;
            }
        }
        false
    }

    pub fn get_tasks_related_to(&self, uid: &str) -> Vec<(String, String)> {
        // Unchanged (uses reverse index)
        if let Some(source_uids) = self.related_from_index.get(uid) {
            source_uids
                .iter()
                .filter_map(|source_uid| {
                    self.get_summary(source_uid)
                        .map(|summary| (source_uid.clone(), summary))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns tasks that are blocked BY the given uid (i.e. successors).
    pub fn get_tasks_blocking(&self, uid: &str) -> Vec<(String, String)> {
        if let Some(blocked_uids) = self.blocking_index.get(uid) {
            blocked_uids
                .iter()
                .filter_map(|blocked_uid| {
                    self.get_summary(blocked_uid)
                        .map(|summary| (blocked_uid.clone(), summary))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn rebuild_relation_index(&mut self) {
        self.related_from_index.clear();
        self.blocking_index.clear();

        let mut relationships = Vec::new();
        let mut blocking_rels = Vec::new();

        // Iterate map values
        for map in self.calendars.values() {
            for task in map.values() {
                for target in &task.related_to {
                    relationships.push((target.clone(), task.uid.clone()));
                }
                // If `task` depends on `dep_uid`, then `dep_uid` is BLOCKING `task`.
                for dep_uid in &task.dependencies {
                    blocking_rels.push((dep_uid.clone(), task.uid.clone()));
                }
            }
        }

        for (target, source) in relationships {
            self.related_from_index
                .entry(target)
                .or_default()
                .push(source);
        }

        // Populate reverse dependency (blocking) map
        for (blocker, blocked) in blocking_rels {
            self.blocking_index
                .entry(blocker)
                .or_default()
                .push(blocked);
        }
    }

    // --- CRITICAL OPTIMIZATION: Filter using References ---
    pub fn filter(&self, options: FilterOptions) -> FilterResult {
        // Pre-calculate blocked/done status for O(1) checks during filter
        let mut completed_uids: HashSet<String> = HashSet::new();
        // Iterate maps
        for map in self.calendars.values() {
            for t in map.values() {
                if t.status.is_done() {
                    completed_uids.insert(t.uid.clone());
                }
            }
        }

        let check_is_blocked = |t: &Task, done_set: &HashSet<String>| -> bool {
            if t.categories.contains(&"blocked".to_string()) {
                return true;
            }
            if t.dependencies.is_empty() {
                return false;
            }
            for dep in &t.dependencies {
                // Use index for O(1) existence check
                if self.index.contains_key(dep) && !done_set.contains(dep) {
                    return true;
                }
            }
            false
        };

        let search_lower = options.search_term.to_lowercase();
        let is_ready_mode = search_lower.contains("is:ready");
        let is_blocked_mode = search_lower.contains("is:blocked");
        let now = Utc::now();

        // 1. ITERATE REFERENCES (Not Clones!)
        // Create an iterator over all tasks in filtered calendars
        let task_iter = self
            .calendars
            .iter()
            .filter(|(href, _)| {
                if let Some(active) = options.active_cal_href {
                    *href == active && !options.hidden_calendars.contains(*href)
                } else {
                    !options.hidden_calendars.contains(*href)
                }
            })
            .flat_map(|(_, map)| map.values());

        // 2. Filter References
        let visible_refs: Vec<&Task> = task_iter
            .filter(|t| {
                let has_status_filter = search_lower.contains("is:done")
                    || search_lower.contains("is:active")
                    || search_lower.contains("is:started")
                    || search_lower.contains("is:ongoing");

                if !has_status_filter && t.status.is_done() && options.hide_completed_global {
                    return false;
                }

                // Logic checks...
                if is_ready_mode {
                    if t.status.is_done() {
                        return false;
                    }
                    if let Some(start) = &t.dtstart
                        && start.to_start_comparison_time() > now
                    {
                        return false;
                    }
                    if check_is_blocked(t, &completed_uids) {
                        return false;
                    }
                }

                if is_blocked_mode && !check_is_blocked(t, &completed_uids) {
                    return false;
                }

                // Category/Duration/Location checks (on references)...
                if let Some(mins) = t.estimated_duration {
                    if let Some(min) = options.min_duration
                        && mins < min
                    {
                        return false;
                    }
                    if let Some(max) = options.max_duration
                        && mins > max
                    {
                        return false;
                    }
                } else if !options.include_unset_duration {
                    return false;
                }

                // ... Tag matching logic (using existing check_match helper logic) ...
                if !options.selected_categories.is_empty() {
                    let filter_uncategorized =
                        options.selected_categories.contains(UNCATEGORIZED_ID);
                    // Helper closure for matching
                    let check_match = |task_cat: &str, selected: &str| -> bool {
                        let tc_lower = task_cat.to_lowercase();
                        let sel_lower = selected.to_lowercase();
                        if tc_lower == sel_lower {
                            return true;
                        }
                        if let Some(stripped) = tc_lower.strip_prefix(&sel_lower) {
                            return stripped.starts_with(':');
                        }
                        false
                    };

                    if options.match_all_categories {
                        for sel in options.selected_categories {
                            if sel == UNCATEGORIZED_ID {
                                if !t.categories.is_empty() {
                                    return false;
                                }
                            } else {
                                let mut has = false;
                                for c in &t.categories {
                                    if check_match(c, sel) {
                                        has = true;
                                        break;
                                    }
                                }
                                if !has {
                                    return false;
                                }
                            }
                        }
                    } else {
                        let mut hit = false;
                        if filter_uncategorized && t.categories.is_empty() {
                            hit = true;
                        } else {
                            for sel in options.selected_categories {
                                if sel != UNCATEGORIZED_ID {
                                    for c in &t.categories {
                                        if check_match(c, sel) {
                                            hit = true;
                                            break;
                                        }
                                    }
                                }
                                if hit {
                                    break;
                                }
                            }
                        }
                        if !hit {
                            return false;
                        }
                    }
                }

                if !options.selected_locations.is_empty() {
                    if let Some(loc) = &t.location {
                        let mut hit = false;
                        for sel in options.selected_locations {
                            if loc == sel
                                || (loc.starts_with(sel) && loc.chars().nth(sel.len()) == Some(':'))
                            {
                                hit = true;
                                break;
                            }
                        }
                        if !hit {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect();

        // 3. Search Term Pass
        let final_refs = if options.search_term.is_empty() {
            visible_refs
        } else {
            // Build temporary children_map for filtered set
            let mut children_map = HashMap::new();
            for t in &visible_refs {
                if let Some(p) = &t.parent_uid {
                    children_map
                        .entry(p.clone())
                        .or_insert_with(Vec::new)
                        .push(t.uid.clone());
                }
            }

            let mut matched_uids = HashSet::new();
            let mut queue = Vec::new();

            for t in &visible_refs {
                if t.matches_search_term(options.search_term) && matched_uids.insert(t.uid.clone())
                {
                    queue.push(t.uid.clone());
                }
            }

            let mut idx = 0;
            while idx < queue.len() {
                let curr = queue[idx].clone();
                idx += 1;
                if let Some(children) = children_map.get(&curr) {
                    for child in children {
                        if matched_uids.insert(child.clone()) {
                            queue.push(child.clone());
                        }
                    }
                }
            }

            visible_refs
                .into_iter()
                .filter(|t| matched_uids.contains(&t.uid))
                .collect()
        };

        // --- NEW: Calculate Tags and Locations Dynamically from Filtered Result ---
        let mut cat_active_counts: HashMap<String, usize> = HashMap::new();
        let mut cat_display_names: HashMap<String, String> = HashMap::new();
        let mut cat_present_lower: HashSet<String> = HashSet::new();
        let mut uncat_active_count = 0;
        let mut uncat_any = false;

        let mut loc_active_counts: HashMap<String, usize> = HashMap::new();
        let mut loc_present: HashSet<String> = HashSet::new();

        for t in &final_refs {
            let is_active = !t.status.is_done();

            // Categories
            if t.categories.is_empty() {
                uncat_any = true;
                if is_active {
                    uncat_active_count += 1;
                }
            } else {
                for cat in &t.categories {
                    let parts: Vec<&str> = cat.split(':').collect();
                    let mut current_hierarchy = String::with_capacity(cat.len());

                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            current_hierarchy.push(':');
                        }
                        current_hierarchy.push_str(part);

                        let lower_key = current_hierarchy.to_lowercase();
                        cat_present_lower.insert(lower_key.clone());
                        cat_display_names
                            .entry(lower_key.clone())
                            .or_insert_with(|| current_hierarchy.clone());

                        if is_active {
                            *cat_active_counts.entry(lower_key.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }

            // Locations
            if let Some(loc) = &t.location {
                let parts: Vec<&str> = loc.split(':').collect();
                let mut current_hierarchy = String::with_capacity(loc.len());
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        current_hierarchy.push(':');
                    }
                    current_hierarchy.push_str(part);
                    loc_present.insert(current_hierarchy.clone());
                    if is_active {
                        *loc_active_counts
                            .entry(current_hierarchy.clone())
                            .or_insert(0) += 1;
                    }
                }
            }
        }

        let mut categories = Vec::new();
        for lower_tag in cat_present_lower {
            let count = *cat_active_counts.get(&lower_tag).unwrap_or(&0);
            let display_name = cat_display_names
                .get(&lower_tag)
                .cloned()
                .unwrap_or(lower_tag.clone());
            let is_forced = options
                .selected_categories
                .iter()
                .any(|f| f.to_lowercase() == lower_tag);

            if !options.hide_fully_completed_tags || count > 0 || is_forced {
                categories.push((display_name, count));
            }
        }

        if uncat_active_count > 0
            || (uncat_any && !options.hide_fully_completed_tags)
            || options.selected_categories.contains(UNCATEGORIZED_ID)
        {
            categories.push((UNCATEGORIZED_ID.to_string(), uncat_active_count));
        }

        categories.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        let mut locations = Vec::new();
        for loc in loc_present {
            let count = *loc_active_counts.get(&loc).unwrap_or(&0);
            let is_forced = options.selected_locations.contains(&loc);

            if !options.hide_fully_completed_tags || count > 0 || is_forced {
                locations.push((loc, count));
            }
        }
        locations.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        // 4. CLONE ONLY FINAL RESULTS & CALCULATE TRANSIENT FIELDS
        let mut final_tasks_processed: Vec<Task> = final_refs
            .into_iter()
            .map(|t_ref| {
                let mut t = t_ref.clone(); // The only major clone step
                t.is_blocked = check_is_blocked(&t, &completed_uids);
                t.effective_priority = t.priority;
                t.effective_due = t.due.clone();
                t.effective_dtstart = t.dtstart.clone();
                t
            })
            .collect();

        // 5. Rank and Sort (same as before)
        for t in final_tasks_processed.iter_mut() {
            t.sort_rank = t.calculate_base_rank(
                options.cutoff_date,
                options.urgent_days,
                options.urgent_prio,
                options.start_grace_period_days,
            );
        }

        // Hierarchy + Propagation logic ...
        // (Copying propagation logic from existing implementation which uses Vec indices)
        let mut uid_to_index = HashMap::new();
        for (i, t) in final_tasks_processed.iter().enumerate() {
            uid_to_index.insert(t.uid.clone(), i);
        }
        let mut parent_to_children = HashMap::new();
        for (i, t) in final_tasks_processed.iter().enumerate() {
            if let Some(p) = &t.parent_uid
                && uid_to_index.contains_key(p)
            {
                parent_to_children
                    .entry(p.clone())
                    .or_insert_with(Vec::new)
                    .push(i);
            }
        }

        #[derive(Clone)]
        struct Effective {
            rank: u8,
            prio: u8,
            due: Option<crate::model::item::DateType>,
            start: Option<crate::model::item::DateType>,
        }

        let mut cache: HashMap<usize, Effective> = HashMap::new();
        let mut visiting: HashSet<usize> = HashSet::new();
        let default_prio = options.default_priority;

        fn resolve(
            idx: usize,
            tasks: &Vec<Task>,
            map: &HashMap<String, Vec<usize>>,
            cache: &mut HashMap<usize, Effective>,
            visiting: &mut HashSet<usize>,
            default_prio: u8,
        ) -> Effective {
            if let Some(c) = cache.get(&idx) {
                return c.clone();
            }
            let t = &tasks[idx];
            let mut best = Effective {
                rank: t.sort_rank,
                prio: t.effective_priority,
                due: t.effective_due.clone(),
                start: t.effective_dtstart.clone(),
            };
            if visiting.contains(&idx) {
                return best;
            }
            visiting.insert(idx);

            let is_suppressed = t.status.is_done();

            if !is_suppressed && let Some(children) = map.get(&t.uid) {
                for &child_idx in children {
                    let child_eff = resolve(child_idx, tasks, map, cache, visiting, default_prio);
                    let ordering = Task::compare_components(
                        child_eff.rank,
                        child_eff.prio,
                        &child_eff.due,
                        &child_eff.start,
                        best.rank,
                        best.prio,
                        &best.due,
                        &best.start,
                        default_prio,
                    );
                    if ordering == std::cmp::Ordering::Less {
                        best = child_eff;
                    }
                }
            }
            visiting.remove(&idx);
            cache.insert(idx, best.clone());
            best
        }

        for i in 0..final_tasks_processed.len() {
            let eff = resolve(
                i,
                &final_tasks_processed,
                &parent_to_children,
                &mut cache,
                &mut visiting,
                default_prio,
            );
            let t = &mut final_tasks_processed[i];
            t.sort_rank = eff.rank;
            t.effective_priority = eff.prio;
            t.effective_due = eff.due;
            t.effective_dtstart = eff.start;
        }

        let tasks = Task::organize_hierarchy(
            final_tasks_processed,
            options.default_priority,
            options.expanded_done_groups,
            options.max_done_roots,
            options.max_done_subtasks,
        );

        FilterResult {
            tasks,
            categories,
            locations,
        }
    }
}
