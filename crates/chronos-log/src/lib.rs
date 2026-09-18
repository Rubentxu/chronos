//! chronos-log — Append-only execution log.
//!
//! One authoritative evidence stream per session. Multiple consumers
//! (agents, indexes, persistence, UI, OTLP exporter) can read
//! independently via per-consumer cursors; reads are non-destructive.
//! Gaps are explicit (`Gap` + `GapReason`) instead of silent loss.
//!
//! The crate ships:
//! - The public types (`EventSeq`, `ExecutionRecord`, `Gap`,
//!   `LogConsumerId`, `ConsumerCursor`, `ReadResult`).
//! - The `ExecutionLogBackend` trait.
//! - The `InMemoryExecutionLog` backend (the only backend in m1-01;
//!   file-backed segments arrive in m1-02).
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
pub mod record;
pub mod replay;
pub mod segment;
pub mod segmented;
pub mod seq;
pub mod tail;
pub mod tripwire_evidence_codec;

pub use backend::{ExecutionLog, ExecutionLogBackend, NewExecutionRecord};
pub use cursor::{ConsumerCursor, LogConsumerId, LogPage, ReadResult};
pub use discovery::{discover_execution_logs, DiscoveredLog, DiscoveryReport, UnmanagedLegacyLog};
pub use error::LogError;
pub use gap::{Gap, GapReason};
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
pub use segmented::{CompactionMetrics, SegmentedConfig, SegmentedExecutionLog};
pub use seq::EventSeq;
pub use tail::{recover_tail_state, SealError, SealedTail, TailState};
