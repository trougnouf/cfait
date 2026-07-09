// SPDX-License-Identifier: GPL-3.0-or-later
// Binary entry point for the TUI application (supports non-interactive CLI subcommands).
//
// This file implements a small CLI dispatcher on top of the existing TUI entry
// point so that users can run quick commands non-interactively (add, list,
// search, toggle, delete, import, export, sync, daemon). The interactive TUI
// still runs when no command is provided.
//
// Note: This file intentionally mirrors the project's existing controller/store
// APIs and uses the app context for config/data paths.

rust_i18n::i18n!("locales", fallback = "en");

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

// Helper function to re-quote arguments that were grouped by the shell
fn shell_arg_to_smart_token(arg: &str, lex: &cfait::model::parser::ParserLexicon) -> String {
    if !arg.contains(' ') {
        return arg.to_string();
    }

    if arg.contains(":=") {
        return arg.to_string();
    }

    let wrap_rem = |rem: &str| -> String {
        if (rem.starts_with('"') && rem.ends_with('"'))
            || (rem.starts_with('{') && rem.ends_with('}'))
        {
            rem.to_string()
        } else {
            format!("\"{}\"", rem)
        }
    };

    if let Some((_, _, rem)) = lex.extract_prefix(arg, &arg.to_lowercase()) {
        let p_len = arg.len() - rem.len();
        let prefix = &arg[..p_len];
        format!("{}{}", prefix, wrap_rem(rem))
    } else if let Some(rem) = arg.strip_prefix("@@@") {
        format!("@@@{}", wrap_rem(rem))
    } else if let Some(rem) = arg.strip_prefix("@@") {
        format!("@@{}", wrap_rem(rem))
    } else if let Some(rem) = arg.strip_prefix("##") {
        format!("##{}", wrap_rem(rem))
    } else if let Some(rem) = arg.strip_prefix('#') {
        format!("#{}", wrap_rem(rem))
    } else {
        arg.to_string()
    }
}

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

// Helper to resolve a collection ID or name back to its full HREF
async fn resolve_collection_href(ctx: &Arc<dyn AppContext>, target: &str) -> String {
    let mut all_cals = Vec::new();
    if let Ok(locals) = cfait::storage::LocalCalendarRegistry::load(ctx.as_ref()) {
        all_cals.extend(locals);
    }
    if let Ok(remotes) = cfait::cache::Cache::load_calendars(ctx.as_ref()) {
        all_cals.extend(remotes);
    }
    if let Some(found) = all_cals.into_iter().find(|c| {
        c.name == target
            || c.href == target
            || c.href.ends_with(&format!("/{}/", target))
            || c.href.ends_with(&format!("/{}", target))
    }) {
        found.href
    } else {
        target.to_string() // Fallback to raw input
    }
}

// Helper to resolve short partial UIDs, summaries, or wiki-links back to a full UID
fn resolve_uid(store: &TaskStore, partial: &str) -> Option<String> {
    match store.resolve_dependency_ref(partial) {
        Ok(uid) => Some(uid),
        Err(msg) => {
            eprintln!("{}", msg);
            None
        }
    }
}

// Helper to determine if we should skip waiting based on args and background presence
fn get_sync_strategy(
    no_wait_flag: bool,
    wait_flag: bool,
    ctx: &Arc<dyn AppContext>,
) -> (bool, bool) {
    if wait_flag {
        (false, false)
    } else if no_wait_flag {
        (true, false)
    } else {
        #[cfg(not(target_os = "android"))]
        let is_present = cfait::storage::PresenceLock::is_present(ctx.as_ref());
        #[cfg(target_os = "android")]
        let is_present = false;

        (is_present, is_present)
    }
}

