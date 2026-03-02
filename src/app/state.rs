use std::collections::HashMap;
use std::time::Instant;

use crate::config::model::{KeySpec, MacroStep, Mapping};
use crate::midi::manager::MidiDeviceInfo;
use crate::midi::trigger::MidiTrigger;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Setup,
    Run,
    LearnMidi,
    LearnAction,
    RecordMacro,
    ConfirmDialog(String),
    ErrorDialog(String),
    TextInput,
    SelectDevice,
    EditMenu,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum LearnState {
    WaitingForMidi,
    WaitingForSingleKey,
    WaitingForChord { pressed: Vec<KeySpec> },
    WaitingForText { buffer: String },
    RecordingMacro { started_at: Instant, steps: Vec<MacroStep> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePane {
    MappingList,
    Details,
    Log,
}

#[allow(dead_code)]
pub struct AppState {
    pub mode: AppMode,
    pub previous_mode: Option<AppMode>,
    pub midi_devices: Vec<MidiDeviceInfo>,
    pub selected_device: Option<usize>,
    pub selected_device_name: Option<String>,
    pub midi_connected: bool,
    pub mappings: Vec<Mapping>,
    pub selected_mapping: Option<usize>,
    pub learn_state: Option<LearnState>,
    pub learned_trigger: Option<MidiTrigger>,
    pub log_lines: Vec<String>,
    pub dirty: bool,
    pub running: bool,
    pub should_quit: bool,
    pub active_pane: ActivePane,
    pub last_midi_event: Option<String>,
    pub last_action: Option<String>,
    pub status_message: Option<String>,
    pub debounce_timers: HashMap<String, Instant>,
    pub last_cc_values: HashMap<(u8, u8), u8>,
    pub panic_stop: bool,
    pub device_list_index: usize,
    pub text_input_buffer: String,
    pub text_input_purpose: TextInputPurpose,
    pub action_menu_open: bool,
    pub action_menu_index: usize,
    pub edit_menu_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum TextInputPurpose {
    MappingName,
    TextAction,
    DebounceMs,
    None,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Setup,
            previous_mode: None,
            midi_devices: Vec::new(),
            selected_device: None,
            selected_device_name: None,
            midi_connected: false,
            mappings: Vec::new(),
            selected_mapping: None,
            learn_state: None,
            learned_trigger: None,
            log_lines: Vec::new(),
            dirty: false,
            running: false,
            should_quit: false,
            active_pane: ActivePane::MappingList,
            last_midi_event: None,
            last_action: None,
            status_message: None,
            debounce_timers: HashMap::new(),
            last_cc_values: HashMap::new(),
            panic_stop: false,
            device_list_index: 0,
            text_input_buffer: String::new(),
            text_input_purpose: TextInputPurpose::None,
            action_menu_open: false,
            action_menu_index: 0,
            edit_menu_index: 0,
        }
    }
}

impl AppState {
    pub fn add_log(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.log_lines.push(msg);
        // Keep log bounded
        if self.log_lines.len() > 200 {
            self.log_lines.drain(0..50);
        }
    }

    pub fn selected_mapping_ref(&self) -> Option<&Mapping> {
        self.selected_mapping
            .and_then(|i| self.mappings.get(i))
    }
}
