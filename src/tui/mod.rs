// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/tui/mod.rs
// Entry point and main loop for the TUI application.
pub mod action;
pub mod handlers;
pub mod network;
pub mod state;
pub mod view;

use crate::config;
use crate::context::AppContext;
use crate::system::{AlarmMessage, SystemEvent};
use crate::tui::action::AppEvent;
use crate::tui::state::{AppState, InputMode};
use crate::tui::view::draw;
use chrono::NaiveDate;

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
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;

pub async fn run(ctx: Arc<dyn AppContext>) -> Result<()> {
    // --- 1. PREAMBLE & CONFIG ---
    let args: Vec<String> = env::args().collect();
    let binary_name = args.first().cloned().unwrap_or_else(|| "cfait".to_string());
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        crate::cli::print_help(&binary_name);
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

    let config_result = config::Config::load_with_credentials(ctx.as_ref());
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
            println!("{}", rust_i18n::t!("tui_welcome_no_config"));
            println!("{}\n", rust_i18n::t!("tui_setup_prompt").trim_end());

            if let Some(Some(warn)) = crate::system::KEYRING_WARNING.get() {
                println!("{}\n", warn);
            }

            println!("{}", rust_i18n::t!("tui_select_mode"));
            println!("  {}", rust_i18n::t!("tui_mode_caldav"));
            println!("  {}", rust_i18n::t!("tui_mode_offline"));

            print!("\n{}", rust_i18n::t!("tui_choice_prompt").trim_start());
            io::stdout().flush()?;

            let mut choice = String::new();
            io::stdin().read_line(&mut choice)?;

            let mut new_config = config::Config::default();

            if choice.trim() == "2" {
                println!("{}", rust_i18n::t!("tui_setting_up_offline"));
                // Config defaults are already suitable for offline (empty url/creds)
            } else {
                // CalDAV Setup Loop
                loop {
                    let title = rust_i18n::t!("tui_caldav_setup_title");
                    println!("\n--- {} ---", title.trim_matches(|c| c == '\n' || c == '-' || c == ' '));

                    print!("{}", rust_i18n::t!("tui_caldav_url_prompt"));
                    io::stdout().flush()?;
                    let mut url = String::new();
                    io::stdin().read_line(&mut url)?;
                    new_config.url = url.trim().to_string();

                    print!("{}", rust_i18n::t!("tui_username_prompt"));
                    io::stdout().flush()?;
                    let mut user = String::new();
                    io::stdin().read_line(&mut user)?;
                    new_config.username = user.trim().to_string();

                    let pass = prompt_password(rust_i18n::t!("tui_password_prompt").to_string())?;
                    new_config.password = pass;

                    print!("{}", rust_i18n::t!("tui_insecure_ssl_prompt"));
                    io::stdout().flush()?;
                    let mut insecure = String::new();
                    io::stdin().read_line(&mut insecure)?;
                    new_config.allow_insecure_certs = insecure.trim().eq_ignore_ascii_case("y");

                    println!("\n{}", rust_i18n::t!("tui_testing_connection").trim_start());

                    let ctx_clone = ctx.clone();
                    let check_result = async {
                        let client = crate::client::RustyClient::new(
                            ctx_clone,
                            &new_config.url,
                            &new_config.username,
                            &new_config.password,
                            new_config.allow_insecure_certs,
                            Some("TUI"),
                        )
                        .map_err(|e| e.to_string())?;

                        match client.get_calendars().await {
                            Ok((cals, _)) => Ok(cals.len()),
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    .await;

                    match check_result {
                        Ok(count) => {
                            if count == 1 {
                                println!("{}", rust_i18n::t!("tui_success_found_calendars.one"));
                            } else {
                                println!("{}", rust_i18n::t!("tui_success_found_calendars.other", count = count));
                            }
                            break;
                        }
                        Err(e) => {
                            eprintln!("{}", rust_i18n::t!("connection_failed", error = e.to_string()));
                            println!("{}", rust_i18n::t!("tui_retry_config_prompt"));
                            let mut retry = String::new();
                            io::stdin().read_line(&mut retry)?;
                            if retry.trim().eq_ignore_ascii_case("n") {
                                println!("{}", rust_i18n::t!("tui_fallback_offline"));
                                break;
                            }
                        }
                    }
                }
            }

            if let Err(e) = new_config.save_with_credentials(ctx.as_ref()) {
                eprintln!("{}", rust_i18n::t!("tui_warning_save_config", error = e.to_string()));
            } else if let Ok(path) = config::Config::get_path_string(ctx.as_ref()) {
                println!("{}", rust_i18n::t!("tui_config_saved", path = path));
            }

            println!("{}", rust_i18n::t!("tui_starting"));
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
        enable_local_mode,
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
        cfg.enable_local_mode,
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
    let mut app_state = AppState::new_with_ctx(ctx.clone());
    app_state.hide_completed = hide_completed;
    app_state.strikethrough_completed = cfg.strikethrough_completed;
    app_state.hide_fully_completed_tags = hide_fully_completed_tags;
    app_state.sort_cutoff_months = sort_cutoff;
    app_state.tag_aliases = tag_aliases;
    app_state.hidden_calendars = hidden_calendars.into_iter().collect();
    app_state.disabled_calendars = disabled_calendars.into_iter().collect();
    app_state.local_mode_enabled = enable_local_mode;
    app_state.urgent_days = urgent_days;
    app_state.urgent_prio = urgent_prio;
    app_state.default_priority = default_priority;
    app_state.start_grace_period_days = start_grace_period_days;
    app_state.snooze_short_mins = snooze_short_mins;
    app_state.snooze_long_mins = snooze_long_mins;

    // --- START ALARM ACTOR ---
    let (gui_alarm_tx, mut gui_alarm_rx) = tokio::sync::mpsc::channel(100);
    // Spawn the alarm system, giving it a channel to talk back to us
    let alarm_actor_tx = crate::system::spawn_alarm_actor(Some(gui_alarm_tx));
    // Store the handle so we can send task updates to it
    app_state.alarm_actor_tx = Some(alarm_actor_tx.clone());
    // -------------------------

    let (action_tx, action_rx) = mpsc::channel(100);
    let (event_tx, mut event_rx) = mpsc::channel(100);

    let mut prev_mode = app_state.mode;

    // --- 4. NETWORK THREAD ---
    let network_config = network::NetworkActorConfig {
        url,
        user,
        pass,
        allow_insecure,
        enable_local_mode,
        default_cal: default_cal.clone(),
    };
    tokio::spawn(network::run_network_actor(
        ctx.clone(),
        network_config,
        action_rx,
        event_tx,
    ));

    // --- 5. UI LOOP ---
    let mut last_refresh = std::time::Instant::now();
    let refresh_interval =
        std::time::Duration::from_secs(cfg.auto_refresh_interval_mins as u64 * 60);

    loop {
        terminal.draw(|f| draw(f, &mut app_state))?;

        // A. Network Events
        if let Ok(event) = event_rx.try_recv() {
            // Check for Sync Complete Status (use stable key emitted by network actor)
            let enable_alarms =
                matches!(event, AppEvent::Status { key: ref k, .. } if k == "ready");
            let is_task_update = matches!(event, AppEvent::TasksLoaded(_));

            handlers::handle_app_event(&mut app_state, event, &default_cal);

            if let Some(tx) = &app_state.alarm_actor_tx {
                if is_task_update {
                    let all_tasks: Vec<_> = app_state
                        .store
                        .calendars
                        .values()
                        .flat_map(|m| m.values())
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
        if let Ok(msg) = gui_alarm_rx.try_recv() {
            match msg {
                AlarmMessage::Fire(t_uid, a_uid) => {
                    // Find the task in the store to display details
                    if let Some((task, _)) = app_state.store.get_task_mut(&t_uid) {
                        app_state.active_alarm = Some((task.clone(), a_uid));
                        // Trigger a refresh so if it's done remotely, it will disappear soon.
                        let _ = action_tx.try_send(crate::tui::action::Action::Refresh);
                    }
                }
                AlarmMessage::FocusTask(_t_uid) => {
                    // Ignored in TUI, terminal interfaces typically don't process OS notification clicks efficiently
                }
            }
        }

        // Prune obsolete active alarm
        if let Some((active_task, a_uid)) = &app_state.active_alarm {
            let mut keep = false;
            if let Some(store_task) = app_state.store.get_task_ref(&active_task.uid)
                && !store_task.status.is_done()
            {
                if a_uid.starts_with("implicit_") {
                    let parts: Vec<&str> = a_uid.split('|').collect();
                    if parts.len() >= 2 {
                        let type_key_with_colon = parts[0];
                        let expected_ts = parts[1];

                        let config =
                            crate::config::Config::load(app_state.ctx.as_ref()).unwrap_or_default();
                        let default_time = chrono::NaiveTime::parse_from_str(
                            &config.default_reminder_time,
                            "%H:%M",
                        )
                        .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());

                        let mut current_ts = None;
                        if type_key_with_colon == "implicit_due:" {
                            if let Some(due) = &store_task.due {
                                let dt = match due {
                                    crate::model::DateType::Specific(t) => *t,
                                    crate::model::DateType::AllDay(d) => d
                                        .and_time(default_time)
                                        .and_local_timezone(chrono::Local)
                                        .unwrap()
                                        .with_timezone(&chrono::Utc),
                                    crate::model::DateType::Month(y, m) => {
                                        let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                        d.and_time(default_time)
                                            .and_local_timezone(chrono::Local)
                                            .unwrap()
                                            .with_timezone(&chrono::Utc)
                                    }
                                    crate::model::DateType::Year(y) => {
                                        let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                        d.and_time(default_time)
                                            .and_local_timezone(chrono::Local)
                                            .unwrap()
                                            .with_timezone(&chrono::Utc)
                                    }
                                };
                                current_ts = Some(dt.to_rfc3339());
                            }
                        } else if type_key_with_colon == "implicit_start:"
                            && let Some(start) = &store_task.dtstart
                        {
                            let dt = match start {
                                crate::model::DateType::Specific(t) => *t,
                                crate::model::DateType::AllDay(d) => d
                                    .and_time(default_time)
                                    .and_local_timezone(chrono::Local)
                                    .unwrap()
                                    .with_timezone(&chrono::Utc),
                                crate::model::DateType::Month(y, m) => {
                                    let d = NaiveDate::from_ymd_opt(*y, *m, 1).unwrap();
                                    d.and_time(default_time)
                                        .and_local_timezone(chrono::Local)
                                        .unwrap()
                                        .with_timezone(&chrono::Utc)
                                }
                                crate::model::DateType::Year(y) => {
                                    let d = NaiveDate::from_ymd_opt(*y, 1, 1).unwrap();
                                    d.and_time(default_time)
                                        .and_local_timezone(chrono::Local)
                                        .unwrap()
                                        .with_timezone(&chrono::Utc)
                                }
                            };
                            current_ts = Some(dt.to_rfc3339());
                        }
                        if current_ts.as_deref() == Some(expected_ts) {
                            keep = true;
                        }
                    }
                } else if let Some(store_alarm) = store_task.alarms.iter().find(|a| a.uid == *a_uid)
                    && store_alarm.acknowledged.is_none()
                {
                    keep = true;
                }
            }
            if !keep {
                app_state.active_alarm = None;
                if app_state.mode == crate::tui::state::InputMode::Snoozing {
                    app_state.mode = crate::tui::state::InputMode::Normal;
                    app_state.reset_input();
                }
            }
        }

        // C. Input Events
        if last_refresh.elapsed() >= refresh_interval {
            let _ = action_tx.send(crate::tui::action::Action::Refresh).await;
            last_refresh = std::time::Instant::now();
        }
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

        // Toggle mouse capture based on mode so user can natively select text
        if app_state.mode != prev_mode {
            if app_state.mode == InputMode::EditingDescription {
                let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
            } else if prev_mode == InputMode::EditingDescription {
                let _ = execute!(terminal.backend_mut(), EnableMouseCapture);
            }
            prev_mode = app_state.mode;
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
