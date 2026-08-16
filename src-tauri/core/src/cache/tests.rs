use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use super::*;

const CHILD_MODE: &str = "YPM_CACHE_TEST_CHILD_MODE";
const CHILD_ROOT: &str = "YPM_CACHE_TEST_CHILD_ROOT";
const CRASH_AFTER_PUBLISH: &str = "YPM_CACHE_TEST_CRASH_AFTER_PUBLISH";

fn request(track_id: i64, quality: AudioQuality) -> CacheWriteRequest {
    CacheWriteRequest::new(CacheKey::new(track_id, quality), AudioCodec::Mp3, 320_000)
}

fn write_entry(cache: &TrackCache, request: CacheWriteRequest, audio: &[u8]) -> CacheMetadata {
    let mut writer = cache.begin_write(request).unwrap();
    for chunk in audio.chunks(7) {
        writer.write_all(chunk).unwrap();
    }
    writer.finish().unwrap()
}

fn read_entry(cache: &TrackCache, key: CacheKey) -> Option<Vec<u8>> {
    let mut lease = cache.lookup(key).unwrap()?;
    let mut audio = Vec::new();
    lease.read_to_end(&mut audio).unwrap();
    Some(audio)
}

#[test]
fn empty_streams_are_rejected_instead_of_cached() {
    let directory = tempfile::tempdir().unwrap();
    let cache = TrackCache::open(directory.path().join("cache")).unwrap();
    let key = CacheKey::new(7, AudioQuality::High320);
    let writer = cache
        .begin_write(CacheWriteRequest::new(key, AudioCodec::Mp3, 320_000))
        .unwrap();
    assert!(matches!(writer.finish(), Err(CacheError::EmptyEntry)));
    assert!(cache.lookup(key).unwrap().is_none());
}

#[test]
fn quality_values_and_stream_integrity_are_part_of_the_public_contract() {
    assert_eq!(AudioQuality::Low128.bitrate(), 128_000);
    assert_eq!(AudioQuality::Medium192.bitrate(), 192_000);
    assert_eq!(AudioQuality::High320.bitrate(), 320_000);
    assert_eq!(AudioQuality::Lossless.bitrate(), 350_000);
    assert_eq!(AudioQuality::HiRes.bitrate(), 999_000);
    assert_eq!(
        AudioQuality::from_bitrate(999_000),
        Some(AudioQuality::HiRes)
    );
    assert_eq!(AudioQuality::from_bitrate(128), None);

    let directory = tempfile::tempdir().unwrap();
    let cache = TrackCache::open(directory.path().join("cache")).unwrap();
    let key = CacheKey::new(42, AudioQuality::High320);
    let expected_md5 = [
        0x5d, 0x41, 0x40, 0x2a, 0xbc, 0x4b, 0x2a, 0x76, 0xb9, 0x71, 0x9d, 0x91, 0x10, 0x17, 0xc5,
        0x92,
    ];
    let valid_request = CacheWriteRequest::new(key, AudioCodec::Mp3, 317_000)
        .with_expected_bytes(5)
        .with_expected_md5(expected_md5);
    let mut writer = cache.begin_write(valid_request).unwrap();
    writer.write_all(b"he").unwrap();
    writer.write_all(b"llo").unwrap();
    let metadata = writer.finish().unwrap();

    assert_eq!(metadata.bytes, 5);
    assert_eq!(metadata.actual_bitrate, 317_000);
    assert_eq!(
        encode_hex(&metadata.sha256),
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );

    let mut lease = cache.lookup(key).unwrap().unwrap();
    assert_eq!(lease.metadata(), &metadata);
    let mut prefix = [0_u8; 2];
    lease.read_exact(&mut prefix).unwrap();
    assert_eq!(&prefix, b"he");
    lease.seek(SeekFrom::Start(0)).unwrap();
    let mut audio = Vec::new();
    lease.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, b"hello");

    let rejected_key = CacheKey::new(43, AudioQuality::High320);
    let rejected_request = request(43, AudioQuality::High320)
        .with_expected_bytes(5)
        .with_expected_md5([0; 16]);
    let mut rejected = cache.begin_write(rejected_request).unwrap();
    rejected.write_all(b"hello").unwrap();
    assert!(matches!(
        rejected.finish(),
        Err(CacheError::Md5Mismatch { .. })
    ));
    assert!(cache.lookup(rejected_key).unwrap().is_none());
}

