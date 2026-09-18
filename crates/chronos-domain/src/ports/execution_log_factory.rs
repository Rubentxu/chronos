//! REC-C3.3.2.5 — factory port for the `ExecutionLog` capability bundle.
//!
//! ## Why a capability bundle, not a single provider
//!
//! Before C3.3.2.5 the factory returned `Arc<dyn ExecutionLogProvider>`
//! and `SessionExecutionLog` carried a `ProviderKind` enum tag. Every
//! maintenance call (`flush`, `compaction_metrics`, `compact_up_to`,
//! `retain_up_to`, `maybe_compact`, `record_gap_on_segmented`) hit a
//! `_ => Err(ExecutionLogMaintenanceUnsupported { ... })` arm because
//! the maintenance capabilities were never on the port. The port
//! only carried canonical-evidence operations.
//!
//! C3.3.2.5 lifts the maintenance and retention capabilities into
//! first-class ports. The factory now returns three `Arc<dyn ...>`
//! that share the same concrete instance underneath:
//!
//! ```text
//! ExecutionLogCapabilities {
//!     evidence:     Arc<dyn ExecutionLogProvider>
//!     retention:    Arc<dyn ExecutionLogRetention>
//!     maintenance:  Arc<dyn ExecutionLogMaintenance>
//! }
//! ```
//!
//! The application layer receives the bundle and uses each port
//! independently; the downcast on `ProviderKind` is gone.
//!
//! ## Object safety
//!
//! Each port is object-safe (it has only `&self` methods returning
//! owned types). The factory port composes them behind an owned
//! struct so consumers still hold three trait objects, never a
//! concrete.
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

use std::path::PathBuf;
use std::sync::Arc;

use crate::ports::execution_log::{ExecutionLogError, ExecutionLogProvider};
use crate::ports::execution_log_maintenance::ExecutionLogMaintenance;
use crate::ports::execution_log_retention::ExecutionLogRetention;
use crate::session_id::SessionId;

/// The capability bundle returned by every `ExecutionLogFactory`
/// operation.
///
/// All three trait objects are backed by the SAME concrete instance.
/// That single-instance guarantee is what makes "move retention
/// frontier X, then ask maintenance to reclaim" semantically correct:
/// the retention port and the maintenance port see the same in-memory
/// state. The factory contract is "the three `Arc`s are views onto one
/// instance", not "we built three of them and they happen to agree".
#[derive(Clone)]
pub struct ExecutionLogCapabilities {
    pub evidence: Arc<dyn ExecutionLogProvider>,
    pub retention: Arc<dyn ExecutionLogRetention>,
    pub maintenance: Arc<dyn ExecutionLogMaintenance>,
}

impl std::fmt::Debug for ExecutionLogCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionLogCapabilities")
            .field("session", &self.evidence.session_id())
            .field("kind", &self.evidence.kind())
            .finish()
    }
}

/// Factory for `ExecutionLogCapabilities` bundles.
///
/// Implementations live in infrastructure crates (`chronos-log` for
/// the canonical segmented + in-memory factories; test crates for
/// mocks). Services consume this trait, never the concrete factory.
pub trait ExecutionLogFactory: Send + Sync {
    /// Create (or open) the log for a NEW session under `dir`.
    ///
    /// See module-level docs for the difference vs `reopen_existing`.
    fn create(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<ExecutionLogCapabilities, ExecutionLogError>;

    /// Reopen an EXISTING durable log at `dir`. Bootstrap uses this
    /// exclusively; it MUST NOT create.
    fn reopen_existing(
        &self,
        dir: PathBuf,
        session_id: SessionId,
    ) -> Result<ExecutionLogCapabilities, ExecutionLogError>;
}
