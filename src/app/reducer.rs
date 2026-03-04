use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::actions::executor::ActionCommand;
use crate::app::state::*;
use crate::config::model::*;
use crate::midi::decode::{detect_direction_relative, event_matches_trigger, MidiEvent};
use crate::midi::trigger::{KnobRotationDirection, MidiTrigger};

/// Side effects that the reducer requests the app loop to perform.
pub enum SideEffect {
    Quit,
    SaveConfig,
    ConnectDevice(usize),
    RefreshDevices,
    ExecuteAction(ActionCommand),
    None,
}

/// Process a terminal key event and return any side effects needed.
pub fn handle_key_event(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    // F12 panic stop always works
    if key.code == KeyCode::F(12) {
        state.panic_stop = !state.panic_stop;
        state.add_log(if state.panic_stop {
            "⚠ PANIC STOP: All mapping execution suspended"
        } else {
            "✓ Panic stop released"
        });
        return vec![SideEffect::None];
    }

    match &state.mode {
        AppMode::Setup => handle_setup_key(state, key),
        AppMode::Run => handle_run_key(state, key),
        AppMode::LearnMidi => handle_learn_midi_key(state, key),
        AppMode::LearnAction => handle_learn_action_key(state, key),
        AppMode::RecordMacro => handle_record_macro_key(state, key),
        AppMode::TextInput => handle_text_input_key(state, key),
        AppMode::SelectDevice => handle_select_device_key(state, key),
        AppMode::EditMenu => handle_edit_menu_key(state, key),
        AppMode::ConfirmDialog(_) => handle_confirm_dialog_key(state, key),
        AppMode::ErrorDialog(_) => handle_error_dialog_key(state, key),
    }
}

fn handle_setup_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Char('q') => {
            if state.dirty {
                state.mode =
                    AppMode::ConfirmDialog("Unsaved changes. Quit anyway? (y/n)".to_string());
                vec![SideEffect::None]
            } else {
                state.should_quit = true;
                vec![SideEffect::Quit]
            }
        }
        KeyCode::Char('s') => {
            vec![SideEffect::SaveConfig]
        }
        KeyCode::Char('a') => {
            // Add mapping - start learn MIDI
            state.previous_mode = Some(AppMode::Setup);
            state.mode = AppMode::LearnMidi;
            state.learn_state = Some(LearnState::WaitingForMidi);
            state.learned_trigger = None;
            state.add_log("Learn MIDI: press a pad/key/knob on your MIDI device...");
            vec![SideEffect::None]
        }
        KeyCode::Char('e') => {
            if state.selected_mapping.is_some() {
                state.mode = AppMode::EditMenu;
                state.edit_menu_index = 0;
                state.add_log("Edit mapping: choose what to edit");
            }
            vec![SideEffect::None]
        }
        KeyCode::Char('d') => {
            if let Some(idx) = state.selected_mapping {
                if idx < state.mappings.len() {
                    let name = state.mappings[idx].name.clone();
                    state.mode = AppMode::ConfirmDialog(format!("Delete mapping '{name}'? (y/n)"));
                }
            }
            vec![SideEffect::None]
        }
        KeyCode::Char('l') => {
            // Learn MIDI for selected mapping
            if state.selected_mapping.is_some() {
                state.mode = AppMode::LearnMidi;
                state.learn_state = Some(LearnState::WaitingForMidi);
                state.add_log("Learn MIDI trigger for selected mapping...");
            }
            vec![SideEffect::None]
        }
        KeyCode::Char('r') => {
            // Toggle run mode
            if state.midi_connected {
                state.mode = AppMode::Run;
                state.running = true;
                state.add_log("▶ Run mode active");
            } else {
                state.add_log("Cannot enter run mode: no MIDI device connected");
            }
            vec![SideEffect::None]
        }
        KeyCode::Char(' ') => {
            // Toggle enable/disable on selected mapping
            if let Some(idx) = state.selected_mapping {
                if let Some(m) = state.mappings.get_mut(idx) {
                    m.enabled = !m.enabled;
                    state.dirty = true;
                }
                if let Some(m) = state.mappings.get(idx) {
                    let status = if m.enabled { "enabled" } else { "disabled" };
                    state.add_log(format!("Mapping '{}' {status}", m.name));
                }
            }
            vec![SideEffect::None]
        }
        KeyCode::Char('D') => {
            // Select device
            state.mode = AppMode::SelectDevice;
            state.device_list_index = 0;
            vec![SideEffect::RefreshDevices]
        }
        KeyCode::Tab => {
            state.active_pane = match state.active_pane {
                ActivePane::MappingList => ActivePane::Details,
                ActivePane::Details => ActivePane::Log,
                ActivePane::Log => ActivePane::MappingList,
            };
            vec![SideEffect::None]
        }
        KeyCode::Up => {
            if state.active_pane == ActivePane::MappingList {
                if let Some(idx) = state.selected_mapping {
                    if idx > 0 {
                        state.selected_mapping = Some(idx - 1);
                    }
                }
            }
            vec![SideEffect::None]
        }
        KeyCode::Down => {
            if state.active_pane == ActivePane::MappingList {
                let max = state.mappings.len().saturating_sub(1);
                if let Some(idx) = state.selected_mapping {
                    if idx < max {
                        state.selected_mapping = Some(idx + 1);
                    }
                } else if !state.mappings.is_empty() {
                    state.selected_mapping = Some(0);
                }
            }
            vec![SideEffect::None]
        }
        _ => vec![SideEffect::None],
    }
}