#[test]
fn separate_connections_share_one_capacity_policy() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let first = TrackCache::open(&root).unwrap();
    let second = TrackCache::open(&root).unwrap();
    assert_eq!(first.max_bytes().unwrap(), DEFAULT_MAX_BYTES);

    first.set_max_bytes(6).unwrap();
    assert_eq!(second.max_bytes().unwrap(), 6);
    let older = CacheKey::new(1, AudioQuality::High320);
    let newer = CacheKey::new(2, AudioQuality::High320);
    write_entry(&first, request(1, AudioQuality::High320), b"1111");
    write_entry(&second, request(2, AudioQuality::High320), b"2222");

    assert!(first.lookup(older).unwrap().is_none());
    assert_eq!(read_entry(&second, newer), Some(b"2222".to_vec()));
    assert_eq!(first.total_bytes().unwrap(), 4);
    assert_eq!(TrackCache::open(&root).unwrap().max_bytes().unwrap(), 6);
}

#[test]
fn an_open_lease_blocks_eviction_until_the_reader_drops_it() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let writer_cache = TrackCache::open(&root).unwrap();
    let evictor_cache = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(7, AudioQuality::Lossless);
    write_entry(&writer_cache, request(7, AudioQuality::Lossless), b"leased");
    let mut lease = writer_cache.lookup(key).unwrap().unwrap();

    evictor_cache.set_max_bytes(0).unwrap();
    assert_eq!(evictor_cache.total_bytes().unwrap(), 6);
    let mut audio = Vec::new();
    lease.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, b"leased");

    drop(lease);
    evictor_cache.set_max_bytes(0).unwrap();
    assert_eq!(evictor_cache.total_bytes().unwrap(), 0);
    assert!(writer_cache.lookup(key).unwrap().is_none());
}

#[test]
fn replacing_a_leased_generation_preserves_the_old_entry_until_retry() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let first = TrackCache::open(&root).unwrap();
    let second = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(70, AudioQuality::High320);
    let original = write_entry(&first, request(70, AudioQuality::High320), b"original");
    let mut lease = first.lookup(key).unwrap().unwrap();

    let mut blocked = second
        .begin_write(request(70, AudioQuality::High320))
        .unwrap();
    blocked.write_all(b"replacement").unwrap();
    assert!(matches!(
        blocked.finish(),
        Err(CacheError::ExistingFileLeased)
    ));

    let current = second.lookup(key).unwrap().unwrap();
    assert_eq!(current.metadata(), &original);
    drop(current);
    assert_eq!(file_count(&root.join("tracks")), 1);
    let mut audio = Vec::new();
    lease.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, b"original");

    drop(lease);
    let replacement = write_entry(&second, request(70, AudioQuality::High320), b"replacement");
    assert!(replacement.generation > original.generation);
    assert_eq!(read_entry(&first, key), Some(b"replacement".to_vec()));
    assert_eq!(file_count(&root.join("tracks")), 1);
}

#[test]
fn an_identical_write_publishes_while_the_same_track_is_playing() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let first = TrackCache::open(&root).unwrap();
    let second = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(73, AudioQuality::High320);
    let original = write_entry(&first, request(73, AudioQuality::High320), b"same audio");
    let mut lease = first.lookup(key).unwrap().unwrap();

    let published = write_entry(&second, request(73, AudioQuality::High320), b"same audio");
    assert!(published.generation > original.generation);
    assert_eq!(file_count(&root.join("tracks")), 1);

    let mut audio = Vec::new();
    lease.read_to_end(&mut audio).unwrap();
    assert_eq!(audio, b"same audio");
    drop(lease);
    assert_eq!(read_entry(&second, key), Some(b"same audio".to_vec()));
}

