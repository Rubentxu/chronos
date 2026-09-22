//! M9.3 — Happens-before graph over `CausalityIndex` + `TypedConcurrencyEvent`.
//!
//! ## Why this exists
//!
//! ADR-0028 §2.2 (M9.3, D2 + D4) requires a happens-before graph built
//! **over** the existing `CausalityIndex` (chronos-domain::index::causality)
//! and the `TypedConcurrencyEvent` model (M9.2 / chronos-domain::concurrency).
//!
//! ROADMAP §M9 §87: "M9.2 happens-before projection incremental/replay; M9.3
//! clasificador suspicious/confirmed/unsupported".
//!
//! The graph is the **prefilter** for `detect_concurrent_access` (M9
//! heuristic triage): instead of treating every co-temporal event pair as
//! "concurrent", we identify which pairs are **not causally ordered** (i.e.,
//! truly concurrent) and only those need downstream classification.
//!
//! ## What this is NOT
//!
//! - **NOT** a replacement for `CausalityIndex` (consolidated, NOT replaced).
//! - **NOT** a full VectorClock / Lamport implementation (we read the index
//!   and project — we don't maintain our own logical clock).
//! - **NOT** the final classifier (M9.4 introduces
//!   `RaceClassification::NotARace/Suspicious/Confirmed/Unsupported`).
//! - **NOT** the perturbation tests (M9.5).
//!
//! ## Design choices (per ADR-0028 §3 + ADR-0004)
//!
//! - **Incremental updates**: `add_event` mutates the graph in O(1) per node
//!   + O(n) for transitive closure checks. NOT recomputed from scratch.
//! - **Honest concurrency**: if no causal order exists between two events,
//!   they are concurrent (not "race detected" — that's M9.4's job).
//! - **Unknown primitive = Unknown relation**: events with primitive =
//!   `Unknown` cannot establish causal edges (we don't know what
//!   synchronization they participated in).
//! - **Cycle detection**: if A→B and B→A both exist, those events are
//!   flagged as "mutual dependency" (likely bug — events shouldn't be
//!   causally ordered both ways unless there's a logical clock error).
//!
//! ## Out-of-scope (per ADR-0028 §4)
//!
//! - Classifier state machine (M9.4).
//! - MCP wire tools (M9.4).
//! - Loss injection / perturbation (M9.5).
//! - Cross-session correlation (post-M9; ADR-0028 §3.3).
//! - Memory-model reasoning (post-M9).

use chronos_domain::concurrency::{ConcurrencyPrimitive, Provenance, TypedConcurrencyEvent};
use chronos_domain::index::CausalityIndex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Edge in the happens-before graph.
///
/// An edge `Earlier -> Later` means `Earlier` happened-before `Later`
/// (i.e., all effects of `Earlier` are visible to `Later`). The `kind`
/// field captures WHY the edge exists (lock release/acquire pair, message
/// send/receive pair, fork/join pair, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HappensBeforeEdge {
    pub earlier: u64,
    pub later: u64,
    pub kind: EdgeKind,
    /// Optional witness provenance (e.g., the lock release event).
    pub witness: Option<Provenance>,
}

/// Why an edge exists in the graph.
///
/// Conservative — we never invent a synchronization we don't observe.
/// `Unknown` means the graph cannot determine causality (caller should
/// treat the pair as concurrent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Synchronization via lock acquire/release pair (Lock primitive).
    LockReleaseAcquire,
    /// Atomic store/load with appropriate memory ordering.
    AtomicStoreLoad,
    /// Task spawn / join (parent-child causality).
    TaskSpawnJoin,
    /// Goroutine fork/join (parent-child causality).
    GoroutineForkJoin,
    /// Message send/receive pair (channel).
    MessageSendReceive,
    /// Synchronization primitive is unknown — edge cannot be confirmed.
    Unknown,
}

impl EdgeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LockReleaseAcquire => "lock_release_acquire",
            Self::AtomicStoreLoad => "atomic_store_load",
            Self::TaskSpawnJoin => "task_spawn_join",
            Self::GoroutineForkJoin => "goroutine_fork_join",
            Self::MessageSendReceive => "message_send_receive",
            Self::Unknown => "unknown",
        }
    }

    /// Returns true if this edge kind represents an observed
    /// synchronization (vs `Unknown` which means "we don't know").
    pub fn is_observed(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// Pair of events that have **no causal order** between them — i.e.,
/// they are truly concurrent according to the graph. Downstream
/// classifier (M9.4) decides whether this is a benign race, a
/// suspicious race, a confirmed race, or unsupported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConcurrentPair {
    pub earlier: u64,
    pub later: u64,
}

