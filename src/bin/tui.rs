// Binary entry point for the TUI application (supports non-interactive CLI subcommands).
//
// This file implements a small CLI dispatcher on top of the existing TUI entry
// point so that users can run quick commands non-interactively (add, list,
// search, toggle, delete, import, export, sync, daemon). The interactive TUI
// still runs when no command is provided.
//
// Note: This file intentionally mirrors the project's existing controller/store
// APIs and uses the app context for config/data paths.

use anyhow::Result;
use cfait::context::{AppContext, StandardContext};
use cfait::model::Task;
use cfait::storage::LocalStorage;
use cfait::store::{FilterOptions, TaskStore};
use chrono::Utc;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

// Helper to quickly build the store from local files and cache for CLI reads
async fn build_store_cli(ctx: &Arc<dyn AppContext>) -> TaskStore {
    let mut store = TaskStore::new(ctx.clone());

    if let Ok(locals) = cfait::storage::LocalCalendarRegistry::load(ctx.as_ref()) {
        for loc in locals {
            if let Ok(mut tasks) =
                cfait::storage::LocalStorage::load_for_href(ctx.as_ref(), &loc.href)
            {
                cfait::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &loc.href);
                store.insert(loc.href, tasks);
            }
        }
    }

    if let Ok(cals) = cfait::cache::Cache::load_calendars(ctx.as_ref()) {
        for cal in cals {
            if cal.href.starts_with("local://") {
                continue;
            }
            if let Ok((mut tasks, _)) = cfait::cache::Cache::load(ctx.as_ref(), &cal.href) {
                cfait::journal::Journal::apply_to_tasks(ctx.as_ref(), &mut tasks, &cal.href);
                store.insert(cal.href, tasks);
            }
        }
    }

    store
}

// Helper to resolve short partial UIDs back to a full UID
fn resolve_uid(store: &TaskStore, partial: &str) -> Option<String> {
    let mut matches: Vec<String> = Vec::new();
    for map in store.calendars.values() {
        for uid in map.keys() {
            if uid.starts_with(partial) {
                matches.push(uid.clone());
            }
        }
    }

    match matches.len() {
        1 => Some(matches.into_iter().next().unwrap()),
        0 => {
            eprintln!("Error: No task matches UID '{}'", partial);
            None
        }
        _ => {
            eprintln!("Error: Ambiguous UID '{}'. Matches:", partial);
            for m in matches {
                if let Some(t) = store.get_task_ref(&m) {
                    let short = &m[..std::cmp::min(8, m.len())];
                    eprintln!("  {} - {}", short, t.summary);
                }
            }
            None
        }
    }
}

// Best-effort sync helper that can be called without passing a pre-loaded config
// Delegate to `sync_background` so the shared helper is used and keeps logic centralized.
async fn maybe_sync(ctx: Arc<dyn AppContext>) -> Result<(), String> {
    if let Ok(config) = cfait::config::Config::load(ctx.as_ref()) {
        // Reuse the existing background sync helper which already implements a
        // timeout and client fallback. This ensures `sync_background` is referenced.
        sync_background(ctx, config).await
    } else {
        Ok(())
    }
}