#[test]
fn a_future_schema_version_is_rejected_without_touching_the_data() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let cache = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(74, AudioQuality::High320);
    write_entry(&cache, request(74, AudioQuality::High320), b"kept");
    drop(cache);

    let index = Connection::open(root.join("index.sqlite3")).unwrap();
    index.pragma_update(None, "user_version", 2).unwrap();
    drop(index);
    assert!(matches!(
        TrackCache::open(&root),
        Err(CacheError::UnsupportedSchema(2))
    ));

    let index = Connection::open(root.join("index.sqlite3")).unwrap();
    index.pragma_update(None, "user_version", 1).unwrap();
    drop(index);
    let reopened = TrackCache::open(&root).unwrap();
    assert_eq!(read_entry(&reopened, key), Some(b"kept".to_vec()));
}

#[test]
fn invalidation_is_generation_cas_and_waits_for_active_leases() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let first = TrackCache::open(&root).unwrap();
    let second = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(71, AudioQuality::High320);

    let old = write_entry(&first, request(71, AudioQuality::High320), b"same");
    let new = write_entry(&second, request(71, AudioQuality::High320), b"same");
    assert!(new.generation > old.generation);
    assert!(!first.invalidate(&old).unwrap());
    assert_eq!(read_entry(&first, key), Some(b"same".to_vec()));

    let mut wrong_hash = new;
    wrong_hash.sha256[0] ^= 0xff;
    assert!(!first.invalidate(&wrong_hash).unwrap());

    let lease = first.lookup(key).unwrap().unwrap();
    let observed = *lease.metadata();
    assert!(!second.invalidate(&observed).unwrap());
    assert_eq!(read_entry(&second, key), Some(b"same".to_vec()));

    drop(lease);
    assert!(second.invalidate(&observed).unwrap());
    assert!(first.lookup(key).unwrap().is_none());
}

#[test]
fn publish_replaces_a_corrupt_content_addressed_file_without_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    let cache = TrackCache::open(&root).unwrap();
    let key = CacheKey::new(72, AudioQuality::High320);
    let first = write_entry(&cache, request(72, AudioQuality::High320), b"original");
    let path = cache.entry_path(&first);

    fs::write(&path, b"corrupt!").unwrap();
    let repaired = write_entry(&cache, request(72, AudioQuality::High320), b"original");

    assert!(repaired.generation > first.generation);
    assert_eq!(read_entry(&cache, key), Some(b"original".to_vec()));
    assert_eq!(file_count(&root.join("tracks")), 1);
}

#[test]
fn concurrent_same_key_writes_publish_one_complete_generation() {
    if let Some(mode @ ("write-a" | "write-b")) = std::env::var(CHILD_MODE).ok().as_deref() {
        run_concurrent_writer_child(mode);
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("cache");
    TrackCache::open(&root).unwrap();
    let first_audio = vec![0x11; 32 * 1024];
    let second_audio = vec![0x22; 48 * 1024];
    let mut children = Vec::new();
    for mode in ["write-a", "write-b"] {
        children.push(
            Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("cache::tests::concurrent_same_key_writes_publish_one_complete_generation")
                .env(CHILD_MODE, mode)
                .env(CHILD_ROOT, &root)
                .env_remove(CRASH_AFTER_PUBLISH)
                .spawn()
                .unwrap(),
        );
    }
    fs::write(root.join("start-writes"), []).unwrap();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }

    let cache = TrackCache::open(&root).unwrap();
    let audio = read_entry(&cache, CacheKey::new(99, AudioQuality::High320)).unwrap();
    assert!(audio == first_audio || audio == second_audio);
    assert_eq!(file_count(&root.join("tracks")), 1);
}

