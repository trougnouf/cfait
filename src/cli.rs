// File: ./src/cli.rs
//! Shared command-line interface logic, like printing help.

pub fn print_help(binary_name: &str) {
    let is_gui = binary_name.contains("gui");

    // Localized title (uses locales/en.json key `cli_title`)
    println!(
        "{}",
        rust_i18n::t!(
            "cli_title",
            version = env!("CARGO_PKG_VERSION"),
            mode = if is_gui { "GUI" } else { "TUI" }
        )
    );
    println!();

    // Usage (fall back to existing English headings where appropriate)
    println!("USAGE:");
    if is_gui {
        println!("{}", rust_i18n::t!("cli_usage_gui", binary = binary_name));
    } else {
        println!("{}", rust_i18n::t!("cli_usage_tui", binary = binary_name));
        println!(
            "{}",
            rust_i18n::t!("cli_usage_export", binary = binary_name)
        );
        println!(
            "{}",
            rust_i18n::t!("cli_usage_import", binary = binary_name)
        );
        println!("{}", rust_i18n::t!("cli_option_help"));
    }
    println!();

    println!("{}", rust_i18n::t!("cli_options_heading"));
    if is_gui {
        // GUI-specific option help lines localized
        println!("{}", rust_i18n::t!("cli_option_force_ssd"));
        println!("{}", rust_i18n::t!("cli_option_force_csd"));
        println!("{}", rust_i18n::t!("cli_import_command"));
    } else {
        println!("{}", rust_i18n::t!("cli_option_root"));
    }
    println!("{}", rust_i18n::t!("cli_option_help"));

    println!();

    // Sync/Export sections (localized)
    if !is_gui {
        println!("{}", rust_i18n::t!("cli_sync_commands_heading"));
        println!(
            "{}",
            rust_i18n::t!("cli_sync_command_sync", binary = binary_name)
        );
        println!(
            "{}",
            rust_i18n::t!("cli_sync_command_daemon", binary = binary_name)
        );
        // Import examples/localized short descriptions
        println!("{}", rust_i18n::t!("cli_import_command"));
        println!(
            "{}",
            rust_i18n::t!("cli_import_examples", binary = binary_name)
        );
        println!();

        println!("{}", rust_i18n::t!("cli_export_command"));
        println!(
            "{}",
            rust_i18n::t!("cli_usage_export", binary = binary_name)
        );
        println!(
            "{}",
            rust_i18n::t!("cli_export_examples", binary = binary_name)
        );
        println!();
    }

    // GUI vs TUI specific notes
    if is_gui {
        println!("{}", rust_i18n::t!("cli_gui_note"));
    } else {
        println!("{}", rust_i18n::t!("cli_keybindings_heading"));
        println!("{}", rust_i18n::t!("cli_press_question"));
        println!();
        println!("{}", rust_i18n::t!("cli_smart_input_heading"));
        for sec in crate::help::get_syntax_help() {
            for item in &sec.items {
                let padded = format!("{:width$}", item.keys, width = 18);
                println!("    {} {}", padded, item.desc);
            }
        }
        println!();
        println!("{}", rust_i18n::t!("cli_examples"));
        // The `cli_examples` key contains a multi-line examples block in the SSOT
        println!("{}", rust_i18n::t!("cli_examples"));
    }

    println!();
    println!("{}", rust_i18n::t!("cli_more_info_repo"));
    println!("{}", rust_i18n::t!("cli_more_info_license"));
}
