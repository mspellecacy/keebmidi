use crate::midi::trigger::MidiTrigger;

/// A raw MIDI event decoded from bytes, before normalization into a trigger.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiEvent {
    pub trigger: MidiTrigger,
    pub raw_velocity: Option<u8>,
    pub raw_value: Option<u8>,
    pub timestamp_us: u64,
}

/// Decode raw MIDI bytes into a MidiEvent.
/// Returns None for system realtime, clock, active sensing, and sysex messages.
pub fn decode_midi_message(data: &[u8], timestamp_us: u64) -> Option<MidiEvent> {
    if data.is_empty() {
        return None;
    }

    let status = data[0];

    // Filter out system realtime and system common noise
    match status {
        0xF0..=0xFF => return None, // sysex, clock, active sensing, etc.
        _ => {}
    }

    let msg_type = status & 0xF0;
    let channel = (status & 0x0F) + 1; // 1-indexed channel

    match msg_type {
        // Note Off
        0x80 if data.len() >= 3 => Some(MidiEvent {
            trigger: MidiTrigger::NoteOff {
                channel,
                note: data[1],
            },
            raw_velocity: Some(data[2]),
            raw_value: None,
            timestamp_us,
        }),
        // Note On
        0x90 if data.len() >= 3 => {
            let velocity = data[2];
            // Note On with velocity 0 is conventionally Note Off
            if velocity == 0 {
                Some(MidiEvent {
                    trigger: MidiTrigger::NoteOff {
                        channel,
                        note: data[1],
                    },
                    raw_velocity: Some(0),
                    raw_value: None,
                    timestamp_us,
                })
            } else {
                Some(MidiEvent {
                    trigger: MidiTrigger::NoteOn {
                        channel,
                        note: data[1],
                        min_velocity: None,
                        max_velocity: None,
                    },
                    raw_velocity: Some(velocity),
                    raw_value: None,
                    timestamp_us,
                })
            }
        }
        // Control Change
        0xB0 if data.len() >= 3 => Some(MidiEvent {
            trigger: MidiTrigger::ControlChange {
                channel,
                controller: data[1],
                min_value: None,
                max_value: None,
            },
            raw_velocity: None,
            raw_value: Some(data[2]),
            timestamp_us,
        }),
        // Program Change
        0xC0 if data.len() >= 2 => Some(MidiEvent {
            trigger: MidiTrigger::ProgramChange {
                channel,
                program: data[1],
            },
            raw_velocity: None,
            raw_value: None,
            timestamp_us,
        }),
        // Pitch Bend
        0xE0 if data.len() >= 3 => {
            let _value = ((data[2] as i16) << 7 | (data[1] as i16)) - 8192;
            Some(MidiEvent {
                trigger: MidiTrigger::PitchBend {
                    channel,
                    min_value: None,
                    max_value: None,
                },
                raw_velocity: None,
                raw_value: None,
                timestamp_us,
            })
        }
        _ => None,
    }
}

