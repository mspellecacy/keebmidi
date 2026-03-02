use serde::{Deserialize, Serialize};
use std::fmt;

use crate::midi::trigger::MidiTrigger;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySpec {
    Char(char),
    Enter,
    Tab,
    Esc,
    Backspace,
    Space,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Ctrl,
    Alt,
    Shift,
    Meta,
}

impl fmt::Display for KeySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeySpec::Char(c) => write!(f, "{c}"),
            KeySpec::Enter => write!(f, "Enter"),
            KeySpec::Tab => write!(f, "Tab"),
            KeySpec::Esc => write!(f, "Esc"),
            KeySpec::Backspace => write!(f, "Backspace"),
            KeySpec::Space => write!(f, "Space"),
            KeySpec::Up => write!(f, "Up"),
            KeySpec::Down => write!(f, "Down"),
            KeySpec::Left => write!(f, "Left"),
            KeySpec::Right => write!(f, "Right"),
            KeySpec::F(n) => write!(f, "F{n}"),
            KeySpec::Ctrl => write!(f, "Ctrl"),
            KeySpec::Alt => write!(f, "Alt"),
            KeySpec::Shift => write!(f, "Shift"),
            KeySpec::Meta => write!(f, "Meta"),
        }
    }
}

impl KeySpec {
    /// Parse a key name string into a KeySpec.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "enter" | "return" => Some(KeySpec::Enter),
            "tab" => Some(KeySpec::Tab),
            "esc" | "escape" => Some(KeySpec::Esc),
            "backspace" => Some(KeySpec::Backspace),
            "space" => Some(KeySpec::Space),
            "up" => Some(KeySpec::Up),
            "down" => Some(KeySpec::Down),
            "left" => Some(KeySpec::Left),
            "right" => Some(KeySpec::Right),
            "ctrl" | "control" => Some(KeySpec::Ctrl),
            "alt" => Some(KeySpec::Alt),
            "shift" => Some(KeySpec::Shift),
            "meta" | "super" | "win" | "cmd" => Some(KeySpec::Meta),
            s if s.starts_with('f') && s.len() <= 3 => {
                s[1..].parse::<u8>().ok().map(KeySpec::F)
            }
            s if s.len() == 1 => s.chars().next().map(KeySpec::Char),
            _ => None,
        }
    }

    pub fn is_modifier(&self) -> bool {
        matches!(self, KeySpec::Ctrl | KeySpec::Alt | KeySpec::Shift | KeySpec::Meta)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputAction {
    KeyTap { key: KeySpec },
    KeyChord { keys: Vec<KeySpec> },
    Text { text: String },
    Macro { spec: MacroSpec },
}

impl fmt::Display for OutputAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputAction::KeyTap { key } => write!(f, "Key: {key}"),
            OutputAction::KeyChord { keys } => {
                let names: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
                write!(f, "Chord: {}", names.join("+"))
            }
            OutputAction::Text { text } => {
                let preview = if text.len() > 20 {
                    format!("{}…", &text[..20])
                } else {
                    text.clone()
                };
                write!(f, "Text: \"{preview}\"")
            }
            OutputAction::Macro { spec } => {
                write!(f, "Macro: {} steps", spec.steps.len())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroSpec {
    pub steps: Vec<MacroStep>,
    #[serde(default)]
    pub playback_mode: PlaybackMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroStep {
    KeyDown(KeySpec),
    KeyUp(KeySpec),
    KeyTap(KeySpec),
    Text(String),
    DelayMs(u64),
}

impl fmt::Display for MacroStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroStep::KeyDown(k) => write!(f, "{k} down"),
            MacroStep::KeyUp(k) => write!(f, "{k} up"),
            MacroStep::KeyTap(k) => write!(f, "{k} tap"),
            MacroStep::Text(t) => write!(f, "Text \"{t}\""),
            MacroStep::DelayMs(ms) => write!(f, "Delay {ms}ms"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    #[default]
    FireAndForget,
    CancelAndRestart,
    IgnoreIfRunning,
    Queue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: MidiTrigger,
    pub action: OutputAction,
    #[serde(default)]
    pub options: MappingOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MappingOptions {
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default)]
    pub suppress_retrigger_while_held: bool,
    #[serde(default)]
    pub trigger_on_value_change_only: bool,
    #[serde(default = "default_true")]
    pub allow_overlap: bool,
}

fn default_debounce_ms() -> u64 {
    50
}

fn default_true() -> bool {
    true
}

impl Default for MappingOptions {
    fn default() -> Self {
        Self {
            debounce_ms: 50,
            suppress_retrigger_while_held: false,
            trigger_on_value_change_only: false,
            allow_overlap: true,
        }
    }
}

/// Top-level config file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device: Option<String>,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

fn default_version() -> u32 {
    1
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            selected_device: None,
            mappings: Vec::new(),
        }
    }
}

