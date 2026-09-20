//! `Blake3DiffEngine` — REC-C3.5-residual-inversion R.2.
//!
//! Production adapter for the [`DiffEngine`] port. Implements the
//! BLAKE3 hash-based symmetric-difference algorithm that previously
//! lived in `chronos_store::diff::TraceDiff::compare` as a free
//! function on a zero-size struct.
//!
//! ## Why a port + adapter
//!
//! `chronos_services::ChronosDiffService::compare_sessions` previously
//! delegated the heavy lifting to
//! `chronos_store::TraceDiff::compare(...)`, which kept a residual
//! production edge `chronos-services -> chronos-store`. After R.2 the
//! service consumes the port through `DiffContext::engine` and the
//! composition root (`chronos_mcp::composition::default_diff_engine`)
//! is the only place that constructs the adapter.
//!
//! The algorithm is **identical** to the previous
//! `TraceDiff::compare`:
//!
//! 1. Hash every `TraceEvent` with BLAKE3 after bincode serialization
//!    plus lz4 compression. The compression step preserves the dedup
//!    property of identical events producing identical hashes.
//! 2. Compute the symmetric difference of the two hash sets.
//! 3. Map the resulting hashes back to the original events for
//!    reconstruction.
//! 4. Compute similarity as
//!    `common / (only_in_a + only_in_b + common) * 100`.
//! 5. Compute the timing delta from the session metadata's
//!    `duration_ms`.
//!
//! ## Why a port, not a static function
//!
//! - Audit §3.2 A1 (ISP) is preserved: the contract is narrow.
//! - Audit §6.2 (composition only at driving adapter) is satisfied:
//!   services no longer imports `chronos_store::diff`.
//! - A second impl is plausible (e.g. a future `simhash` engine for
//!   fuzzy matching); the port makes that a drop-in swap.

use std::collections::{HashMap, HashSet};

use blake3::hash;
use chronos_domain::ports::diff::{DiffEngine, DiffReport, TimingDelta};
use chronos_domain::{SessionMetadata, TraceEvent};
use lz4_flex::compress_prepend_size;

/// Production [`DiffEngine`] impl backed by BLAKE3 hash-based
/// symmetric-difference. Zero-size struct (no state), so instances
/// are `Clone + Send + Sync` for free.
#[derive(Debug, Default, Clone, Copy)]
pub struct Blake3DiffEngine;

impl DiffEngine for Blake3DiffEngine {
    fn compare(
        &self,
        session_a_id: &str,
        session_b_id: &str,
        events_a: &[TraceEvent],
        events_b: &[TraceEvent],
        meta_a: &SessionMetadata,
        meta_b: &SessionMetadata,
    ) -> DiffReport {
        // Hash all events
        let hashes_a: HashSet<String> = events_a.iter().map(hash_event).collect();
        let hashes_b: HashSet<String> = events_b.iter().map(hash_event).collect();

        let hashes_a: Vec<String> = hashes_a.into_iter().collect();
        let hashes_b: Vec<String> = hashes_b.into_iter().collect();

        let set_a: HashSet<_> = hashes_a.iter().collect();
        let set_b: HashSet<_> = hashes_b.iter().collect();

        // Symmetric difference
        let only_in_a: Vec<String> = set_a.difference(&set_b).copied().cloned().collect();
        let only_in_b: Vec<String> = set_b.difference(&set_a).copied().cloned().collect();
        let common_count = set_a.intersection(&set_b).count();

        // Build hash → event maps for reconstruction
        let map_a: HashMap<_, _> = events_a
            .iter()
            .map(|e| (hash_event(e), e.clone()))
            .collect();
        let map_b: HashMap<_, _> = events_b
            .iter()
            .map(|e| (hash_event(e), e.clone()))
            .collect();

        let only_a_events: Vec<TraceEvent> = only_in_a
            .iter()
            .filter_map(|h| map_a.get(h).cloned())
            .collect();
        let only_b_events: Vec<TraceEvent> = only_in_b
            .iter()
            .filter_map(|h| map_b.get(h).cloned())
            .collect();

        // Compute similarity
        let total_unique = only_in_a.len() + only_in_b.len() + common_count;
        let similarity_pct = if total_unique > 0 {
            (common_count as f64 / total_unique as f64) * 100.0
        } else {
            100.0
        };

        // Timing delta
        let timing_delta = Some(TimingDelta {
            duration_ms_a: meta_a.duration_ms,
            duration_ms_b: meta_b.duration_ms,
            delta_ms: meta_b.duration_ms as i64 - meta_a.duration_ms as i64,
            slower_session: if meta_b.duration_ms > meta_a.duration_ms {
                Some(meta_b.session_id.clone())
            } else if meta_a.duration_ms > meta_b.duration_ms {
                Some(meta_a.session_id.clone())
            } else {
                None
            },
        });

        DiffReport {
            session_a_id: session_a_id.to_string(),
            session_b_id: session_b_id.to_string(),
            only_in_a: only_a_events,
            only_in_b: only_b_events,
            common_count,
            similarity_pct,
            timing_delta,
        }
    }
}

/// Hash a single event with BLAKE3 after bincode serialization + lz4 compression.
fn hash_event(event: &TraceEvent) -> String {
    let serialized = bincode::serialize(event).unwrap_or_default();
    let compressed = compress_prepend_size(&serialized);
    hash(&compressed).to_hex().to_string()
}
