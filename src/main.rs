#![allow(dead_code)]

mod actions;
mod app;
mod config;
mod errors;
mod input;
mod midi;
mod platform;
mod ui;

use std::io;
use std::panic;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_channel::{bounded, select, tick};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tracing::info;

use crate::actions::executor::{spawn_executor, ActionCommand};
use crate::actions::keyboard::EnigoKeyboardSink;
use crate::app::events::AppEvent;
use crate::app::reducer::{SideEffect, handle_key_event, handle_midi_event};
use crate::app::state::AppState;
use crate::config::load::{default_config_path, load_config};
use crate::config::model::AppConfig;
use crate::config::save::{dump_default_config, save_config};
use crate::input::terminal::poll_terminal_event;
use crate::midi::manager::MidiManager;
use crate::platform::capabilities::check_platform_warnings;
use crate::ui::draw::draw;

#[derive(Parser, Debug)]
#[command(name = "keebmidi", version, about = "Map MIDI inputs to keyboard keys and macros")]
struct Cli {
    /// Path to config file
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Auto-select MIDI device by name
    #[arg(long, value_name = "NAME")]
    device: Option<String>,

    /// Start directly in run mode
    #[arg(long)]
    run: bool,

    /// Do not enter run mode automatically
    #[arg(long)]
    no_run: bool,

    /// Enable verbose debug logging
    #[arg(long)]
    verbose: bool,

    /// Print default config and exit
    #[arg(long)]
    dump_default_config: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.dump_default_config {
        println!("{}", dump_default_config());
        return Ok(());
    }

    // Set up tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(io::stderr)
        .init();

    // Load config
    let config_path = cli.config.unwrap_or_else(default_config_path);
    let app_config = load_config(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {e}, using defaults");
        AppConfig::default()
    });

    // Initialize app state from config
    let mut state = AppState::default();
    state.mappings = app_config.mappings;
    if !state.mappings.is_empty() {
        state.selected_mapping = Some(0);
    }

    // Check platform capabilities
    let warnings = check_platform_warnings();
    for w in &warnings {
        state.add_log(format!("⚠ {w}"));
    }

    // Set up channels
    let (event_tx, event_rx) = bounded::<AppEvent>(256);
    let (action_tx, action_rx) = bounded::<ActionCommand>(64);

    // Spawn action executor
    let keyboard_sink = Box::new(EnigoKeyboardSink::new().context("Failed to init keyboard sink")?);
    let _executor_handle = spawn_executor(action_rx, event_tx.clone(), keyboard_sink);

    // Set up MIDI manager
    let mut midi_manager = MidiManager::new(event_tx.clone());

    // Auto-select device if specified
    if let Some(ref device_name) = cli.device.or(app_config.selected_device.clone()) {
        let devices = MidiManager::enumerate_devices();
        state.midi_devices = devices.clone();
        if let Some(dev) = devices.iter().find(|d| d.name.contains(device_name.as_str())) {
            match midi_manager.connect(dev.id) {
                Ok(name) => {
                    state.selected_device = Some(dev.id);
                    state.selected_device_name = Some(name);
                    state.midi_connected = true;
                    state.add_log(format!("Connected to: {}", dev.name));
                }
                Err(e) => {
                    state.add_log(format!("Failed to connect to '{}': {e}", dev.name));
                }
            }
        } else {
            state.add_log(format!("Device '{device_name}' not found"));
        }
    }

    // Auto-enter run mode if requested
    if cli.run && !cli.no_run && state.midi_connected {
        state.mode = crate::app::state::AppMode::Run;
        state.running = true;
        state.add_log("▶ Started in run mode (--run)");
    }

    state.add_log("keebmidi started. Press 'D' to select MIDI device, 'a' to add mapping.");

    // Set up terminal with panic-safe restore
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Main event loop
    let tick_rate = tick(Duration::from_millis(100));

    loop {
        // Draw UI
        terminal.draw(|f| draw(f, &state))?;

        // Poll terminal events (non-blocking, short timeout)
        let _ = poll_terminal_event(&event_tx, Duration::from_millis(10));

        // Process all pending events
        select! {
            recv(event_rx) -> msg => {
                if let Ok(event) = msg {
                    let effects = process_event(&mut state, event);
                    for effect in effects {
                        handle_side_effect(
                            effect,
                            &mut state,
                            &mut midi_manager,
                            &action_tx,
                            &config_path,
                        );
                    }
                }
            }
            recv(tick_rate) -> _ => {
                // Just a tick for redraw
            }
        }

        if state.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    // Disconnect MIDI
    midi_manager.disconnect();

    info!("keebmidi exited cleanly");
    Ok(())
}

fn process_event(state: &mut AppState, event: AppEvent) -> Vec<SideEffect> {
    match event {
        AppEvent::Tick => vec![SideEffect::None],
        AppEvent::TerminalKey(key) => handle_key_event(state, key),
        AppEvent::TerminalResize(_, _) => vec![SideEffect::None],
        AppEvent::MidiReceived(midi_event) => handle_midi_event(state, midi_event),
        AppEvent::ActionCompleted(id) => {
            state.add_log(format!("✓ Action completed: {id}"));
            vec![SideEffect::None]
        }
        AppEvent::ActionFailed(id, err) => {
            state.add_log(format!("✗ Action failed [{id}]: {err}"));
            vec![SideEffect::None]
        }
        AppEvent::DeviceChanged => {
            state.midi_devices = MidiManager::enumerate_devices();
            state.add_log("MIDI device list updated");
            vec![SideEffect::None]
        }
    }
}

fn handle_side_effect(
    effect: SideEffect,
    state: &mut AppState,
    midi_manager: &mut MidiManager,
    action_tx: &crossbeam_channel::Sender<ActionCommand>,
    config_path: &PathBuf,
) {
    match effect {
        SideEffect::Quit => {
            state.should_quit = true;
        }
        SideEffect::SaveConfig => {
            let config = AppConfig {
                version: 1,
                selected_device: state.selected_device_name.clone(),
                mappings: state.mappings.clone(),
            };
            match save_config(&config, config_path) {
                Ok(()) => {
                    state.dirty = false;
                    state.add_log(format!("Config saved to {}", config_path.display()));
                }
                Err(e) => {
                    state.add_log(format!("Failed to save config: {e}"));
                    state.mode =
                        crate::app::state::AppMode::ErrorDialog(format!("Save failed: {e}"));
                }
            }
        }
        SideEffect::ConnectDevice(idx) => {
            match midi_manager.connect(idx) {
                Ok(name) => {
                    state.selected_device = Some(idx);
                    state.selected_device_name = Some(name.clone());
                    state.midi_connected = true;
                    state.add_log(format!("Connected to: {name}"));
                }
                Err(e) => {
                    state.midi_connected = false;
                    state.add_log(format!("Connection failed: {e}"));
                    state.mode = crate::app::state::AppMode::ErrorDialog(format!(
                        "MIDI connection failed: {e}"
                    ));
                }
            }
        }
        SideEffect::RefreshDevices => {
            state.midi_devices = MidiManager::enumerate_devices();
            state.add_log(format!("Found {} MIDI devices", state.midi_devices.len()));
        }
        SideEffect::ExecuteAction(cmd) => {
            if let Err(e) = action_tx.send(cmd) {
                state.add_log(format!("Failed to queue action: {e}"));
            }
        }
        SideEffect::None => {}
    }
}
