use std::path::Path;

use anyhow::{Context, Result};
use tracing::debug;

use crate::config::model::AppConfig;

/// Save config to a TOML file. Creates parent directories if needed.
pub fn save_config(config: &AppConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context(format!(
            "Failed to create config directory {}",
            parent.display()
        ))?;
    }

    let content = toml::to_string_pretty(config).context("Failed to serialize config to TOML")?;

    std::fs::write(path, &content)
        .context(format!("Failed to write config to {}", path.display()))?;

    debug!(
        "Saved config with {} mappings to {}",
        config.mappings.len(),
        path.display()
    );
    Ok(())
}

/// Generate default config as a TOML string for --dump-default-config.
pub fn dump_default_config() -> String {
    let config = AppConfig::default();
    toml::to_string_pretty(&config).unwrap_or_else(|_| String::from("# Failed to generate config"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load::load_config;
    use crate::config::model::*;
    use crate::midi::trigger::MidiTrigger;

    #[test]
    fn test_config_roundtrip() {
        let config = AppConfig {
            version: 1,
            selected_device: Some("MPK mini IV".to_string()),
            mappings: vec![
                Mapping {
                    id: "map_01".to_string(),
                    name: "Kick Pad -> Space".to_string(),
                    enabled: true,
                    trigger: MidiTrigger::NoteOn {
                        channel: 1,
                        note: 36,
                        min_velocity: None,
                        max_velocity: None,
                    },
                    action: OutputAction::KeyTap {
                        key: KeySpec::Space,
                    },
                    options: MappingOptions::default(),
                },
                Mapping {
                    id: "map_02".to_string(),
                    name: "Pedal -> Ctrl+Shift+P".to_string(),
                    enabled: true,
                    trigger: MidiTrigger::ControlChange {
                        channel: 1,
                        controller: 64,
                        min_value: Some(1),
                        max_value: Some(127),
                    },
                    action: OutputAction::KeyChord {
                        keys: vec![KeySpec::Ctrl, KeySpec::Shift, KeySpec::Char('P')],
                    },
                    options: MappingOptions {
                        debounce_ms: 100,
                        suppress_retrigger_while_held: false,
                        trigger_on_value_change_only: true,
                        allow_overlap: false,
                    },
                },
            ],
        };

        let dir = std::env::temp_dir().join("keebmidi_test_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_config.toml");

        save_config(&config, &path).unwrap();
        let loaded = load_config(&path).unwrap();

        assert_eq!(loaded.version, config.version);
        assert_eq!(loaded.selected_device, config.selected_device);
        assert_eq!(loaded.mappings.len(), config.mappings.len());
        assert_eq!(loaded.mappings[0].id, "map_01");
        assert_eq!(loaded.mappings[0].name, "Kick Pad -> Space");
        assert_eq!(loaded.mappings[1].id, "map_02");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let path = std::path::PathBuf::from("/tmp/keebmidi_nonexistent_test.toml");
        let config = load_config(&path).unwrap();
        assert_eq!(config.version, 2);
        assert!(config.mappings.is_empty());
    }
}
