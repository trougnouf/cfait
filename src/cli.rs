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

    println!("USAGE:");
    if is_gui {
        println!(
            "    {} [--root <path>] [--force-ssd] [--force-csd] [{}]",
            binary_name,
            rust_i18n::t!("cli_file_path_placeholder")
        );
    } else {
        println!("    {} [--root <path>]", binary_name);
        println!("    {} export [--collection <id>]", binary_name);
        println!(
            "    {} import <{}> [--collection <id>]",
            binary_name,
            rust_i18n::t!("cli_file_placeholder")
        );
        println!("    {} --help", binary_name);
    }
    println!();

    // Helper to align commands and descriptions uniformly without tabs
    let print_cmd = |cmd: &str, desc: String| {
        // Pads the command column to exactly 43 characters
        println!("    {cmd:43} {desc}");
    };

    println!("{}", rust_i18n::t!("cli_options_heading"));
    if is_gui {
        print_cmd(
            "--force-ssd",
            rust_i18n::t!("cli_desc_force_ssd").to_string(),
        );
        print_cmd(
            "--force-csd",
            rust_i18n::t!("cli_desc_force_csd").to_string(),
        );
    } else {
        print_cmd(
            "-r, --root <path>",
            rust_i18n::t!("cli_desc_root").to_string(),
        );
    }
    print_cmd("-h, --help", rust_i18n::t!("cli_desc_help").to_string());

    println!();

    if !is_gui {
        println!("{}", rust_i18n::t!("cli_sync_commands_heading"));
        print_cmd(
            &format!("{binary_name} sync"),
            rust_i18n::t!("cli_desc_sync").to_string(),
        );

        let daemon_cmd = format!("{binary_name} daemon");
        let daemon_desc = rust_i18n::t!("cli_desc_daemon").to_string();
        let mut lines = daemon_desc.split('\n');

        // Print first line with the command
        if let Some(first) = lines.next() {
            print_cmd(&daemon_cmd, first.to_string());
        }
        // Print subsequent lines perfectly aligned under the description column
        for line in lines {
            println!("    {:43} {}", "", line);
        }
        println!();

        println!("{}", rust_i18n::t!("cli_import_command"));
        print_cmd(
            &format!(
                "{binary_name} import <{}>",
                rust_i18n::t!("cli_file_placeholder")
            ),
            rust_i18n::t!("cli_desc_import_default").to_string(),
        );
        print_cmd(
            &format!(
                "{binary_name} import <{}> --collection <id>",
                rust_i18n::t!("cli_file_placeholder")
            ),
            rust_i18n::t!("cli_desc_import_specific").to_string(),
        );
        println!();

        println!("{}", rust_i18n::t!("cli_export_command"));
        print_cmd(
            &format!("{binary_name} export"),
            rust_i18n::t!("cli_desc_export_default").to_string(),
        );
        print_cmd(
            &format!("{binary_name} export --collection <id>"),
            rust_i18n::t!("cli_desc_export_specific").to_string(),
        );
        print_cmd(
            &format!("{binary_name} export > backup.ics"),
            rust_i18n::t!("cli_desc_export_file").to_string(),
        );
        print_cmd(
            &format!("{binary_name} export | grep 'SUMMARY'"),
            rust_i18n::t!("cli_desc_export_filter").to_string(),
        );
        println!();
    }

    if is_gui {
        println!("{}", rust_i18n::t!("cli_gui_note"));
    } else {
        println!("{}", rust_i18n::t!("cli_keybindings_heading"));
        println!("    {}", rust_i18n::t!("cli_press_question"));
        println!();
        println!("{}", rust_i18n::t!("cli_smart_input_heading"));
        for sec in crate::help::get_syntax_help() {
            for item in &sec.items {
                let padded = format!("{:width$}", item.keys, width = 18);
                println!("    {} {}", padded, item.desc);
            }
        }
        println!();
        println!("{}", rust_i18n::t!("cli_examples_heading"));
        println!("{}", rust_i18n::t!("cli_examples"));
    }

    println!();
    println!("MORE INFO:");
    println!(
        "    {:<15} https://codeberg.org/trougnouf/cfait",
        rust_i18n::t!("cli_repo_label")
    );
    println!(
        "    {:<15} GPL-3.0",
        rust_i18n::t!("cli_license_label")
    );
}
