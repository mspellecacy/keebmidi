use anyhow::Result;
use enigo::{
    Enigo, Keyboard as EnigoKeyboard, Key, Settings,
    Direction::{Click, Press, Release},
};
use crate::config::model::KeySpec;

/// Abstraction over keyboard injection for testability and platform fallbacks.
pub trait KeyboardSink: Send {
    fn key_down(&mut self, key: &KeySpec) -> Result<()>;
    fn key_up(&mut self, key: &KeySpec) -> Result<()>;
    fn key_tap(&mut self, key: &KeySpec) -> Result<()>;
    fn text(&mut self, text: &str) -> Result<()>;
}

pub struct EnigoKeyboardSink {
    enigo: Enigo,
}

impl EnigoKeyboardSink {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {e}"))?;
        Ok(Self { enigo })
    }
}

fn keyspec_to_enigo(key: &KeySpec) -> Key {
    match key {
        KeySpec::Char(c) => Key::Unicode(*c),
        KeySpec::Enter => Key::Return,
        KeySpec::Tab => Key::Tab,
        KeySpec::Esc => Key::Escape,
        KeySpec::Backspace => Key::Backspace,
        KeySpec::Space => Key::Space,
        KeySpec::Up => Key::UpArrow,
        KeySpec::Down => Key::DownArrow,
        KeySpec::Left => Key::LeftArrow,
        KeySpec::Right => Key::RightArrow,
        KeySpec::F(n) => match n {
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            12 => Key::F12,
            _ => Key::F1, // fallback for unsupported F-keys
        },
        KeySpec::Ctrl => Key::Control,
        KeySpec::Alt => Key::Alt,
        KeySpec::Shift => Key::Shift,
        KeySpec::Meta => Key::Meta,
        KeySpec::VolumeUp => Key::VolumeUp,
        KeySpec::VolumeDown => Key::VolumeDown,
        KeySpec::VolumeMute => Key::VolumeMute,
        KeySpec::MediaPlayPause => Key::MediaPlayPause,
        #[cfg(not(target_os = "macos"))]
        KeySpec::MediaStop => Key::MediaStop,
        #[cfg(target_os = "macos")]
        KeySpec::MediaStop => Key::Unicode('\0'),
        KeySpec::MediaNextTrack => Key::MediaNextTrack,
        KeySpec::MediaPrevTrack => Key::MediaPrevTrack,
        // Brightness keys: platform-specific
        #[cfg(target_os = "macos")]
        KeySpec::BrightnessUp => Key::BrightnessUp,
        #[cfg(target_os = "macos")]
        KeySpec::BrightnessDown => Key::BrightnessDown,
        #[cfg(all(unix, not(target_os = "macos")))]
        KeySpec::BrightnessUp => Key::Other(0x1008FF02), // XF86MonBrightnessUp
        #[cfg(all(unix, not(target_os = "macos")))]
        KeySpec::BrightnessDown => Key::Other(0x1008FF03), // XF86MonBrightnessDown
        #[cfg(target_os = "windows")]
        KeySpec::BrightnessUp => Key::Unicode('\0'),
        #[cfg(target_os = "windows")]
        KeySpec::BrightnessDown => Key::Unicode('\0'),
    }
}

impl KeyboardSink for EnigoKeyboardSink {
    fn key_down(&mut self, key: &KeySpec) -> Result<()> {
        self.enigo
            .key(keyspec_to_enigo(key), Press)
            .map_err(|e| anyhow::anyhow!("key_down failed: {e}"))
    }

    fn key_up(&mut self, key: &KeySpec) -> Result<()> {
        self.enigo
            .key(keyspec_to_enigo(key), Release)
            .map_err(|e| anyhow::anyhow!("key_up failed: {e}"))
    }

    fn key_tap(&mut self, key: &KeySpec) -> Result<()> {
        self.enigo
            .key(keyspec_to_enigo(key), Click)
            .map_err(|e| anyhow::anyhow!("key_tap failed: {e}"))
    }

    fn text(&mut self, text: &str) -> Result<()> {
        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("text input failed: {e}"))
    }
}

/// Mock keyboard sink for testing.
#[derive(Default)]
pub struct MockKeyboardSink {
    pub events: Vec<MockKeyEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MockKeyEvent {
    KeyDown(KeySpec),
    KeyUp(KeySpec),
    KeyTap(KeySpec),
    Text(String),
}

impl KeyboardSink for MockKeyboardSink {
    fn key_down(&mut self, key: &KeySpec) -> Result<()> {
        self.events.push(MockKeyEvent::KeyDown(key.clone()));
        Ok(())
    }

    fn key_up(&mut self, key: &KeySpec) -> Result<()> {
        self.events.push(MockKeyEvent::KeyUp(key.clone()));
        Ok(())
    }

    fn key_tap(&mut self, key: &KeySpec) -> Result<()> {
        self.events.push(MockKeyEvent::KeyTap(key.clone()));
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        self.events.push(MockKeyEvent::Text(text.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_keyboard_sink() {
        let mut sink = MockKeyboardSink::default();
        sink.key_tap(&KeySpec::Space).unwrap();
        sink.key_down(&KeySpec::Ctrl).unwrap();
        sink.key_tap(&KeySpec::Char('c')).unwrap();
        sink.key_up(&KeySpec::Ctrl).unwrap();
        sink.text("hello").unwrap();

        assert_eq!(sink.events.len(), 5);
        assert_eq!(sink.events[0], MockKeyEvent::KeyTap(KeySpec::Space));
        assert_eq!(sink.events[1], MockKeyEvent::KeyDown(KeySpec::Ctrl));
        assert_eq!(
            sink.events[2],
            MockKeyEvent::KeyTap(KeySpec::Char('c'))
        );
        assert_eq!(sink.events[3], MockKeyEvent::KeyUp(KeySpec::Ctrl));
        assert_eq!(
            sink.events[4],
            MockKeyEvent::Text("hello".to_string())
        );
    }
}
