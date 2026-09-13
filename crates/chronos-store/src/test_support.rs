//! Test-only support shared by this crate's module test suites.
//!
//! m9-74 needs to observe *how many durability barriers* a code path issues,
//! because that — not wall-clock time — is the cost the CAS batching removes.
//! A timing assertion would be environment-dependent (and meaningless on an
//! in-memory backend, where redb never syncs), so instead we wrap redb's real
//! `FileBackend` in a counting shim and assert on the counter. `sync_data` is
//! exactly the call redb's default immediate durability makes at every
//! `commit()`, so the count is commit count.
#![cfg(test)]

use redb::StorageBackend;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A `FileBackend` that counts durability barriers.
#[derive(Debug)]
pub(crate) struct CountingBackend {
    inner: redb::backends::FileBackend,
    syncs: Arc<AtomicUsize>,
}

impl StorageBackend for CountingBackend {
    fn len(&self) -> io::Result<u64> {
        self.inner.len()
    }

    fn read(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        self.inner.read(offset, len)
    }

    fn set_len(&self, len: u64) -> io::Result<()> {
        self.inner.set_len(len)
    }

    fn sync_data(&self, eventual: bool) -> io::Result<()> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        self.inner.sync_data(eventual)
    }

    fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        self.inner.write(offset, data)
    }
}

/// A real, file-backed database whose durability barriers are counted.
///
/// File-backed on purpose: an in-memory backend never syncs, so it cannot
/// observe the behaviour under test.
#[cfg(unix)]
pub(crate) fn counting_file_db(dir: &Path, name: &str) -> (Arc<redb::Database>, Arc<AtomicUsize>) {
    let path = dir.join(name);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("create the counting backend's file");
    let syncs = Arc::new(AtomicUsize::new(0));
    let db = redb::Builder::new()
        .create_with_backend(CountingBackend {
            inner: redb::backends::FileBackend::new(file).expect("wrap the file backend"),
            syncs: syncs.clone(),
        })
        .expect("create the file-backed database");
    (Arc::new(db), syncs)
}
