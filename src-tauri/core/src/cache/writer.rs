use std::io::{self, Write};

use md5::Md5;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use super::{CacheError, CacheMetadata, CacheWriteRequest, TrackCache};

pub struct CacheWriter<'cache> {
    cache: &'cache TrackCache,
    staging: NamedTempFile,
    request: CacheWriteRequest,
    bytes: u64,
    sha256: Sha256,
    md5: Md5,
}

pub(crate) struct CompletedWrite {
    pub staging: NamedTempFile,
    pub metadata: CacheMetadata,
}

impl<'cache> CacheWriter<'cache> {
    pub(crate) fn new(
        cache: &'cache TrackCache,
        staging: NamedTempFile,
        request: CacheWriteRequest,
    ) -> Self {
        Self {
            cache,
            staging,
            request,
            bytes: 0,
            sha256: Sha256::new(),
            md5: Md5::new(),
        }
    }

    /// Validates and durably publishes the completed stream.
    pub fn finish(mut self) -> Result<CacheMetadata, CacheError> {
        self.staging.as_file_mut().flush()?;

        if let Some(expected) = self.request.expected_bytes {
            if expected != self.bytes {
                return Err(CacheError::LengthMismatch {
                    expected,
                    actual: self.bytes,
                });
            }
        }

        let actual_md5: [u8; 16] = self.md5.finalize().into();
        if let Some(expected) = self.request.expected_md5 {
            if expected != actual_md5 {
                return Err(CacheError::Md5Mismatch {
                    expected,
                    actual: actual_md5,
                });
            }
        }
        self.staging.as_file().sync_all()?;

        let metadata = CacheMetadata {
            key: self.request.key,
            codec: self.request.codec,
            actual_bitrate: self.request.actual_bitrate,
            bytes: self.bytes,
            sha256: self.sha256.finalize().into(),
            generation: 0,
        };
        let cache = self.cache;
        cache.commit_write(CompletedWrite {
            staging: self.staging,
            metadata,
        })
    }
}

impl Write for CacheWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.staging.as_file_mut().write(buffer)?;
        let written_bytes = &buffer[..written];
        self.sha256.update(written_bytes);
        self.md5.update(written_bytes);
        self.bytes = self
            .bytes
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("cache entry length overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.staging.as_file_mut().flush()
    }
}
