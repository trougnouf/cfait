// File: ./src/cli.rs
//! Shared command-line interface logic, like printing help.

pub fn print_help(binary_name: &str) {
    let is_gui = binary_name.contains("gui");

    println!(
        "Cfait v{} - A powerful, fast and elegant CalDAV task manager ({})",
        env!("CARGO_PKG_VERSION"),
        if is_gui { "GUI" } else { "TUI" }
    );
    println!();
    println!("USAGE:");
    if is_gui {
        println!(
            "    {} [--root <path>] [--force-ssd] [--force-csd] [path/to/file.ics]",
            binary_name
        );
    } else {
        println!("    {} [--root <path>]", binary_name);
        println!("    {} export [--calendar <id>]", binary_name);
        println!("    {} import <file.ics> [--calendar <id>]", binary_name);
        println!("    {} --help", binary_name);
    }
    println!();
    println!("OPTIONS:");
    if is_gui {
        println!("    <path/to/file.ics>    Open an ICS file on startup to import it.");
    }
    println!("    -r, --root <path>     Use a different directory for config and data.");
    if is_gui {
        println!("    --force-ssd           Force server-side (native) window decorations.");
        println!(
            "    --force-csd           Force client-side (custom) window decorations (override)."
        );
    }
    println!("    -h, --help            Show this help message.");
    println!();

    if !is_gui {
        println!("SYNC COMMANDS:");
        println!("    {} sync                                    Fetch and push changes, then exit", binary_name);
        println!("    {} daemon                                  Run continuously in background and sync", binary_name);
        println!("                                               at the interval specified in settings");
        println!();
        println!("IMPORT COMMAND:");
        println!(
            "    {} import tasks.ics                        Import to default local calendar",
            binary_name
        );
        println!(
            "    {} import tasks.ics --calendar <id>        Import to specific local calendar",
            binary_name
        );
        println!(
            "    {} import backup.ics --calendar my-cal     Import to 'my-cal' calendar",
            binary_name
        );
        println!();

        println!("EXPORT COMMAND:");
        println!(
            "    {} export                              Export default local calendar",
            binary_name
        );
        println!(
            "    {} export --calendar <id>              Export specific local calendar",
            binary_name
        );
        println!(
            "    {} export > backup.ics                 Save tasks to file",
            binary_name
        );
        println!(
            "    {} export --calendar my-cal > my.ics   Export specific calendar to file",
            binary_name
        );
        println!(
            "    {} export | grep 'SUMMARY'             Filter output",
            binary_name
        );
        println!();
    }

    if is_gui {
        println!(
            "This will open the graphical interface. For detailed smart input syntax and other"
        );
        println!("command-line operations (like import/export), see 'cfait --help'.");
    } else {
        println!("KEYBINDINGS:");
        println!("    Press '?' inside the app for full interactive help");
        println!();
        println!("SMART INPUT SYNTAX:");
        for sec in crate::help::SYNTAX_HELP {
            for item in sec.items {
                let padded = format!("{:width$}", item.keys, width = 18);
                println!("    {} {}", padded, item.desc);
            }
        }
        println!();
        println!("EXAMPLES:");
        println!("    Buy cookies !1 @2025-01-16 #shopping rem:2025-01-16 8am");
        println!("    Exercise @daily ~30m #health rem:8am");
        println!("    Meeting @tomorrow 2pm ~1h +cal (force create calendar event)");
        println!("    Plant plum tree #tree_planting !3 ~2h @@home:garden");
        println!("    #tree_planting:=#gardening,@@home");
        println!("    @@aldi:=#groceries,#shopping (location alias)");
    }

    println!();
    println!("MORE INFO:");
    println!("    Repository: https://codeberg.org/trougnouf/cfait");
    println!("    License:    GPL-3.0");
}