/// Check if an incoming MidiEvent matches a configured MidiTrigger.
pub fn event_matches_trigger(event: &MidiEvent, trigger: &MidiTrigger) -> bool {
    match (&event.trigger, trigger) {
        (
            MidiTrigger::NoteOn {
                channel: ec,
                note: en,
                ..
            },
            MidiTrigger::NoteOn {
                channel: tc,
                note: tn,
                min_velocity,
                max_velocity,
            },
        ) => {
            if ec != tc || en != tn {
                return false;
            }
            if let Some(vel) = event.raw_velocity {
                if let Some(min) = min_velocity {
                    if vel < *min {
                        return false;
                    }
                }
                if let Some(max) = max_velocity {
                    if vel > *max {
                        return false;
                    }
                }
            }
            true
        }
        (
            MidiTrigger::NoteOff {
                channel: ec,
                note: en,
            },
            MidiTrigger::NoteOff {
                channel: tc,
                note: tn,
            },
        ) => ec == tc && en == tn,
        (
            MidiTrigger::ControlChange {
                channel: ec,
                controller: ecc,
                ..
            },
            MidiTrigger::ControlChange {
                channel: tc,
                controller: tcc,
                min_value,
                max_value,
            },
        ) => {
            if ec != tc || ecc != tcc {
                return false;
            }
            if let Some(val) = event.raw_value {
                if let Some(min) = min_value {
                    if val < *min {
                        return false;
                    }
                }
                if let Some(max) = max_value {
                    if val > *max {
                        return false;
                    }
                }
            }
            true
        }
        (
            MidiTrigger::ProgramChange {
                channel: ec,
                program: ep,
            },
            MidiTrigger::ProgramChange {
                channel: tc,
                program: tp,
            },
        ) => ec == tc && ep == tp,
        (
            MidiTrigger::PitchBend { channel: ec, .. },
            MidiTrigger::PitchBend {
                channel: tc,
                min_value,
                max_value,
            },
        ) => {
            if ec != tc {
                return false;
            }
            // PitchBend value matching would need the raw pitch value stored
            // For now, match on channel only if no range specified
            min_value.is_none() && max_value.is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_note_on() {
        let data = [0x90, 60, 100]; // ch1 note 60 vel 100
        let event = decode_midi_message(&data, 0).unwrap();
        assert_eq!(
            event.trigger,
            MidiTrigger::NoteOn {
                channel: 1,
                note: 60,
                min_velocity: None,
                max_velocity: None,
            }
        );
        assert_eq!(event.raw_velocity, Some(100));
    }

    #[test]
    fn test_decode_note_on_vel_zero_is_note_off() {
        let data = [0x90, 60, 0];
        let event = decode_midi_message(&data, 0).unwrap();
        assert_eq!(
            event.trigger,
            MidiTrigger::NoteOff {
                channel: 1,
                note: 60,
            }
        );
    }

    #[test]
    fn test_decode_note_off() {
        let data = [0x80, 60, 64];
        let event = decode_midi_message(&data, 0).unwrap();
        assert_eq!(
            event.trigger,
            MidiTrigger::NoteOff {
                channel: 1,
                note: 60,
            }
        );
    }

    #[test]
    fn test_decode_cc() {
        let data = [0xB0, 64, 127]; // ch1 cc64 val127
        let event = decode_midi_message(&data, 0).unwrap();
        assert_eq!(
            event.trigger,
            MidiTrigger::ControlChange {
                channel: 1,
                controller: 64,
                min_value: None,
                max_value: None,
            }
        );
        assert_eq!(event.raw_value, Some(127));
    }

    #[test]
    fn test_decode_program_change() {
        let data = [0xC0, 5];
        let event = decode_midi_message(&data, 0).unwrap();
        assert_eq!(
            event.trigger,
            MidiTrigger::ProgramChange {
                channel: 1,
                program: 5,
            }
        );
    }

    #[test]
    fn test_decode_channel_offset() {
        let data = [0x95, 60, 100]; // ch6 (0x05 + 1)
        let event = decode_midi_message(&data, 0).unwrap();
        match event.trigger {
            MidiTrigger::NoteOn { channel, .. } => assert_eq!(channel, 6),
            _ => panic!("expected NoteOn"),
        }
    }

    #[test]
    fn test_decode_system_realtime_filtered() {
        assert!(decode_midi_message(&[0xF8], 0).is_none()); // clock
        assert!(decode_midi_message(&[0xFE], 0).is_none()); // active sensing
        assert!(decode_midi_message(&[0xF0, 0x7E, 0xF7], 0).is_none()); // sysex
    }

    #[test]
    fn test_decode_empty() {
        assert!(decode_midi_message(&[], 0).is_none());
    }

    #[test]
    fn test_trigger_matching_note_on() {
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
        };
        let trigger = MidiTrigger::NoteOn {
            channel: 1,
            note: 36,
            min_velocity: None,
            max_velocity: None,
        };
        assert!(event_matches_trigger(&event, &trigger));
    }

    #[test]
    fn test_trigger_matching_note_on_velocity_range() {
        let event = MidiEvent {
            trigger: MidiTrigger::NoteOn {
                channel: 1,
                note: 36,
                min_velocity: None,
                max_velocity: None,
            },
            raw_velocity: Some(50),
            raw_value: None,
            timestamp_us: 0,
        };
        let trigger_in_range = MidiTrigger::NoteOn {
            channel: 1,
            note: 36,
            min_velocity: Some(10),
            max_velocity: Some(100),
        };
        assert!(event_matches_trigger(&event, &trigger_in_range));

        let trigger_out_range = MidiTrigger::NoteOn {
            channel: 1,
            note: 36,
            min_velocity: Some(60),
            max_velocity: Some(127),
        };
        assert!(!event_matches_trigger(&event, &trigger_out_range));
    }

    #[test]
    fn test_trigger_matching_cc() {
        let event = MidiEvent {
            trigger: MidiTrigger::ControlChange {
                channel: 1,
                controller: 64,
                min_value: None,
                max_value: None,
            },
            raw_velocity: None,
            raw_value: Some(127),
            timestamp_us: 0,
        };
        let trigger = MidiTrigger::ControlChange {
            channel: 1,
            controller: 64,
            min_value: Some(1),
            max_value: Some(127),
        };
        assert!(event_matches_trigger(&event, &trigger));
    }

    #[test]
    fn test_trigger_no_cross_type_match() {
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
        };
        let trigger = MidiTrigger::ControlChange {
            channel: 1,
            controller: 36,
            min_value: None,
            max_value: None,
        };
        assert!(!event_matches_trigger(&event, &trigger));
    }
}
