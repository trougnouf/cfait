// Binary entry point for the TUI application.

// File: ./src/bin/tui.rs
use anyhow::Result;
use cfait::context::{AppContext, StandardContext}; // Import AppContext traits
use cfait::storage::LocalStorage;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

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
        args.remove(pos); // remove flag
        args.remove(pos); // remove value (which is now at the same index)
    }

    // Create the application context (StandardContext for production use) with the override
    let ctx: Arc<dyn AppContext> = Arc::new(StandardContext::new(override_root));

    // Handle help flag
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h" || args[1] == "help") {
        cfait::cli::print_help(&binary_name);
        return Ok(());
    }

    // CLI Command: cfait import
    if args.len() > 1 && args[1] == "import" {
        if args.len() < 3 {
            eprintln!("Error: Missing file path");
            eprintln!("Usage: cfait import <file.ics> [--collection <id>]");
            std::process::exit(1);
        }

        let file_path = &args[2];

        // Check for --collection flag
        let collection_id = if args.len() > 4 && args[3] == "--collection" {
            Some(args[4].clone())
        } else {
            None
        };

        // Read ICS file
        let ics_content = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path, e);
                std::process::exit(1);
            }
        };

        // Determine target collection
        let href = if let Some(col_id) = collection_id {
            if col_id == "default" {
                "local://default".to_string()
            } else {
                format!("local://{}", col_id)
            }
        } else {
            "local://default".to_string()
        };

        // Import tasks
        match LocalStorage::import_from_ics(ctx.as_ref(), &href, &ics_content) {
            Ok(count) => {
                println!(
                    "Successfully imported {} task(s) to collection '{}'",
                    count, href
                );
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error importing tasks: {}", e);
                std::process::exit(1);
            }
        }
    }

    // CLI Command: cfait export
    if args.len() > 1 && args[1] == "export" {
        // Check for --collection flag
        let collection_id = if args.len() > 3 && args[2] == "--collection" {
            Some(args[3].clone())
        } else {
            None
        };

        let tasks = if let Some(col_id) = collection_id {
            // Export specific collection
            let href = if col_id == "default" {
                "local://default".to_string()
            } else {
                format!("local://{}", col_id)
            };
            LocalStorage::load_for_href(ctx.as_ref(), &href)?
        } else {
            // Export default collection for backward compatibility
            // LocalStorage::load() was removed, use load_for_href with default
            LocalStorage::load_for_href(ctx.as_ref(), cfait::storage::LOCAL_CALENDAR_HREF)?
        };

        let ics = LocalStorage::to_ics_string(&tasks);
        println!("{}", ics);
        return Ok(());
    }

    // CLI Command: cfait sync
    if args.len() > 1 && args[1] == "sync" {
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

    // CLI Command: cfait daemon
    if args.len() > 1 && args[1] == "daemon" {
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
                        // _lock is dropped here, allowing UIs to open instantly
                    }
                    Ok(None) => {
                        // A UI is open holding a shared lock. Stay quiet.
                    }
                    Err(e) => eprintln!("Daemon: Failed to check instance lock: {}", e),
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(interval as u64 * 60)).await;
        }
    }

    // Grab the shared lock for the TUI (allows multiple UIs, blocks daemon)
    #[cfg(not(target_os = "android"))]
    let _ui_lock = cfait::storage::DaemonLock::acquire_shared(ctx.as_ref())
        .map_err(|e| eprintln!("Warning: Could not acquire shared UI lock: {}", e))
        .ok();

    // Normal TUI startup
    cfait::tui::run(ctx).await
}

// Help printing is provided by the shared CLI module:
// use `cfait::cli::print_help(&binary_name)` instead.