// Background sync trigger so the CLI stays incredibly fast (keeps old helper for callers that prefer to pass config)
async fn sync_background(
    ctx: Arc<dyn AppContext>,
    config: cfait::config::Config,
) -> Result<(), String> {
    if config.url.is_empty() {
        return Ok(());
    }

    let ctx_clone = ctx.clone();
    let cfg_clone = config.clone();
    let sync_future = async move {
        // Pass an Arc<dyn AppContext> (clone) to the client helper which expects an Arc.
        if let Ok((client, _, _, _, _)) = cfait::client::RustyClient::connect_with_fallback(
            ctx_clone.clone(),
            cfg_clone,
            Some("CLI"),
        )
        .await
        {
            if let Err(e) = client.sync_journal().await {
                return Err(e);
            }
        }
        Ok(())
    };

    // Give it up to 10 seconds to sync, otherwise gracefully detach
    match tokio::time::timeout(std::time::Duration::from_secs(10), sync_future).await {
        Ok(res) => res,
        Err(_) => Err("Sync timed out (changes are safely queued for next sync)".to_string()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = env::args().collect();
    let binary_name = args.first().cloned().unwrap_or_else(|| "cfait".to_string());

    // Parse for --root argument before creating the context
    let mut override_root: Option<PathBuf> = None;
    if let Some(pos) = args.iter().position(|arg| arg == "--root" || arg == "-r")
        && pos + 1 < args.len()
    {
        override_root = Some(PathBuf::from(args[pos + 1].clone()));
        // Remove the flag and its value so they don't interfere with other parsing
        args.remove(pos);
        args.remove(pos);
    }

    let ctx: Arc<dyn AppContext> = Arc::new(StandardContext::new(override_root));
    cfait::config::init_locale(ctx.as_ref());

    let command = args.get(1).map(|s| s.as_str()).unwrap_or("");

    if command.starts_with('-') || command == "help" {
        cfait::cli::print_help(&binary_name);
        return Ok(());
    }

    match command {
        "import" => {
            if args.len() < 3 {
                eprintln!("Error: Missing file path");
                eprintln!("Usage: cfait import <file.ics> [--collection <id>]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let collection_id = if args.len() > 4 && args[3] == "--collection" {
                Some(args[4].clone())
            } else {
                None
            };
            let ics_content = std::fs::read_to_string(file_path).unwrap_or_else(|e| {
                eprintln!("Error reading file '{}': {}", file_path, e);
                std::process::exit(1);
            });
            let href = if let Some(col_id) = collection_id {
                if col_id == "default" {
                    "local://default".to_string()
                } else {
                    format!("local://{}", col_id)
                }
            } else {
                "local://default".to_string()
            };

            match LocalStorage::import_from_ics(ctx.as_ref(), &href, &ics_content) {
                Ok(count) => println!(
                    "Successfully imported {} task(s) to collection '{}'",
                    count, href
                ),
                Err(e) => {
                    eprintln!("Error importing tasks: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        "export" => {
            let collection_id = if args.len() > 3 && args[2] == "--collection" {
                Some(args[3].clone())
            } else {
                None
            };
            let tasks = if let Some(col_id) = collection_id {
                let href = if col_id == "default" {
                    "local://default".to_string()
                } else {
                    format!("local://{}", col_id)
                };
                LocalStorage::load_for_href(ctx.as_ref(), &href)?
            } else {
                LocalStorage::load_for_href(ctx.as_ref(), cfait::storage::LOCAL_CALENDAR_HREF)?
            };
            println!("{}", LocalStorage::to_ics_string(&tasks));
            return Ok(());
        }
        "sync" => {
            let config = cfait::config::Config::load(ctx.as_ref()).unwrap_or_default();
            if config.url.is_empty() {
                println!("Offline mode configured; nothing to sync.");
                return Ok(());
            }
            println!("Syncing with {}...", config.url);
            match cfait::client::RustyClient::connect_with_fallback(
                ctx.clone(),
                config,
                Some("CLI-Sync"),
            )
            .await
            {
                Ok(_) => println!("Sync completed successfully."),
                Err(e) => {
                    eprintln!("Sync failed: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        "daemon" => {
            println!("Starting Cfait background daemon...");
            loop {
                let config = cfait::config::Config::load(ctx.as_ref()).unwrap_or_default();
                let interval = config.auto_refresh_interval_mins;
                if interval == 0 {
                    println!("Auto-refresh is disabled in config. Daemon exiting.");
                    return Ok(());
                }
                if config.url.is_empty() {
                    println!("Offline mode configured. Daemon sleeping...");
                } else {
                    #[cfg(not(target_os = "android"))]
                    match cfait::storage::DaemonLock::try_acquire_exclusive(ctx.as_ref()) {
                        Ok(Some(_lock)) => {
                            println!("Daemon: Syncing with {}...", config.url);
                            let _ = cfait::client::RustyClient::connect_with_fallback(
                                ctx.clone(),
                                config,
                                Some("CLI-Daemon"),
                            )
                            .await;
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("Daemon: Failed to check instance lock: {}", e),
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(interval as u64 * 60)).await;
            }
        }
        "add" | "create" => {
            let input = args[2..].join(" ");
            if input.trim().is_empty() {
                eprintln!("Error: Task description cannot be empty.");
                std::process::exit(1);
            }

            let mut config = cfait::config::Config::load(ctx.as_ref()).unwrap_or_default();
            let def_time =
                chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

            // Allow ad-hoc alias definition via CLI
            let (clean_input, new_aliases) = cfait::model::extract_inline_aliases(&input);
            if !new_aliases.is_empty() {
                for (k, v) in &new_aliases {
                    let _ = cfait::model::validate_alias_integrity(k, v, &config.tag_aliases);
                    config.tag_aliases.insert(k.clone(), v.clone());
                }
                let _ = config.save(ctx.as_ref());
            }

            let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);

            let mut target_href = config
                .default_calendar
                .clone()
                .unwrap_or_else(|| cfait::storage::LOCAL_CALENDAR_HREF.to_string());

            let mut all_cals = Vec::new();
            if let Ok(locals) = cfait::storage::LocalCalendarRegistry::load(ctx.as_ref()) {
                all_cals.extend(locals);
            }
            if let Ok(remotes) = cfait::cache::Cache::load_calendars(ctx.as_ref()) {
                all_cals.extend(remotes);
            }

            let mut matched = false;
            if let Some(found) = all_cals.iter().find(|c| {
                c.name == target_href
                    || c.href == target_href
                    || c.href.ends_with(&format!("/{}/", target_href))
                    || c.href.ends_with(&format!("/{}", target_href))
            }) {
                target_href = found.href.clone();
                matched = true;
            }

            if !matched && !target_href.starts_with("local://") && !target_href.starts_with('/') {
                eprintln!(
                    "Warning: Calendar '{}' not found. Task will be saved to local recovery.",
                    target_href
                );
                target_href = "local://recovery".to_string();
            }

            task.calendar_href = target_href;

            let store = Arc::new(tokio::sync::Mutex::new(TaskStore::new(ctx.clone())));
            let client = Arc::new(tokio::sync::Mutex::new(None));
            let controller = cfait::controller::TaskController::new(store, client, ctx.clone());

            let uid = controller
                .create_task(task)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            println!(
                "Task added successfully. (UID: {})",
                &uid[..std::cmp::min(8, uid.len())]
            );

            // Best-effort background sync of the journal
            if let Err(e) = maybe_sync(ctx.clone()).await {
                eprintln!("Warning: Background sync failed: {}", e);
            }
            return Ok(());
        }
        "list" | "search" => {
            // Parse arguments: support explicit --all which overrides hidden calendars and completed hiding
            let mut show_all = false;
            let mut query_parts: Vec<String> = Vec::new();
            // Iterate by reference so we don't move `args` (which is used later).
            for arg in args.iter().skip(2) {
                if arg == "--all" {
                    show_all = true;
                } else {
                    query_parts.push(arg.clone());
                }
            }
            let query = if command == "search" {
                query_parts.join(" ")
            } else {
                String::new()
            };

            let config = cfait::config::Config::load(ctx.as_ref()).unwrap_or_default();
            let store = build_store_cli(&ctx).await;

            let mut hidden: HashSet<String> = HashSet::new();
            let mut hide_completed = config.hide_completed;
            if !show_all {
                hidden.extend(config.hidden_calendars.into_iter());
                hidden.extend(config.disabled_calendars.into_iter());
            } else {
                hide_completed = false;
            }

            let cutoff_date = if show_all {
                None
            } else {
                config
                    .sort_cutoff_months
                    .map(|m| Utc::now() + chrono::Duration::days(m as i64 * 30))
            };

            // Local empty sets to satisfy FilterOptions references
            let selected_categories: HashSet<String> = HashSet::new();
            let selected_locations: HashSet<String> = HashSet::new();
            let expanded_done_groups: HashSet<String> = HashSet::new();

            let res = store.filter(FilterOptions {
                active_cal_href: None,
                hidden_calendars: &hidden,
                selected_categories: &selected_categories,
                selected_locations: &selected_locations,
                match_all_categories: false,
                search_term: &query,
                hide_completed_global: hide_completed,
                hide_fully_completed_tags: !show_all && config.hide_fully_completed_tags,
                cutoff_date,
                min_duration: None,
                max_duration: None,
                include_unset_duration: true,
                urgent_days: config.urgent_days_horizon,
                urgent_prio: config.urgent_priority_threshold,
                default_priority: config.default_priority,
                start_grace_period_days: config.start_grace_period_days,
                expanded_done_groups: &expanded_done_groups,
                max_done_roots: usize::MAX,
                max_done_subtasks: usize::MAX,
            });

            if res.tasks.is_empty() {
                println!("No tasks found.");
                return Ok(());
            }

            for t in res.tasks {
                if matches!(
                    t.virtual_state,
                    cfait::model::VirtualState::Expand(_) | cfait::model::VirtualState::Collapse(_)
                ) {
                    continue; // Skip virtual rows in CLI
                }
                let symbol = t.checkbox_symbol();
                let indent = "  ".repeat(t.depth);
                let smart = t.to_smart_string();
                let summary_escaped = cfait::model::parser::escape_summary(&t.summary);
                let metadata = smart.replacen(&summary_escaped, "", 1).trim().to_string();
                let uid_short = &t.uid[..std::cmp::min(8, t.uid.len())];

                let meta_str = if metadata.is_empty() {
                    String::new()
                } else {
                    format!(" {}", metadata)
                };
                println!(
                    "{}{} {}{} [{}]",
                    indent, symbol, t.summary, meta_str, uid_short
                );
            }
            return Ok(());
        }
        "view" | "show" => {
            let store = build_store_cli(&ctx).await;
            let partial = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let uid =
                resolve_uid(&store, partial).ok_or_else(|| anyhow::anyhow!("UID required"))?;
            let t = store.get_task_ref(&uid).unwrap();

            println!("Summary:  {}", t.summary);
            println!("Status:   {:?} {}", t.status, t.checkbox_symbol());
            println!("UID:      {}", t.uid);
            if let Some(d) = &t.due {
                println!("Due:      {}", d.format_smart());
            }
            if !t.categories.is_empty() {
                println!("Tags:     {}", t.categories.join(", "));
            }
            if let Some(l) = &t.location {
                println!("Location: {}", l);
            }
            if !t.description.is_empty() {
                println!("\nDescription:\n{}", t.description);
            }
            return Ok(());
        }
        "start" | "pause" | "toggle" | "done" | "complete" => {
            let store = build_store_cli(&ctx).await;
            let partial_uid = args.get(2).cloned().unwrap_or_default();
            if partial_uid.is_empty() {
                eprintln!("Error: Missing UID.");
                std::process::exit(1);
            }
            let full_uid = match resolve_uid(&store, &partial_uid) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let store_arc = Arc::new(tokio::sync::Mutex::new(store));
            let client_arc = Arc::new(tokio::sync::Mutex::new(None));
            let controller =
                cfait::controller::TaskController::new(store_arc, client_arc, ctx.clone());

            match command {
                "start" => {
                    // Mark the task and its ancestry as InProcess in the in-memory store,
                    // then persist each updated task via controller.update_task.
                    let mut store_lock = controller.store.lock().await;
                    let updated = store_lock.set_status_in_process(&full_uid);
                    drop(store_lock);
                    for t in updated {
                        controller
                            .update_task(t)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))?;
                    }
                    println!("Task {} started.", partial_uid);
                }
                "pause" => {
                    // Pause the task subtree in-memory (commit timing/sessions), then persist.
                    let mut store_lock = controller.store.lock().await;
                    let updated = store_lock.pause_task(&full_uid);
                    drop(store_lock);
                    for t in updated {
                        controller
                            .update_task(t)
                            .await
                            .map_err(|e| anyhow::anyhow!(e))?;
                    }
                    println!("Task {} paused.", partial_uid);
                }
                _ => {
                    controller
                        .toggle_task(&full_uid)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("Task {} toggled.", partial_uid);
                }
            }

            // Best-effort background sync
            if let Err(e) = maybe_sync(ctx.clone()).await {
                eprintln!("Warning: Background sync failed: {}", e);
            }
            return Ok(());
        }
        "delete" | "rm" => {
            let partial_uid = args.get(2).cloned().unwrap_or_default();
            if partial_uid.is_empty() {
                eprintln!("Error: Missing UID.");
                std::process::exit(1);
            }
            let store = build_store_cli(&ctx).await;
            let full_uid = match resolve_uid(&store, &partial_uid) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let store_arc = Arc::new(tokio::sync::Mutex::new(store));
            let client_arc = Arc::new(tokio::sync::Mutex::new(None));
            let controller =
                cfait::controller::TaskController::new(store_arc, client_arc, ctx.clone());

            controller
                .delete_task(&full_uid)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("Task {} deleted.", partial_uid);

            // Best-effort background sync
            if let Err(e) = maybe_sync(ctx.clone()).await {
                eprintln!("Warning: Background sync failed: {}", e);
            }
            return Ok(());
        }
        "" => {
            // No non-interactive command provided; fall through to start the interactive TUI.
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }

    // --- Start interactive TUI ---
    #[cfg(not(target_os = "android"))]
    let _ui_lock = cfait::storage::DaemonLock::acquire_shared(ctx.as_ref())
        .map_err(|e| eprintln!("Warning: Could not acquire shared UI lock: {}", e))
        .ok();

    cfait::tui::run(ctx).await
}
