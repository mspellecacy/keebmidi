use anyhow::Result;
use crossbeam_channel::Sender;
use midir::{Ignore, MidiInput, MidiInputConnection};
use tracing::{error, info};

use crate::app::events::AppEvent;
use crate::midi::decode::decode_midi_message;

#[derive(Debug, Clone)]
pub struct MidiDeviceInfo {
    pub id: usize,
    pub name: String,
}

pub struct MidiManager {
    connection: Option<MidiInputConnection<()>>,
    event_tx: Sender<AppEvent>,
}

impl MidiManager {
    pub fn new(event_tx: Sender<AppEvent>) -> Self {
        Self {
            connection: None,
            event_tx,
        }
    }

    pub fn enumerate_devices() -> Vec<MidiDeviceInfo> {
        let midi_in = match MidiInput::new("keebmidi-enum") {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create MIDI input for enumeration: {e}");
                return Vec::new();
            }
        };

        let ports = midi_in.ports();
        ports
            .iter()
            .enumerate()
            .filter_map(|(i, port)| {
                midi_in
                    .port_name(port)
                    .ok()
                    .map(|name| MidiDeviceInfo { id: i, name })
            })
            .collect()
    }

    pub fn connect(&mut self, device_id: usize) -> Result<String> {
        // Drop existing connection
        self.disconnect();

        let mut midi_in = MidiInput::new("keebmidi-input")
            .map_err(|e| anyhow::anyhow!("MIDI init error: {e}"))?;
        midi_in.ignore(Ignore::ActiveSense | Ignore::Time);

        let ports = midi_in.ports();
        let port = ports
            .get(device_id)
            .ok_or_else(|| anyhow::anyhow!("MIDI device index {device_id} not found"))?;

        let port_name = midi_in.port_name(port)
            .map_err(|e| anyhow::anyhow!("Failed to get port name: {e}"))?;
        let tx = self.event_tx.clone();

        let connection = midi_in.connect(
            port,
            "keebmidi-read",
            move |timestamp_us, data, _| {
                if let Some(event) = decode_midi_message(data, timestamp_us) {
                    if let Err(e) = tx.send(AppEvent::MidiReceived(event)) {
                        error!("Failed to send MIDI event: {e}");
                    }
                }
            },
            (),
        )
        .map_err(|e| anyhow::anyhow!("MIDI connect error: {e}"))?;

        info!("Connected to MIDI device: {port_name}");
        self.connection = Some(connection);
        Ok(port_name)
    }

    pub fn disconnect(&mut self) {
        if let Some(conn) = self.connection.take() {
            conn.close();
            info!("MIDI connection closed");
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }
}
