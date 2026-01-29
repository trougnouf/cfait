// Binary entry point for the TUI application.

// File: ./src/bin/tui.rs
use anyhow::Result;
use cfait::context::{AppContext, StandardContext}; // Import AppContext traits
use cfait::storage::LocalStorage;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Create the application context (StandardContext for production use)
    let ctx: Arc<dyn AppContext> = Arc::new(StandardContext::new(None));

    // Handle help flag
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h" || args[1] == "help") {
        print_help();
        return Ok(());
    }

    // CLI Command: cfait import
    if args.len() > 1 && args[1] == "import" {
        if args.len() < 3 {
            eprintln!("Error: Missing file path");
            eprintln!("Usage: cfait import <file.ics> [--calendar <id>]");
            std::process::exit(1);
        }

        let file_path = &args[2];

        // Check for --calendar flag
        let calendar_id = if args.len() > 4 && args[3] == "--calendar" {
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

        // Determine target calendar
        let href = if let Some(cal_id) = calendar_id {
            if cal_id == "default" {
                "local://default".to_string()
            } else {
                format!("local://{}", cal_id)
            }
        } else {
            "local://default".to_string()
        };

        // Import tasks
        match LocalStorage::import_from_ics(ctx.as_ref(), &href, &ics_content) {
            Ok(count) => {
                println!(
                    "Successfully imported {} task(s) to calendar '{}'",
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
        // Check for --calendar flag
        let calendar_id = if args.len() > 3 && args[2] == "--calendar" {
            Some(args[3].clone())
        } else {
            None
        };

        let tasks = if let Some(cal_id) = calendar_id {
            // Export specific calendar
            let href = if cal_id == "default" {
                "local://default".to_string()
            } else {
                format!("local://{}", cal_id)
            };
            LocalStorage::load_for_href(ctx.as_ref(), &href)?
        } else {
            // Export default calendar for backward compatibility
            // LocalStorage::load() was removed, use load_for_href with default
            LocalStorage::load_for_href(ctx.as_ref(), cfait::storage::LOCAL_CALENDAR_HREF)?
        };

        let ics = LocalStorage::to_ics_string(&tasks);
        println!("{}", ics);
        return Ok(());
    }

    // Normal TUI startup
    cfait::tui::run().await
}

fn print_help() {
    println!(
        "Cfait v{} - A powerful, fast and elegant CalDAV task manager (TUI)",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE:");
    println!("    cfait                                    Start interactive TUI");
    println!(
        "    cfait export [--calendar <id>]           Export local tasks as .ics file to stdout"
    );
    println!("    cfait import <file.ics> [--calendar <id>] Import tasks from .ics file");
    println!("    cfait --help                             Show this help message");
    println!();
    println!("IMPORT COMMAND:");
    println!("    cfait import tasks.ics                        Import to default local calendar");
    println!("    cfait import tasks.ics --calendar <id>        Import to specific local calendar");
    println!("    cfait import backup.ics --calendar my-cal     Import to 'my-cal' calendar");
    println!();
    println!("EXPORT COMMAND:");
    println!("    cfait export                              Export default local calendar");
    println!("    cfait export --calendar <id>              Export specific local calendar");
    println!("    cfait export > backup.ics                 Save tasks to file");
    println!("    cfait export --calendar my-cal > my.ics   Export specific calendar to file");
    println!("    cfait export | grep 'SUMMARY'             Filter output");
    println!();
    println!("KEYBINDINGS:");
    println!("    Press '?' inside the app for full interactive help");
    println!();
    println!("SMART INPUT SYNTAX:");
    println!("    !1-9              Priority (1=highest, 9=lowest)");
    println!("    #tag              Add category/tag (supports hierarchy: #work:project)");
    println!("    @@location        Add location (supports hierarchy: @@home:office)");
    println!("    @date             Set due date (@tomorrow, @2d, @next friday)");
    println!("    ^date             Set start date (^next week, ^2025-01-01)");
    println!("    ~duration         Set duration (~30m, ~1.5h, ~1h-2h)");
    println!("    @daily            Recurrence (@daily, @weekly, @every 3 days)");
    println!("    until <date>      End date for recurrence (@daily until 2025-12-31)");
    println!("    except <date>     Skip dates (@daily except 2025-12-25,2026-01-01)");
    println!("    except <weekday>  Exclude weekdays (@daily except mo,tue or saturdays,sundays)");
    println!("    except <month>    Exclude months (@monthly except oct,nov,dec)");
    println!("    @friday           Next weekday (@friday = @next friday)");
    println!("    @next X           Next week/month/year (@next week, @next month)");
    println!("    \"in\" optional     @2 weeks = @in 2 weeks (the word \"in\" is optional)");
    println!("    #alias:=#tags     Define tag alias inline (retroactive)");
    println!("    @@alias:=#tags    Define location alias (@@aldi:=#groceries,#shopping)");
    println!("    url:              Attach URL");
    println!("    geo:              Add coordinates");
    println!("    desc:             Add description");
    println!("    rem:10m           Relative reminder (before due date, adjusts)");
    println!("    rem:in 5m         Relative from now (becomes absolute)");
    println!("    rem:next friday   Next occurrence (becomes absolute)");
    println!("    rem:8am           Absolute reminder (fixed time)");
    println!("    +cal              Force create calendar event (override global setting)");
    println!("    -cal              Prevent calendar event creation (override global setting)");
    println!("    \\#text            Escape special characters");
    println!();
    println!("EXAMPLES:");
    println!("    Buy cookies !1 @2025-01-16 #shopping rem:2025-01-16 8am");
    println!("    Exercise @daily ~30m #health rem:8am");
    println!("    Meeting @tomorrow 2pm ~1h +cal (force create calendar event)");
    println!("    Plant plum tree #tree_planting !3 ~2h @@home:garden");
    println!("    #tree_planting:=#gardening,@@home");
    println!("    @@aldi:=#groceries,#shopping (location alias)");
    println!();
    println!("MORE INFO:");
    println!("    Repository: https://codeberg.org/trougnouf/cfait");
    println!("    License:    GPL-3.0");
}
