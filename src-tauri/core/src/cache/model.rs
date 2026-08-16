pub use crate::media::{AudioCodec, AudioQuality, ParseAudioCodecError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    pub track_id: i64,
    pub quality: AudioQuality,
}

impl CacheKey {
    pub const fn new(track_id: i64, quality: AudioQuality) -> Self {
        Self { track_id, quality }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheWriteRequest {
    pub key: CacheKey,
    pub codec: AudioCodec,
    pub actual_bitrate: u32,
    pub expected_bytes: Option<u64>,
    pub expected_md5: Option<[u8; 16]>,
}

impl CacheWriteRequest {
    pub const fn new(key: CacheKey, codec: AudioCodec, actual_bitrate: u32) -> Self {
        Self {
            key,
            codec,
            actual_bitrate,
            expected_bytes: None,
            expected_md5: None,
        }
    }

    pub const fn with_expected_bytes(mut self, expected_bytes: u64) -> Self {
        self.expected_bytes = Some(expected_bytes);
        self
    }

    pub const fn with_expected_md5(mut self, expected_md5: [u8; 16]) -> Self {
        self.expected_md5 = Some(expected_md5);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheMetadata {
    pub key: CacheKey,
    pub codec: AudioCodec,
    pub actual_bitrate: u32,
    pub bytes: u64,
    pub sha256: [u8; 32],
    pub generation: u64,
}