// Best-effort sync helper that can be called without passing a pre-loaded config
// Delegate to `sync_background` so the shared helper is used and keeps logic centralized.
async fn maybe_sync(ctx: Arc<dyn AppContext>) -> Result<(), String> {
    if let Ok(config) = cfait::config::Config::load_with_credentials(ctx.as_ref()) {
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

    // Read journal to know which calendars are affected BEFORE connect_with_fallback consumes it
    let journal = cfait::journal::Journal::load(ctx.as_ref());
    let mut affected_cals = std::collections::HashSet::new();
    for action in &journal.queue {
        match action {
            cfait::journal::Action::Create(t)
            | cfait::journal::Action::Update(t)
            | cfait::journal::Action::Delete(t) => {
                affected_cals.insert(t.calendar_href.clone());
            }
            cfait::journal::Action::Move(t, target) => {
                affected_cals.insert(t.calendar_href.clone());
                affected_cals.insert(target.clone());
            }
        }
    }

    let ctx_clone = ctx.clone();
    let cfg_clone = config.clone();
    let sync_future = async move {
        // Pass an Arc<dyn AppContext> (clone) to the client helper which expects an Arc.
        if let Ok((client, cals, _, active_href, _)) =
            cfait::client::RustyClient::connect_with_fallback(
                ctx_clone.clone(),
                cfg_clone,
                Some("CLI"),
            )
            .await
        {
            let mut cals_to_fetch = Vec::new();
            for cal in cals {
                // connect_with_fallback already fetched the active calendar.
                if affected_cals.contains(&cal.href) && Some(&cal.href) != active_href.as_ref() {
                    cals_to_fetch.push(cal);
                }
            }
            if !cals_to_fetch.is_empty() {
                let _ = client.get_all_tasks(&cals_to_fetch).await;
            }
        }
        Ok(())
    };

    // Give it up to 15 seconds to sync, otherwise gracefully detach
    match tokio::time::timeout(std::time::Duration::from_secs(15), sync_future).await {
        Ok(res) => res,
        Err(_) => Err(rust_i18n::t!("sync_timed_out").to_string()),
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

    // If command is empty, we are launching the interactive TUI.
    // It is ONLY safe to use stderr if we are NOT in the interactive TUI.
    let is_interactive_tui = command.is_empty();
    let config = cfait::config::Config::load(ctx.as_ref()).unwrap_or_default();
    cfait::system::init_logging(
        ctx.as_ref(),
        !is_interactive_tui,
        Some(config.log_level.to_level_filter()),
    );
    cfait::system::init_keyring(); // <-- ADD THIS LINE

    if command.starts_with('-')
        || command == "help"
        || args.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        cfait::cli::print_help(&binary_name);
        return Ok(());
    }

    match command {
        "import" => {
            if args.len() < 3 {
                eprintln!("{}", rust_i18n::t!("error_missing_file_path"));
                eprintln!("{}", rust_i18n::t!("cli_usage_import"));
                std::process::exit(1);
            }
            let file_path = &args[2];
            let collection_id = if args.len() > 4 && args[3] == "--collection" {
                Some(args[4].clone())
            } else {
                None
            };
            let ics_content = std::fs::read_to_string(file_path).unwrap_or_else(|e| {
                eprintln!(
                    "{}",
                    rust_i18n::t!(
                        "error_reading_file",
                        path = file_path,
                        error = e.to_string()
                    )
                );
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
                Ok(count) => {
                    if count == 1 {
                        println!("{}", rust_i18n::t!("import_success", count = 1));
                    } else {
                        println!("{}", rust_i18n::t!("import_success", count = count));
                    }
                }
                Err(e) => {
                    eprintln!("{}", rust_i18n::t!("import_error", error = e.to_string()));
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
            let config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            if config.url.is_empty() {
                println!("{}", rust_i18n::t!("offline_mode_configured"));
                return Ok(());
            }
            println!("{}", rust_i18n::t!("syncing"));
            match cfait::client::RustyClient::connect_with_fallback(
                ctx.clone(),
                config,
                Some("CLI-Sync"),
            )
            .await
            {
                Ok((client, cals, _, _, _)) => {
                    if let Err(e) = client.get_all_tasks(&cals).await {
                        eprintln!("{}", rust_i18n::t!("sync_error", error = e.to_string()));
                        std::process::exit(1);
                    }
                    println!("{}", rust_i18n::t!("sync_completed_successfully"));
                }
                Err(e) => {
                    eprintln!("{}", rust_i18n::t!("sync_error", error = e.to_string()));
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        "daemon" => {
            println!("{}", rust_i18n::t!("starting_daemon"));
            #[cfg(not(target_os = "android"))]
            let _presence_lock = cfait::storage::PresenceLock::acquire_shared(ctx.as_ref()).ok();
            loop {
                let config =
                    cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
                let interval = config.auto_refresh_interval_mins;
                if interval == 0 {
                    println!("{}", rust_i18n::t!("daemon_auto_refresh_disabled"));
                    return Ok(());
                }
                if config.url.is_empty() {
                    println!("{}", rust_i18n::t!("daemon_offline_sleeping"));
                } else {
                    #[cfg(not(target_os = "android"))]
                    match cfait::storage::DaemonLock::try_acquire_exclusive(ctx.as_ref()) {
                        Ok(Some(_lock)) => {
                            println!("{}", rust_i18n::t!("daemon_syncing"));
                            if let Ok((client, cals, _, _, _)) =
                                cfait::client::RustyClient::connect_with_fallback(
                                    ctx.clone(),
                                    config,
                                    Some("CLI-Daemon"),
                                )
                                .await
                            {
                                let _ = client.get_all_tasks(&cals).await;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!(
                            "{}",
                            rust_i18n::t!("daemon_lock_failed", error = e.to_string())
                        ),
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(interval as u64 * 60)).await;
            }
        }
        "add" | "create" => {
            let mut col_href = None;
            let mut desc_text = None;
            let mut parent_uid_arg = None;
            let mut no_wait = false;
            let mut wait = false;
            let mut i = 2;
            let mut task_args = Vec::new();
            while i < args.len() {
                if args[i] == "--no-wait" || args[i] == "-n" {
                    no_wait = true;
                    i += 1;
                } else if args[i] == "--wait" || args[i] == "-w" {
                    wait = true;
                    i += 1;
                } else if args[i] == "--collection" || args[i] == "-c" {
                    if i + 1 < args.len() {
                        col_href = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for {}", args[i]);
                        std::process::exit(1);
                    }
                } else if args[i] == "--desc" {
                    if i + 1 < args.len() {
                        desc_text = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for --desc");
                        std::process::exit(1);
                    }
                } else if args[i] == "--parent" || args[i] == "-p" {
                    if i + 1 < args.len() {
                        parent_uid_arg = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for --parent");
                        std::process::exit(1);
                    }
                } else {
                    task_args.push(args[i].clone());
                    i += 1;
                }
            }
            let input = {
                let lex_guard = cfait::model::parser::LEXICON.read().unwrap();
                let lex = &*lex_guard;
                task_args
                    .iter()
                    .map(|arg| shell_arg_to_smart_token(arg, lex))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            if input.trim().is_empty() {
                eprintln!("{}", rust_i18n::t!("error_empty_task_description"));
                std::process::exit(1);
            }

            let mut config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let def_time =
                chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

            // Allow ad-hoc alias definition via CLI
            let (clean_input_1, new_goals) = cfait::model::extract_inline_goals(&input);
            let (clean_input, new_aliases) = cfait::model::extract_inline_aliases(&clean_input_1);

            let mut config_changed = false;
            if !new_goals.is_empty() {
                config.goals.extend(new_goals);
                config_changed = true;
            }
            if !new_aliases.is_empty() {
                for (k, v) in &new_aliases {
                    let _ = cfait::model::validate_alias_integrity(k, v, &config.tag_aliases);
                    config.tag_aliases.insert(k.clone(), v.clone());
                }
                config_changed = true;
            }
            if config_changed {
                let _ = config.save_with_credentials(ctx.as_ref());
            }

            let trimmed = clean_input.trim();
            if trimmed.is_empty()
                || (!trimmed.contains(' ')
                    && (trimmed.contains(":=") || trimmed.to_lowercase().starts_with("loc:")))
            {
                println!("{}", rust_i18n::t!("goal_or_alias_updated"));
                return Ok(());
            }

            let temp_store = build_store_cli(&ctx).await;
            let full_parent_uid = if let Some(partial) = parent_uid_arg {
                match resolve_uid(&temp_store, &partial) {
                    Some(uid) => Some(uid),
                    None => std::process::exit(1),
                }
            } else {
                None
            };

            let mut task = Task::new(&clean_input, &config.tag_aliases, def_time);
            if let Err(e) = temp_store.resolve_dependencies(&mut task) {
                eprintln!("{}", e);
                std::process::exit(1);
            }

            if let Some(d) = desc_text {
                task.description = d;
            }
            if let Some(p) = full_parent_uid {
                task.parent_uid = Some(p);
            }

            let target_href_input = col_href.unwrap_or_else(|| {
                config
                    .default_calendar
                    .clone()
                    .unwrap_or_else(|| cfait::storage::LOCAL_CALENDAR_HREF.to_string())
            });

            let mut target_href = resolve_collection_href(&ctx, &target_href_input).await;

            if !target_href.starts_with("local://")
                && !target_href.starts_with('/')
                && !target_href.starts_with("http")
            {
                eprintln!(
                    "{}",
                    rust_i18n::t!("warning_calendar_not_found", calendar = target_href)
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
                "{}",
                rust_i18n::t!(
                    "task_added_successfully",
                    uid = &uid[..std::cmp::min(8, uid.len())]
                )
            );

            let (effective_no_wait, is_auto) = get_sync_strategy(no_wait, wait, &ctx);

            if !effective_no_wait {
                // Best-effort background sync of the journal
                if let Err(e) = maybe_sync(ctx.clone()).await {
                    eprintln!(
                        "{}",
                        rust_i18n::t!("warning_background_sync_failed", error = e.to_string())
                    );
                }
            } else if is_auto {
                println!("{}", rust_i18n::t!("cli_action_queued_auto"));
            } else {
                println!("{}", rust_i18n::t!("cli_action_queued"));
            }
            return Ok(());
        }
        "edit" | "append" => {
            let is_append = command == "append";
            let mut col_href = None;
            let mut desc_text = None;
            let mut parent_uid_arg = None;
            let mut clear_parent = false;
            let mut clear_due = false;
            let mut clear_start = false;
            let mut clear_tags = false;
            let mut clear_loc = false;
            let mut clear_deps = false;
            let mut no_wait = false;
            let mut wait = false;
            let mut i = 2;
            let mut task_args = Vec::new();
            while i < args.len() {
                if args[i] == "--no-wait" || args[i] == "-n" {
                    no_wait = true;
                    i += 1;
                } else if args[i] == "--wait" || args[i] == "-w" {
                    wait = true;
                    i += 1;
                } else if args[i] == "--collection" || args[i] == "-c" {
                    if is_append {
                        eprintln!("Error: --collection not supported for append command");
                        std::process::exit(1);
                    }
                    if i + 1 < args.len() {
                        col_href = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for {}", args[i]);
                        std::process::exit(1);
                    }
                } else if args[i] == "--desc" {
                    if i + 1 < args.len() {
                        desc_text = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for --desc");
                        std::process::exit(1);
                    }
                } else if args[i] == "--parent" || args[i] == "-p" {
                    if is_append {
                        eprintln!("Error: --parent not supported for append command");
                        std::process::exit(1);
                    }
                    if i + 1 < args.len() {
                        parent_uid_arg = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for --parent");
                        std::process::exit(1);
                    }
                } else if args[i] == "--clear-parent" {
                    if !is_append {
                        clear_parent = true;
                    } else {
                        eprintln!("Error: --clear-parent not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else if args[i] == "--clear-due" {
                    if !is_append {
                        clear_due = true;
                    } else {
                        eprintln!("Error: --clear-due not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else if args[i] == "--clear-start" {
                    if !is_append {
                        clear_start = true;
                    } else {
                        eprintln!("Error: --clear-start not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else if args[i] == "--clear-tags" {
                    if !is_append {
                        clear_tags = true;
                    } else {
                        eprintln!("Error: --clear-tags not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else if args[i] == "--clear-loc" {
                    if !is_append {
                        clear_loc = true;
                    } else {
                        eprintln!("Error: --clear-loc not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else if args[i] == "--clear-deps" {
                    if !is_append {
                        clear_deps = true;
                    } else {
                        eprintln!("Error: --clear-deps not supported for append command");
                        std::process::exit(1);
                    }
                    i += 1;
                } else {
                    task_args.push(args[i].clone());
                    i += 1;
                }
            }
            if task_args.is_empty() {
                if is_append {
                    eprintln!(
                        "{}",
                        rust_i18n::t!("cli_usage_append", binary_name = binary_name)
                    );
                } else {
                    eprintln!(
                        "{}",
                        rust_i18n::t!("cli_usage_edit", binary_name = binary_name)
                    );
                }
                std::process::exit(1);
            }
            let partial_uid = task_args[0].clone();

            let input = {
                let lex_guard = cfait::model::parser::LEXICON.read().unwrap();
                let lex = &*lex_guard;
                task_args[1..]
                    .iter()
                    .map(|arg| shell_arg_to_smart_token(arg, lex))
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            let mut store = build_store_cli(&ctx).await;
            let full_uid = match resolve_uid(&store, &partial_uid) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let full_parent_uid = if let Some(partial) = parent_uid_arg {
                match resolve_uid(&store, &partial) {
                    Some(uid) => {
                        if uid == full_uid {
                            eprintln!("{}", rust_i18n::t!("error_cannot_be_child_of_self"));
                            std::process::exit(1);
                        }
                        Some(uid)
                    }
                    None => std::process::exit(1),
                }
            } else {
                None
            };

            let mut config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let def_time =
                chrono::NaiveTime::parse_from_str(&config.default_reminder_time, "%H:%M").ok();

            let mut actions = Vec::new();
            let mut changed = false;

            if !input.trim().is_empty() {
                let (clean_input_1, new_goals) = cfait::model::extract_inline_goals(&input);
                let (clean_input, new_aliases) =
                    cfait::model::extract_inline_aliases(&clean_input_1);

                let mut config_changed = false;
                if !new_goals.is_empty() {
                    config.goals.extend(new_goals);
                    config_changed = true;
                }
                if !new_aliases.is_empty() {
                    for (k, v) in &new_aliases {
                        let _ = cfait::model::validate_alias_integrity(k, v, &config.tag_aliases);
                        config.tag_aliases.insert(k.clone(), v.clone());
                    }
                    config_changed = true;
                }
                if config_changed {
                    let _ = config.save_with_credentials(ctx.as_ref());
                }

                let mut temp_task = None;
                if let Some((task_mut, _)) = store.get_task_mut(&full_uid) {
                    let input_to_apply = if is_append {
                        let mut existing = task_mut.to_smart_string();
                        if !clean_input.trim().is_empty() {
                            existing.push(' ');
                            existing.push_str(clean_input.trim());
                        }
                        existing
                    } else {
                        clean_input
                    };
                    let mut t = task_mut.clone();

                    if clear_deps {
                        t.dependencies.clear();
                    }

                    t.apply_smart_input(&input_to_apply, &config.tag_aliases, def_time);
                    temp_task = Some(t);
                }
                if let Some(mut t) = temp_task {
                    if let Err(e) = store.resolve_dependencies(&mut t) {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                    if let Some((task_mut, _)) = store.get_task_mut(&full_uid) {
                        *task_mut = t;
                        changed = true;
                    }
                }
            }

            if let Some(d) = desc_text
                && let Some((task_mut, _)) = store.get_task_mut(&full_uid)
            {
                if is_append {
                    if !task_mut.description.is_empty() {
                        task_mut.description.push_str("\n\n");
                    }
                    task_mut.description.push_str(&d);
                } else {
                    task_mut.description = d;
                }
                changed = true;
            }

            if let Some((task_mut, _)) = store.get_task_mut(&full_uid)
                && !is_append
            {
                if clear_parent {
                    if task_mut.parent_uid.is_some() {
                        task_mut.parent_uid = None;
                        changed = true;
                    }
                } else if let Some(p) = &full_parent_uid
                    && task_mut.parent_uid.as_deref() != Some(p.as_str())
                {
                    task_mut.parent_uid = Some(p.clone());
                    changed = true;
                }

                if clear_due && task_mut.due.is_some() {
                    task_mut.due = None;
                    changed = true;
                }
                if clear_start && task_mut.dtstart.is_some() {
                    task_mut.dtstart = None;
                    changed = true;
                }
                if clear_tags && !task_mut.categories.is_empty() {
                    task_mut.categories.clear();
                    changed = true;
                }
                if clear_loc && task_mut.location.is_some() {
                    task_mut.location = None;
                    changed = true;
                }
                if clear_deps && input.trim().is_empty() && !task_mut.dependencies.is_empty() {
                    task_mut.dependencies.clear();
                    changed = true;
                }
            }

            if changed && let Some((task_mut, _)) = store.get_task_mut(&full_uid) {
                task_mut.sequence += 1;
                actions.push(cfait::journal::Action::Update(task_mut.clone()));
            }

            if !is_append && let Some(target_href_input) = col_href {
                let matched_href = resolve_collection_href(&ctx, &target_href_input).await;
                let intent = cfait::model::AppIntent::MoveTask {
                    uid: full_uid.clone(),
                    target_href: matched_href,
                };
                let move_actions = store.apply_task_intent(&intent, &config);
                actions.extend(move_actions);
            }

            if !actions.is_empty() {
                let store_arc = Arc::new(tokio::sync::Mutex::new(store));
                let client_arc = Arc::new(tokio::sync::Mutex::new(None));
                let controller =
                    cfait::controller::TaskController::new(store_arc, client_arc, ctx.clone());

                controller
                    .persist_changes(actions)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                println!(
                    "{}",
                    rust_i18n::t!("task_updated_successfully", uid = partial_uid)
                );

                let (effective_no_wait, is_auto) = get_sync_strategy(no_wait, wait, &ctx);

                if !effective_no_wait {
                    if let Err(e) = maybe_sync(ctx.clone()).await {
                        eprintln!(
                            "{}",
                            rust_i18n::t!("warning_background_sync_failed", error = e.to_string())
                        );
                    }
                } else if is_auto {
                    println!("{}", rust_i18n::t!("cli_action_queued_auto"));
                } else {
                    println!("{}", rust_i18n::t!("cli_action_queued"));
                }
            } else {
                println!(
                    "{}",
                    rust_i18n::t!("task_no_changes_made", uid = partial_uid)
                );
            }
            return Ok(());
        }
        "list" | "search" => {
            // Parse arguments: support explicit --all, --json, and -c <id>
            let mut show_all = false;
            let mut as_json = false;
            let mut col_href = None;
            let mut parent_uid_arg = None;
            let mut query_parts: Vec<String> = Vec::new();

            let mut i = 2;
            while i < args.len() {
                if args[i] == "--all" {
                    show_all = true;
                    i += 1;
                } else if args[i] == "--json" {
                    as_json = true;
                    i += 1;
                } else if args[i] == "--collection" || args[i] == "-c" {
                    if i + 1 < args.len() {
                        col_href = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for {}", args[i]);
                        std::process::exit(1);
                    }
                } else if args[i] == "--parent" || args[i] == "-p" {
                    if i + 1 < args.len() {
                        parent_uid_arg = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: Missing value for --parent");
                        std::process::exit(1);
                    }
                } else {
                    query_parts.push(args[i].clone());
                    i += 1;
                }
            }

            let query = if command == "search" {
                query_parts.join(" ")
            } else {
                String::new()
            };

            let config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let store = build_store_cli(&ctx).await;

            let mut hidden: HashSet<String> = HashSet::new();
            let mut hide_completed = config.hide_completed;
            if !show_all {
                hidden.extend(config.hidden_calendars.into_iter());
                hidden.extend(config.disabled_calendars.into_iter());
            } else {
                hide_completed = false;
            }

            let mut target_href = None;
            if let Some(col_id) = col_href {
                target_href = Some(resolve_collection_href(&ctx, &col_id).await);
            }

            if target_href.as_deref() != Some(cfait::storage::LOCAL_TRASH_HREF) {
                hidden.insert(cfait::storage::LOCAL_TRASH_HREF.to_string());
            }
            if target_href.as_deref() != Some("local://recovery") {
                hidden.insert("local://recovery".to_string());
            }

            let full_parent_uid = if let Some(partial) = parent_uid_arg {
                match resolve_uid(&store, &partial) {
                    Some(uid) => Some(uid),
                    None => std::process::exit(1),
                }
            } else {
                None
            };

            let cutoff_date = if show_all {
                None
            } else {
                config
                    .sort_cutoff_days
                    .map(|d| Utc::now() + chrono::Duration::days(d as i64))
            };

            // Local empty sets to satisfy FilterOptions references
            let selected_categories: HashSet<String> = HashSet::new();
            let selected_locations: HashSet<String> = HashSet::new();
            let expanded_done_groups: HashSet<String> = HashSet::new();
            let expanded_tags: HashSet<String> = HashSet::new();
            let expanded_locations: HashSet<String> = HashSet::new();
            let search_collapsed_tasks: HashSet<String> = HashSet::new();

            let res = store.filter(FilterOptions {
                active_cal_href: target_href.as_deref(),
                hidden_calendars: &hidden,
                selected_categories: &selected_categories,
                selected_locations: &selected_locations,
                match_all_categories: false,
                search_term: &query,
                hide_completed_global: hide_completed,
                hide_fully_completed_tags: !show_all && config.hide_fully_completed_tags,
                hide_aliases_in_sidebar: config.hide_aliases_in_sidebar,
                cutoff_date,
                min_duration: None,
                max_duration: None,
                include_unset_duration: true,
                urgent_days: config.urgent_days_horizon,
                urgent_prio: config.urgent_priority_threshold,
                default_priority: config.default_priority,
                start_grace_period_days: config.start_grace_period_days,
                sort_standard_by_priority: config.sort_standard_by_priority,
                sort_preset: config.sort_preset,
                expanded_done_groups: &expanded_done_groups,
                expanded_tags: &expanded_tags,
                expanded_locations: &expanded_locations,
                max_done_roots: usize::MAX,
                max_done_subtasks: usize::MAX,
                tag_aliases: &config.tag_aliases,
                search_collapsed_tasks: &search_collapsed_tasks,
                focused_task_uid: full_parent_uid.as_deref(),
            });

            if as_json {
                let tasks: Vec<&Task> = res
                    .items
                    .iter()
                    .filter_map(|i| {
                        if let cfait::store::TaskListItem::Task(t) = i {
                            Some(t.as_ref())
                        } else {
                            None
                        }
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tasks).unwrap_or_default()
                );
                return Ok(());
            }

            if res.items.is_empty() {
                println!("{}", rust_i18n::t!("status_no_tasks_found"));
                return Ok(());
            }

            for item in res.items {
                if let cfait::store::TaskListItem::Task(t) = item {
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
            }
            return Ok(());
        }
        "tree" => {
            let mut partial = String::new();
            for arg in args.iter().skip(2) {
                partial = arg.clone();
            }
            if partial.is_empty() {
                eprintln!("{}", rust_i18n::t!("error_uid_required"));
                std::process::exit(1);
            }

            let store = build_store_cli(&ctx).await;
            let uid = match resolve_uid(&store, &partial) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let tree_md = cfait::model::extractor::serialize_task_tree(&store, &uid);
            println!("{}", tree_md);
            return Ok(());
        }
        "view" | "show" => {
            let mut as_json = false;
            let mut partial = String::new();
            for arg in args.iter().skip(2) {
                if arg == "--json" {
                    as_json = true;
                } else {
                    partial = arg.clone();
                }
            }
            if partial.is_empty() {
                eprintln!("{}", rust_i18n::t!("error_uid_required"));
                std::process::exit(1);
            }

            let store = build_store_cli(&ctx).await;
            let uid = match resolve_uid(&store, &partial) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };
            let t = store
                .get_task_ref(&uid)
                .ok_or_else(|| anyhow::anyhow!(rust_i18n::t!("error_task_not_found")))?;

            if as_json {
                println!("{}", serde_json::to_string_pretty(&t).unwrap_or_default());
                return Ok(());
            }

            println!("{}:  {}", rust_i18n::t!("cli_view_summary"), t.summary);
            println!(
                "{}:   {:?} {}",
                rust_i18n::t!("cli_view_status"),
                t.status,
                t.checkbox_symbol()
            );
            println!("{}:      {}", rust_i18n::t!("cli_view_uid"), t.uid);
            if let Some(d) = &t.due {
                println!(
                    "{}:      {}",
                    rust_i18n::t!("cli_view_due"),
                    d.format_smart()
                );
            }
            if !t.categories.is_empty() {
                println!(
                    "{}:     {}",
                    rust_i18n::t!("cli_view_tags"),
                    t.categories.join(", ")
                );
            }
            if let Some(l) = &t.location {
                println!("{}: {}", rust_i18n::t!("cli_view_location"), l);
            }
            if !t.description.is_empty() {
                println!(
                    "\n{}:\n{}",
                    rust_i18n::t!("cli_view_description"),
                    t.description
                );
            }
            return Ok(());
        }
        "start" | "pause" | "toggle" | "done" | "complete" => {
            let store = build_store_cli(&ctx).await;
            let mut partial_uid = String::new();
            let mut no_wait = false;
            let mut wait = false;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--no-wait" || args[i] == "-n" {
                    no_wait = true;
                } else if args[i] == "--wait" || args[i] == "-w" {
                    wait = true;
                } else if partial_uid.is_empty() {
                    partial_uid = args[i].clone();
                }
                i += 1;
            }
            if partial_uid.is_empty() {
                eprintln!("{}", rust_i18n::t!("error_missing_uid"));
                std::process::exit(1);
            }
            let full_uid = match resolve_uid(&store, &partial_uid) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let store_arc = Arc::new(tokio::sync::Mutex::new(store));
            let client_arc = Arc::new(tokio::sync::Mutex::new(None));
            let controller =
                cfait::controller::TaskController::new(store_arc, client_arc, ctx.clone());

            match command {
                "start" => {
                    let mut store_lock = controller.store.lock().await;
                    let intent = cfait::model::AppIntent::StartTask {
                        uid: full_uid.clone(),
                    };
                    let actions = store_lock.apply_task_intent(&intent, &config);
                    drop(store_lock);
                    controller
                        .persist_changes(actions)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", rust_i18n::t!("task_started", uid = partial_uid));
                }
                "pause" => {
                    let mut store_lock = controller.store.lock().await;
                    let intent = cfait::model::AppIntent::PauseTask {
                        uid: full_uid.clone(),
                    };
                    let actions = store_lock.apply_task_intent(&intent, &config);
                    drop(store_lock);
                    controller
                        .persist_changes(actions)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", rust_i18n::t!("task_paused", uid = partial_uid));
                }
                _ => {
                    let mut store_lock = controller.store.lock().await;
                    let intent = cfait::model::AppIntent::ToggleTask {
                        uid: full_uid.clone(),
                    };
                    let actions = store_lock.apply_task_intent(&intent, &config);
                    drop(store_lock);
                    controller
                        .persist_changes(actions)
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    println!("{}", rust_i18n::t!("task_toggled", uid = partial_uid));
                }
            }

            let (effective_no_wait, is_auto) = get_sync_strategy(no_wait, wait, &ctx);

            if !effective_no_wait {
                // Best-effort background sync
                if let Err(e) = maybe_sync(ctx.clone()).await {
                    eprintln!(
                        "{}",
                        rust_i18n::t!("warning_background_sync_failed", error = e.to_string())
                    );
                }
            } else if is_auto {
                println!("{}", rust_i18n::t!("cli_action_queued_auto"));
            } else {
                println!("{}", rust_i18n::t!("cli_action_queued"));
            }
            return Ok(());
        }
        "collection" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("list");
            if sub == "list" {
                let as_json = args.iter().any(|a| a == "--json");
                let mut all_cals = Vec::new();
                if let Ok(locals) = cfait::storage::LocalCalendarRegistry::load(ctx.as_ref()) {
                    all_cals.extend(locals);
                }
                if let Ok(remotes) = cfait::cache::Cache::load_calendars(ctx.as_ref()) {
                    all_cals.extend(remotes);
                }
                if as_json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&all_cals).unwrap_or_default()
                    );
                    return Ok(());
                }
                println!("{:<30} {:<40} COLOR", "NAME", "HREF");
                for cal in all_cals {
                    println!(
                        "{:<30} {:<40} {}",
                        cal.name,
                        cal.href,
                        cal.color.unwrap_or_default()
                    );
                }
                return Ok(());
            }

            if args.len() < 4 {
                eprintln!(
                    "{}",
                    rust_i18n::t!("cli_usage_collection", binary_name = binary_name)
                );
                std::process::exit(1);
            }
            let sub = &args[2];
            let config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let client = match cfait::client::RustyClient::new(
                ctx.clone(),
                &config.url,
                &config.username,
                &config.password,
                config.allow_insecure_certs,
                Some("CLI"),
            ) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to initialize client: {}", e);
                    std::process::exit(1);
                }
            };

            match sub.as_str() {
                "create" => {
                    let name = &args[3];
                    let mut color = None;
                    if args.len() >= 6 && args[4] == "--color" {
                        color = Some(args[5].as_str());
                    }
                    match client.create_calendar(name, color).await {
                        Ok(href) => {
                            println!("{}", rust_i18n::t!("collection_created_href", href = href))
                        }
                        Err(e) => {
                            eprintln!("{}", rust_i18n::t!("error_creating_collection", error = e));
                            std::process::exit(1);
                        }
                    }
                }
                "edit" => {
                    let href = &args[3];
                    let mut name = None;
                    let mut color = None;
                    let mut i = 4;
                    while i < args.len() {
                        if args[i] == "--name" && i + 1 < args.len() {
                            name = Some(args[i + 1].as_str());
                            i += 2;
                        } else if args[i] == "--color" && i + 1 < args.len() {
                            color = Some(args[i + 1].as_str());
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    if let Some(n) = name {
                        match client.update_calendar(href, n, color).await {
                            Ok(_) => println!(
                                "{}",
                                rust_i18n::t!("collection_updated_href", href = href)
                            ),
                            Err(e) => {
                                eprintln!(
                                    "{}",
                                    rust_i18n::t!("error_updating_collection", error = e)
                                );
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("{}", rust_i18n::t!("error_name_required"));
                        std::process::exit(1);
                    }
                }
                _ => {
                    eprintln!("{}", rust_i18n::t!("error_unknown_collection_command"));
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        "delete" | "rm" => {
            let mut partial_uid = String::new();
            let mut no_wait = false;
            let mut wait = false;
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--no-wait" || args[i] == "-n" {
                    no_wait = true;
                } else if args[i] == "--wait" || args[i] == "-w" {
                    wait = true;
                } else if partial_uid.is_empty() {
                    partial_uid = args[i].clone();
                }
                i += 1;
            }
            if partial_uid.is_empty() {
                eprintln!("{}", rust_i18n::t!("error_missing_uid"));
                std::process::exit(1);
            }
            let store = build_store_cli(&ctx).await;
            let full_uid = match resolve_uid(&store, &partial_uid) {
                Some(uid) => uid,
                None => std::process::exit(1),
            };

            let config =
                cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap_or_default();
            let store_arc = Arc::new(tokio::sync::Mutex::new(store));
            let client_arc = Arc::new(tokio::sync::Mutex::new(None));
            let controller =
                cfait::controller::TaskController::new(store_arc, client_arc, ctx.clone());

            let actions = {
                let mut store_lock = controller.store.lock().await;
                let intent = cfait::model::AppIntent::DeleteTask {
                    uid: full_uid.clone(),
                };
                store_lock.apply_task_intent(&intent, &config)
            };
            controller
                .persist_changes(actions)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", rust_i18n::t!("task_deleted", uid = partial_uid));

            let (effective_no_wait, is_auto) = get_sync_strategy(no_wait, wait, &ctx);

            if !effective_no_wait {
                // Best-effort background sync
                if let Err(e) = maybe_sync(ctx.clone()).await {
                    eprintln!(
                        "{}",
                        rust_i18n::t!("warning_background_sync_failed", error = e.to_string())
                    );
                }
            } else if is_auto {
                println!("{}", rust_i18n::t!("cli_action_queued_auto"));
            } else {
                println!("{}", rust_i18n::t!("cli_action_queued"));
            }
            return Ok(());
        }
        "" => {
            // No non-interactive command provided; fall through to start the interactive TUI.
        }
        _ => {
            eprintln!(
                "{}",
                rust_i18n::t!("error_unknown_command", command = command)
            );
            std::process::exit(1);
        }
    }

    // --- Start interactive TUI ---
    #[cfg(not(target_os = "android"))]
    let _ui_lock = cfait::storage::DaemonLock::acquire_shared(ctx.as_ref())
        .map_err(|e| eprintln!("Warning: Could not acquire shared UI lock: {}", e))
        .ok();

    #[cfg(not(target_os = "android"))]
    let _presence_lock = cfait::storage::PresenceLock::acquire_shared(ctx.as_ref()).ok();

    cfait::tui::run(ctx).await
}
