//! SQLite-indexed audio cache shared by local YesPlayMusic processes.
//!
//! SQLite serializes publication and eviction. OS file locks keep an opened
//! lease readable while another process trims the cache.

mod model;
mod writer;

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use model::{
    AudioCodec, AudioQuality, CacheKey, CacheMetadata, CacheWriteRequest, ParseAudioCodecError,
};
pub use writer::CacheWriter;

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use tempfile::{Builder as TempFileBuilder, NamedTempFile};
use writer::CompletedWrite;

pub const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;

const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
DROP TABLE IF EXISTS tracks;
DROP TABLE IF EXISTS cache_policy;

CREATE TABLE tracks (
    track_id       INTEGER NOT NULL,
    quality        INTEGER NOT NULL CHECK (quality IN (128000, 192000, 320000, 350000, 999000)),
    codec          INTEGER NOT NULL CHECK (codec IN (1, 2, 3, 4)),
    actual_bitrate INTEGER NOT NULL CHECK (actual_bitrate > 0 AND actual_bitrate <= 4294967295),
    bytes          INTEGER NOT NULL CHECK (bytes >= 0),
    sha256         BLOB    NOT NULL CHECK (length(sha256) = 32),
    generation     INTEGER NOT NULL CHECK (generation > 0),
    last_access    INTEGER NOT NULL,
    PRIMARY KEY (track_id, quality)
) WITHOUT ROWID;

CREATE TABLE cache_policy (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    max_bytes INTEGER NOT NULL CHECK (max_bytes >= 0),
    next_generation INTEGER NOT NULL CHECK (next_generation >= 0)
);

