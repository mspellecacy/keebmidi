use crossterm::event::KeyEvent;

use crate::midi::decode::MidiEvent;

/// All events that flow through the main event channel.
#[allow(dead_code)]
pub enum AppEvent {
    /// Periodic tick for UI refresh.
    Tick,
    /// Terminal keyboard input.
    TerminalKey(KeyEvent),
    /// Terminal resize.
    TerminalResize(u16, u16),
    /// MIDI message received from device.
    MidiReceived(MidiEvent),
    /// Action executor completed successfully.
    ActionCompleted(String),
    /// Action executor failed.
    ActionFailed(String, String),
    /// MIDI device list changed (connect/disconnect).
    DeviceChanged,
}