fn handle_run_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Char('r') | KeyCode::Esc => {
            state.mode = AppMode::Setup;
            state.running = false;
            state.add_log("■ Run mode stopped");
            vec![SideEffect::None]
        }
        KeyCode::Char('q') => {
            state.running = false;
            state.should_quit = true;
            vec![SideEffect::Quit]
        }
        _ => vec![SideEffect::None],
    }
}

fn handle_learn_midi_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Esc => {
            state.mode = state.previous_mode.take().unwrap_or(AppMode::Setup);
            state.learn_state = None;
            state.learned_trigger = None;
            state.add_log("Learn MIDI cancelled");
        }
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(LearnState::KnobDetected {
                channel,
                controller,
                values,
            }) = &state.learn_state
            {
                let mode = detect_knob_mode(values);
                let ch = *channel;
                let ctrl = *controller;
                state.add_log(format!("Detected mode: {mode:?}. Turn the knob CLOCKWISE and press Enter…"));
                state.learn_state = Some(LearnState::KnobLearnCW {
                    channel: ch,
                    controller: ctrl,
                    mode,
                });
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if let Some(LearnState::KnobDetected {
                channel,
                controller,
                ..
            }) = &state.learn_state
            {
                let trigger = MidiTrigger::ControlChange {
                    channel: *channel,
                    controller: *controller,
                    min_value: None,
                    max_value: None,
                };
                state.learned_trigger = Some(trigger.clone());
                state.add_log(format!("Learned trigger: {trigger}"));
                state.mode = AppMode::LearnAction;
                state.action_menu_open = true;
                state.action_menu_index = 0;
                state.learn_state = None;
                state.add_log("Choose action type for this mapping");
            }
        }
        KeyCode::Enter => {
            match &state.learn_state {
                Some(LearnState::KnobLearnCW {
                    channel,
                    controller,
                    mode,
                }) => {
                    let ch = *channel;
                    let ctrl = *controller;
                    let m = mode.clone();
                    let cw_trigger = MidiTrigger::KnobRotation {
                        channel: ch,
                        controller: ctrl,
                        direction: KnobRotationDirection::Clockwise,
                        mode: m.clone(),
                    };
                    state.learned_trigger = Some(cw_trigger);
                    state.mode = AppMode::LearnAction;
                    state.action_menu_open = true;
                    state.action_menu_index = 0;
                    state.learn_state = None;
                    state.add_log("Choose action for CW rotation");
                }
                Some(LearnState::KnobLearnCCW {
                    channel,
                    controller,
                    mode,
                }) => {
                    let ch = *channel;
                    let ctrl = *controller;
                    let m = mode.clone();
                    let ccw_trigger = MidiTrigger::KnobRotation {
                        channel: ch,
                        controller: ctrl,
                        direction: KnobRotationDirection::CounterClockwise,
                        mode: m,
                    };
                    state.learned_trigger = Some(ccw_trigger);
                    state.mode = AppMode::LearnAction;
                    state.action_menu_open = true;
                    state.action_menu_index = 0;
                    state.learn_state = None;
                    state.add_log("Choose action for CCW rotation");
                }
                _ => {}
            }
        }
        _ => {}
    }
    vec![SideEffect::None]
}

