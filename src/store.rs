/*
File: cfait/src/store.rs

Optimized in-memory store for tasks.

Overview & Rationale:
- The TaskStore is the single-process in-memory representation of all task data
  grouped by calendar (calendar_href -> HashMap<uid, Task>).
- Performance goals:
  * O(1) lookup by UID (index: uid -> calendar_href)
  * Minimal cloning: filter pipeline operates over &Task references and only clones
    the final, visible tasks returned to UI layers.
  * Relation indices (related_from_index, blocking_index) are maintained to allow
    efficient reverse-lookup for "related to" and "blocked by" queries without
    scanning the whole dataset repeatedly.

Behavioral notes (important):
- Blocking semantics:
  * `is_blocked` on a Task represents an explicit block (e.g. tag `blocked` or
    an unresolved dependency).
  * `is_implicitly_blocked` is a transient flag that indicates inherited block
    status coming from an ancestor in the parent hierarchy. Both are used to
    decide "is:ready" semantics and ranking, but UI badges generally use only
    the explicit `is_blocked`.
- Hierarchy and virtual rows:
  * The model supports injecting small "virtual" tasks (Expand/Collapse rows)
    used by UI layers to truncate large groups of completed subtasks while still
    providing a way to expand them. These virtual tasks are created during the
    `organize_hierarchy` stage and are not persisted.
- Storage interactions:
  * The store keeps an index for fast lookups; persistent save/load is handled
    by higher-level modules (LocalStorage / Cache). Store provides convenient
    methods that ensure relation indices are rebuilt when data mutates.

API guarantees:
- Methods that mutate the internal maps update the relation indices so callers
  can rely on the reverse-maps (get_tasks_related_to / get_tasks_blocking).
- get_task_ref / get_task_mut provide direct map-backed access and will auto-fix
  inconsistent index state by removing dangling index entries.

Implementation notes:
- The file favors clarity of the logic in filter/propagation/hierarchy routines.
- Filtering is done in multiple passes:
  1. Build a reference iterator of candidate tasks from allowed calendars.
  2. Apply predicate filters on &Task references to avoid cloning.
  3. Optionally expand the set to include parent/child context when search term
     requires it.
  4. Clone only the final set of tasks to produce the FilterResult (where
     transient attributes are computed).
*/

use crate::context::AppContext;
use crate::model::{Task, TaskStatus};
use chrono::{DateTime, Utc};
use fastrand;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub const UNCATEGORIZED_ID: &str = ":::uncategorized:::";

/// Result container returned by the `filter` pipeline.
pub struct FilterResult {
    pub tasks: Vec<Task>,
    pub categories: Vec<(String, usize)>,
    pub locations: Vec<(String, usize)>,
}

/// Select an index from `tasks` at random weighted by priority.
/// - Tasks with priority 0 use the provided `default_priority`.
/// - Lower numeric priority indicates higher importance; we invert to produce
///   weights where priority 1 -> weight 9, priority 9 -> weight 1.
/// - Filters for "is:ready" criteria:
///   * Must not be done (Completed/Cancelled)
///   * Must not be explicitly OR implicitly blocked
///   * Must not have a future start date
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

            // 2. Must not be blocked (explicit or inherited)
            if t.is_blocked || t.is_implicitly_blocked {
                return 0;
            }

            // 3. Must not start in the future (is:ready logic)
            if let Some(start) = &t.dtstart
                && start.to_start_comparison_time() > now
            {
                return 0;
            }

            // Invert priority so 1 is high weight (range 1..=9 -> weight 9..=1)
            let p = if t.priority == 0 {
                default_priority
            } else {
                t.priority
            };
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
    /// calendars: calendar_href -> (uid -> Task)
    pub calendars: HashMap<String, HashMap<String, Task>>,
    /// index: uid -> calendar_href (fast lookup for operations that only know a UID)
    pub index: HashMap<String, String>,
    /// Reverse lookup for related_to: target_uid -> Vec<source_uid>
    pub related_from_index: HashMap<String, Vec<String>>,
    /// Reverse lookup for dependencies: dep_uid -> Vec<task_uid_that_depend_on_dep>
    pub blocking_index: HashMap<String, Vec<String>>,
    /// AppContext used for persistence operations (if store needs to save).
    pub ctx: Arc<dyn AppContext>,
}

