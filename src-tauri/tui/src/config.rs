//! ~/.config/ypm/config.toml — strong defaults, every field optional.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// UI language: zh | en | ja
    #[serde(deserialize_with = "deserialize_language")]
    pub language: String,
    /// NCM quality level: 128 | 320 | exhigh | lossless | hires
    pub quality: String,
    /// Built-in theme name; later also a file in themes/.
    pub theme: String,
    /// Song cache cap in MiB.
    pub cache_limit_mib: u64,
    /// Enter on a list: true = the list becomes the queue from that song
    /// (desktop/NCM semantics), false = play just that one song.
    pub enter_replaces_queue: bool,
    /// Playing layout: "side" = cover fills the height, lyrics beside;
    /// "stacked" = cover centered on top, lyrics below.
    pub layout: String,
    /// Optional image for the idle dashboard; run through the same pixel
    /// pipeline as covers. Falls back to the built-in vinyl.
    pub idle_art: Option<String>,
    /// Progress bar look: "dot" = thin line with a playhead dot,
    /// "bar" = thick block line, no dot.
    pub progress_style: String,
    /// Pixel density multiplier for cover/idle art (0.5 chunky … 2.0 fine).
    pub pixel_scale: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "zh".into(),
            quality: "exhigh".into(),
            theme: "db16".into(),
            cache_limit_mib: 2048,
            enter_replaces_queue: true,
            layout: "side".into(),
            idle_art: None,
            progress_style: "dot".into(),
            pixel_scale: 1.0,
        }
    }
}

impl Config {
    /// Missing or invalid config falls back to defaults — the TUI must
    /// always start. A missing file gets a commented template so the
    /// options are discoverable without reading docs.
    pub fn load() -> Self {
        let path = config_dir().join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let _ = write_template(&path);
                Self::default()
            }
        }
    }
}

const TEMPLATE: &str = r#"# ypm 配置 — 保存后重启生效。所有项都可省略（用默认值）。

# language = "zh"            # zh | en | ja
# quality = "exhigh"          # 128 | 320 | exhigh | lossless | hires
# theme = "db16"              # db16 | pico8 | gameboy | everforest | tokyo-night | tokyo-night-storm | one-dark | transparent
# layout = "side"             # side（封面撑满高度）| stacked（封面居中在上）
# progress_style = "dot"      # dot（细线+圆点）| bar（粗块）
# enter_replaces_queue = true # Enter：整列表成为队列；false = 只播这一首
# idle_art = "~/my-art.png"   # 开屏像素画（png/jpg/webp/gif，自动像素化）
# cache_limit_mib = 2048
# pixel_scale = 1.0            # 像素细腻度：0.5 更复古块状，2.0 更细腻
"#;

fn deserialize_language<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "zh" | "en" | "ja" => value,
        _ => "zh".into(),
    })
}

fn write_template(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, TEMPLATE)
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
        assert_eq!(config.language, "zh");
        assert_eq!(config.quality, "exhigh");
        assert_eq!(config.theme, "db16");

        let parsed: Config = toml::from_str("quality = \"lossless\"").unwrap();
        assert_eq!(parsed.quality, "lossless");
        assert_eq!(parsed.theme, "db16");

        let parsed: Config = toml::from_str("language = \"ja\"").unwrap();
        assert_eq!(parsed.language, "ja");
        let parsed: Config = toml::from_str("language = \"fr\"").unwrap();
        assert_eq!(parsed.language, "zh");
    }
}
