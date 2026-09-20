//! chronos-store — persistent storage for trace sessions.
//!
//! Provides:
//! - [`ContentStore`]: Content-addressable storage for `TraceEvent`s using
//!   BLAKE3 hashing + LZ4 compression + redb.
//! - [`SessionStore`]: Session-level storage that builds on the CAS to persist
//!   sessions with full event replay.
//! - [`TraceDiff`]: Session comparison via hash-based set difference.

pub mod cas;
pub mod counterexample_repository;
pub mod counterexample_storage;
pub mod diff;
pub mod diff_engine_adapter;
pub mod error;
pub mod lifecycle_store_adapter;
pub mod session_archive;
pub mod session_reader_adapter;
pub mod storage;
mod table_error;
#[cfg(test)]
mod test_support;

pub use cas::{ContentHash, ContentStore};
#[allow(deprecated)]
pub use diff::{DiffReport, TimingDelta, TraceDiff};
pub use diff_engine_adapter::Blake3DiffEngine;
pub use error::StoreError;
pub use storage::{SessionMetadata, SessionStore};
