/*
 * cfait/src/journal.rs
 *
 * Offline action journal for syncing changes.
 *
 * This module uses an explicit `AppContext` for resolving filesystem locations.
 * All public IO functions take a `&dyn AppContext` argument; there are no
 * hidden globals here.
 */

use crate::context::AppContext;
use crate::model::Task;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Create(Task),
    Update(Task),
    Delete(Task),
    Move(Task, String),
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Journal {
    pub queue: Vec<Action>,
}

impl Journal {
    /// Return the on-disk journal path for the given context, if available.
    pub fn get_path(ctx: &dyn AppContext) -> Option<PathBuf> {
        ctx.get_journal_path()
    }

    /// Internal helper: load journal structure from a path without acquiring locks.
    fn load_internal(path: &PathBuf) -> Self {
        if path.exists()
            && let Ok(content) = fs::read_to_string(path)
            && let Ok(journal) = serde_json::from_str(&content)
        {
            return journal;
        }
        Self::default()
    }

    /// Load the journal from disk using the provided context.
    pub fn load(ctx: &dyn AppContext) -> Self {
        if let Some(path) = Self::get_path(ctx) {
            if !path.exists() {
                return Self::default();
            }
            return LocalStorage::with_lock(&path, || Ok(Self::load_internal(&path)))
                .unwrap_or_default();
        }
        Self::default()
    }

    /// Modify the journal by applying a closure to the queue, persisting changes.
    pub fn modify<F>(ctx: &dyn AppContext, f: F) -> Result<()>
    where
        F: FnOnce(&mut Vec<Action>),
    {
        if let Some(path) = Self::get_path(ctx) {
            LocalStorage::with_lock(&path, || {
                let mut journal = Self::load_internal(&path);
                f(&mut journal.queue);
                let json = serde_json::to_string_pretty(&journal)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Push a new action into the journal.
    pub fn push(ctx: &dyn AppContext, action: Action) -> Result<()> {
        Self::modify(ctx, |queue| queue.push(action))
    }

    /// Is the in-memory journal empty?
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Compact the journal by merging redundant operations for the same UID.
    pub fn compact(&mut self) {
        let mut uid_map: HashMap<String, usize> = HashMap::new();
        let mut compacted: Vec<Option<Action>> = Vec::new();

        for action in self.queue.drain(..) {
            let uid = match &action {
                Action::Create(t) => t.uid.clone(),
                Action::Update(t) => t.uid.clone(),
                Action::Delete(t) => t.uid.clone(),
                Action::Move(t, _) => t.uid.clone(),
            };

            let mut merged = false;
            if let Some(&idx) = uid_map.get(&uid)
                && let Some(prev) = &compacted[idx]
            {
                match (prev, &action) {
                    (Action::Create(_), Action::Update(t)) => {
                        compacted[idx] = Some(Action::Create(t.clone()));
                        merged = true;
                    }
                    (Action::Update(_), Action::Update(t)) => {
                        compacted[idx] = Some(Action::Update(t.clone()));
                        merged = true;
                    }
                    (Action::Create(_), Action::Delete(_)) => {
                        compacted[idx] = None;
                        uid_map.remove(&uid);
                        merged = true;
                    }
                    (Action::Update(_), Action::Delete(t)) => {
                        compacted[idx] = Some(Action::Delete(t.clone()));
                        merged = true;
                    }
                    _ => {}
                }
            }

            if !merged {
                compacted.push(Some(action));
                uid_map.insert(uid, compacted.len() - 1);
            }
        }

        self.queue = compacted.into_iter().flatten().collect();
    }

    /// Apply journaled actions to an existing task list for a given calendar.
    ///
    /// This merges creates/updates/deletes/moves into `tasks` in-memory so the
    /// caller can present or operate on the final state prior to syncing.
    pub fn apply_to_tasks(ctx: &dyn AppContext, tasks: &mut Vec<Task>, calendar_href: &str) {
        let journal = Self::load(ctx);

        let mut pending_uids = HashSet::new();
        for action in &journal.queue {
            match action {
                Action::Create(t) | Action::Update(t) => {
                    if t.calendar_href == calendar_href {
                        pending_uids.insert(t.uid.clone());
                    }
                }
                Action::Move(t, target) => {
                    if target == calendar_href {
                        pending_uids.insert(t.uid.clone());
                    }
                }
                _ => {}
            }
        }

        // For remote calendars, drop entries without an etag unless they are pending in the journal.
        if !calendar_href.starts_with("local://") {
            tasks.retain(|t| !t.etag.is_empty() || pending_uids.contains(&t.uid));
        }

        if journal.is_empty() {
            return;
        }

        let mut task_map: HashMap<String, Task> =
            tasks.drain(..).map(|t| (t.uid.clone(), t)).collect();

        for action in journal.queue {
            match action {
                Action::Create(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.insert(t.uid.clone(), t);
                    }
                }
                Action::Update(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.insert(t.uid.clone(), t);
                    }
                }
                Action::Delete(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.remove(&t.uid);
                    }
                }
                Action::Move(t, new_href) => {
                    if t.calendar_href == calendar_href {
                        task_map.remove(&t.uid);
                    } else if new_href == calendar_href {
                        let mut moved_task = t;
                        moved_task.calendar_href = new_href;
                        task_map.insert(moved_task.uid.clone(), moved_task);
                    }
                }
            }
        }

        *tasks = task_map.into_values().collect();
    }
}
