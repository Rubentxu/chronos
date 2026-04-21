//! chronos-store — persistent storage for trace sessions.
//!
//! Provides:
//! - [`ContentStore`]: Content-addressable storage for `TraceEvent`s using
//!   BLAKE3 hashing + LZ4 compression + redb.
//! - [`SessionStore`]: Session-level storage that builds on the CAS to persist
//!   sessions with full event replay.
//! - [`TraceDiff`]: Session comparison via hash-based set difference.

pub mod cas;
pub mod diff;
pub mod error;
pub mod storage;

pub use cas::{ContentHash, ContentStore};
pub use diff::{DiffReport, TimingDelta, TraceDiff};
pub use error::StoreError;
pub use storage::{SessionMetadata, SessionStore};