INSERT INTO cache_policy (singleton, max_bytes, next_generation) VALUES (1, 8589934592, 0);
PRAGMA user_version = 1;
";

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cache I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("cache index failed: {0}")]
    Index(#[from] rusqlite::Error),
    #[error("cache schema version {0} is newer than this build supports")]
    UnsupportedSchema(i64),
    #[error("cache index contains invalid {0}")]
    CorruptIndex(&'static str),
    #[error("actual bitrate must be greater than zero")]
    InvalidBitrate,
    #[error("{field} value {value} exceeds SQLite's integer range")]
    ValueTooLarge { field: &'static str, value: u64 },
    #[error("cache entry generation is exhausted")]
    GenerationExhausted,
    #[error("audio length mismatch: expected {expected} bytes, received {actual}")]
    LengthMismatch { expected: u64, actual: u64 },
    #[error("audio MD5 mismatch: expected {expected:02x?}, received {actual:02x?}")]
    Md5Mismatch {
        expected: [u8; 16],
        actual: [u8; 16],
    },
    #[error("the content-addressed file is currently leased")]
    ExistingFileLeased,
}

#[derive(Debug)]
pub struct CacheLease {
    file: File,
    metadata: CacheMetadata,
}

impl CacheLease {
    pub const fn metadata(&self) -> &CacheMetadata {
        &self.metadata
    }
}

impl Read for CacheLease {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Seek for CacheLease {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

pub struct TrackCache {
    root: PathBuf,
    conn: Connection,
}

impl TrackCache {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, CacheError> {
        let root = root.into();
        fs::create_dir_all(root.join("tracks"))?;
        fs::create_dir_all(root.join("staging"))?;

        let initialization_lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("open.lock"))?;
        File::lock(&initialization_lock)?;

        let mut conn = Connection::open(root.join("index.sqlite3"))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        migrate(&mut conn)?;
        drop(initialization_lock);

        let cache = Self { root, conn };
        cache.reconcile()?;
        cache.evict_to_cap()?;
        Ok(cache)
    }

    pub fn begin_write(&self, request: CacheWriteRequest) -> Result<CacheWriter<'_>, CacheError> {
        if request.actual_bitrate == 0 {
            return Err(CacheError::InvalidBitrate);
        }

        // Creation and locking are serialized with reconciliation so a fresh
        // staging file cannot be mistaken for a crashed writer.
        let transaction = self.immediate_transaction()?;
        let staging = TempFileBuilder::new()
            .prefix("track-")
            .suffix(".part")
            .tempfile_in(self.staging_dir())?;
        File::lock(staging.as_file())?;
        transaction.commit()?;
        Ok(CacheWriter::new(self, staging, request))
    }

    pub fn lookup(&self, key: CacheKey) -> Result<Option<CacheLease>, CacheError> {
        loop {
            let Some(entry) = query_entry(&self.conn, key)? else {
                return Ok(None);
            };
            let path = self.entry_path(&entry.metadata);
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if self.delete_row_if_current(&entry)? {
                        return Ok(None);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            File::lock_shared(&file)?;
            if file.metadata()?.len() != entry.metadata.bytes {
                drop(file);
                if self.invalidate(&entry.metadata)? {
                    return Ok(None);
                }
                let current = query_entry(&self.conn, key)?;
                if current
                    .as_ref()
                    .is_some_and(|current| current.generation != entry.generation)
                {
                    continue;
                }
                return Ok(None);
            }

            let refreshed = self.conn.execute(
                "UPDATE tracks SET last_access = ?4
                 WHERE track_id = ?1 AND quality = ?2 AND generation = ?3",
                params![
                    key.track_id,
                    i64::from(key.quality.bitrate()),
                    entry.generation,
                    unix_now_ns()
                ],
            )?;
            if refreshed == 1 {
                return Ok(Some(CacheLease {
                    file,
                    metadata: entry.metadata,
                }));
            }
        }
    }

    pub fn max_bytes(&self) -> Result<u64, CacheError> {
        let value: i64 = self.conn.query_row(
            "SELECT max_bytes FROM cache_policy WHERE singleton = 1",
            [],
            |row| row.get(0),
        )?;
        u64::try_from(value).map_err(|_| CacheError::CorruptIndex("max_bytes"))
    }

    pub fn set_max_bytes(&self, max_bytes: u64) -> Result<(), CacheError> {
        let max_bytes = sqlite_integer("max_bytes", max_bytes)?;
        let transaction = self.immediate_transaction()?;
        transaction.execute(
            "UPDATE cache_policy SET max_bytes = ?1 WHERE singleton = 1",
            [max_bytes],
        )?;
        transaction.commit()?;
        self.evict_to_cap()
    }

    pub fn total_bytes(&self) -> Result<u64, CacheError> {
        let total: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(bytes), 0) FROM tracks", [], |row| {
                    row.get(0)
                })?;
        u64::try_from(total).map_err(|_| CacheError::CorruptIndex("total bytes"))
    }

    /// Removes exactly the generation observed by a decoder.
    ///
    /// Drop the corresponding [`CacheLease`] before calling this method.
    /// `false` means the entry was replaced, was already absent, or another
    /// lease still has the same file open. Retrying after leases are dropped
    /// cannot remove a newer generation.
    pub fn invalidate(&self, metadata: &CacheMetadata) -> Result<bool, CacheError> {
        let generation = sqlite_integer("generation", metadata.generation)?;
        let transaction = self.immediate_transaction()?;
        let Some(current) = query_entry(&transaction, metadata.key)? else {
            transaction.commit()?;
            return Ok(false);
        };
        if current.generation != generation || current.metadata.sha256 != metadata.sha256 {
            transaction.commit()?;
            return Ok(false);
        }

        let path = self.entry_path(&current.metadata);
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(file) = file.as_ref() {
            if !try_lock_exclusive(file)? {
                transaction.commit()?;
                return Ok(false);
            }
        }

        let deleted = transaction.execute(
            "DELETE FROM tracks
             WHERE track_id = ?1 AND quality = ?2 AND generation = ?3 AND sha256 = ?4",
            params![
                metadata.key.track_id,
                i64::from(metadata.key.quality.bitrate()),
                generation,
                &metadata.sha256[..]
            ],
        )? == 1;
        if deleted && file.is_some() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            sync_directory(self.tracks_dir())?;
        }
        transaction.commit()?;
        Ok(deleted)
    }

    pub(crate) fn commit_write(
        &self,
        completed: CompletedWrite,
    ) -> Result<CacheMetadata, CacheError> {
        let mut metadata = completed.metadata;
        let bytes = sqlite_integer("entry bytes", metadata.bytes)?;
        let final_path = self.entry_path(&metadata);
        let transaction = self.immediate_transaction()?;
        let previous = query_entry(&transaction, metadata.key)?;
        let replaced_file = match previous.as_ref() {
            Some(previous) if self.entry_path(&previous.metadata) != final_path => {
                let path = self.entry_path(&previous.metadata);
                let file = match OpenOptions::new().read(true).write(true).open(&path) {
                    Ok(file) => Some(file),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                if let Some(file) = file.as_ref() {
                    if !try_lock_exclusive(file)? {
                        return Err(CacheError::ExistingFileLeased);
                    }
                }
                file.map(|file| (path, file))
            }
            _ => None,
        };
        let generation = allocate_generation(&transaction)?;
        metadata.generation =
            u64::try_from(generation).map_err(|_| CacheError::CorruptIndex("generation"))?;

        let published = publish(
            completed.staging,
            &final_path,
            metadata.bytes,
            metadata.sha256,
        )?;
        sync_directory(self.tracks_dir())?;
        sync_directory(self.staging_dir())?;

        #[cfg(test)]
        if std::env::var_os("YPM_CACHE_TEST_CRASH_AFTER_PUBLISH").is_some() {
            std::process::exit(86);
        }

        transaction.execute(
            "INSERT INTO tracks
                (track_id, quality, codec, actual_bitrate, bytes, sha256, generation, last_access)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(track_id, quality) DO UPDATE SET
                codec = excluded.codec,
                actual_bitrate = excluded.actual_bitrate,
                bytes = excluded.bytes,
                sha256 = excluded.sha256,
                generation = excluded.generation,
                last_access = excluded.last_access",
            params![
                metadata.key.track_id,
                i64::from(metadata.key.quality.bitrate()),
                metadata.codec.database_value(),
                i64::from(metadata.actual_bitrate),
                bytes,
                &metadata.sha256[..],
                generation,
                unix_now_ns(),
            ],
        )?;
        transaction.commit()?;
        drop(published);

        if let Some((path, locked_file)) = replaced_file {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            drop(locked_file);
            sync_directory(self.tracks_dir())?;
        }
        self.evict_to_cap()?;
        Ok(metadata)
    }

    fn reconcile(&self) -> Result<(), CacheError> {
        let transaction = self.immediate_transaction()?;
        self.remove_abandoned_staging()?;

        let entries = query_all_entries(&transaction)?;
        let mut referenced = HashSet::with_capacity(entries.len());
        for entry in entries {
            let path = self.entry_path(&entry.metadata);
            match fs::metadata(&path) {
                Ok(file_metadata) if file_metadata.len() == entry.metadata.bytes => {
                    referenced.insert(path);
                }
                Ok(_) => {
                    if remove_file_if_unlocked(&path)? {
                        delete_entry_cas(&transaction, &entry)?;
                    } else {
                        referenced.insert(path);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    delete_entry_cas(&transaction, &entry)?;
                }
                Err(error) => return Err(error.into()),
            }
        }

        for item in fs::read_dir(self.tracks_dir())? {
            let item = item?;
            if item.file_type()?.is_file() && !referenced.contains(&item.path()) {
                remove_file_if_unlocked(&item.path())?;
            }
        }
        sync_directory(self.tracks_dir())?;
        sync_directory(self.staging_dir())?;
        transaction.commit()?;
        Ok(())
    }

    fn remove_abandoned_staging(&self) -> Result<(), CacheError> {
        for item in fs::read_dir(self.staging_dir())? {
            let item = item?;
            if item.file_type()?.is_file() {
                remove_file_if_unlocked(&item.path())?;
            }
        }
        Ok(())
    }

    fn evict_to_cap(&self) -> Result<(), CacheError> {
        loop {
            let transaction = self.immediate_transaction()?;
            let max_bytes: i64 = transaction.query_row(
                "SELECT max_bytes FROM cache_policy WHERE singleton = 1",
                [],
                |row| row.get(0),
            )?;
            let total_bytes: i64 =
                transaction.query_row("SELECT COALESCE(SUM(bytes), 0) FROM tracks", [], |row| {
                    row.get(0)
                })?;
            if total_bytes <= max_bytes {
                transaction.commit()?;
                return Ok(());
            }

            let candidates = query_all_entries_by_age(&transaction)?;
            let mut removed = false;
            for entry in candidates {
                let path = self.entry_path(&entry.metadata);
                let file = match OpenOptions::new().read(true).write(true).open(&path) {
                    Ok(file) => Some(file),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };

                if let Some(file) = file.as_ref() {
                    if !try_lock_exclusive(file)? {
                        continue;
                    }
                }
                if delete_entry_cas(&transaction, &entry)? == 0 {
                    continue;
                }
                if file.is_some() {
                    match fs::remove_file(&path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                removed = true;
                break;
            }

            sync_directory(self.tracks_dir())?;
            transaction.commit()?;
            if !removed {
                return Ok(());
            }
        }
    }

    fn delete_row_if_current(&self, entry: &IndexedEntry) -> Result<bool, CacheError> {
        let transaction = self.immediate_transaction()?;
        let deleted = delete_entry_cas(&transaction, entry)? == 1;
        transaction.commit()?;
        Ok(deleted)
    }

    fn immediate_transaction(&self) -> Result<Transaction<'_>, CacheError> {
        Ok(Transaction::new_unchecked(
            &self.conn,
            TransactionBehavior::Immediate,
        )?)
    }

    fn tracks_dir(&self) -> PathBuf {
        self.root.join("tracks")
    }

    fn staging_dir(&self) -> PathBuf {
        self.root.join("staging")
    }

    fn entry_path(&self, metadata: &CacheMetadata) -> PathBuf {
        let hash = encode_hex(&metadata.sha256);
        self.tracks_dir().join(format!(
            "{}-{}-{hash}.{}",
            metadata.key.track_id,
            metadata.key.quality.bitrate(),
            metadata.codec.extension()
        ))
    }
}

#[derive(Clone, Debug)]
struct IndexedEntry {
    metadata: CacheMetadata,
    generation: i64,
}

#[derive(Debug)]
struct RawEntry {
    track_id: i64,
    quality: i64,
    codec: i64,
    actual_bitrate: i64,
    bytes: i64,
    sha256: Vec<u8>,
    generation: i64,
}

fn migrate(conn: &mut Connection) -> Result<(), CacheError> {
    let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => transaction.execute_batch(SCHEMA)?,
        SCHEMA_VERSION => {}
        other => return Err(CacheError::UnsupportedSchema(other)),
    }
    transaction.commit()?;
    Ok(())
}

fn query_entry(conn: &Connection, key: CacheKey) -> Result<Option<IndexedEntry>, CacheError> {
    let raw = conn
        .query_row(
            "SELECT track_id, quality, codec, actual_bitrate, bytes, sha256, generation
             FROM tracks WHERE track_id = ?1 AND quality = ?2",
            params![key.track_id, i64::from(key.quality.bitrate())],
            raw_entry_from_row,
        )
        .optional()?;
    raw.map(decode_entry).transpose()
}

fn query_all_entries(conn: &Connection) -> Result<Vec<IndexedEntry>, CacheError> {
    query_entries(conn, "")
}

fn query_all_entries_by_age(conn: &Connection) -> Result<Vec<IndexedEntry>, CacheError> {
    query_entries(conn, " ORDER BY last_access ASC, track_id ASC, quality ASC")
}

fn query_entries(conn: &Connection, suffix: &str) -> Result<Vec<IndexedEntry>, CacheError> {
    let sql = format!(
        "SELECT track_id, quality, codec, actual_bitrate, bytes, sha256, generation \
         FROM tracks{suffix}"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], raw_entry_from_row)?;
    rows.map(|row| decode_entry(row?)).collect()
}