/// Normalize macro steps: collapse duplicate modifier noise, drop zero-length delays,
/// clamp small delays to minimum, and cap total duration.
pub fn normalize_macro(steps: &[MacroStep]) -> Vec<MacroStep> {
    const MIN_DELAY_MS: u64 = 15;
    const MAX_TOTAL_MS: u64 = 30_000;

    let mut result = Vec::new();
    let mut total_delay: u64 = 0;

    for step in steps {
        match step {
            MacroStep::DelayMs(ms) => {
                if *ms == 0 {
                    continue;
                }
                let clamped = (*ms).max(MIN_DELAY_MS);
                if total_delay + clamped > MAX_TOTAL_MS {
                    let remaining = MAX_TOTAL_MS.saturating_sub(total_delay);
                    if remaining >= MIN_DELAY_MS {
                        result.push(MacroStep::DelayMs(remaining));
                    }
                    break;
                }
                total_delay += clamped;
                result.push(MacroStep::DelayMs(clamped));
            }
            other => {
                result.push(other.clone());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyspec_from_name() {
        assert_eq!(KeySpec::from_name("Enter"), Some(KeySpec::Enter));
        assert_eq!(KeySpec::from_name("ctrl"), Some(KeySpec::Ctrl));
        assert_eq!(KeySpec::from_name("F1"), Some(KeySpec::F(1)));
        assert_eq!(KeySpec::from_name("f12"), Some(KeySpec::F(12)));
        assert_eq!(KeySpec::from_name("a"), Some(KeySpec::Char('a')));
        assert_eq!(KeySpec::from_name("Space"), Some(KeySpec::Space));
        assert_eq!(KeySpec::from_name("Meta"), Some(KeySpec::Meta));
        assert_eq!(KeySpec::from_name("cmd"), Some(KeySpec::Meta));
    }

    #[test]
    fn test_keyspec_is_modifier() {
        assert!(KeySpec::Ctrl.is_modifier());
        assert!(KeySpec::Alt.is_modifier());
        assert!(KeySpec::Shift.is_modifier());
        assert!(KeySpec::Meta.is_modifier());
        assert!(!KeySpec::Enter.is_modifier());
        assert!(!KeySpec::Char('a').is_modifier());
    }

    #[test]
    fn test_normalize_macro_drops_zero_delays() {
        let steps = vec![
            MacroStep::KeyTap(KeySpec::Char('a')),
            MacroStep::DelayMs(0),
            MacroStep::KeyTap(KeySpec::Char('b')),
        ];
        let normalized = normalize_macro(&steps);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0], MacroStep::KeyTap(KeySpec::Char('a')));
        assert_eq!(normalized[1], MacroStep::KeyTap(KeySpec::Char('b')));
    }

    #[test]
    fn test_normalize_macro_clamps_small_delays() {
        let steps = vec![
            MacroStep::KeyTap(KeySpec::Char('a')),
            MacroStep::DelayMs(5), // below minimum
            MacroStep::KeyTap(KeySpec::Char('b')),
        ];
        let normalized = normalize_macro(&steps);
        assert_eq!(normalized[1], MacroStep::DelayMs(15));
    }

    #[test]
    fn test_normalize_macro_caps_total_duration() {
        let steps = vec![
            MacroStep::DelayMs(25_000),
            MacroStep::KeyTap(KeySpec::Char('a')),
            MacroStep::DelayMs(25_000), // would exceed 30s
        ];
        let normalized = normalize_macro(&steps);
        // First delay passes (25s), key tap passes, second delay capped
        assert_eq!(normalized.len(), 3);
        match &normalized[2] {
            MacroStep::DelayMs(ms) => assert!(*ms <= 5_000),
            _ => panic!("expected delay"),
        }
    }

    #[test]
    fn test_output_action_display() {
        let tap = OutputAction::KeyTap {
            key: KeySpec::Space,
        };
        assert_eq!(format!("{tap}"), "Key: Space");

        let chord = OutputAction::KeyChord {
            keys: vec![KeySpec::Ctrl, KeySpec::Shift, KeySpec::Char('p')],
        };
        assert_eq!(format!("{chord}"), "Chord: Ctrl+Shift+p");
    }

    #[test]
    fn test_mapping_options_default() {
        let opts = MappingOptions::default();
        assert_eq!(opts.debounce_ms, 50);
        assert!(!opts.suppress_retrigger_while_held);
        assert!(!opts.trigger_on_value_change_only);
        assert!(opts.allow_overlap);
    }
}
