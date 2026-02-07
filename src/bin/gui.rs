// Binary entry point for the GUI application.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let binary_name = args
        .first()
        .cloned()
        .unwrap_or_else(|| "cfait-gui".to_string());

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        cfait::cli::print_help(&binary_name);
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