/// Options to parameterize a filter operation. Using a struct keeps the signature
/// manageable as the filter logic supports many toggles.
pub struct FilterOptions<'a> {
    pub active_cal_href: Option<&'a str>,
    pub hidden_calendars: &'a HashSet<String>,
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
    /// Construct a new TaskStore with an AppContext reference for persistence.
    pub fn new(ctx: Arc<dyn AppContext>) -> Self {
        Self {
            calendars: HashMap::new(),
            index: HashMap::new(),
            related_from_index: HashMap::new(),
            blocking_index: HashMap::new(),
            ctx,
        }
    }

    /// Whether the store contains any tasks (fast O(1) via index map).
    /// This is used to differentiate between a truly-empty app vs filters hiding items.
    pub fn has_any_tasks(&self) -> bool {
        !self.index.is_empty()
    }

    /// Replace or insert an entire calendar's tasks.
    /// This sets up the internal uid index and rebuilds relation indices for correctness.
    pub fn insert(&mut self, calendar_href: String, tasks: Vec<Task>) {
        let mut map = HashMap::new();
        for task in tasks {
            self.index.insert(task.uid.clone(), calendar_href.clone());
            map.insert(task.uid.clone(), task);
        }
        self.calendars.insert(calendar_href, map);
        self.rebuild_relation_index();
    }

    /// Add a single task into the store. If it already exists, it will be overwritten
    /// in the calendar map and indices are rebuilt to reflect the new relationships.
    pub fn add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());
        self.calendars
            .entry(href)
            .or_default()
            .insert(task.uid.clone(), task);
        self.rebuild_relation_index();
    }

    /// Update an existing task or insert it if missing. This method attempts to handle
    /// moves between calendars by checking the uid index and adjusting maps accordingly.
    pub fn update_or_add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        let uid = task.uid.clone();
        if let Some(existing_href) = self.index.get(&uid) {
            if existing_href == &href {
                if let Some(map) = self.calendars.get_mut(&href) {
                    map.insert(uid.clone(), task);
                } else {
                    self.calendars
                        .entry(href.clone())
                        .or_default()
                        .insert(uid.clone(), task);
                }
            } else {
                // Task was moved between calendars: remove from old map and insert in new
                if let Some(map) = self.calendars.get_mut(existing_href) {
                    map.remove(&uid);
                }
                self.index.insert(uid.clone(), href.clone());
                self.calendars
                    .entry(href.clone())
                    .or_default()
                    .insert(uid.clone(), task);
            }
        } else {
            // New task
            self.index.insert(uid.clone(), href.clone());
            self.calendars
                .entry(href.clone())
                .or_default()
                .insert(uid.clone(), task);
        }
        self.rebuild_relation_index();
    }

    /// Remove all tasks and indices from the store.
    pub fn clear(&mut self) {
        self.calendars.clear();
        self.index.clear();
        self.related_from_index.clear();
        self.blocking_index.clear();
    }

    /// Remove an entire calendar from the store and drop related index entries.
    pub fn remove(&mut self, calendar_href: &str) {
        if let Some(tasks_map) = self.calendars.remove(calendar_href) {
            for uid in tasks_map.keys() {
                self.index.remove(uid);
            }
        }
        self.rebuild_relation_index();
    }

    /// Get a mutable reference to a task together with its calendar href.
    /// Returns None if the uid is not present or index is inconsistent (auto-fix).
    pub fn get_task_mut(&mut self, uid: &str) -> Option<(&mut Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(map) = self.calendars.get_mut(&href)
            && let Some(task) = map.get_mut(uid)
        {
            return Some((task, href));
        }
        // Index pointed to a non-existent entry; clean it up.
        self.index.remove(uid);
        None
    }

    /// Immutable reference by uid (O(1)).
    pub fn get_task_ref(&self, uid: &str) -> Option<&Task> {
        let href = self.index.get(uid)?;
        self.calendars.get(href).and_then(|map| map.get(uid))
    }

    /// O(1) delete a task and return (task, calendar_href) on success.
    /// Relation indices are rebuilt afterwards.
    pub fn delete_task(&mut self, uid: &str) -> Option<(Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(map) = self.calendars.get_mut(&href)
            && let Some(task) = map.remove(uid)
        {
            self.index.remove(uid);
            self.rebuild_relation_index();
            return Some((task, href));
        }
        None
    }

    /// Toggle convenience: Completed <-> NeedsAction (returns primary, optional secondary, reset children)
    pub fn toggle_task(&mut self, uid: &str) -> Option<(Task, Option<Task>, Vec<Task>)> {
        let current_status = self.get_task_ref(uid)?.status;
        let next_status = if current_status.is_done() {
            TaskStatus::NeedsAction
        } else {
            TaskStatus::Completed
        };
        self.set_status(uid, next_status)
    }

    /// Set the status for a task. Special handling:
    /// - If the task is recurring and is being completed, `recycle` is invoked which may
    ///   return (history_snapshot, Some(next_instance)). Both are persisted to the store.
    /// - When recurring completion advances, descendants that were completed will be
    ///   reset to NeedsAction so the recurrence semantics remain coherent. The list of
    ///   reset children is returned for callers to persist/journal.
    pub fn set_status(
        &mut self,
        uid: &str,
        status: TaskStatus,
    ) -> Option<(Task, Option<Task>, Vec<Task>)> {
        let task_copy = self.get_task_ref(uid)?.clone();
        // If the task is recurring and we are completing it we may need to reset children
        let should_reset_children = task_copy.rrule.is_some() && status.is_done();

        // Model-level helper computes the primary (history or updated) and optional secondary
        let (primary, secondary) = task_copy.recycle(status);

        // Save primary and secondary into the store (optimistic/instant UI update)
        self.update_or_add_task(primary.clone());

        if let Some(sec) = &secondary {
            self.update_or_add_task(sec.clone());
        }

        // Reset descendants if appropriate (recurrence completion path)
        let mut reset_children: Vec<Task> = Vec::new();

        if should_reset_children && secondary.is_some() {
            // Build adjacency (Parent -> [Children]) across all calendars
            let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
            for map in self.calendars.values() {
                for t in map.values() {
                    if let Some(p) = &t.parent_uid {
                        adjacency.entry(p.clone()).or_default().push(t.uid.clone());
                    }
                }
            }

            // BFS to discover all descendant UIDs
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

            // Reset each done descendant to NeedsAction and persist
            for child_uid in descendants {
                if let Some((child, _)) = self.get_task_mut(&child_uid)
                    && child.status.is_done()
                {
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

        Some((primary, secondary, reset_children))
    }

    /// Mark a task InProcess and begin timing; bubble to parent tasks so the timer
    /// context is preserved. Returns the set of tasks that were updated.
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

    /// Pause a task and all descendants (stop timing and record a session as appropriate).
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

    /// Stop a running task and its subtree; commit time and close sessions.
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

    /// Adjust priority by `delta` with sensible clamping. Delta semantics:
    /// positive -> increase importance (lower numeric), negative -> decrease importance.
    pub fn change_priority(&mut self, uid: &str, delta: i8, default_priority: u8) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            // If priority is unset, use the default and then apply the delta in one atomic step.
            let mut p = task.priority as i16;
            if p == 0 {
                p = default_priority as i16;
            }

            if delta > 0 {
                p = (p - delta as i16).max(1);
            } else if delta < 0 {
                p = (p - delta as i16).min(9);
            }

            task.priority = p as u8;
            return Some(task.clone());
        }
        None
    }

    /// Set or unset a parent relationship for a task.
    pub fn set_parent(&mut self, child_uid: &str, parent_uid: Option<String>) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(child_uid) {
            task.parent_uid = parent_uid;
            return Some(task.clone());
        }
        None
    }

    /// Add a dependency (task_uid depends on dep_uid). Maintain reverse blocking index.
    pub fn add_dependency(&mut self, task_uid: &str, dep_uid: String) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && !task.dependencies.contains(&dep_uid)
        {
            task.dependencies.push(dep_uid.clone());
            let task_clone = task.clone();
            self.blocking_index
                .entry(dep_uid)
                .or_default()
                .push(task_uid.to_string());
            return Some(task_clone);
        }
        None
    }

    /// Remove a dependency and update reverse index.
    pub fn remove_dependency(&mut self, task_uid: &str, dep_uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && let Some(pos) = task.dependencies.iter().position(|d| d == dep_uid)
        {
            task.dependencies.remove(pos);
            let task_clone = task.clone();
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

    /// Add a related_to relationship and update reverse index for fast lookups.
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

    /// Remove a related_to relationship and update reverse index.
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

    /// Move a task between calendars in an atomic fashion:
    /// - delete from old calendar, adjust index, insert into target calendar
    /// - returns (original, updated) pair when a move occurred
    pub fn move_task(&mut self, uid: &str, target_href: String) -> Option<(Task, Task)> {
        if let Some((mut task, old_href)) = self.delete_task(uid) {
            if old_href == target_href {
                // No-op move; re-add to same calendar
                self.add_task(task);
                return None;
            }
            let original = task.clone();
            task.calendar_href = target_href.clone();
            self.add_task(task.clone());
            return Some((original, task));
        }
        None
    }

    /// Apply an alias retroactively: examine tasks that match the alias key (or its children)
    /// and mutate categories/location/priority as specified by `raw_values`. Returns the set
    /// of modified tasks for callers to persist/save.
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
                    let mut needs_update = false;
                    for val in raw_values {
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
                for val in raw_values {
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
                }
                task.categories.sort();
                task.categories.dedup();
                modified_tasks.push(task.clone());
            }
        }
        modified_tasks
    }

    /// Convenience: is task done by uid.
    pub fn is_task_done(&self, uid: &str) -> Option<bool> {
        self.get_task_ref(uid).map(|t| t.status.is_done())
    }

    /// Determine explicit blocking for a task by checking 'blocked' category and dependencies.
    /// This uses the store's index for O(1) existence checks of dependency UIDs.
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

    /// Get list of tasks that DECLARE they are related to the provided uid (i.e. sources).
    pub fn get_tasks_related_to(&self, uid: &str) -> Vec<(String, String)> {
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

    /// Rebuild both reverse indices (related_from_index and blocking_index).
    /// This is called after bulk mutations and ensures the indices are consistent.
    pub fn rebuild_relation_index(&mut self) {
        self.related_from_index.clear();
        self.blocking_index.clear();

        let mut relationships = Vec::new();
        let mut blocking_rels = Vec::new();

        // Iterate all calendars and tasks only once to collect relations
        for map in self.calendars.values() {
            for (uid, task) in map {
                for r in &task.related_to {
                    relationships.push((r.clone(), uid.clone()));
                }
                for dep in &task.dependencies {
                    blocking_rels.push((dep.clone(), uid.clone()));
                }
            }
        }

        // Populate reverse maps
        for (from, to) in relationships {
            self.related_from_index.entry(from).or_default().push(to);
        }
        for (from, to) in blocking_rels {
            self.blocking_index.entry(from).or_default().push(to);
        }
    }

    /// Fast summary retrieval (O(1) via index + map lookup)
    pub fn get_summary(&self, uid: &str) -> Option<String> {
        self.get_task_ref(uid).map(|t| t.summary.clone())
    }

    /// Main filter pipeline that performs multi-stage filtering and returns
    /// prepared results (cloned tasks and aggregated category/location lists).
    pub fn filter(&self, options: FilterOptions) -> FilterResult {
        // Build set of completed UIDs for quick membership checks (used by blocking checks)
        let mut completed_uids: HashSet<String> = HashSet::new();
        for map in self.calendars.values() {
            for t in map.values() {
                if t.status.is_done() {
                    completed_uids.insert(t.uid.clone());
                }
            }
        }

        // Helper: explicit blocked state (ignores inherited parent blocking)
        let check_is_blocked_explicit = |t: &Task, done_set: &HashSet<String>| -> bool {
            if t.categories.contains(&"blocked".to_string()) {
                return true;
            }
            if t.dependencies.is_empty() {
                return false;
            }
            for dep in &t.dependencies {
                if self.index.contains_key(dep) && !done_set.contains(dep) {
                    return true;
                }
            }
            false
        };

        // Helper: determine whether a task is effectively blocked by checking ancestors
        let check_is_effectively_blocked = |t: &Task, done_set: &HashSet<String>| -> bool {
            let mut current = t;
            let mut visited = HashSet::new();

            loop {
                if check_is_blocked_explicit(current, done_set) {
                    return true;
                }

                if let Some(p_uid) = &current.parent_uid {
                    if !visited.insert(p_uid.clone()) {
                        // cycle / already visited: stop
                        break;
                    }
                    if let Some(p_task) = self.get_task_ref(p_uid) {
                        current = p_task;
                        continue;
                    }
                }
                break;
            }
            false
        };

        let search_lower = options.search_term.to_lowercase();
        let is_ready_mode = search_lower.contains("is:ready");
        let is_blocked_mode = search_lower.contains("is:blocked");
        let now = Utc::now();

        // 1) Build iterator over allowed calendars (respecting active/hidden calendar restrictions).
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

        // 2) Filter references (no cloning). This pass performs the bulk of predicate checks.
        let visible_refs: Vec<&Task> = task_iter
            .filter(|t| {
                // Status-based filtering (is:done / is:active / started / ongoing)
                let has_status_filter = search_lower.contains("is:done")
                    || search_lower.contains("is:active")
                    || search_lower.contains("is:started")
                    || search_lower.contains("is:ongoing");

                if !has_status_filter && t.status.is_done() && options.hide_completed_global {
                    return false;
                }

                if is_ready_mode {
                    if t.status.is_done() {
                        return false;
                    }
                    if let Some(start) = &t.dtstart
                        && start.to_start_comparison_time() > now
                    {
                        return false;
                    }
                    if check_is_effectively_blocked(t, &completed_uids) {
                        return false;
                    }
                }

                if is_blocked_mode && !check_is_effectively_blocked(t, &completed_uids) {
                    return false;
                }

                // Duration filters
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

                // Category matching (supports hierarchical prefixes and UNCATEGORIZED token)
                if !options.selected_categories.is_empty() {
                    let filter_uncategorized =
                        options.selected_categories.contains(UNCATEGORIZED_ID);
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

                // Location matching
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

        // 3) If a search term exists, expand matched results to include their children
        // so that partial matches still result in a useful list with context.
        let final_refs = if options.search_term.is_empty() {
            visible_refs
        } else {
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

        // 4) Build category and location aggregates from the final refs (cloned)
        let mut cat_active_counts: HashMap<String, usize> = HashMap::new();
        let mut cat_display_names: HashMap<String, String> = HashMap::new();
        let mut cat_present_lower: HashSet<String> = HashSet::new();
        let mut uncat_active_count = 0;
        let mut uncat_any = false;

        let mut loc_active_counts: HashMap<String, usize> = HashMap::new();
        let mut loc_present: HashSet<String> = HashSet::new();

        for t in &final_refs {
            let is_active = !t.status.is_done();

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

            if let Some(loc) = &t.location {
                let parts: Vec<&str> = loc.split(':').collect();
                let mut current_hierarchy = String::with_capacity(loc.len());
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        current_hierarchy.push(':');
                    }
                    current_hierarchy.push_str(part);
                    if is_active {
                        *loc_active_counts
                            .entry(current_hierarchy.clone())
                            .or_insert(0) += 1;
                    }
                    loc_present.insert(current_hierarchy.clone());
                }
            }
        }

        // Convert category maps into sorted vectors for UI
        let mut categories: Vec<(String, usize)> = cat_active_counts
            .into_iter()
            .map(|(k, v)| {
                // Prefer display name if present
                let label = cat_display_names.get(&k).cloned().unwrap_or(k.clone());
                (label, v)
            })
            .collect();

        if uncat_any {
            categories.push(("Uncategorized".to_string(), uncat_active_count));
        }

        categories.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        // Convert locations into sorted vector
        let mut locations: Vec<(String, usize)> = loc_active_counts.into_iter().collect();
        locations.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        // 5) Clone final results into owned Task structs and compute transient fields.
        let mut final_tasks_processed: Vec<Task> = final_refs
            .into_iter()
            .map(|t_ref| {
                let mut t = t_ref.clone();
                // Compute blocked flags: explicit vs implicit
                t.is_blocked = check_is_blocked_explicit(&t, &completed_uids);
                t.is_implicitly_blocked =
                    !t.is_blocked && check_is_effectively_blocked(&t, &completed_uids);
                t.effective_priority = t.priority;
                t.effective_due = t.due.clone();
                t.effective_dtstart = t.dtstart.clone();
                t
            })
            .collect();

        // 6) Compute rank and sort order for the final tasks.
        for t in final_tasks_processed.iter_mut() {
            let eff_blocked = t.is_blocked || t.is_implicitly_blocked;
            t.sort_rank = t.calculate_base_rank(
                options.cutoff_date,
                options.urgent_days,
                options.urgent_prio,
                options.start_grace_period_days,
                eff_blocked,
            );
        }

        // Propagation: certain UI operations look up best child/parent contributions.
        // Build helper maps for O(1) access by index into the vector.
        let mut uid_to_index = HashMap::new();
        for (i, t) in final_tasks_processed.iter().enumerate() {
            uid_to_index.insert(t.uid.clone(), i);
        }

        // Children map (index-based) used by propagation resolution
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, t) in final_tasks_processed.iter().enumerate() {
            if let Some(p) = &t.parent_uid {
                map.entry(p.clone()).or_default().push(i);
            }
        }

        // Resolve function used to compute the 'best' child result used in some UI heuristics.
        fn resolve(
            idx: usize,
            tasks: &Vec<Task>,
            map: &HashMap<String, Vec<usize>>,
            cache: &mut HashMap<usize, Task>,
            visiting: &mut HashSet<usize>,
            default_prio: u8,
        ) -> Task {
            if let Some(cached) = cache.get(&idx) {
                return cached.clone();
            }
            if visiting.contains(&idx) {
                return tasks[idx].clone();
            }

            visiting.insert(idx);
            let t = &tasks[idx];
            let mut best = t.clone();

            let is_suppressed = t.status.is_done() || t.is_blocked || t.is_implicitly_blocked;

            if !is_suppressed {
                if let Some(children) = map.get(&t.uid) {
                    for &child_idx in children {
                        let child_eff =
                            resolve(child_idx, tasks, map, cache, visiting, default_prio);
                        let ordering = Task::compare_components(
                            child_eff.sort_rank,
                            child_eff.effective_priority,
                            &child_eff.effective_due,
                            &child_eff.effective_dtstart,
                            best.sort_rank,
                            best.effective_priority,
                            &best.effective_due,
                            &best.effective_dtstart,
                            default_prio,
                        );
                        if ordering == std::cmp::Ordering::Less {
                            best = child_eff;
                        }
                    }
                }
            }

            visiting.remove(&idx);
            cache.insert(idx, best.clone());
            best
        }

        // Run propagation resolver for every node to produce any necessary transient values.
        let mut cache: HashMap<usize, Task> = HashMap::new();
        let mut visiting: HashSet<usize> = HashSet::new();
        for i in 0..final_tasks_processed.len() {
            if !cache.contains_key(&i) {
                let _ = resolve(
                    i,
                    &final_tasks_processed,
                    &map,
                    &mut cache,
                    &mut visiting,
                    options.default_priority,
                );
            }
        }

        // Final sorting using compare_for_sort to produce a deterministic order for UI rendering.
        final_tasks_processed.sort_by(|a, b| a.compare_for_sort(b, options.default_priority));

        FilterResult {
            tasks: final_tasks_processed,
            categories,
            locations,
        }
    }
}
