//! ~/.config/ypm/config.toml — strong defaults, every field optional.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer};
use yesplaymusic_core::cache::AudioQuality;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CoverMode {
    #[default]
    Pixel,
    Original,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// UI language: zh | en | ja
    #[serde(deserialize_with = "deserialize_language")]
    pub language: String,
    /// NCM quality level: 128 | 192 | 320/exhigh | lossless | hires
    #[serde(deserialize_with = "deserialize_quality")]
    pub quality: AudioQuality,
    /// Built-in theme name; later also a file in themes/.
    pub theme: String,
    /// Explicit shared song-cache cap in MiB. None keeps the database policy.
    pub cache_limit_mib: Option<u64>,
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
    /// Cover renderer: palette pixel art or terminal graphics protocol.
    pub cover_mode: CoverMode,
    /// Pixel sampling detail (0.5 chunky … 2.0 fine). The final cell
    /// footprint is unchanged.
    pub pixel_scale: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "zh".into(),
            quality: AudioQuality::High320,
            theme: "db16".into(),
            cache_limit_mib: None,
            enter_replaces_queue: true,
            layout: "side".into(),
            idle_art: None,
            progress_style: "dot".into(),
            cover_mode: CoverMode::Pixel,
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
# quality = "exhigh"          # 128 | 192 | 320/exhigh | lossless | hires
# theme = "db16"              # db16 | pico8 | gameboy | everforest | tokyo-night | tokyo-night-storm | one-dark | transparent
# layout = "side"             # side（封面撑满高度）| stacked（封面居中在上）
# progress_style = "dot"      # dot（细线+圆点）| bar（粗块）
# cover_mode = "pixel"        # pixel（主题像素画）| original（终端原图协议，不支持时回退 pixel）
# enter_replaces_queue = true # Enter：整列表成为队列；false = 只播这一首
# idle_art = "~/my-art.png"   # 开屏像素画（png/jpg/webp/gif，自动像素化）
# cache_limit_mib = 8192       # 仅显式设置时更新 GUI/TUI 共用上限
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

#[derive(Deserialize)]
#[serde(untagged)]
enum QualitySetting {
    Name(String),
    Number(u32),
}

fn deserialize_quality<'de, D>(deserializer: D) -> Result<AudioQuality, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let setting = QualitySetting::deserialize(deserializer)?;
    let quality = match setting {
        QualitySetting::Name(name) => match name.as_str() {
            "128" => Some(AudioQuality::Low128),
            "192" => Some(AudioQuality::Medium192),
            "320" | "exhigh" => Some(AudioQuality::High320),
            "lossless" => Some(AudioQuality::Lossless),
            "hires" => Some(AudioQuality::HiRes),
            _ => None,
        },
        QualitySetting::Number(number) => match number {
            128 => Some(AudioQuality::Low128),
            192 => Some(AudioQuality::Medium192),
            320 => Some(AudioQuality::High320),
            _ => None,
        },
    };
    quality.ok_or_else(|| D::Error::custom("unsupported audio quality"))
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
        assert_eq!(config.quality, AudioQuality::High320);
        assert_eq!(config.theme, "db16");
        assert_eq!(config.cover_mode, CoverMode::Pixel);
        assert_eq!(config.cache_limit_mib, None);

        let parsed: Config = toml::from_str("quality = \"lossless\"").unwrap();
        assert_eq!(parsed.quality, AudioQuality::Lossless);
        assert_eq!(parsed.theme, "db16");

        let parsed: Config = toml::from_str("language = \"ja\"").unwrap();
        assert_eq!(parsed.language, "ja");
        let parsed: Config = toml::from_str("language = \"fr\"").unwrap();
        assert_eq!(parsed.language, "zh");

        let parsed: Config = toml::from_str("cover_mode = \"original\"").unwrap();
        assert_eq!(parsed.cover_mode, CoverMode::Original);

        let parsed: Config = toml::from_str("cache_limit_mib = 4096").unwrap();
        assert_eq!(parsed.cache_limit_mib, Some(4096));
    }

    #[test]
    fn every_supported_quality_maps_to_the_shared_wire_value() {
        let cases = [
            ("\"128\"", AudioQuality::Low128, 128_000),
            ("\"192\"", AudioQuality::Medium192, 192_000),
            ("\"320\"", AudioQuality::High320, 320_000),
            ("\"exhigh\"", AudioQuality::High320, 320_000),
            ("\"lossless\"", AudioQuality::Lossless, 350_000),
            ("\"hires\"", AudioQuality::HiRes, 999_000),
        ];

        for (setting, expected, wire) in cases {
            let parsed: Config = toml::from_str(&format!("quality = {setting}")).unwrap();
            assert_eq!(parsed.quality, expected);
            assert_eq!(parsed.quality.bitrate(), wire);
        }

        let numeric: Config = toml::from_str("quality = 192").unwrap();
        assert_eq!(numeric.quality, AudioQuality::Medium192);
    }
}