/// Heuristic to detect knob encoding mode from observed values.
fn detect_knob_mode(values: &[u8]) -> KnobMode {
    if values.is_empty() {
        return KnobMode::Absolute;
    }
    let has_low = values.iter().any(|&v| v >= 1 && v <= 63);
    let has_high = values.iter().any(|&v| v >= 65 && v <= 127);
    let has_smooth_progression = values.windows(2).all(|w| {
        let diff = (w[1] as i16 - w[0] as i16).unsigned_abs();
        diff <= 5
    });

    if has_smooth_progression && values.len() >= 2 {
        KnobMode::Absolute
    } else if has_low && has_high {
        KnobMode::Relative1
    } else {
        KnobMode::Absolute
    }
}

fn handle_learn_action_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    if state.action_menu_open {
        return handle_action_menu_key(state, key);
    }

    match &state.learn_state {
        Some(LearnState::WaitingForSingleKey) => {
            if key.code == KeyCode::Esc {
                state.mode = AppMode::Setup;
                state.learn_state = None;
                state.add_log("Learn key cancelled");
                return vec![SideEffect::None];
            }
            if let Some(keyspec) = crossterm_key_to_keyspec(&key) {
                if keyspec.is_modifier() {
                    // Start chord capture
                    state.learn_state = Some(LearnState::WaitingForChord {
                        pressed: vec![keyspec],
                    });
                    return vec![SideEffect::None];
                }
                // Check for modifiers in the event
                let mut keys = Vec::new();
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    keys.push(KeySpec::Ctrl);
                }
                if key.modifiers.contains(KeyModifiers::ALT) {
                    keys.push(KeySpec::Alt);
                }
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    keys.push(KeySpec::Shift);
                }
                if key.modifiers.contains(KeyModifiers::SUPER) {
                    keys.push(KeySpec::Meta);
                }

                if keys.is_empty() {
                    // Single key tap
                    finalize_action(state, OutputAction::KeyTap { key: keyspec });
                } else {
                    // Chord
                    keys.push(keyspec);
                    finalize_action(state, OutputAction::KeyChord { keys });
                }
            }
            vec![SideEffect::None]
        }
        Some(LearnState::WaitingForChord { pressed }) => {
            if key.code == KeyCode::Esc {
                state.mode = AppMode::Setup;
                state.learn_state = None;
                state.add_log("Learn chord cancelled");
                return vec![SideEffect::None];
            }
            if let Some(keyspec) = crossterm_key_to_keyspec(&key) {
                let mut pressed = pressed.clone();
                if !keyspec.is_modifier() {
                    pressed.push(keyspec);
                    finalize_action(state, OutputAction::KeyChord { keys: pressed });
                } else if !pressed.contains(&keyspec) {
                    pressed.push(keyspec);
                    state.learn_state = Some(LearnState::WaitingForChord { pressed });
                }
            }
            vec![SideEffect::None]
        }
        _ => {
            if key.code == KeyCode::Esc {
                state.mode = AppMode::Setup;
                state.learn_state = None;
            }
            vec![SideEffect::None]
        }
    }
}

fn handle_action_menu_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    // Menu: 0=Key/Chord, 1=Text, 2=Record Macro
    match key.code {
        KeyCode::Esc => {
            state.action_menu_open = false;
            state.mode = AppMode::Setup;
            state.add_log("Action selection cancelled");
        }
        KeyCode::Up => {
            if state.action_menu_index > 0 {
                state.action_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if state.action_menu_index < 2 {
                state.action_menu_index += 1;
            }
        }
        KeyCode::Enter => {
            state.action_menu_open = false;
            match state.action_menu_index {
                0 => {
                    state.learn_state = Some(LearnState::WaitingForSingleKey);
                    state.add_log("Press a key or chord...");
                }
                1 => {
                    state.mode = AppMode::TextInput;
                    state.text_input_buffer.clear();
                    state.text_input_purpose = TextInputPurpose::TextAction;
                    state.add_log("Type text, press Enter to confirm...");
                }
                2 => {
                    state.mode = AppMode::RecordMacro;
                    state.learn_state = Some(LearnState::RecordingMacro {
                        started_at: Instant::now(),
                        steps: Vec::new(),
                    });
                    state.add_log("Recording macro... Press Enter to stop, Esc to cancel");
                }
                _ => {}
            }
        }
        _ => {}
    }
    vec![SideEffect::None]
}

