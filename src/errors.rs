use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AppError {
    #[error("MIDI device not found: {0}")]
    MidiDeviceNotFound(String),

    #[error("MIDI connection lost: {0}")]
    MidiConnectionLost(String),

    #[error("MIDI initialization error: {0}")]
    MidiInitError(String),

    #[error("Config parse failure: {0}")]
    ConfigParseError(String),

    #[error("Config save failure: {0}")]
    ConfigSaveError(String),

    #[error("Keyboard injection failure: {0}")]
    KeyboardError(String),

    #[error("Unsupported key on current platform: {0}")]
    UnsupportedKey(String),

    #[error("Terminal initialization failure: {0}")]
    TerminalError(String),

    #[error("Channel send error: {0}")]
    ChannelError(String),
}
