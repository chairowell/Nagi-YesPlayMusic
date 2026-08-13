use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::api::SongRow;

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredSong {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub pic_url: Option<String>,
}

impl From<&SongRow> for StoredSong {
    fn from(row: &SongRow) -> Self {
        Self {
            id: row.id,
            title: row.title.clone(),
            artist: row.artist.clone(),
            duration_ms: row.duration_ms,
            pic_url: row.pic_url.clone(),
        }
    }
}

impl StoredSong {
    pub fn into_song_row(self) -> SongRow {
        SongRow {
            id: self.id,
            title: self.title,
            artist: self.artist,
            duration_ms: self.duration_ms,
            pic_url: self.pic_url,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LibraryStore {
    root: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    saved_at_unix: u64,
    rows: Vec<StoredSong>,
}

impl LibraryStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn load(&self, uid: i64, source: &str) -> Option<Vec<StoredSong>> {
        self.read_snapshot(uid, source)
            .map(|snapshot| snapshot.rows)
    }

    pub fn save(&self, uid: i64, source: &str, rows: &[StoredSong]) -> io::Result<()> {
        validate_source(source)?;
        fs::create_dir_all(&self.root)?;

        let snapshot = Snapshot {
            version: SNAPSHOT_VERSION,
            saved_at_unix: unix_now()?,
            rows: rows.to_vec(),
        };
        let bytes = serde_json::to_vec(&snapshot).map_err(io::Error::other)?;
        let path = self.snapshot_path(uid, source);
        let temporary = self.temporary_path(uid, source);
        fs::write(&temporary, bytes)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
        Ok(())
    }

    fn read_snapshot(&self, uid: i64, source: &str) -> Option<Snapshot> {
        validate_source(source).ok()?;
        let bytes = fs::read(self.snapshot_path(uid, source)).ok()?;
        let snapshot: Snapshot = serde_json::from_slice(&bytes).ok()?;
        (snapshot.version == SNAPSHOT_VERSION).then_some(snapshot)
    }

    fn snapshot_path(&self, uid: i64, source: &str) -> PathBuf {
        self.root.join(format!("{uid}-{source}.json"))
    }

    fn temporary_path(&self, uid: i64, source: &str) -> PathBuf {
        self.root.join(format!("{uid}-{source}.json.tmp"))
    }
}

fn validate_source(source: &str) -> io::Result<()> {
    if !source.is_empty() && source.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "library source must contain only lowercase ASCII letters",
    ))
}

fn unix_now() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;

    use tempfile::tempdir;

    use super::{LibraryStore, StoredSong};
    use crate::api::SongRow;

    #[test]
    fn roundtrips_rows_and_song_row_conversion() {
        let directory = tempdir().unwrap();
        let store = LibraryStore::new(directory.path().join("library"));
        let source = SongRow {
            id: 42,
            title: "晚风经过月台".into(),
            artist: "遠い灯".into(),
            duration_ms: 218_000,
            pic_url: Some("https://example.test/cover.jpg".into()),
        };
        let stored = StoredSong::from(&source);

        store
            .save(7, "liked", std::slice::from_ref(&stored))
            .unwrap();
        let loaded = store.load(7, "liked").unwrap();

        assert_eq!(loaded, vec![stored.clone()]);
        let restored = stored.into_song_row();
        assert_eq!(restored.id, source.id);
        assert_eq!(restored.title, source.title);
        assert_eq!(restored.artist, source.artist);
        assert_eq!(restored.duration_ms, source.duration_ms);
        assert_eq!(restored.pic_url, source.pic_url);
    }

    #[test]
    fn damaged_json_returns_none() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("library");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("9-daily.json"), b"{not json").unwrap();
        let store = LibraryStore::new(root);

        assert_eq!(store.load(9, "daily"), None);
    }

    #[test]
    fn unsupported_version_returns_none() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("library");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("11-cloud.json"),
            br#"{"version":0,"saved_at_unix":1,"rows":[]}"#,
        )
        .unwrap();
        let store = LibraryStore::new(root);

        assert_eq!(store.load(11, "cloud"), None);
    }

    #[test]
    fn save_creates_the_root_directory() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("nested/library");
        let store = LibraryStore::new(root.clone());

        store.save(13, "liked", &[]).unwrap();

        assert!(root.join("13-liked.json").is_file());
        assert_eq!(store.load(13, "liked"), Some(Vec::new()));
    }

    #[test]
    fn atomic_save_leaves_no_temporary_file() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("library");
        let store = LibraryStore::new(root.clone());

        store.save(15, "daily", &[song(1)]).unwrap();
        store.save(15, "daily", &[song(2)]).unwrap();

        assert!(root.join("15-daily.json").is_file());
        assert!(!root.join("15-daily.json.tmp").exists());
        assert_eq!(store.load(15, "daily"), Some(vec![song(2)]));
    }

    #[test]
    fn invalid_source_is_rejected_without_path_traversal() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("library");
        let store = LibraryStore::new(root.clone());

        let error = store.save(17, "../x", &[song(1)]).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
        assert_eq!(store.load(17, "../x"), None);
        assert!(!root.exists());
        assert!(!directory.path().join("x.json").exists());
    }

    fn song(id: i64) -> StoredSong {
        StoredSong {
            id,
            title: format!("Track {id}"),
            artist: "Artist".into(),
            duration_ms: 180_000,
            pic_url: None,
        }
    }
}
