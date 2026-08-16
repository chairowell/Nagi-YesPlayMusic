//! Audio quality and container types shared by the NCM client and the cache.

use std::fmt;
use std::str::FromStr;

/// The five quality requests understood by the NetEase audio endpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AudioQuality {
    Low128,
    Medium192,
    High320,
    Lossless,
    HiRes,
}

impl AudioQuality {
    pub const fn bitrate(self) -> u32 {
        match self {
            Self::Low128 => 128_000,
            Self::Medium192 => 192_000,
            Self::High320 => 320_000,
            Self::Lossless => 350_000,
            Self::HiRes => 999_000,
        }
    }

    pub const fn from_bitrate(bitrate: u32) -> Option<Self> {
        match bitrate {
            128_000 => Some(Self::Low128),
            192_000 => Some(Self::Medium192),
            320_000 => Some(Self::High320),
            350_000 => Some(Self::Lossless),
            999_000 => Some(Self::HiRes),
            _ => None,
        }
    }
}

/// Audio containers currently returned by the NetEase and UNM playback paths.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AudioCodec {
    Mp3,
    Flac,
    Aac,
    M4a,
}

impl AudioCodec {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::M4a => "m4a",
        }
    }

    #[cfg(feature = "cache")]
    pub(crate) const fn database_value(self) -> i64 {
        match self {
            Self::Mp3 => 1,
            Self::Flac => 2,
            Self::Aac => 3,
            Self::M4a => 4,
        }
    }

    #[cfg(feature = "cache")]
    pub(crate) const fn from_database_value(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Mp3),
            2 => Some(Self::Flac),
            3 => Some(Self::Aac),
            4 => Some(Self::M4a),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseAudioCodecError;

impl fmt::Display for ParseAudioCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unsupported audio codec")
    }
}

impl std::error::Error for ParseAudioCodecError {}

impl FromStr for AudioCodec {
    type Err = ParseAudioCodecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "mp3" => Ok(Self::Mp3),
            "flac" => Ok(Self::Flac),
            "aac" => Ok(Self::Aac),
            "m4a" => Ok(Self::M4a),
            _ => Err(ParseAudioCodecError),
        }
    }
}
