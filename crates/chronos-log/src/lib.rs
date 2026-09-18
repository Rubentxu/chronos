//! chronos-log — Append-only execution log.
//!
//! One authoritative evidence stream per session. Multiple consumers
//! (agents, indexes, persistence, UI, OTLP exporter) can read
//! independently via per-consumer cursors; reads are non-destructive.
//! Gaps are explicit (`Gap` + `GapReason`) instead of silent loss.
//!
//! The crate ships:
//! - The legacy `ExecutionLogBackend` trait (kept for the segmented
//!   adapter to implement until REC-C3.3.1 wires the new
//!   `chronos_domain::ports::execution_log::ExecutionLogProvider`).
//! - The `InMemoryExecutionLog` backend (m1-01; file-backed segments
//!   arrive in m1-02).
//!
//! See `docs/chronos-agentic-reconstruction/docs/specs/EXECUTION_LOG.md`
//! for the canonical behavior contract.

pub mod analytics;
pub mod backend;
pub mod call_graph;
pub mod checkpoint;
pub mod cursor;
pub mod discovery;
pub mod error;
pub mod gap;
pub mod location;
pub mod memory;
pub mod provider;
pub mod record;
pub mod replay;
pub mod segment;
pub mod segmented;
pub mod seq;
pub mod tail;
pub mod tripwire_evidence_codec;

pub use backend::{ExecutionLog, ExecutionLogBackend};
pub use cursor::{ConsumerCursor, LogConsumerId, LogPage, ReadResult};
pub use discovery::{discover_execution_logs, DiscoveredLog, DiscoveryReport, UnmanagedLegacyLog};
pub use error::LogError;
// REC-C3.3.1: Gap/GapReason/NewExecutionRecord live in chronos_domain::evidence.
// Re-export them here so existing `use chronos_log::Gap` paths keep resolving.
pub use chronos_domain::{Gap, GapReason, NewExecutionRecord};
pub use location::{execution_log_dir, execution_log_dir_for_session, resolve_execution_log_root};
pub use memory::InMemoryExecutionLog;
pub use record::{
    ExecutionKind, ExecutionPayload, ExecutionRecord, TripwireFiredEvidence,
    TRIPWIRE_FIRED_EVIDENCE_TAG,
};
// REC-C3.3.1: `SessionId` is the single canonical identity newtype
// owned by `chronos_domain`. The duplicate definition that used to
// live in `chronos_log::record` is deleted; this re-export keeps
// `use chronos_log::SessionId` resolving to the same type as
// `use chronos_domain::session_id::SessionId`.
pub use chronos_domain::session_id::SessionId;
pub use replay::{
    apply_replay_plan, build_replay_plan, plan_gaps, ReplayIntegrityError, ReplayPlan,
};
pub use segment::{segment_path, write_segment, DecodedSegment, SegmentEntry, SegmentMetadata};
pub use segmented::{CompactionMetrics, CompactionOutcome, SegmentedConfig, SegmentedExecutionLog};
// REC-C3.3.1: `ExecutionLogProvider` is the application-shape port in
// `chronos_domain::ports::execution_log`. The two adapters that
// implement it (segmented file-backed, in-memory) live here and are
// re-exported so callers write `use chronos_log::SegmentedExecutionLogProvider`
// without reaching into the `provider` module directly.
pub use provider::{InMemoryExecutionLogProvider, SegmentedExecutionLogProvider};
pub use seq::EventSeq;
// REC-C3.3.1: TailState/SealedTail live in chronos_domain::evidence.
// The clock-aware wrapper `sealed_now` plus `recover_tail_state`,
// `TailRecovery`, and `SealError` stay in `chronos_log::tail` because
// they read filesystem state.
pub use chronos_domain::{SealedTail, TailState};
pub use tail::{recover_tail_state, sealed_now, SealError, TailRecovery};