fn raw_entry_from_row(row: &Row<'_>) -> rusqlite::Result<RawEntry> {
    Ok(RawEntry {
        track_id: row.get(0)?,
        quality: row.get(1)?,
        codec: row.get(2)?,
        actual_bitrate: row.get(3)?,
        bytes: row.get(4)?,
        sha256: row.get(5)?,
        generation: row.get(6)?,
    })
}

fn decode_entry(raw: RawEntry) -> Result<IndexedEntry, CacheError> {
    let quality = u32::try_from(raw.quality)
        .ok()
        .and_then(AudioQuality::from_bitrate)
        .ok_or(CacheError::CorruptIndex("quality"))?;
    let codec =
        AudioCodec::from_database_value(raw.codec).ok_or(CacheError::CorruptIndex("codec"))?;
    let actual_bitrate =
        u32::try_from(raw.actual_bitrate).map_err(|_| CacheError::CorruptIndex("bitrate"))?;
    let bytes = u64::try_from(raw.bytes).map_err(|_| CacheError::CorruptIndex("entry bytes"))?;
    let sha256: [u8; 32] = raw
        .sha256
        .try_into()
        .map_err(|_| CacheError::CorruptIndex("SHA-256"))?;
    if raw.generation <= 0 {
        return Err(CacheError::CorruptIndex("generation"));
    }

    Ok(IndexedEntry {
        metadata: CacheMetadata {
            key: CacheKey::new(raw.track_id, quality),
            codec,
            actual_bitrate,
            bytes,
            sha256,
            generation: u64::try_from(raw.generation)
                .map_err(|_| CacheError::CorruptIndex("generation"))?,
        },
        generation: raw.generation,
    })
}