fn run_concurrent_writer_child(mode: &str) {
    let root = PathBuf::from(std::env::var_os(CHILD_ROOT).unwrap());
    while !root.join("start-writes").exists() {
        thread::sleep(std::time::Duration::from_millis(5));
    }
    let cache = TrackCache::open(root).unwrap();
    let audio = match mode {
        "write-a" => vec![0x11; 32 * 1024],
        "write-b" => vec![0x22; 48 * 1024],
        _ => unreachable!(),
    };
    write_entry(&cache, request(99, AudioQuality::High320), &audio);
}

#[test]
fn concurrent_commit_and_eviction_leave_the_cache_within_policy() {
    let directory = tempfile::tempdir().unwrap();
    let root = Arc::new(directory.path().join("cache"));
    TrackCache::open(root.as_ref()).unwrap();
    let barrier = Arc::new(Barrier::new(2));

    let writer_root = Arc::clone(&root);
    let writer_barrier = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        let cache = TrackCache::open(writer_root.as_ref()).unwrap();
        let mut pending = cache
            .begin_write(request(5, AudioQuality::High320).with_expected_bytes(16 * 1024))
            .unwrap();
        pending.write_all(&vec![0x5a; 16 * 1024]).unwrap();
        writer_barrier.wait();
        pending.finish().unwrap();
    });

    let evictor_root = Arc::clone(&root);
    let evictor_barrier = Arc::clone(&barrier);
    let evictor = thread::spawn(move || {
        let cache = TrackCache::open(evictor_root.as_ref()).unwrap();
        evictor_barrier.wait();
        cache.set_max_bytes(0).unwrap();
    });

    writer.join().unwrap();
    evictor.join().unwrap();
    let cache = TrackCache::open(root.as_ref()).unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 0);
    assert!(cache
        .lookup(CacheKey::new(5, AudioQuality::High320))
        .unwrap()
        .is_none());
}

#[test]
fn maintain_reconciles_staging_and_published_files_left_by_process_crashes() {
    if let Some(mode) = std::env::var_os(CHILD_MODE) {
        run_crashing_child(&mode.to_string_lossy());
    }

    let directory = tempfile::tempdir().unwrap();
    for mode in ["staging", "published"] {
        let root = directory.path().join(mode);
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("cache::tests::maintain_reconciles_staging_and_published_files_left_by_process_crashes")
            .arg("--nocapture")
            .env(CHILD_MODE, mode)
            .env(CHILD_ROOT, &root)
            .env_remove(CRASH_AFTER_PUBLISH);
        if mode == "published" {
            command.env(CRASH_AFTER_PUBLISH, "1");
        }
        let status = command.status().unwrap();
        assert_eq!(status.code(), Some(86));

        let before_reopen = file_count(&root.join("tracks")) + file_count(&root.join("staging"));
        assert_eq!(before_reopen, 1);
        let cache = TrackCache::open(&root).unwrap();
        cache.maintain().unwrap();
        assert!(cache
            .lookup(CacheKey::new(808, AudioQuality::High320))
            .unwrap()
            .is_none());
        assert_eq!(file_count(&root.join("tracks")), 0);
        assert_eq!(file_count(&root.join("staging")), 0);

        write_entry(
            &cache,
            request(808, AudioQuality::High320),
            b"after-recovery",
        );
        assert_eq!(
            read_entry(&cache, CacheKey::new(808, AudioQuality::High320)),
            Some(b"after-recovery".to_vec())
        );
    }
}

fn run_crashing_child(mode: &str) -> ! {
    let root = PathBuf::from(std::env::var_os(CHILD_ROOT).unwrap());
    let cache = TrackCache::open(root).unwrap();
    let mut writer = cache
        .begin_write(request(808, AudioQuality::High320).with_expected_bytes(13))
        .unwrap();
    writer.write_all(b"crash-payload").unwrap();
    match mode {
        "staging" => {
            std::mem::forget(writer);
            std::process::exit(86);
        }
        "published" => {
            let _ = writer.finish();
            panic!("publish crash hook did not exit");
        }
        _ => panic!("unknown child mode"),
    }
}

fn file_count(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|items| items.filter_map(Result::ok).count())
        .unwrap_or(0)
}
