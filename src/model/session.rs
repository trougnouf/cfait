// File: ./src/model/session.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Session state management for the Rust core.

use crate::config::Config;
use crate::store::{FilterOptions, FilterResult, TaskStore};
use std::collections::HashSet;

/// Unified session state held by the Rust core for each active client.
#[cfg_attr(feature = "mobile", derive(uniffi::Record))]
#[derive(Clone, Debug, Default)]
pub struct SessionState {
    pub search_term: String,
    pub selected_categories: Vec<String>, // Using Vec because UniFFI doesn't support HashSet
    pub selected_locations: Vec<String>,
    pub active_calendar_href: Option<String>,
    pub match_all_categories: bool,
    pub expanded_done_groups: Vec<String>,
}

impl SessionState {
    /// The single source of truth for building the UI view based on current session state.
    pub fn get_filtered_view(&self, store: &TaskStore, config: &Config) -> FilterResult {
        let mut hidden = config
            .hidden_calendars
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        hidden.extend(config.disabled_calendars.clone());

        let cutoff = config
            .sort_cutoff_months
            .map(|m| chrono::Utc::now() + chrono::Duration::days(m as i64 * 30));

        let selected_categories: HashSet<String> =
            self.selected_categories.iter().cloned().collect();
        let selected_locations: HashSet<String> = self.selected_locations.iter().cloned().collect();
        let expanded_done_groups: HashSet<String> =
            self.expanded_done_groups.iter().cloned().collect();

        store.filter(FilterOptions {
            active_cal_href: None, // Logic handled by hidden_calendars
            hidden_calendars: &hidden,
            selected_categories: &selected_categories,
            selected_locations: &selected_locations,
            match_all_categories: self.match_all_categories,
            search_term: &self.search_term,
            hide_completed_global: config.hide_completed,
            hide_fully_completed_tags: config.hide_fully_completed_tags,
            cutoff_date: cutoff,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: config.urgent_days_horizon,
            urgent_prio: config.urgent_priority_threshold,
            default_priority: config.default_priority,
            start_grace_period_days: config.start_grace_period_days,
            sort_standard_by_priority: config.sort_standard_by_priority,
            expanded_done_groups: &expanded_done_groups,
            max_done_roots: config.max_done_roots,
            max_done_subtasks: config.max_done_subtasks,
            tag_aliases: &config.tag_aliases,
        })
    }

    /// Applies session-specific intents to modify the view filters.
    pub fn apply_session_intent(&mut self, intent: &AppIntent) {
        match intent {
            AppIntent::SetSearchTerm { term } => self.search_term = term.clone(),
            AppIntent::ToggleTagFilter { tag } => {
                if let Some(pos) = self.selected_categories.iter().position(|x| x == tag) {
                    self.selected_categories.remove(pos);
                } else {
                    self.selected_categories.push(tag.clone());
                }
            }
            AppIntent::ToggleLocationFilter { location } => {
                if let Some(pos) = self.selected_locations.iter().position(|x| x == location) {
                    self.selected_locations.remove(pos);
                } else {
                    self.selected_locations.push(location.clone());
                }
            }
            AppIntent::ClearFilters => {
                self.search_term.clear();
                self.selected_categories.clear();
                self.selected_locations.clear();
            }
            AppIntent::ToggleMatchAllCategories => {
                self.match_all_categories = !self.match_all_categories
            }
            AppIntent::SetSidebarCalendar { href } => {
                self.active_calendar_href = Some(href.clone())
            }
            AppIntent::ClearTagFilters => self.selected_categories.clear(),
            AppIntent::ClearLocationFilters => self.selected_locations.clear(),
            AppIntent::ToggleDoneGroup { key } => {
                if let Some(pos) = self.expanded_done_groups.iter().position(|x| x == key) {
                    self.expanded_done_groups.remove(pos);
                } else {
                    self.expanded_done_groups.push(key.clone());
                }
            }
            _ => {} // Ignore task-specific intents
        }
    }
}

/// A generic intent dispatched by any UI (Mobile, GUI, TUI).
#[cfg_attr(feature = "mobile", derive(uniffi::Enum))]
#[derive(Clone, Debug)]
pub enum AppIntent {
    ToggleTask { uid: String },
    ToggleTaskShift { uid: String },
    DeleteTask { uid: String },
    DeleteTaskTree { uid: String },
    CancelTask { uid: String },
    ChangePriority { uid: String, delta: i8 },
    StartTask { uid: String },
    PauseTask { uid: String },
    StopTask { uid: String },
    MoveTask { uid: String, target_href: String },
    DuplicateTaskTree { uid: String },
    RemoveParent { uid: String },
    MakeChild { uid: String, parent_uid: String },
    AddDependency { uid: String, blocker_uid: String },
    RemoveDependency { uid: String, blocker_uid: String },
    AddRelatedTo { uid: String, related_uid: String },
    RemoveRelatedTo { uid: String, related_uid: String },

    SetSearchTerm { term: String },
    ToggleTagFilter { tag: String },
    ToggleLocationFilter { location: String },
    ClearFilters,
    ToggleMatchAllCategories,
    SetSidebarCalendar { href: String },
    ClearTagFilters,
    ClearLocationFilters,
    ToggleTreeCollapse { uid: String },
    ToggleDoneGroup { key: String },
}
