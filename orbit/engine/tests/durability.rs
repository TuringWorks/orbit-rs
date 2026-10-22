//! Durability and integrity checks for the unified store.
//!
//! These tests exist because a green build proves nothing about whether data
//! survives. Each one stops the store, does something hostile to it, and then
//! asks the store a question it can only answer correctly if the data really
//! reached the disk intact.

#![cfg(feature = "storage-rocksdb")]

use std::path::{Path, PathBuf};

use orbit_engine::unified::rocksdb_backend::{Compression, RocksDbBackend, RocksDbBackendConfig};
use orbit_engine::unified::storage::UnifiedStorageBackend;

/// A scratch directory that is emptied before use, so a previous run cannot
/// make this one pass.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("orbit-durability-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    path
}

/// Every file under `path` whose extension matches, deepest first.
fn files_with_extension(path: &Path, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect()
}

/// Overwrite a stretch of bytes in the middle of `path` with a value that is
/// certainly not what was written there.
fn corrupt_middle(path: &Path) {
    use std::io::{Seek, SeekFrom, Write};

    let length = std::fs::metadata(path).expect("stat").len();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for corruption");

    // A quarter of the way in is past the header and inside the data blocks,
    // which is where a silent bit-flip would do its damage.
    file.seek(SeekFrom::Start(length / 4)).expect("seek");
    file.write_all(&[0xA5; 512]).expect("scribble");
    file.sync_all().expect("sync");
}

/// The whole point of the backend: what was written is still there after the
/// process that wrote it is gone.
#[tokio::test]
async fn data_survives_reopening_the_store() {
    let path = scratch("survives");

    let store = RocksDbBackend::open(&path).expect("opens");
    for index in 0..100 {
        store
            .put(
                &format!("row:{index:04}"),
                format!("value-{index}").as_bytes(),
            )
            .await
            .expect("put");
    }
    store.shutdown().await.expect("shutdown");
    drop(store);

    let reopened = RocksDbBackend::open(&path).expect("reopens");
    for index in 0..100 {
        let found = reopened
            .get(&format!("row:{index:04}"))
            .await
            .expect("get")
            .expect("the row should still be there");
        assert_eq!(found, format!("value-{index}").into_bytes());
    }

    let _ = std::fs::remove_dir_all(&path);
}

/// Data written and never explicitly flushed must still come back, because a
/// process that is killed does not get to run `shutdown`. This is the write
/// path a crash actually leaves behind: records in the write-ahead log with no
/// clean close, replayed on the next open.
#[tokio::test]
async fn data_survives_without_a_clean_shutdown() {
    let path = scratch("unclean");

    let store = RocksDbBackend::open(&path).expect("opens");
    for index in 0..100 {
        store
            .put(
                &format!("row:{index:04}"),
                format!("value-{index}").as_bytes(),
            )
            .await
            .expect("put");
    }
    // No shutdown, no flush — exactly what a SIGKILL leaves behind.
    drop(store);

    let reopened = RocksDbBackend::open(&path).expect("reopens");
    let rows = reopened.scan_prefix("row:", None).await.expect("scan");
    assert_eq!(
        rows.len(),
        100,
        "the write-ahead log should have replayed every row"
    );

    let _ = std::fs::remove_dir_all(&path);
}

/// The defaults are the durable ones. A backend that has to be configured
/// carefully to avoid losing data will eventually be deployed without that
/// care, so the default is the safe end and speed is the opt-in.
#[test]
fn the_default_configuration_is_the_durable_one() {
    let config = RocksDbBackendConfig::default();
    assert!(
        config.sync_writes,
        "an acknowledged write must reach the disk by default"
    );
    assert!(
        config.enable_wal,
        "the write-ahead log must be on by default"
    );

    let fast = RocksDbBackendConfig::unsafe_fast();
    assert!(!fast.sync_writes, "the fast profile trades away the fsync");
}

