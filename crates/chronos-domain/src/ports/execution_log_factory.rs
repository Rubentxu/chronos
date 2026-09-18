//! REC-C3.3.2 — factory port for `ExecutionLogProvider`.
//!
//! This port separates **how an `ExecutionLogProvider` is constructed**
//! (segmented on disk, in-memory, mock) from **how it is consumed**
//! (`Arc<dyn ExecutionLogProvider>` via the canonical-evidence port).
//!
//! Before C3.3.2, `chronos_services::session_log::SessionExecutionLog`
//! called `SegmentedExecutionLog::open` / `open_existing` and wrapped
//! the concrete `SegmentedExecutionLog` with `SegmentedExecutionLogProvider`.
//! That broke the hexagon: services owned the construction of a
//! concrete adapter, so the composition root had no seam to swap it
//! (tests, alternate backends, future ones).
//!
//! With this port the composition root constructs and wires the
//! factory; services call `factory.create(...)` and get back
//! `Arc<dyn ExecutionLogProvider>`. The factory itself lives in
//! `chronos-log` (segmented + in-memory) or in a test crate (mocks);
//! services consume the trait, not the concrete.
//!
//! ## Two operations, deliberately split
//!
//! `create` and `reopen_existing` are NOT the same operation:
//!
//! - `create` opens or creates a brand-new log under a fresh `dir`.
//!   It must be a no-op if the dir is empty; it must fail if the
//!   dir already holds a sealed log for a different `session_id`.
//! - `reopen_existing` opens a previously-written log. It MUST NOT
//!   create. It MUST fail if the dir is empty or holds a different
//!   session. Bootstrap (C1.5.4) uses this exclusively.
//!
//! `try_adopt` is a third seam — used when an external component
//! (the native probe backend in C3.3.1) opened the concrete log and
//! hands ownership to services. C3.3.2 keeps the trait small; the
//! `adopt` path is handled by the factory's implementation when
//! needed (today the segmented factory exposes it because the
//! `chronos_native` backend already constructs the concrete; after
//! C3.3.2 the backend consumes the port instead and `adopt` may be
//! removed in a later cycle).

use std::path::PathBuf;
use std::sync::Arc;

use crate::ports::execution_log::{ExecutionLogError, ExecutionLogProvider};
use crate::session_id::SessionId;

/// Factory for `Arc<dyn ExecutionLogProvider>` instances.
///
/// Implementations live in infrastructure crates and are wired at the
/// composition root (`chronos-mcp::composition`). `chronos_services`
/// holds an `Arc<dyn ExecutionLogFactory>` injected at construction
/// time and calls these methods; it never names the concrete factory
/// type.
pub trait ExecutionLogFactory: Send + Sync {
    /// Create (or open) the log for a NEW session under `dir`.
    ///
    /// See module-level docs for the difference vs `reopen_existing`.
    fn create(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<Arc<dyn ExecutionLogProvider>, ExecutionLogError>;

    /// Reopen an EXISTING durable log at `dir`. Bootstrap uses this
    /// exclusively; it MUST NOT create.
    fn reopen_existing(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<Arc<dyn ExecutionLogProvider>, ExecutionLogError>;
}
