use serde::{Deserialize, Serialize};
use std::fmt;

use crate::config::model::KnobMode;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiTrigger {
    NoteOn {
        channel: u8,
        note: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_velocity: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_velocity: Option<u8>,
    },
    NoteOff {
        channel: u8,
        note: u8,
    },
    #[serde(rename = "cc")]
    ControlChange {
        channel: u8,
        controller: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_value: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_value: Option<u8>,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    PitchBend {
        channel: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_value: Option<i16>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_value: Option<i16>,
    },
    /// Fires when a CC knob is rotated in a specific direction.
    KnobRotation {
        channel: u8,
        controller: u8,
        direction: KnobRotationDirection,
        #[serde(default)]
        mode: KnobMode,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnobRotationDirection {
    Clockwise,
    CounterClockwise,
}

impl fmt::Display for MidiTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiTrigger::NoteOn {
                channel,
                note,
                min_velocity,
                max_velocity,
            } => {
                write!(f, "NoteOn ch={channel} note={note}")?;
                match (min_velocity, max_velocity) {
                    (Some(min), Some(max)) => write!(f, " vel={min}-{max}"),
                    (Some(min), None) => write!(f, " vel>={min}"),
                    (None, Some(max)) => write!(f, " vel<={max}"),
                    (None, None) => write!(f, " vel=*"),
                }
            }
            MidiTrigger::NoteOff { channel, note } => {
                write!(f, "NoteOff ch={channel} note={note}")
            }
            MidiTrigger::ControlChange {
                channel,
                controller,
                min_value,
                max_value,
            } => {
                write!(f, "CC ch={channel} ctrl={controller}")?;
                match (min_value, max_value) {
                    (Some(min), Some(max)) => write!(f, " val={min}-{max}"),
                    (Some(min), None) => write!(f, " val>={min}"),
                    (None, Some(max)) => write!(f, " val<={max}"),
                    (None, None) => write!(f, " val=*"),
                }
            }
            MidiTrigger::ProgramChange { channel, program } => {
                write!(f, "PC ch={channel} prog={program}")
            }
            MidiTrigger::PitchBend {
                channel,
                min_value,
                max_value,
            } => {
                write!(f, "PitchBend ch={channel}")?;
                match (min_value, max_value) {
                    (Some(min), Some(max)) => write!(f, " val={min}-{max}"),
                    _ => write!(f, " val=*"),
                }
            }
            MidiTrigger::KnobRotation {
                channel,
                controller,
                direction,
                ..
            } => {
                let dir = match direction {
                    KnobRotationDirection::Clockwise => "CW",
                    KnobRotationDirection::CounterClockwise => "CCW",
                };
                write!(f, "Knob {dir} ch={channel} ctrl={controller}")
            }
        }
    }
}

impl fmt::Display for KnobRotationDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KnobRotationDirection::Clockwise => write!(f, "Clockwise"),
            KnobRotationDirection::CounterClockwise => write!(f, "Counter-Clockwise"),
        }
    }
}
