use std::path::Path;

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::config::model::AppConfig;

/// Load config from a TOML file. Returns default config if file doesn't exist.
pub fn load_config(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        info!("No config file found at {}, using defaults", path.display());
        return Ok(AppConfig::default());
    }

    let content =
        std::fs::read_to_string(path).context(format!("Failed to read {}", path.display()))?;

    let config: AppConfig = toml::from_str(&content).context(format!(
        "Failed to parse config at {}",
        path.display()
    ))?;

    if config.version != 1 {
        warn!(
            "Config version {} is not supported (expected 1), loading anyway",
            config.version
        );
    }

    info!(
        "Loaded config with {} mappings from {}",
        config.mappings.len(),
        path.display()
    );
    Ok(config)
}

/// Get the default config file path.
pub fn default_config_path() -> std::path::PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    config_dir.join("keebmidi").join("config.toml")
}
