// Binary entry point for the GUI application.
fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();

    // Check if an ICS file path was provided
    let ics_file_path = if args.len() > 1 && !args[1].starts_with("--") && !args[1].starts_with("-") {
        Some(args[1].clone())
    } else {
        None
    };

    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!(
            "Cfait v{} - A powerful, fast and elegant CalDAV task manager (GUI)",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("USAGE:");
        println!("    cfait-gui");
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
        println!("    ~duration         Set duration (~30m, ~1.5h)");
        println!("    @daily            Recurrence (@daily, @weekly, @every 3 days)");
        println!("    until <date>      End date for recurrence (@daily until 2025-12-31)");
        println!("    except <date>     Skip dates (@daily except 2025-12-25,2026-01-01)");
        println!(
            "    except <weekday>  Exclude weekdays (@daily except mo,tue or saturdays,sundays)"
        );
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
        return Ok(());
    }

    cfait::gui::run_with_ics_file(ics_file_path)
}
