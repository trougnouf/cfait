// SPDX-License-Identifier: GPL-3.0-or-later
//! Performance benchmarks for the hot paths called out in CONTRIBUTING.md
//! ("Keep it Fast" — the project targets 100k+ tasks without stuttering).
//!
//! Run locally with: `cargo bench`
//! In CI these run report-only (never fail the pipeline) and the criterion
//! output is uploaded as an artifact for manual inspection.
//!
//! Covers:
//!   - Filter pipeline (`TaskStore::filter`) over flat and hierarchical stores.
//!   - Smart-input parsing (`Task::new` / `apply_smart_input` / `tokenize_smart_input`).
//!   - Sort & hierarchy (`organize_hierarchy` + `children_index` lookups),
//!     exercised through `filter` on a forest of parent/child trees.

use cfait::config::{PausedSortBehavior, SortPreset};
use cfait::context::TestContext;
use cfait::model::Task;
use cfait::model::parser::tokenize_smart_input;
use cfait::store::{FilterOptions, TaskStore};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Keep CI friendly: fewer samples / shorter runs than criterion defaults.
const SAMPLE_SIZE: usize = 20;
const MEASUREMENT_SECS: u64 = 3;

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .measurement_time(std::time::Duration::from_secs(MEASUREMENT_SECS))
}

// ---------------------------------------------------------------------------
// Store builders
// ---------------------------------------------------------------------------

/// Build a store full of flat (no hierarchy) tasks spread across a few
/// calendars, with varied priority / tags / due dates so the filter pipeline
/// has something realistic to chew on.
fn build_flat_store(n: usize) -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();
    let calendars = ["cal1", "cal2", "cal3"];
    for i in 0..n {
        let prio = (i % 9) + 1; // !1 .. !9
        let tag = match i % 4 {
            0 => "#work",
            1 => "#home",
            2 => "#bench",
            _ => "#errands",
        };
        let due = match i % 5 {
            0 => "@tomorrow",
            1 => "@next week",
            2 => "@in 3d",
            _ => "",
        };
        let mut t = Task::new(
            &format!("Flat task number {i} !{prio} {tag} {due}"),
            &aliases,
            None,
        );
        t.calendar_href = calendars[i % calendars.len()].to_string();
        store.add_task(t);
    }
    store
}

/// Build a forest: `groups` roots, each with `branching` direct children, and
/// each child gets `branching` grandchildren (depth 3). Every non-root task
/// sets `parent_uid`, which exercises `children_index` membership checks on
/// every task during `filter` (the path the recent O(n^2) `has_children`
/// fixes targeted).
fn build_hierarchy_store(groups: usize, branching: usize) -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    for g in 0..groups {
        let root_uid = format!("root-{g}");
        let mut root = Task::new(
            &format!("Project {g} !1 #project @next week"),
            &aliases,
            None,
        );
        root.uid = root_uid.clone();
        root.calendar_href = "cal1".to_string();
        store.add_task(root);

        for c in 0..branching {
            let child_uid = format!("root-{g}-c{c}");
            let mut child = Task::new(
                &format!("Subtask {c} of project {g} !2 #sub @tomorrow"),
                &aliases,
                None,
            );
            child.uid = child_uid.clone();
            child.parent_uid = Some(root_uid.clone());
            child.calendar_href = "cal1".to_string();
            store.add_task(child);

            for gc in 0..branching {
                let mut grand = Task::new(
                    &format!("Grandchild {gc} under {g}-{c} !3 #leaf"),
                    &aliases,
                    None,
                );
                grand.uid = format!("root-{g}-c{c}-g{gc}");
                grand.parent_uid = Some(child_uid.clone());
                grand.calendar_href = "cal1".to_string();
                store.add_task(grand);
            }
        }
    }
    store
}

// ---------------------------------------------------------------------------
// FilterOptions helpers
// ---------------------------------------------------------------------------

fn default_options<'a>(
    empty_h: &'a HashSet<String>,
    empty_m: &'a HashMap<String, Vec<String>>,
) -> FilterOptions<'a> {
    FilterOptions {
        active_cal_href: None,
        hidden_calendars: empty_h,
        selected_categories: empty_h,
        selected_locations: empty_h,
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        hide_fully_completed_tags: false,
        hide_aliases_in_sidebar: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 5,
        default_priority: 5,
        start_grace_period_days: 1,
        sort_standard_by_priority: false,
        sort_preset: SortPreset::default(),
        expanded_done_groups: empty_h,
        expanded_tags: empty_h,
        expanded_locations: empty_h,
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: empty_m,
        search_collapsed_tasks: empty_h,
        focused_task_uid: None,
        paused_sort_behavior: PausedSortBehavior::default(),
        sort_tiebreak_recent: false,
    }
}