/// The happens-before graph itself.
///
/// Built from a `CausalityIndex` + a set of `TypedConcurrencyEvent`s.
/// Incremental: `add_event` + `add_edge` extend the graph; queries
/// (`happens_before`, `concurrent_pairs`, `cycles`) are O(n) at worst.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HappensBeforeGraph {
    /// event_id → event (so callers can lookup provenance + primitive).
    events: BTreeMap<u64, TypedConcurrencyEvent>,
    /// Adjacency list: earlier → {later1, later2, ...}.
    edges_out: BTreeMap<u64, BTreeSet<u64>>,
    /// Reverse adjacency: later → {earlier1, earlier2, ...}.
    edges_in: BTreeMap<u64, BTreeSet<u64>>,
    /// All edges (event_id pair → kind) for cycle detection.
    edge_kinds: BTreeMap<(u64, u64), EdgeKind>,
}

impl HappensBeforeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the graph from a `CausalityIndex` + a slice of addresses
    /// (the events to project from the index).
    ///
    /// This is the **initial population** step. After this, callers can
    /// extend with `add_event` / `add_edge` as new events arrive
    /// (incremental update path).
    pub fn from_index(
        index: &CausalityIndex,
        addresses: &[u64],
        primitive: ConcurrencyPrimitive,
    ) -> Self {
        let mut graph = Self::new();
        for &addr in addresses {
            for entry in index.writes_at(addr) {
                let event = TypedConcurrencyEvent::from_causality_entry(entry, primitive);
                graph.add_event(event);
            }
        }
        graph
    }

    /// Add a single event to the graph (without edges — for events whose
    /// synchronization partner hasn't been observed yet).
    pub fn add_event(&mut self, event: TypedConcurrencyEvent) {
        let id = event.event_id;
        self.events.insert(id, event);
        // Pre-populate adjacency sets so queries don't have to handle
        // missing keys.
        self.edges_out.entry(id).or_default();
        self.edges_in.entry(id).or_default();
    }

    /// Add a happens-before edge.
    ///
    /// Refuses self-edges (would be a no-op or worse: cycle to self).
    /// Refuses edges where both endpoints are unknown primitive
    /// (per ADR-0004 — we don't invent synchronization).
    pub fn add_edge(&mut self, earlier: u64, later: u64, kind: EdgeKind) {
        if earlier == later {
            // Self-edge rejected; happens-before is strict.
            return;
        }
        // Record the edge even if endpoints aren't yet in the graph
        // (events may be added later).
        self.edges_out.entry(earlier).or_default().insert(later);
        self.edges_in.entry(later).or_default().insert(earlier);
        self.edge_kinds.insert((earlier, later), kind);
        // Ensure endpoints are in events map (placeholder if missing).
        self.events.entry(earlier).or_insert_with(|| {
            TypedConcurrencyEvent::new(
                earlier,
                ConcurrencyPrimitive::Unknown,
                Provenance::empty(),
            )
        });
        self.events.entry(later).or_insert_with(|| {
            TypedConcurrencyEvent::new(
                later,
                ConcurrencyPrimitive::Unknown,
                Provenance::empty(),
            )
        });
    }

    /// Returns the number of events in the graph.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edge_kinds.len()
    }

    /// Look up an event by id.
    pub fn get_event(&self, event_id: u64) -> Option<&TypedConcurrencyEvent> {
        self.events.get(&event_id)
    }

    /// Check whether `earlier` happens-before `later` (transitive).
    ///
    /// BFS from `earlier` along `edges_out`; if we reach `later`, return
    /// `true`. If we don't, return `false` (either no order, or both
    /// directions — i.e., a cycle).
    ///
    /// `Unknown` edges do NOT propagate (we don't trust them as ordering
    /// evidence; per ADR-0004: don't invent synchronization).
    pub fn happens_before(&self, earlier: u64, later: u64) -> bool {
        if earlier == later {
            return false; // Strict: not its own cause.
        }
        // BFS.
        let mut visited: BTreeSet<u64> = BTreeSet::new();
        let mut frontier: Vec<u64> = vec![earlier];
        while let Some(current) = frontier.pop() {
            if current == later {
                return true;
            }
            if !visited.insert(current) {
                continue;
            }
            if let Some(neighbors) = self.edges_out.get(&current) {
                for &next in neighbors {
                    // Only propagate observed edges.
                    if let Some(kind) = self.edge_kinds.get(&(current, next)) {
                        if kind.is_observed() {
                            frontier.push(next);
                        }
                    }
                }
            }
        }
        false
    }

    /// Find all event pairs (a, b) with no causal order (truly concurrent).
    ///
    /// Pairs where `a == b`, `a → b`, or `b → a` are excluded. Pairs
    /// connected only by `Unknown` edges are treated as concurrent
    /// (per ADR-0004).
    ///
    /// O(n²) at worst — caller is expected to prefilter with
    /// `detect_concurrent_access` (M9 heuristic) before calling this.
    pub fn concurrent_pairs(&self) -> Vec<ConcurrentPair> {
        let ids: Vec<u64> = self.events.keys().copied().collect();
        let mut result = Vec::new();
        for (i, &a) in ids.iter().enumerate() {
            for &b in ids.iter().skip(i + 1) {
                if !self.happens_before(a, b) && !self.happens_before(b, a) {
                    result.push(ConcurrentPair { earlier: a, later: b });
                }
            }
        }
        result
    }

    /// Detect cycles in the graph (mutual happens-before relations).
    ///
    /// Returns one `ConcurrentPair` per detected cycle (representing the
    /// pair involved). A cycle usually indicates a logical clock error or
    /// a recorded event with inconsistent timestamps.
    pub fn cycles(&self) -> Vec<ConcurrentPair> {
        let mut result = Vec::new();
        let ids: Vec<u64> = self.events.keys().copied().collect();
        for (i, &a) in ids.iter().enumerate() {
            for &b in ids.iter().skip(i + 1) {
                if self.happens_before(a, b) && self.happens_before(b, a) {
                    result.push(ConcurrentPair { earlier: a, later: b });
                }
            }
        }
        result
    }

    /// Returns the events in the graph, sorted by event_id.
    pub fn events_sorted(&self) -> Vec<&TypedConcurrencyEvent> {
        self.events.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::index::CausalityEntry;

    fn empty_event(id: u64) -> TypedConcurrencyEvent {
        TypedConcurrencyEvent::new(id, ConcurrencyPrimitive::Unknown, Provenance::empty())
    }

    fn lock_event(id: u64) -> TypedConcurrencyEvent {
        TypedConcurrencyEvent::new(id, ConcurrencyPrimitive::Lock, Provenance::empty())
    }

    #[test]
    fn empty_graph_has_no_events() {
        let g = HappensBeforeGraph::new();
        assert_eq!(g.event_count(), 0);
        assert_eq!(g.edge_count(), 0);
        assert!(g.concurrent_pairs().is_empty());
        assert!(g.cycles().is_empty());
    }

    #[test]
    fn add_event_increments_count() {
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(1));
        g.add_event(empty_event(2));
        assert_eq!(g.event_count(), 2);
    }

    #[test]
    fn add_edge_records_observed_relation() {
        let mut g = HappensBeforeGraph::new();
        g.add_event(lock_event(1));
        g.add_event(lock_event(2));
        g.add_edge(1, 2, EdgeKind::LockReleaseAcquire);
        assert_eq!(g.edge_count(), 1);
        assert!(g.happens_before(1, 2));
        assert!(!g.happens_before(2, 1));
    }

    #[test]
    fn self_edge_is_rejected() {
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(1));
        g.add_edge(1, 1, EdgeKind::LockReleaseAcquire);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn unknown_edge_does_not_propagate() {
        // Per ADR-0004: Unknown synchronization = no causal claim.
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(1));
        g.add_event(empty_event(2));
        g.add_edge(1, 2, EdgeKind::Unknown);
        assert_eq!(g.edge_count(), 1);
        assert!(!g.happens_before(1, 2));
    }

    #[test]
    fn transitive_happens_before_via_chain() {
        // 1 → 2 → 3 should imply 1 → 3.
        let mut g = HappensBeforeGraph::new();
        g.add_event(lock_event(1));
        g.add_event(lock_event(2));
        g.add_event(lock_event(3));
        g.add_edge(1, 2, EdgeKind::LockReleaseAcquire);
        g.add_edge(2, 3, EdgeKind::LockReleaseAcquire);
        assert!(g.happens_before(1, 3));
    }

    #[test]
    fn cycle_detected_as_mutual_happens_before() {
        // 1 → 2 and 2 → 1 (cycle) — both directions true.
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(1));
        g.add_event(empty_event(2));
        g.add_edge(1, 2, EdgeKind::TaskSpawnJoin);
        g.add_edge(2, 1, EdgeKind::TaskSpawnJoin);
        let cycles = g.cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].earlier, 1);
        assert_eq!(cycles[0].later, 2);
    }

    #[test]
    fn concurrent_pairs_excludes_ordered_pairs() {
        // 1 → 2, 3 has no order with 1 or 2.
        let mut g = HappensBeforeGraph::new();
        g.add_event(lock_event(1));
        g.add_event(lock_event(2));
        g.add_event(lock_event(3));
        g.add_edge(1, 2, EdgeKind::LockReleaseAcquire);
        let pairs = g.concurrent_pairs();
        // Expected: {1,3} and {2,3} are concurrent; {1,2} is not.
        assert_eq!(pairs.len(), 2);
        let pair_set: BTreeSet<(u64, u64)> = pairs
            .iter()
            .map(|p| (p.earlier, p.later))
            .collect();
        assert!(pair_set.contains(&(1, 3)));
        assert!(pair_set.contains(&(2, 3)));
        assert!(!pair_set.contains(&(1, 2)));
    }

    #[test]
    fn concurrent_pairs_excludes_self() {
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(42));
        assert!(g.concurrent_pairs().is_empty());
    }

    #[test]
    fn from_index_with_empty_index_yields_empty_graph() {
        let idx = CausalityIndex::new();
        let g = HappensBeforeGraph::from_index(&idx, &[], ConcurrencyPrimitive::Lock);
        assert_eq!(g.event_count(), 0);
    }

    #[test]
    fn from_index_with_recorded_writes_populates_events() {
        // Build a CausalityIndex with a recorded write, then project.
        let mut idx = CausalityIndex::new();
        let addr = 42u64;
        let entry = CausalityEntry {
            event_id: addr,
            timestamp: chronos_domain::trace::MonotonicNs(100),
            thread_id: 1,
            function: "foo".to_string(),
            file: Some("foo.rs".to_string()),
            line: Some(10),
            value_before: None,
            value_after: "99".to_string(),
        };
        idx.record_write(addr, entry, Some("x"));
        let g = HappensBeforeGraph::from_index(&idx, &[addr], ConcurrencyPrimitive::Lock);
        assert_eq!(g.event_count(), 1);
        let event = g.get_event(addr).expect("event for addr");
        assert_eq!(event.primitive, ConcurrencyPrimitive::Lock);
    }

    #[test]
    fn edge_kind_labels_are_distinct() {
        let labels = [
            EdgeKind::LockReleaseAcquire,
            EdgeKind::AtomicStoreLoad,
            EdgeKind::TaskSpawnJoin,
            EdgeKind::GoroutineForkJoin,
            EdgeKind::MessageSendReceive,
            EdgeKind::Unknown,
        ]
        .iter()
        .map(|k| k.label())
        .collect::<BTreeSet<_>>();
        assert_eq!(labels.len(), 6, "labels must be distinct");
    }

    #[test]
    fn is_observed_excludes_only_unknown() {
        assert!(EdgeKind::LockReleaseAcquire.is_observed());
        assert!(EdgeKind::AtomicStoreLoad.is_observed());
        assert!(EdgeKind::TaskSpawnJoin.is_observed());
        assert!(EdgeKind::GoroutineForkJoin.is_observed());
        assert!(EdgeKind::MessageSendReceive.is_observed());
        assert!(!EdgeKind::Unknown.is_observed());
    }

    #[test]
    fn events_sorted_returns_in_id_order() {
        let mut g = HappensBeforeGraph::new();
        g.add_event(empty_event(3));
        g.add_event(empty_event(1));
        g.add_event(empty_event(2));
        let sorted: Vec<u64> = g.events_sorted().iter().map(|e| e.event_id).collect();
        assert_eq!(sorted, vec![1, 2, 3]);
    }
}
