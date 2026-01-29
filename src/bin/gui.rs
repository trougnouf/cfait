// Binary entry point for the GUI application.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    let mut override_root: Option<PathBuf> = None;
    let mut ics_file_path: Option<String> = None;
    let mut force_ssd = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--root" | "-r" => {
                if i + 1 < args.len() {
                    override_root = Some(args[i + 1].clone().into());
                    i += 1; // Also consumed the value
                }
            }
            "--force-ssd" => {
                force_ssd = true;
            }
            arg if !arg.starts_with('-') => {
                // If it's not a flag, assume it's the ICS file path.
                // Only take the first one.
                if ics_file_path.is_none() {
                    ics_file_path = Some(arg.to_string());
                }
            }
            _ => { /* Ignore unknown flags */ }
        }
        i += 1;
    }

    cfait::gui::run_with_ics_file(ics_file_path, override_root, force_ssd)
}

fn print_help() {
    println!(
        "Cfait v{} - A powerful, fast and elegant CalDAV task manager (GUI)",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("USAGE:");
    println!("    cfait-gui [--root <path>] [--force-ssd] [path/to/file.ics]");
    println!();
    println!("OPTIONS:");
    println!("    <path/to/file.ics>    Open an ICS file on startup to import it.");
    println!("    -r, --root <path>     Use a different directory for config and data.");
    println!("    --force-ssd           Force server-side (native) window decorations.");
    println!("    -h, --help            Show this help message.");
    println!();
    println!("This will open the graphical interface. For detailed smart input syntax and other");
    println!("command-line operations (like import/export), see 'cfait --help'.");
}
