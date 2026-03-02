use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::Sender;
use crossterm::event::{self, Event};

use crate::app::events::AppEvent;

/// Poll terminal events and send them into the app event channel.
/// This runs on the UI thread as the main event source.
/// Returns Ok(true) if an event was read, Ok(false) on timeout.
pub fn poll_terminal_event(
    event_tx: &Sender<AppEvent>,
    timeout: Duration,
) -> Result<bool> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key_event) => {
                let _ = event_tx.send(AppEvent::TerminalKey(key_event));
            }
            Event::Resize(w, h) => {
                let _ = event_tx.send(AppEvent::TerminalResize(w, h));
            }
            _ => {}
        }
        Ok(true)
    } else {
        Ok(false)
    }
}
