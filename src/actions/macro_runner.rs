use std::thread;
use std::time::Duration;

use anyhow::Result;
use crate::actions::keyboard::KeyboardSink;
use crate::config::model::{MacroSpec, MacroStep};

/// Execute a macro's steps sequentially using the given keyboard sink.
/// This function blocks for the duration of delays and should be called
/// from a worker thread, never from the UI thread.
pub fn run_macro(spec: &MacroSpec, sink: &mut dyn KeyboardSink) -> Result<()> {
    for step in &spec.steps {
        match step {
            MacroStep::KeyDown(key) => {
                sink.key_down(key)?;
            }
            MacroStep::KeyUp(key) => {
                sink.key_up(key)?;
            }
            MacroStep::KeyTap(key) => {
                sink.key_tap(key)?;
            }
            MacroStep::Text(text) => {
                sink.text(text)?;
            }
            MacroStep::DelayMs(ms) => {
                thread::sleep(Duration::from_millis(*ms));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::keyboard::{MockKeyEvent, MockKeyboardSink};
    use crate::config::model::{KeySpec, PlaybackMode};

    #[test]
    fn test_run_macro_basic() {
        let spec = MacroSpec {
            steps: vec![
                MacroStep::KeyDown(KeySpec::Ctrl),
                MacroStep::KeyTap(KeySpec::Char('k')),
                MacroStep::KeyUp(KeySpec::Ctrl),
            ],
            playback_mode: PlaybackMode::FireAndForget,
        };
        let mut sink = MockKeyboardSink::default();
        run_macro(&spec, &mut sink).unwrap();
        assert_eq!(sink.events.len(), 3);
        assert_eq!(sink.events[0], MockKeyEvent::KeyDown(KeySpec::Ctrl));
        assert_eq!(sink.events[1], MockKeyEvent::KeyTap(KeySpec::Char('k')));
        assert_eq!(sink.events[2], MockKeyEvent::KeyUp(KeySpec::Ctrl));
    }

    #[test]
    fn test_run_macro_with_text() {
        let spec = MacroSpec {
            steps: vec![
                MacroStep::Text("hello".to_string()),
                MacroStep::KeyTap(KeySpec::Enter),
            ],
            playback_mode: PlaybackMode::FireAndForget,
        };
        let mut sink = MockKeyboardSink::default();
        run_macro(&spec, &mut sink).unwrap();
        assert_eq!(sink.events.len(), 2);
        assert_eq!(sink.events[0], MockKeyEvent::Text("hello".to_string()));
        assert_eq!(sink.events[1], MockKeyEvent::KeyTap(KeySpec::Enter));
    }
}