fn delete_entry_cas(conn: &Connection, entry: &IndexedEntry) -> Result<usize, CacheError> {
    Ok(conn.execute(
        "DELETE FROM tracks WHERE track_id = ?1 AND quality = ?2 AND generation = ?3",
        params![
            entry.metadata.key.track_id,
            i64::from(entry.metadata.key.quality.bitrate()),
            entry.generation
        ],
    )?)
}

fn allocate_generation(conn: &Connection) -> Result<i64, CacheError> {
    let current: i64 = conn.query_row(
        "SELECT next_generation FROM cache_policy WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    let next = current
        .checked_add(1)
        .ok_or(CacheError::GenerationExhausted)?;
    conn.execute(
        "UPDATE cache_policy SET next_generation = ?1 WHERE singleton = 1",
        [next],
    )?;
    Ok(next)
}

fn publish(
    staging: NamedTempFile,
    final_path: &Path,
    expected_bytes: u64,
    expected_sha256: [u8; 32],
) -> Result<File, CacheError> {
    let mut staging = staging;
    loop {
        match staging.persist_noclobber(final_path) {
            Ok(file) => return Ok(file),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                staging = error.file;
                let mut existing = match OpenOptions::new().read(true).write(true).open(final_path)
                {
                    Ok(file) => file,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                if !try_lock_exclusive(&existing)? {
                    return Err(CacheError::ExistingFileLeased);
                }
                if existing.metadata()?.len() == expected_bytes
                    && hash_file(&mut existing)? == expected_sha256
                {
                    drop(staging);
                    return Ok(existing);
                }
                fs::remove_file(final_path)?;
                drop(existing);
                if let Some(parent) = final_path.parent() {
                    sync_directory(parent.to_owned())?;
                }
            }
            Err(error) => return Err(error.error.into()),
        }
    }
}

fn hash_file(file: &mut File) -> Result<[u8; 32], CacheError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn remove_file_if_unlocked(path: &Path) -> Result<bool, CacheError> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !try_lock_exclusive(&file)? {
        return Ok(false);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn try_lock_exclusive(file: &File) -> Result<bool, CacheError> {
    match File::try_lock(file) {
        Ok(()) => Ok(true),
        Err(error) => {
            let error: io::Error = error.into();
            if error.kind() == io::ErrorKind::WouldBlock {
                Ok(false)
            } else {
                Err(error.into())
            }
        }
    }
}

fn sqlite_integer(field: &'static str, value: u64) -> Result<i64, CacheError> {
    i64::try_from(value).map_err(|_| CacheError::ValueTooLarge { field, value })
}

fn unix_now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(unix)]
fn sync_directory(path: PathBuf) -> Result<(), CacheError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: PathBuf) -> Result<(), CacheError> {
    Ok(())
}

#[cfg(test)]
mod tests;