fn handle_record_macro_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    if key.code == KeyCode::Esc {
        state.mode = AppMode::Setup;
        state.learn_state = None;
        state.add_log("Macro recording cancelled");
        return vec![SideEffect::None];
    }

    if key.code == KeyCode::Enter {
        // Finalize macro
        if let Some(LearnState::RecordingMacro { steps, .. }) = state.learn_state.take() {
            let normalized = normalize_macro(&steps);
            if normalized.is_empty() {
                state.add_log("Macro recording empty, discarded");
            } else {
                let spec = MacroSpec {
                    steps: normalized,
                    playback_mode: PlaybackMode::FireAndForget,
                };
                finalize_action(state, OutputAction::Macro { spec });
                return vec![SideEffect::None];
            }
        }
        state.mode = AppMode::Setup;
        return vec![SideEffect::None];
    }

    // Record the key event
    if let Some(LearnState::RecordingMacro {
        started_at,
        ref mut steps,
    }) = state.learn_state
    {
        let elapsed = started_at.elapsed().as_millis() as u64;
        if let Some(last_time) = steps.iter().rev().find_map(|s| match s {
            MacroStep::DelayMs(_) => None,
            _ => Some(elapsed),
        }) {
            // Insert delay since last non-delay step
            let delay = elapsed.saturating_sub(last_time);
            if delay > 0 {
                steps.push(MacroStep::DelayMs(delay));
            }
        }

        if let Some(keyspec) = crossterm_key_to_keyspec(&key) {
            steps.push(MacroStep::KeyTap(keyspec));
        }
    }
    vec![SideEffect::None]
}

