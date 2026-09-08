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
pub mod error;
pub mod gap;
pub mod memory;
pub mod record;
pub mod segment;
pub mod segmented;
pub mod seq;

pub use backend::{ExecutionLog, ExecutionLogBackend, NewExecutionRecord};
pub use cursor::{ConsumerCursor, LogConsumerId, ReadResult};
pub use error::LogError;
pub use gap::{Gap, GapReason};
pub use memory::InMemoryExecutionLog;
pub use record::{ExecutionKind, ExecutionPayload, ExecutionRecord, SessionId};
pub use segment::{segment_path, write_segment, DecodedSegment, SegmentEntry, SegmentMetadata};
pub use segmented::{CompactionMetrics, SegmentedConfig, SegmentedExecutionLog};
pub use seq::EventSeq;
