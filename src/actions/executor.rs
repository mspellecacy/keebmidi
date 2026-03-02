use std::thread;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use crate::actions::keyboard::KeyboardSink;
use crate::actions::macro_runner::run_macro;
use crate::app::events::AppEvent;
use crate::config::model::OutputAction;

/// A command sent to the action executor worker thread.
pub struct ActionCommand {
    pub mapping_id: String,
    pub action: OutputAction,
}

/// Spawns a dedicated worker thread that processes output actions sequentially.
/// Actions are received via the command_rx channel. Completion/failure notifications
/// are sent back via event_tx.
pub fn spawn_executor(
    command_rx: Receiver<ActionCommand>,
    event_tx: Sender<AppEvent>,
    sink: Box<dyn KeyboardSink>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut sink = sink;
        for cmd in command_rx {
            let id = cmd.mapping_id.clone();
            let result = execute_action(&cmd.action, &mut *sink);
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::ActionCompleted(id));
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::ActionFailed(id, e.to_string()));
                }
            }
        }
    })
}

fn execute_action(action: &OutputAction, sink: &mut dyn KeyboardSink) -> Result<()> {
    match action {
        OutputAction::KeyTap { key } => {
            sink.key_tap(key)?;
        }
        OutputAction::KeyChord { keys } => {
            // Press all modifiers and keys in order, then release in reverse
            for key in keys {
                sink.key_down(key)?;
            }
            for key in keys.iter().rev() {
                sink.key_up(key)?;
            }
        }
        OutputAction::Text { text } => {
            sink.text(text)?;
        }
        OutputAction::Macro { spec } => {
            run_macro(spec, sink)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::keyboard::{MockKeyEvent, MockKeyboardSink};
    use crate::config::model::KeySpec;

    #[test]
    fn test_execute_key_tap() {
        let mut sink = MockKeyboardSink::default();
        let action = OutputAction::KeyTap {
            key: KeySpec::Space,
        };
        execute_action(&action, &mut sink).unwrap();
        assert_eq!(sink.events, vec![MockKeyEvent::KeyTap(KeySpec::Space)]);
    }

    #[test]
    fn test_execute_key_chord() {
        let mut sink = MockKeyboardSink::default();
        let action = OutputAction::KeyChord {
            keys: vec![KeySpec::Ctrl, KeySpec::Char('c')],
        };
        execute_action(&action, &mut sink).unwrap();
        assert_eq!(
            sink.events,
            vec![
                MockKeyEvent::KeyDown(KeySpec::Ctrl),
                MockKeyEvent::KeyDown(KeySpec::Char('c')),
                MockKeyEvent::KeyUp(KeySpec::Char('c')),
                MockKeyEvent::KeyUp(KeySpec::Ctrl),
            ]
        );
    }

    #[test]
    fn test_execute_text() {
        let mut sink = MockKeyboardSink::default();
        let action = OutputAction::Text {
            text: "hello".to_string(),
        };
        execute_action(&action, &mut sink).unwrap();
        assert_eq!(
            sink.events,
            vec![MockKeyEvent::Text("hello".to_string())]
        );
    }

    #[test]
    fn test_executor_thread() {
        let (cmd_tx, cmd_rx) = crossbeam_channel::bounded(10);
        let (event_tx, event_rx) = crossbeam_channel::bounded(10);

        let sink = Box::new(MockKeyboardSink::default());
        let handle = spawn_executor(cmd_rx, event_tx, sink);

        cmd_tx
            .send(ActionCommand {
                mapping_id: "test_01".to_string(),
                action: OutputAction::KeyTap {
                    key: KeySpec::Enter,
                },
            })
            .unwrap();

        // Drop sender to signal the worker to stop
        drop(cmd_tx);
        handle.join().unwrap();

        let event = event_rx.recv().unwrap();
        match event {
            AppEvent::ActionCompleted(id) => assert_eq!(id, "test_01"),
            _ => panic!("expected ActionCompleted"),
        }
    }
}
