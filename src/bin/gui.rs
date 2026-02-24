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
    // Detect Windows 10 specifically: enable server-side (native) decorations by default on Win10.
    // Users can override this behavior with --force-csd (force client-side decorations)
    // or --force-ssd to explicitly force server-side decorations.
    let mut force_ssd: bool = {
        #[cfg(target_os = "windows")]
        {
            // Lightweight runtime detection using `os_info`.
            // Treat reported Windows versions whose version string starts with "10"
            // as Windows 10 (e.g., "10", "10.0", "10.0.19041").
            let info = os_info::get();
            matches!(info.os_type(), os_info::Type::Windows)
                && info.version().to_string().starts_with("10")
        }
        #[cfg(not(target_os = "windows"))]
        {
            // On non-Windows platforms, don't reference `os_info` and default to false.
            false
        }
    };

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
            "--force-csd" => {
                force_ssd = false;
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

    // Create context to grab the lock
    let ctx: std::sync::Arc<dyn cfait::context::AppContext> =
        std::sync::Arc::new(cfait::context::StandardContext::new(override_root.clone()));

    // Grab the shared lock for the GUI (allows multiple UIs, blocks daemon)
    #[cfg(not(target_os = "android"))]
    let _ui_lock = cfait::storage::DaemonLock::acquire_shared(ctx.as_ref())
        .map_err(|e| eprintln!("Warning: Could not acquire shared UI lock: {}", e))
        .ok();

    cfait::gui::run_with_ics_file(ics_file_path, override_root, force_ssd)
}