fn handle_edit_menu_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    // Menu items:
    // 0 = Rename
    // 1 = Change Action
    // 2 = Debounce (ms)
    // 3 = Suppress Retrigger While Held
    // 4 = Trigger on Value Change Only
    // 5 = Allow Overlap
    const EDIT_MENU_ITEMS: usize = 6;

    match key.code {
        KeyCode::Esc => {
            state.mode = AppMode::Setup;
            state.add_log("Edit menu closed");
        }
        KeyCode::Up => {
            if state.edit_menu_index > 0 {
                state.edit_menu_index -= 1;
            }
        }
        KeyCode::Down => {
            if state.edit_menu_index + 1 < EDIT_MENU_ITEMS {
                state.edit_menu_index += 1;
            }
        }
        KeyCode::Enter => match state.edit_menu_index {
            0 => {
                // Rename
                state.mode = AppMode::TextInput;
                state.text_input_buffer = state
                    .selected_mapping_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                state.text_input_purpose = TextInputPurpose::MappingName;
                state.add_log("Enter new mapping name...");
            }
            1 => {
                // Change Action
                state.mode = AppMode::LearnAction;
                state.action_menu_open = true;
                state.action_menu_index = 0;
                state.add_log("Edit mapping action: choose action type");
            }
            2 => {
                // Debounce
                state.mode = AppMode::TextInput;
                state.text_input_buffer = state
                    .selected_mapping_ref()
                    .map(|m| m.options.debounce_ms.to_string())
                    .unwrap_or_else(|| "50".to_string());
                state.text_input_purpose = TextInputPurpose::DebounceMs;
                state.add_log("Enter debounce value in ms...");
            }
            3 => {
                // Toggle suppress retrigger
                if let Some(idx) = state.selected_mapping {
                    if let Some(m) = state.mappings.get_mut(idx) {
                        m.options.suppress_retrigger_while_held =
                            !m.options.suppress_retrigger_while_held;
                        let val = m.options.suppress_retrigger_while_held;
                        state.dirty = true;
                        state.add_log(format!("Suppress retrigger: {val}"));
                    }
                }
            }
            4 => {
                // Toggle trigger on value change only
                if let Some(idx) = state.selected_mapping {
                    if let Some(m) = state.mappings.get_mut(idx) {
                        m.options.trigger_on_value_change_only =
                            !m.options.trigger_on_value_change_only;
                        let val = m.options.trigger_on_value_change_only;
                        state.dirty = true;
                        state.add_log(format!("Trigger on value change only: {val}"));
                    }
                }
            }
            5 => {
                // Toggle allow overlap
                if let Some(idx) = state.selected_mapping {
                    if let Some(m) = state.mappings.get_mut(idx) {
                        m.options.allow_overlap = !m.options.allow_overlap;
                        let val = m.options.allow_overlap;
                        state.dirty = true;
                        state.add_log(format!("Allow overlap: {val}"));
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    vec![SideEffect::None]
}

fn handle_text_input_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Esc => {
            state.mode = AppMode::Setup;
            state.text_input_buffer.clear();
            state.add_log("Text input cancelled");
        }
        KeyCode::Enter => {
            let text = state.text_input_buffer.clone();
            state.text_input_buffer.clear();
            match state.text_input_purpose {
                TextInputPurpose::TextAction => {
                    if text.is_empty() {
                        state.add_log("Empty text, discarded");
                        state.mode = AppMode::Setup;
                    } else {
                        finalize_action(state, OutputAction::Text { text });
                    }
                }
                TextInputPurpose::MappingName => {
                    if !text.is_empty() {
                        if let Some(idx) = state.selected_mapping {
                            let old_name = state.mappings.get(idx)
                                .map(|m| m.name.clone())
                                .unwrap_or_default();
                            if let Some(m) = state.mappings.get_mut(idx) {
                                m.name = text.clone();
                                state.dirty = true;
                            }
                            state.add_log(format!("Renamed '{old_name}' -> '{text}'"));
                        }
                    } else {
                        state.add_log("Empty name, keeping previous".to_string());
                    }
                    state.mode = AppMode::Setup;
                }
                TextInputPurpose::DebounceMs => {
                    match text.parse::<u64>() {
                        Ok(ms) => {
                            if let Some(idx) = state.selected_mapping {
                                if let Some(m) = state.mappings.get_mut(idx) {
                                    m.options.debounce_ms = ms;
                                    state.dirty = true;
                                    state.add_log(format!("Debounce set to {ms}ms"));
                                }
                            }
                        }
                        Err(_) => {
                            state.add_log("Invalid number, keeping previous debounce".to_string());
                        }
                    }
                    state.mode = AppMode::Setup;
                }
                TextInputPurpose::None => {
                    state.mode = AppMode::Setup;
                }
            }
        }
        KeyCode::Backspace => {
            state.text_input_buffer.pop();
        }
        KeyCode::Char(c) => {
            state.text_input_buffer.push(c);
        }
        _ => {}
    }
    vec![SideEffect::None]
}

fn handle_select_device_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Esc => {
            state.mode = AppMode::Setup;
        }
        KeyCode::Up => {
            if state.device_list_index > 0 {
                state.device_list_index -= 1;
            }
        }
        KeyCode::Down => {
            if state.device_list_index + 1 < state.midi_devices.len() {
                state.device_list_index += 1;
            }
        }
        KeyCode::Enter => {
            if !state.midi_devices.is_empty() {
                let idx = state.device_list_index;
                state.mode = AppMode::Setup;
                return vec![SideEffect::ConnectDevice(idx)];
            }
        }
        KeyCode::Char('r') => {
            return vec![SideEffect::RefreshDevices];
        }
        _ => {}
    }
    vec![SideEffect::None]
}

fn handle_confirm_dialog_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            let msg = if let AppMode::ConfirmDialog(ref m) = state.mode {
                m.clone()
            } else {
                String::new()
            };

            if msg.contains("Quit") {
                state.should_quit = true;
                return vec![SideEffect::Quit];
            }
            if msg.contains("Delete") {
                if let Some(idx) = state.selected_mapping {
                    if idx < state.mappings.len() {
                        let removed = state.mappings.remove(idx);
                        state.add_log(format!("Deleted mapping '{}'", removed.name));
                        state.dirty = true;
                        if state.mappings.is_empty() {
                            state.selected_mapping = None;
                        } else {
                            state.selected_mapping =
                                Some(idx.min(state.mappings.len() - 1));
                        }
                    }
                }
            }
            state.mode = AppMode::Setup;
            vec![SideEffect::None]
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            state.mode = AppMode::Setup;
            vec![SideEffect::None]
        }
        _ => vec![SideEffect::None],
    }
}

