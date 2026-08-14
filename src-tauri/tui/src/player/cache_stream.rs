use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;

use stream_download::storage::StorageProvider;
use tempfile::NamedTempFile;
use yesplaymusic_core::cache::{CacheWriteRequest, TrackCache};

#[derive(Clone, Debug)]
pub struct CacheWritePlan {
    pub root: PathBuf,
    pub request: CacheWriteRequest,
}

#[derive(Debug)]
pub(super) struct CacheStreamProvider {
    owner: Arc<NamedTempFile>,
}

impl CacheStreamProvider {
    pub(super) fn new() -> io::Result<(Self, CacheImportReader)> {
        let owner = Arc::new(NamedTempFile::new()?);
        let import = CacheImportReader::new(Arc::clone(&owner))?;
        Ok((Self { owner }, import))
    }
}

impl StorageProvider for CacheStreamProvider {
    type Reader = CacheImportReader;
    type Writer = CacheImportReader;

    fn into_reader_writer(
        self,
        _content_length: Option<u64>,
    ) -> io::Result<(Self::Reader, Self::Writer)> {
        let reader = CacheImportReader::new(Arc::clone(&self.owner))?;
        let writer = CacheImportReader::new(self.owner)?;
        Ok((reader, writer))
    }
}

#[derive(Debug)]
pub(super) struct CacheImportReader {
    file: File,
    _owner: Arc<NamedTempFile>,
}

impl CacheImportReader {
    fn new(owner: Arc<NamedTempFile>) -> io::Result<Self> {
        let file = owner.reopen()?;
        Ok(Self {
            file,
            _owner: owner,
        })
    }
}

impl Read for CacheImportReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Write for CacheImportReader {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Seek for CacheImportReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

pub(super) fn publish(mut import: CacheImportReader, plan: CacheWritePlan) -> anyhow::Result<()> {
    import.seek(SeekFrom::Start(0))?;
    let cache = TrackCache::open(plan.root)?;
    let mut writer = cache.begin_write(plan.request)?;
    io::copy(&mut import, &mut writer)?;
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek, SeekFrom, Write};

    use stream_download::storage::StorageProvider;

    use super::CacheStreamProvider;

    #[test]
    fn provider_cursors_keep_independent_positions() {
        let (provider, mut import) = CacheStreamProvider::new().expect("create cache stream");
        let (mut reader, mut writer) = provider
            .into_reader_writer(None)
            .expect("open stream cursors");

        writer.write_all(b"abcdef").expect("write stream bytes");
        writer.flush().expect("flush stream bytes");

        let mut prefix = [0_u8; 2];
        reader.read_exact(&mut prefix).expect("read player cursor");
        assert_eq!(&prefix, b"ab");

        let mut whole = Vec::new();
        import.read_to_end(&mut whole).expect("read import cursor");
        assert_eq!(whole, b"abcdef");
        assert_eq!(reader.stream_position().expect("player position"), 2);
        assert_eq!(writer.stream_position().expect("writer position"), 6);

        reader.seek(SeekFrom::Start(4)).expect("seek player cursor");
        let mut suffix = Vec::new();
        reader.read_to_end(&mut suffix).expect("read suffix");
        assert_eq!(suffix, b"ef");
        assert_eq!(import.stream_position().expect("import position"), 6);
    }
}