/// Same as `default_options` but with a search term that matches a fraction
/// of the stored tasks (the word "task" for flat stores, "project" for
/// hierarchical ones).
fn search_options<'a>(
    empty_h: &'a HashSet<String>,
    empty_m: &'a HashMap<String, Vec<String>>,
    term: &'a str,
) -> FilterOptions<'a> {
    let mut o = default_options(empty_h, empty_m);
    o.search_term = term;
    o
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

const FLAT_SIZES: &[usize] = &[1_000, 10_000, 100_000];

fn bench_filter_flat(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/flat");
    let empty_h = HashSet::new();
    let empty_m = HashMap::new();

    for &n in FLAT_SIZES {
        let store = build_flat_store(n);

        group.bench_with_input(BenchmarkId::new("default", n), &n, |b, _| {
            b.iter(|| store.filter(default_options(&empty_h, &empty_m)))
        });

        group.bench_with_input(BenchmarkId::new("search", n), &n, |b, _| {
            b.iter(|| store.filter(search_options(&empty_h, &empty_m, "task")))
        });
    }
    group.finish();
}

fn bench_filter_hierarchy(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter/hierarchy");
    let empty_h = HashSet::new();
    let empty_m = HashMap::new();

    // (groups, branching) -> total tasks = groups * (1 + branching + branching^2)
    for (groups, branching) in [(100, 9), (1_000, 9), (5_000, 3)] {
        let total = groups * (1 + branching + branching * branching);
        let store = build_hierarchy_store(groups, branching);

        group.bench_with_input(
            BenchmarkId::new("default", format!("{total}_tasks")),
            &total,
            |b, _| b.iter(|| store.filter(default_options(&empty_h, &empty_m))),
        );

        group.bench_with_input(
            BenchmarkId::new("search", format!("{total}_tasks")),
            &total,
            |b, _| b.iter(|| store.filter(search_options(&empty_h, &empty_m, "project"))),
        );
    }
    group.finish();
}

/// Smart-input parsing: `Task::new` parses the input string end-to-end
/// (tokenize -> apply -> field assignment).
fn bench_parse_task_new(c: &mut Criterion) {
    let aliases = HashMap::new();
    let inputs: &[(&str, &str)] = &[
        ("simple", "Buy milk"),
        ("priority", "Ship the release !1"),
        (
            "full",
            "Prepare demo !2 #work @@Office @tomorrow ~45m rec:FREQ=WEEKLY",
        ),
        (
            "escaped",
            "@@\"San Francisco\" desc:\"Line one\\nLine two\" url:example.com",
        ),
    ];

    let mut group = c.benchmark_group("parse/task_new");
    for (label, input) in inputs {
        group.bench_function(*label, |b| b.iter(|| Task::new(input, &aliases, None)));
    }
    group.finish();
}

/// `apply_smart_input` on an already-constructed task (re-parse path used by
/// edits in the TUI/GUI).
fn bench_parse_apply(c: &mut Criterion) {
    let aliases = HashMap::new();
    let mut group = c.benchmark_group("parse/apply_smart_input");
    let input = "Reschedule !1 #work @@Home @next week ~2h rec:FREQ=DAILY";

    group.bench_function("reparse_full", |b| {
        b.iter_batched(
            || Task::new("Original task", &aliases, None),
            |mut task| task.apply_smart_input(input, &aliases, None),
            criterion::BatchSize::SmallInput,
        )
    });
    group.finish();
}

/// Tokenizer in isolation (the first stage of smart-input parsing and of the
/// search-query parser).
fn bench_parse_tokenize(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse/tokenize");
    let inputs: &[(&str, &str)] = &[
        ("short", "Buy milk !1 #errands"),
        (
            "medium",
            "Prepare demo !2 #work @Office ^tomorrow *45m desc:\"long body here\"",
        ),
        (
            "braces",
            "gaming{genre={metroidvania, platform}, multiplayer{coop, online}} !1 #fun",
        ),
    ];

    for (label, input) in inputs {
        group.bench_function(*label, |b| b.iter(|| tokenize_smart_input(input, false)));
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets =
        bench_filter_flat,
        bench_filter_hierarchy,
        bench_parse_task_new,
        bench_parse_apply,
        bench_parse_tokenize
}
criterion_main!(benches);
