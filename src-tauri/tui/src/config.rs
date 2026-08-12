//! ~/.config/ypm/config.toml — strong defaults, every field optional.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// NCM quality level: 128 | 320 | exhigh | lossless | hires
    pub quality: String,
    /// Built-in theme name; later also a file in themes/.
    pub theme: String,
    /// Song cache cap in MiB.
    pub cache_limit_mib: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            quality: "exhigh".into(),
            theme: "db16".into(),
            cache_limit_mib: 2048,
        }
    }
}

impl Config {
    /// Missing or invalid config falls back to defaults — the TUI must
    /// always start.
    pub fn load() -> Self {
        let path = config_dir().join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}

/// All platforms use the ~/.config style directory — that is the TUI
/// user's muscle memory, and later the GUI shares ~/.cache/ypm.
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/ypm")
}

pub fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache/ypm")
}

pub fn state_dir() -> PathBuf {
    cache_dir().join("state")
}

pub fn session_path() -> PathBuf {
    config_dir().join("session.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_hold_when_config_is_missing_or_broken() {
        let config = Config::default();
        assert_eq!(config.quality, "exhigh");
        assert_eq!(config.theme, "db16");

        let parsed: Config = toml::from_str("quality = \"lossless\"").unwrap();
        assert_eq!(parsed.quality, "lossless");
        assert_eq!(parsed.theme, "db16");
    }
}
