//! Multi-process-safe track cache: audio bytes on disk, SQLite index, LRU cap.
//! Designed from day one to be shared — the TUI uses it now, the Sidecar can
//! point at the same directory later without a migration of this layer.
//!
//! Concurrency model: WAL + busy_timeout for cross-process index access,
//! tmp + rename for atomic file writes, and lookups that self-heal rows
//! whose file vanished (the other process may have evicted it).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS tracks (
    track_id    INTEGER NOT NULL,
    level       TEXT    NOT NULL,
    ext         TEXT    NOT NULL,
    bytes       INTEGER NOT NULL,
    last_access INTEGER NOT NULL,
    PRIMARY KEY (track_id, level)
);
";

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cache I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("cache index failed: {0}")]
    Index(#[from] rusqlite::Error),
}

pub struct TrackCache {
    root: PathBuf,
    conn: Connection,
    max_bytes: u64,
}

impl TrackCache {
    pub fn open(root: impl Into<PathBuf>, max_bytes: u64) -> Result<Self, CacheError> {
        let root = root.into();
        fs::create_dir_all(root.join("tracks"))?;
        let conn = Connection::open(root.join("index.sqlite3"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            root,
            conn,
            max_bytes,
        })
    }

    /// Cache hit returns the audio file path and refreshes its LRU stamp.
    /// A row whose file was removed by another process heals to a miss.
    pub fn lookup(&self, track_id: i64, level: &str) -> Result<Option<PathBuf>, CacheError> {
        let ext: Option<String> = self
            .conn
            .query_row(
                "SELECT ext FROM tracks WHERE track_id = ?1 AND level = ?2",
                params![track_id, level],
                |row| row.get(0),
            )
            .optional()?;
        let Some(ext) = ext else { return Ok(None) };
        let path = self.track_path(track_id, level, &ext);
        if !path.is_file() {
            self.conn.execute(
                "DELETE FROM tracks WHERE track_id = ?1 AND level = ?2",
                params![track_id, level],
            )?;
            return Ok(None);
        }
        self.conn.execute(
            "UPDATE tracks SET last_access = ?3 WHERE track_id = ?1 AND level = ?2",
            params![track_id, level, unix_now()],
        )?;
        Ok(Some(path))
    }

    /// Atomically lands the audio bytes, indexes them, then trims to the cap.
    pub fn store(
        &self,
        track_id: i64,
        level: &str,
        ext: &str,
        audio: &[u8],
    ) -> Result<PathBuf, CacheError> {
        let path = self.track_path(track_id, level, ext);
        let tmp = path.with_extension(format!("{ext}.tmp-{}", std::process::id()));
        fs::write(&tmp, audio)?;
        fs::rename(&tmp, &path)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO tracks (track_id, level, ext, bytes, last_access)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![track_id, level, ext, audio.len() as i64, unix_now()],
        )?;
        self.evict_to_cap()?;
        Ok(path)
    }

    pub fn total_bytes(&self) -> Result<u64, CacheError> {
        let total: i64 = self
            .conn
            .query_row("SELECT COALESCE(SUM(bytes), 0) FROM tracks", [], |row| {
                row.get(0)
            })?;
        Ok(total.max(0) as u64)
    }

    fn evict_to_cap(&self) -> Result<(), CacheError> {
        while self.total_bytes()? > self.max_bytes {
            let oldest: Option<(i64, String, String)> = self
                .conn
                .query_row(
                    "SELECT track_id, level, ext FROM tracks
                     ORDER BY last_access ASC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((track_id, level, ext)) = oldest else {
                break;
            };
            match fs::remove_file(self.track_path(track_id, &level, &ext)) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            }
            self.conn.execute(
                "DELETE FROM tracks WHERE track_id = ?1 AND level = ?2",
                params![track_id, level],
            )?;
        }
        Ok(())
    }

    fn track_path(&self, track_id: i64, level: &str, ext: &str) -> PathBuf {
        self.root.join("tracks").join(format!("{track_id}-{level}.{ext}"))
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(max_bytes: u64) -> (tempfile::TempDir, TrackCache) {
        let dir = tempfile::tempdir().unwrap();
        let cache = TrackCache::open(dir.path().join("ypm-cache"), max_bytes).unwrap();
        (dir, cache)
    }

    #[test]
    fn stores_and_looks_up_audio() {
        let (_dir, cache) = cache(1024);
        assert!(cache.lookup(1, "exhigh").unwrap().is_none());
        let path = cache.store(1, "exhigh", "mp3", b"audio-bytes").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"audio-bytes");
        assert_eq!(cache.lookup(1, "exhigh").unwrap(), Some(path));
        assert_eq!(cache.total_bytes().unwrap(), 11);
    }

    #[test]
    fn evicts_least_recently_used_beyond_the_cap() {
        let (_dir, cache) = cache(25);
        cache.store(1, "exhigh", "mp3", &[0_u8; 10]).unwrap();
        cache.store(2, "exhigh", "mp3", &[0_u8; 10]).unwrap();
        // Age track 1 below track 2, then overflow the cap.
        cache
            .conn
            .execute("UPDATE tracks SET last_access = 1 WHERE track_id = 1", [])
            .unwrap();
        cache.store(3, "exhigh", "mp3", &[0_u8; 10]).unwrap();

        assert!(cache.lookup(1, "exhigh").unwrap().is_none());
        assert!(cache.lookup(2, "exhigh").unwrap().is_some());
        assert!(cache.lookup(3, "exhigh").unwrap().is_some());
        assert!(cache.total_bytes().unwrap() <= 25);
    }

    #[test]
    fn heals_index_rows_whose_file_was_evicted_elsewhere() {
        let (_dir, cache) = cache(1024);
        let path = cache.store(7, "lossless", "flac", b"x").unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(cache.lookup(7, "lossless").unwrap().is_none());
        // The healed row is gone; a fresh store works again.
        cache.store(7, "lossless", "flac", b"y").unwrap();
        assert!(cache.lookup(7, "lossless").unwrap().is_some());
    }

    #[test]
    fn different_levels_are_distinct_entries() {
        let (_dir, cache) = cache(1024);
        cache.store(9, "exhigh", "mp3", b"aac").unwrap();
        cache.store(9, "lossless", "flac", b"flac").unwrap();
        assert!(cache.lookup(9, "exhigh").unwrap().is_some());
        assert!(cache.lookup(9, "lossless").unwrap().is_some());
        assert_eq!(cache.total_bytes().unwrap(), 7);
    }
}
