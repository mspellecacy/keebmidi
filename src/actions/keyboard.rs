use anyhow::Result;
use enigo::{
    Enigo, Keyboard as EnigoKeyboard, Key, Settings,
    Direction::{self, Click, Press, Release},
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

// ---------------------------------------------------------------------------
// Linux media key subprocess dispatch
//
// enigo 0.6.x has a bug in its Wayland backend where XF86 media keys cannot
// be injected. The bug affects both `key()` (broken keysym name stripping in
// `map_key()`) and `raw()` (keycodes not in the virtual keyboard's XKB keymap
// are silently ignored by the compositor).
//
// Instead of trying to work around enigo's broken Wayland virtual keyboard
// path, we dispatch media key actions directly to system commands:
//   - Volume:     `wpctl` (PipeWire) or `pactl` (PulseAudio)
//   - Playback:   `playerctl` (MPRIS/D-Bus)
//   - Brightness: `brightnessctl`
//
// This bypasses the Wayland virtual keyboard protocol entirely and talks
// directly to the relevant system services, which is more reliable than
// simulating keyboard input for these specific actions.
// ---------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
mod media_commands {
    use std::process::Command;
    use anyhow::{Result, Context};
    use tracing::{debug, warn};
    use crate::config::model::KeySpec;

    /// Volume step percentage used for raise/lower commands.
    const VOLUME_STEP: &str = "5";

    /// Brightness step percentage used for raise/lower commands.
    const BRIGHTNESS_STEP: &str = "5";

    /// Returns `true` if the given `KeySpec` is a media key that should be
    /// handled via subprocess commands rather than enigo.
    pub(super) fn is_media_key(key: &KeySpec) -> bool {
        matches!(
            key,
            KeySpec::VolumeUp
                | KeySpec::VolumeDown
                | KeySpec::VolumeMute
                | KeySpec::MediaPlayPause
                | KeySpec::MediaNextTrack
                | KeySpec::MediaPrevTrack
                | KeySpec::MediaStop
                | KeySpec::BrightnessUp
                | KeySpec::BrightnessDown
        )
    }

    /// Execute the system command corresponding to a media key tap.
    /// Returns `Ok(())` on success or an error if no suitable command was found.
    pub(super) fn send_media_key(key: &KeySpec) -> Result<()> {
        match key {
            KeySpec::VolumeUp => volume_up(),
            KeySpec::VolumeDown => volume_down(),
            KeySpec::VolumeMute => volume_mute_toggle(),
            KeySpec::MediaPlayPause => playerctl("play-pause"),
            KeySpec::MediaNextTrack => playerctl("next"),
            KeySpec::MediaPrevTrack => playerctl("previous"),
            KeySpec::MediaStop => playerctl("stop"),
            KeySpec::BrightnessUp => brightness_up(),
            KeySpec::BrightnessDown => brightness_down(),
            _ => unreachable!("is_media_key guard should prevent this"),
        }
    }

    // -- Volume ---------------------------------------------------------------

    fn volume_up() -> Result<()> {
        // Try wpctl (PipeWire) first, fall back to pactl (PulseAudio)
        if try_run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{VOLUME_STEP}%+")]).is_ok() {
            return Ok(());
        }
        run("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &format!("+{VOLUME_STEP}%")])
    }

    fn volume_down() -> Result<()> {
        if try_run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{VOLUME_STEP}%-")]).is_ok() {
            return Ok(());
        }
        run("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &format!("-{VOLUME_STEP}%")])
    }

    fn volume_mute_toggle() -> Result<()> {
        if try_run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).is_ok() {
            return Ok(());
        }
        run("pactl", &["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
    }

    // -- Media playback -------------------------------------------------------

    fn playerctl(action: &str) -> Result<()> {
        run("playerctl", &[action])
    }

    // -- Brightness -----------------------------------------------------------

    fn brightness_up() -> Result<()> {
        run("brightnessctl", &["set", &format!("{BRIGHTNESS_STEP}%+")])
    }

    fn brightness_down() -> Result<()> {
        run("brightnessctl", &["set", &format!("{BRIGHTNESS_STEP}%-")])
    }

    // -- Helpers --------------------------------------------------------------

    /// Run a command and return `Ok(())` if it exits successfully.
    fn run(program: &str, args: &[&str]) -> Result<()> {
        debug!("media_commands: running {} {:?}", program, args);
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("{program} not found or failed to execute"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{program} failed (exit {}): {stderr}", output.status)
        }
    }

    /// Try to run a command, returning `Ok(())` on success or `Err` on any
    /// failure (including the program not being installed). Used for fallback
    /// chains where we want to silently try the next option.
    fn try_run(program: &str, args: &[&str]) -> Result<()> {
        debug!("media_commands: trying {} {:?}", program, args);
        match Command::new(program).args(args).output() {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("media_commands: {program} failed: {stderr}");
                anyhow::bail!("{program} failed")
            }
            Err(e) => {
                debug!("media_commands: {program} not available: {e}");
                anyhow::bail!("{program} not found")
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_is_media_key() {
            assert!(is_media_key(&KeySpec::VolumeUp));
            assert!(is_media_key(&KeySpec::VolumeDown));
            assert!(is_media_key(&KeySpec::VolumeMute));
            assert!(is_media_key(&KeySpec::MediaPlayPause));
            assert!(is_media_key(&KeySpec::MediaNextTrack));
            assert!(is_media_key(&KeySpec::MediaPrevTrack));
            assert!(is_media_key(&KeySpec::MediaStop));
            assert!(is_media_key(&KeySpec::BrightnessUp));
            assert!(is_media_key(&KeySpec::BrightnessDown));
        }

        #[test]
        fn test_non_media_keys() {
            assert!(!is_media_key(&KeySpec::Space));
            assert!(!is_media_key(&KeySpec::Enter));
            assert!(!is_media_key(&KeySpec::F(1)));
            assert!(!is_media_key(&KeySpec::Char('a')));
            assert!(!is_media_key(&KeySpec::Ctrl));
            assert!(!is_media_key(&KeySpec::Alt));
        }
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

impl EnigoKeyboardSink {
    /// Send a key event. On Linux, media/XF86 keys are dispatched via system
    /// commands (wpctl/pactl/playerctl/brightnessctl) to bypass enigo's broken
    /// Wayland XF86 key injection. All other keys use the normal enigo path.
    fn send_key(&mut self, key: &KeySpec, direction: Direction) -> Result<()> {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            // Media keys only fire on tap (Click) or Press — skip Release to
            // avoid double-firing when key_down + key_up are called separately.
            if media_commands::is_media_key(key)
                && (direction == Click || direction == Press)
            {
                return media_commands::send_media_key(key);
            }
            // For Release of a media key, just succeed silently.
            if media_commands::is_media_key(key) && direction == Release {
                return Ok(());
            }
        }
        self.enigo
            .key(keyspec_to_enigo(key), direction)
            .map_err(|e| anyhow::anyhow!("key send failed: {e}"))
    }
}

impl KeyboardSink for EnigoKeyboardSink {
    fn key_down(&mut self, key: &KeySpec) -> Result<()> {
        self.send_key(key, Press)
    }

    fn key_up(&mut self, key: &KeySpec) -> Result<()> {
        self.send_key(key, Release)
    }

    fn key_tap(&mut self, key: &KeySpec) -> Result<()> {
        self.send_key(key, Click)
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