/// Asking for synchronous writes with no log to synchronise is a contradiction
/// RocksDB reports one failed write at a time. It has to be caught at startup,
/// or the server comes up healthy and then refuses every write.
#[test]
fn syncing_a_disabled_log_is_refused_at_startup() {
    let path = scratch("contradiction");
    let config = RocksDbBackendConfig {
        sync_writes: true,
        enable_wal: false,
        ..Default::default()
    };

    let error = RocksDbBackend::open_with(&path, &config)
        .err()
        .expect("the contradiction should be refused");
    let message = error.to_string();
    assert!(
        message.contains("sync_wal") && message.contains("enable_wal"),
        "the error should name both settings: {message}"
    );
    let _ = std::fs::remove_dir_all(&path);
}

/// Every compression codec the configuration can name must be linked into the
/// binary. RocksDB refuses to open a database asking for a codec it does not
/// have, so an unlinked codec is a server that will not start.
#[tokio::test]
async fn every_named_compression_codec_can_actually_open_a_database() {
    for (name, compression) in [
        ("none", Compression::None),
        ("lz4", Compression::Lz4),
        ("zstd", Compression::Zstd),
    ] {
        assert_eq!(
            name.parse::<Compression>().expect("the name should parse"),
            compression
        );

        let path = scratch(&format!("codec-{name}"));
        let config = RocksDbBackendConfig {
            compression,
            ..Default::default()
        };
        let store = RocksDbBackend::open_with(&path, &config)
            .unwrap_or_else(|e| panic!("{name} should be linked in, but opening failed: {e}"));

        store.put("k", b"v").await.expect("put");
        store.shutdown().await.expect("shutdown");
        drop(store);

        let reopened = RocksDbBackend::open_with(&path, &config).expect("reopens");
        assert_eq!(
            reopened.get("k").await.expect("get").as_deref(),
            Some(&b"v"[..]),
            "{name}-compressed data should read back"
        );
        let _ = std::fs::remove_dir_all(&path);
    }
}

/// An unknown codec is refused by name rather than quietly falling back to
/// storing everything uncompressed.
#[test]
fn an_unknown_compression_codec_is_refused() {
    let error = "brotli"
        .parse::<Compression>()
        .expect_err("brotli is not linked in");
    let message = error.to_string();
    assert!(
        message.contains("brotli") && message.contains("lz4"),
        "the error should name what was asked for and what is available: {message}"
    );
}

/// Corruption on disk must be reported, not served. A store that hands back a
/// silently mangled value is worse than one that refuses, because nothing
/// downstream can tell that the answer is wrong.
#[tokio::test]
async fn corrupted_data_on_disk_is_detected_rather_than_served() {
    let path = scratch("corruption");

    let store = RocksDbBackend::open(&path).expect("opens");
    // Enough distinct rows to fill several data blocks, so the scribble below
    // lands inside one rather than in padding.
    for index in 0..5_000 {
        store
            .put(
                &format!("row:{index:06}"),
                format!("value-{index}-{}", "x".repeat(200)).as_bytes(),
            )
            .await
            .expect("put");
    }
    // Force the rows out of the memtable into an SST file, which is the
    // artifact that can rot on disk.
    store.shutdown().await.expect("shutdown");
    drop(store);

    let ssts = files_with_extension(&path, "sst");
    assert!(
        !ssts.is_empty(),
        "expected at least one SST file to corrupt, found none in {}",
        path.display()
    );
    for sst in &ssts {
        corrupt_middle(sst);
    }

    let reopened = RocksDbBackend::open(&path).expect("reopens");
    let outcome = reopened.scan_prefix("row:", None).await;

    match outcome {
        Err(error) => {
            let message = error.to_string().to_lowercase();
            assert!(
                message.contains("corrupt") || message.contains("checksum"),
                "the error should name the corruption, said: {error}"
            );
        }
        Ok(rows) => panic!(
            "a full scan over a corrupted SST returned {} rows instead of an error — \
             corrupted data was served as if it were good",
            rows.len()
        ),
    }

    let _ = std::fs::remove_dir_all(&path);
}
