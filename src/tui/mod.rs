// File: src/tui/mod.rs
pub mod action;
pub mod handlers;
pub mod network;
pub mod state;
pub mod view;

use crate::config;
use crate::system::{AlarmMessage, SystemEvent}; // Import AlarmMessage and SystemEvent
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
use std::{env, io, time::Duration};
use tokio::sync::mpsc;

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
        println!("    ~duration         Set duration (~30m, ~1.5h)");
        println!("    @daily            Recurrence (@daily, @weekly, @every 3 days)");
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
        println!("    \\#text            Escape special characters");
        println!();
        println!("EXAMPLES:");
        println!("    Buy cookies !1 @2025-01-16 #shopping rem:2025-01-16 8am");
        println!("    Exercise @daily ~30m #health rem:8am");
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
    ) = match config_result {
        Ok(cfg) => (
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
        ),
        Err(_) => {
            let path_str =
                config::Config::get_path_string().unwrap_or("[path unknown]".to_string());
            eprintln!("Config file not found: {}", path_str);
            return Ok(());
        }
    };

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
    app_state.tag_aliases = tag_aliases;
    app_state.sort_cutoff_months = sort_cutoff;
    app_state.hidden_calendars = hidden_calendars.into_iter().collect();
    app_state.disabled_calendars = disabled_calendars.into_iter().collect();
    app_state.urgent_days = urgent_days;
    app_state.urgent_prio = urgent_prio;

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
