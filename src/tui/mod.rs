// File: ./src/tui/mod.rs
// Entry point and main loop for the TUI application.
pub mod action;
pub mod handlers;
pub mod network;
pub mod state;
pub mod view;

use crate::config;
use crate::system::{AlarmMessage, SystemEvent};
use crate::tui::action::AppEvent;
use crate::tui::state::{AppState, InputMode};
use crate::tui::view::draw;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use rpassword::prompt_password;
use std::{
    env,
    io::{self, Write},
    time::Duration,
};
use tokio::sync::mpsc; // <-- Import the function

pub async fn run() -> Result<()> {
    // --- 1. PREAMBLE & CONFIG ---
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!(
            "Cfait v{} - A powerful, fast and elegant CalDAV task manager (TUI)",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("USAGE:");
        println!("    cfait");
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
        println!("    ^@date            Set both start and due dates (^@tomorrow, ^@2d)");
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

    // Panic Hook
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("cfait_panic.log")
        {
            let _ = writeln!(file, "PANIC: {:?}", info);
        }
        default_hook(info);
    }));

    let config_result = config::Config::load();
    let cfg = match config_result {
        Ok(c) => c,
        Err(e) => {
            // If the error is NOT a missing config file, it's a syntax/permission error.
            // Report it and exit instead of treating it as a fresh install/onboarding.
            if !config::Config::is_missing_config_error(&e) {
                eprintln!("Error loading configuration:\n{}", e);
                std::process::exit(1);
            }

            // Interactive Onboarding
            println!("Welcome to Cfait (TUI). No configuration file found.");
            println!("Let's set up your task manager.\n");

            println!("Select mode:");
            println!("  [1] Connect to CalDAV Server (Radicale, Nextcloud, etc.)");
            println!("  [2] Offline Mode (Local tasks only)");

            print!("\nChoice [1]: ");
            io::stdout().flush()?;

            let mut choice = String::new();
            io::stdin().read_line(&mut choice)?;

            let mut new_config = config::Config::default();

            if choice.trim() == "2" {
                println!("Setting up Offline Mode...");
                // Config defaults are already suitable for offline (empty url/creds)
            } else {
                // CalDAV Setup Loop
                loop {
                    println!("\n--- CalDAV Connection Setup ---");

                    print!("Server URL (e.g. https://cloud.example.com/remote.php/dav/): ");
                    io::stdout().flush()?;
                    let mut url = String::new();
                    io::stdin().read_line(&mut url)?;
                    new_config.url = url.trim().to_string();

                    print!("Username: ");
                    io::stdout().flush()?;
                    let mut user = String::new();
                    io::stdin().read_line(&mut user)?;
                    new_config.username = user.trim().to_string();

                    // --- CHANGE STARTS HERE ---
                    let pass = prompt_password("Password: ")?;
                    new_config.password = pass;
                    // --- CHANGE ENDS HERE ---

                    print!("Allow insecure SSL certificates? (y/N): ");
                    io::stdout().flush()?;
                    let mut insecure = String::new();
                    io::stdin().read_line(&mut insecure)?;
                    new_config.allow_insecure_certs = insecure.trim().eq_ignore_ascii_case("y");

                    println!("\nTesting connection...");

                    let check_result = async {
                        let client = crate::client::RustyClient::new(
                            &new_config.url,
                            &new_config.username,
                            &new_config.password,
                            new_config.allow_insecure_certs,
                        )
                        .map_err(|e| e.to_string())?;

                        match client.get_calendars().await {
                            Ok(cals) => Ok(cals.len()),
                            Err(e) => Err(e),
                        }
                    }
                    .await;

                    match check_result {
                        Ok(count) => {
                            println!("Success! Found {} calendars.", count);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Connection failed: {}", e);
                            println!("Retry configuration? [Y/n]");
                            let mut retry = String::new();
                            io::stdin().read_line(&mut retry)?;
                            if retry.trim().eq_ignore_ascii_case("n") {
                                println!(
                                    "Falling back to offline mode (saving provided details anyway)."
                                );
                                break;
                            }
                        }
                    }
                }
            }

            if let Err(e) = new_config.save() {
                eprintln!("Warning: Could not save config file: {}", e);
            } else if let Ok(path) = config::Config::get_path_string() {
                println!("Configuration saved to: {}", path);
            }

            println!("Starting TUI...");
            std::thread::sleep(Duration::from_secs(1));
            new_config
        }
    };

    let (
        url,
        user,
        pass,
        default_cal,
        hide_completed,
        hide_fully_completed_tags,
        tag_aliases,
        sort_cutoff,
        allow_insecure,
        hidden_calendars,
        disabled_calendars,
        urgent_days,
        urgent_prio,
        default_priority,
        start_grace_period_days,
        snooze_short_mins,
        snooze_long_mins,
    ) = (
        cfg.url,
        cfg.username,
        cfg.password,
        cfg.default_calendar,
        cfg.hide_completed,
        cfg.hide_fully_completed_tags,
        cfg.tag_aliases,
        cfg.sort_cutoff_months,
        cfg.allow_insecure_certs,
        cfg.hidden_calendars,
        cfg.disabled_calendars,
        cfg.urgent_days_horizon,
        cfg.urgent_priority_threshold,
        cfg.default_priority,
        cfg.start_grace_period_days,
        cfg.snooze_short_mins,
        cfg.snooze_long_mins,
    );

    // --- 2. TERMINAL SETUP ---
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- 3. STATE INIT ---
    let mut app_state = AppState::new();
    app_state.hide_completed = hide_completed;
    app_state.hide_fully_completed_tags = hide_fully_completed_tags;
    app_state.sort_cutoff_months = sort_cutoff;
    app_state.tag_aliases = tag_aliases;
    app_state.hidden_calendars = hidden_calendars.into_iter().collect();
    app_state.disabled_calendars = disabled_calendars.into_iter().collect();
    app_state.urgent_days = urgent_days;
    app_state.urgent_prio = urgent_prio;
    app_state.default_priority = default_priority;
    app_state.start_grace_period_days = start_grace_period_days;
    app_state.snooze_short_mins = snooze_short_mins;
    app_state.snooze_long_mins = snooze_long_mins;

    // --- START ALARM ACTOR ---
    let (gui_alarm_tx, mut gui_alarm_rx) = tokio::sync::mpsc::channel(10);
    // Spawn the alarm system, giving it a channel to talk back to us
    let alarm_actor_tx = crate::system::spawn_alarm_actor(Some(gui_alarm_tx));
    // Store the handle so we can send task updates to it
    app_state.alarm_actor_tx = Some(alarm_actor_tx.clone());
    // -------------------------

    let (action_tx, action_rx) = mpsc::channel(10);
    let (event_tx, mut event_rx) = mpsc::channel(10);

    // --- 4. NETWORK THREAD ---
    tokio::spawn(network::run_network_actor(
        url,
        user,
        pass,
        allow_insecure,
        default_cal.clone(), // Clone for the thread
        action_rx,
        event_tx,
    ));

    // --- 5. UI LOOP ---
    loop {
        terminal.draw(|f| draw(f, &mut app_state))?;

        // A. Network Events
        if let Ok(event) = event_rx.try_recv() {
            // Check for Sync Complete Status
            let enable_alarms = matches!(event, AppEvent::Status(ref s) if s == "Ready.");
            let is_task_update = matches!(event, AppEvent::TasksLoaded(_));

            handlers::handle_app_event(&mut app_state, event, &default_cal);

            if let Some(tx) = &app_state.alarm_actor_tx {
                if is_task_update {
                    let all_tasks: Vec<_> = app_state
                        .store
                        .calendars
                        .values()
                        .flatten()
                        .cloned()
                        .collect();
                    let tx_clone = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx_clone.send(SystemEvent::UpdateTasks(all_tasks)).await;
                    });
                }

                if enable_alarms {
                    let tx_clone = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx_clone.send(SystemEvent::EnableAlarms).await;
                    });
                }
            }
        }

        // B. Alarm Signals
        // Check if the alarm actor sent a "Fire" message
        if let Ok(AlarmMessage::Fire(t_uid, a_uid)) = gui_alarm_rx.try_recv() {
            // Find the task in the store to display details
            if let Some((task, _)) = app_state.store.get_task_mut(&t_uid) {
                app_state.active_alarm = Some((task.clone(), a_uid));
            }
        }

        // C. Input Events
        if crossterm::event::poll(Duration::from_millis(50))? {
            let event = event::read()?;
            match event {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => app_state.next(),
                    MouseEventKind::ScrollUp => app_state.previous(),
                    _ => {}
                },
                Event::Key(key) => {
                    // Filter out KeyRelease events to prevent double input on Windows
                    if key.kind == event::KeyEventKind::Release {
                        continue;
                    }

                    if let Some(action) =
                        handlers::handle_key_event(key, &mut app_state, &action_tx).await
                    {
                        if matches!(action, action::Action::Quit) {
                            break;
                        }
                        let _ = action_tx.send(action).await;
                    }
                    if matches!(app_state.mode, InputMode::Normal)
                        && key.code == crossterm::event::KeyCode::Char('q')
                    {
                        // Double check redundant safety break if handler returned None
                    }
                }
                _ => {}
            }
        }
    }

    // --- 6. CLEANUP ---
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