fn handle_error_dialog_key(state: &mut AppState, key: KeyEvent) -> Vec<SideEffect> {
    match key.code {
        KeyCode::Enter | KeyCode::Esc => {
            state.mode = AppMode::Setup;
        }
        _ => {}
    }
    vec![SideEffect::None]
}

/// Process an incoming MIDI event.
pub fn handle_midi_event(state: &mut AppState, event: MidiEvent) -> Vec<SideEffect> {
    let summary = format!("{}", event.trigger);
    state.last_midi_event = Some(summary.clone());
    state.add_log(format!("MIDI: {summary}"));

    // In learn mode, capture the trigger
    if state.mode == AppMode::LearnMidi {
        match &state.learn_state {
            Some(LearnState::WaitingForMidi) => {
                // Check if this is a CC event that could be a knob
                if let MidiTrigger::ControlChange {
                    channel, controller, ..
                } = &event.trigger
                {
                    if let Some(val) = event.raw_value {
                        // Start knob detection: collect values
                        state.learn_state = Some(LearnState::KnobDetected {
                            channel: *channel,
                            controller: *controller,
                            values: vec![val],
                        });
                        state.add_log(format!(
                            "CC detected on ch={channel} ctrl={controller}. \
                             Knob detected. Configure as rotary knob? [Y/N]"
                        ));
                        return vec![SideEffect::None];
                    }
                }

                // Non-CC trigger: proceed as before
                state.learned_trigger = Some(event.trigger.clone());
                state.add_log(format!("Learned trigger: {summary}"));
                state.mode = AppMode::LearnAction;
                state.action_menu_open = true;
                state.action_menu_index = 0;
                state.learn_state = None;
                state.add_log("Choose action type for this mapping");
                return vec![SideEffect::None];
            }
            Some(LearnState::KnobDetected {
                channel,
                controller,
                values,
            }) => {
                // Accumulate more CC values for mode detection
                if let MidiTrigger::ControlChange {
                    channel: ec,
                    controller: ecc,
                    ..
                } = &event.trigger
                {
                    if ec == channel && ecc == controller {
                        if let Some(val) = event.raw_value {
                            let mut new_values = values.clone();
                            new_values.push(val);
                            state.learn_state = Some(LearnState::KnobDetected {
                                channel: *channel,
                                controller: *controller,
                                values: new_values,
                            });
                        }
                    }
                }
                return vec![SideEffect::None];
            }
            Some(LearnState::KnobLearnCW { channel, controller, .. }) => {
                // Ignore CC events during CW/CCW learn (waiting for key confirmation)
                if let MidiTrigger::ControlChange {
                    channel: ec,
                    controller: ecc,
                    ..
                } = &event.trigger
                {
                    if ec == channel && ecc == controller {
                        return vec![SideEffect::None];
                    }
                }
                return vec![SideEffect::None];
            }
            Some(LearnState::KnobLearnCCW { channel, controller, .. }) => {
                if let MidiTrigger::ControlChange {
                    channel: ec,
                    controller: ecc,
                    ..
                } = &event.trigger
                {
                    if ec == channel && ecc == controller {
                        return vec![SideEffect::None];
                    }
                }
                return vec![SideEffect::None];
            }
            _ => {}
        }
    }

    // In run mode, match and execute
    if state.mode == AppMode::Run && !state.panic_stop {
        // Compute knob direction for CC events before matching
        let mut event = event;
        if let MidiTrigger::ControlChange {
            channel, controller, ..
        } = &event.trigger
        {
            if let Some(raw_val) = event.raw_value {
                let ch = *channel;
                let ctrl = *controller;
                // Check if any knob rotation mapping exists for this (channel, controller)
                let knob_mode = state.mappings.iter().find_map(|m| {
                    if let MidiTrigger::KnobRotation {
                        channel: mc,
                        controller: mctrl,
                        mode,
                        ..
                    } = &m.trigger
                    {
                        if *mc == ch && *mctrl == ctrl {
                            Some(mode.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                if let Some(mode) = knob_mode {
                    let direction = match mode {
                        KnobMode::Absolute => {
                            state.knob_state.detect_direction_absolute(ch, ctrl, raw_val)
                        }
                        _ => detect_direction_relative(raw_val, &mode),
                    };
                    event.knob_direction = direction;
                }
            }
        }
        return match_and_execute(state, &event);
    }

    vec![SideEffect::None]
}

fn match_and_execute(state: &mut AppState, event: &MidiEvent) -> Vec<SideEffect> {
    let now = Instant::now();
    let mut effects = Vec::new();
    let mut log_messages = Vec::new();
    let mut cc_updates = Vec::new();
    let mut debounce_updates = Vec::new();
    let mut last_action_update = None;
    let mut should_break = false;

    for mapping in &state.mappings {
        if should_break {
            break;
        }
        if !mapping.enabled {
            continue;
        }

        if !event_matches_trigger(event, &mapping.trigger) {
            continue;
        }

        // Debounce check
        if mapping.options.debounce_ms > 0 {
            if let Some(last) = state.debounce_timers.get(&mapping.id) {
                if now.duration_since(*last).as_millis() < mapping.options.debounce_ms as u128 {
                    continue;
                }
            }
        }

        // Value change only check for CC and KnobRotation
        if mapping.options.trigger_on_value_change_only {
            let cc_key = match &mapping.trigger {
                MidiTrigger::ControlChange {
                    channel, controller, ..
                } => Some((*channel, *controller)),
                MidiTrigger::KnobRotation {
                    channel, controller, ..
                } => Some((*channel, *controller)),
                _ => None,
            };
            if let Some(key) = cc_key {
                if let Some(raw) = event.raw_value {
                    if let Some(last_val) = state.last_cc_values.get(&key) {
                        if *last_val == raw {
                            continue;
                        }
                    }
                    cc_updates.push((key, raw));
                }
            }
        }

        debounce_updates.push((mapping.id.clone(), now));
        last_action_update = Some(format!("{}: {}", mapping.name, mapping.action));
        log_messages.push(format!("▶ {}: {}", mapping.name, mapping.action));

        effects.push(SideEffect::ExecuteAction(ActionCommand {
            mapping_id: mapping.id.clone(),
            action: mapping.action.clone(),
        }));

        if !mapping.options.allow_overlap {
            should_break = true;
        }
    }

    // Apply deferred mutations
    for (key, val) in cc_updates {
        state.last_cc_values.insert(key, val);
    }
    for (id, time) in debounce_updates {
        state.debounce_timers.insert(id, time);
    }
    if let Some(action) = last_action_update {
        state.last_action = Some(action);
    }
    for msg in log_messages {
        state.add_log(msg);
    }

    effects
}

fn finalize_action(state: &mut AppState, action: OutputAction) {
    let action_desc = format!("{action}");

    if let Some(trigger) = state.learned_trigger.take() {
        // Create new mapping
        let id = uuid::Uuid::new_v4().to_string();
        let name = format!("Mapping {}", state.mappings.len() + 1);
        let mapping = Mapping {
            id,
            name,
            enabled: true,
            trigger,
            action,
            options: MappingOptions::default(),
        };
        state.add_log(format!("Created mapping: {} -> {action_desc}", mapping.name));
        state.mappings.push(mapping);
        state.selected_mapping = Some(state.mappings.len() - 1);
        state.dirty = true;
    } else if let Some(idx) = state.selected_mapping {
        // Update existing mapping action
        let mut name = String::new();
        if let Some(m) = state.mappings.get_mut(idx) {
            m.action = action;
            name = m.name.clone();
            state.dirty = true;
        }
        if !name.is_empty() {
            state.add_log(format!("Updated '{name}' action: {action_desc}"));
        }
    }

    state.mode = AppMode::Setup;
    state.learn_state = None;
}

/// Convert crossterm KeyEvent to our KeySpec.
pub fn crossterm_key_to_keyspec(key: &KeyEvent) -> Option<KeySpec> {
    match key.code {
        KeyCode::Char(c) => Some(KeySpec::Char(c)),
        KeyCode::Enter => Some(KeySpec::Enter),
        KeyCode::Tab => Some(KeySpec::Tab),
        KeyCode::Esc => Some(KeySpec::Esc),
        KeyCode::Backspace => Some(KeySpec::Backspace),
        KeyCode::Up => Some(KeySpec::Up),
        KeyCode::Down => Some(KeySpec::Down),
        KeyCode::Left => Some(KeySpec::Left),
        KeyCode::Right => Some(KeySpec::Right),
        KeyCode::F(n) => Some(KeySpec::F(n)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_quit_clean() {
        let mut state = AppState::default();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let effects = handle_key_event(&mut state, key);
        assert!(state.should_quit);
    }

    #[test]
    fn test_handle_quit_dirty_prompts() {
        let mut state = AppState::default();
        state.dirty = true;
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        handle_key_event(&mut state, key);
        assert!(!state.should_quit);
        assert!(matches!(state.mode, AppMode::ConfirmDialog(_)));
    }

    #[test]
    fn test_toggle_mapping_enabled() {
        let mut state = AppState::default();
        state.mappings.push(Mapping {
            id: "m1".to_string(),
            name: "Test".to_string(),
            enabled: true,
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            action: OutputAction::KeyTap {
                key: KeySpec::Space,
            },
            options: MappingOptions::default(),
        });
        state.selected_mapping = Some(0);

        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        handle_key_event(&mut state, key);
        assert!(!state.mappings[0].enabled);

        handle_key_event(&mut state, key);
        assert!(state.mappings[0].enabled);
    }

    #[test]
    fn test_learn_midi_captures_trigger() {
        let mut state = AppState::default();
        state.mode = AppMode::LearnMidi;
        state.learn_state = Some(LearnState::WaitingForMidi);

        let event = MidiEvent {
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 60,
                min_velocity: None,
                max_velocity: None,
            },
            raw_velocity: Some(100),
            raw_value: None,
            timestamp_us: 0,
            knob_direction: None,
        };

        handle_midi_event(&mut state, event);

        // Should move to LearnAction with the trigger captured
        assert!(matches!(state.mode, AppMode::LearnAction));
        assert!(state.action_menu_open);
    }

    #[test]
    fn test_learn_midi_cancel() {
        let mut state = AppState::default();
        state.mode = AppMode::LearnMidi;
        state.learn_state = Some(LearnState::WaitingForMidi);
        state.previous_mode = Some(AppMode::Setup);

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        handle_key_event(&mut state, key);

        assert!(matches!(state.mode, AppMode::Setup));
        assert!(state.learn_state.is_none());
    }

    #[test]
    fn test_panic_stop_toggle() {
        let mut state = AppState::default();
        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE);
        handle_key_event(&mut state, key);
        assert!(state.panic_stop);
        handle_key_event(&mut state, key);
        assert!(!state.panic_stop);
    }

    #[test]
    fn test_run_mode_matching() {
        let mut state = AppState::default();
        state.mode = AppMode::Run;
        state.mappings.push(Mapping {
            id: "m1".to_string(),
            name: "Test".to_string(),
            enabled: true,
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            action: OutputAction::KeyTap {
                key: KeySpec::Space,
            },
            options: MappingOptions {
                debounce_ms: 0,
                ..MappingOptions::default()
            },
        });

        let event = MidiEvent {
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            raw_velocity: Some(100),
            raw_value: None,
            timestamp_us: 0,
            knob_direction: None,
        };

        let effects = handle_midi_event(&mut state, event);
        assert!(effects
            .iter()
            .any(|e| matches!(e, SideEffect::ExecuteAction(_))));
    }

    #[test]
    fn test_debounce_blocks_rapid_retrigger() {
        let mut state = AppState::default();
        state.mode = AppMode::Run;
        state.mappings.push(Mapping {
            id: "m1".to_string(),
            name: "Test".to_string(),
            enabled: true,
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            action: OutputAction::KeyTap {
                key: KeySpec::Space,
            },
            options: MappingOptions {
                debounce_ms: 1000, // 1 second debounce
                ..MappingOptions::default()
            },
        });

        let event = MidiEvent {
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            raw_velocity: Some(100),
            raw_value: None,
            timestamp_us: 0,
            knob_direction: None,
        };

        // First trigger should work
        let effects1 = handle_midi_event(&mut state, event.clone());
        assert!(effects1
            .iter()
            .any(|e| matches!(e, SideEffect::ExecuteAction(_))));

        // Second immediate trigger should be debounced
        let effects2 = handle_midi_event(&mut state, event);
        assert!(!effects2
            .iter()
            .any(|e| matches!(e, SideEffect::ExecuteAction(_))));
    }
}
