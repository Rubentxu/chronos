//! Symbol-level call graph derived from concrete invocation records.
//!
//! This module answers the M2 roadmap question "who calls what, and how
//! often per caller" *without* treating a symbol as a concrete call. It
//! aggregates the `invocation_id` / `parent_invocation_id` / `symbol_id`
//! fields already carried by v2 `ExecutionRecord`s into a directed,
//! weighted `caller → callee` graph.
//!
//! It does **not** need exit events: the graph is a pure derivation over
//! the identity fields, so it works on M2 capture output where the native
//! tracker never emits `FunctionExit` (exit detection is M3+ DWARF work).
//!
//! Concurrency: `call_graph` snapshots every record in the session under
//! the records lock once (O(N)), then builds the graph on the owned
//! snapshot outside the lock. Lock-hold is O(N) for the snapshot — the
//! same pattern `analytics::recursion_depth` uses.
//!
//! Out of scope for m2-04:
//! - InvocationProjection (pairing Entry/Exit into completed + incomplete
//!   invocations) — blocked until exit detection lands (M3+).
//! - Versioned projection checkpoint / replay-equivalence — later cycle.
//! - MCP tool exposure — m2-NN.
//! - Time / duration-weighted edges — needs exit events.
//! - Cross-session parent resolution — a record whose
//!   `parent_invocation_id` points at an invocation recorded in a
//!   *different* session (or before the snapshot) is attributed as a root
//!   call (`caller == None`). For in-session traces this never happens:
//!   the tracker always links to a previously-emitted parent.

use crate::backend::ExecutionLogBackend;
use crate::cursor::LogConsumerId;
use crate::memory::InMemoryExecutionLog;
use crate::record::{ExecutionRecord, SessionId};

use std::collections::HashMap;

/// One directed, weighted symbol-level call edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphEdge {
    /// Symbol of the calling frame, or `None` for a root call (a record
    /// with no `parent_invocation_id`, or whose parent invocation is not
    /// present in the session snapshot).
    pub caller: Option<chronos_domain::SymbolId>,
    /// Symbol of the called frame (always the record's `symbol_id`).
    pub callee: chronos_domain::SymbolId,
    /// Number of concrete invocations that produced this edge.
    pub calls: u64,
}

/// Aggregate symbol-level call graph for a session.
///
/// Edges and node lists are returned in deterministic order (sorted by
/// `(name_hash, signature_hash, language)`), so callers and tests can
/// rely on stable iteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraph {
    /// Directed edges, sorted deterministically (roots first, then by
    /// caller key then callee key).
    edges: Vec<CallGraphEdge>,
}

/// A stable sort key for a `SymbolId` that does not itself implement
/// `Ord`. Orders by `(name_hash, signature_hash, language)`.
type SymbolSortKey = (u64, u64, u8);

fn symbol_key(s: chronos_domain::SymbolId) -> SymbolSortKey {
    (s.name_hash, s.signature_hash, s.language as u8)
}

/// Sort key for an edge: roots (`caller == None`) come first, then by
/// caller key, then callee key.
type EdgeSortKey = (u8, Option<SymbolSortKey>, SymbolSortKey);

fn edge_key(e: &CallGraphEdge) -> EdgeSortKey {
    let caller = e.caller.map(symbol_key);
    // caller None (root) sorts before Some.
    let caller_rank = if caller.is_some() { 1u8 } else { 0u8 };
    (caller_rank, caller, symbol_key(e.callee))
}

impl CallGraph {
    /// All directed edges in deterministic order.
    pub fn edges(&self) -> &[CallGraphEdge] {
        &self.edges
    }

    /// Every distinct callee symbol, sorted.
    pub fn nodes(&self) -> Vec<chronos_domain::SymbolId> {
        let mut nodes: Vec<chronos_domain::SymbolId> = self
            .edges
            .iter()
            .map(|e| e.callee)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        nodes.sort_by_key(|s| symbol_key(*s));
        nodes
    }

    /// The set of caller symbols that call `callee` (empty if none).
    pub fn callers_of(&self, callee: chronos_domain::SymbolId) -> Vec<chronos_domain::SymbolId> {
        let mut callers: Vec<chronos_domain::SymbolId> = self
            .edges
            .iter()
            .filter(|e| e.callee == callee && e.caller.is_some())
            .filter_map(|e| e.caller)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        callers.sort_by_key(|s| symbol_key(*s));
        callers
    }

    /// The set of callee symbols reached from `caller` (empty if none).
    pub fn callees_of(&self, caller: chronos_domain::SymbolId) -> Vec<chronos_domain::SymbolId> {
        let mut callees: Vec<chronos_domain::SymbolId> = self
            .edges
            .iter()
            .filter(|e| e.caller == Some(caller))
            .map(|e| e.callee)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        callees.sort_by_key(|s| symbol_key(*s));
        callees
    }

    /// Total number of concrete calls into `callee` from any caller
    /// (including root calls where no caller symbol is recorded).
    pub fn call_count(&self, callee: chronos_domain::SymbolId) -> u64 {
        self.edges
            .iter()
            .filter(|e| e.callee == callee)
            .map(|e| e.calls)
            .sum()
    }

    /// Symbols called with no recorded caller (root call sites). Sorted.
    pub fn roots(&self) -> Vec<chronos_domain::SymbolId> {
        let mut roots: Vec<chronos_domain::SymbolId> = self
            .edges
            .iter()
            .filter(|e| e.caller.is_none())
            .map(|e| e.callee)
            .collect();
        roots.sort_by_key(|s| symbol_key(*s));
        roots
    }

    /// Symbols with a direct self-call edge (`caller == callee`), i.e.
    /// direct recursion. Sorted. (Indirect recursion — a cycle through
    /// intermediate symbols — is not detected in this v1.)
    pub fn recursive(&self) -> Vec<chronos_domain::SymbolId> {
        let mut rec: Vec<chronos_domain::SymbolId> = self
            .edges
            .iter()
            .filter(|e| e.caller == Some(e.callee))
            .map(|e| e.callee)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        rec.sort_by_key(|s| symbol_key(*s));
        rec
    }
}

/// Derive the symbol-level call graph for `session` from its concrete
/// invocation records.
///
/// Every record whose `symbol_id` is `Some(callee)` contributes one
/// concrete call of `callee`. The caller is resolved from
/// `parent_invocation_id`: if it is `None`, or does not resolve to any
/// record carrying a `symbol_id` in `session`, the edge is a root call
/// (`caller == None`).
///
/// v1 records (`symbol_id == None`) contribute nothing. An empty or
/// unknown session returns an empty graph.
///
/// See REQ-CallGraphBuild.
pub fn call_graph(log: &InMemoryExecutionLog, session: &SessionId) -> CallGraph {
    let snapshot: Vec<ExecutionRecord> = {
        let consumer = LogConsumerId::new("__call_graph__");
        match log.read_after(session.clone(), consumer, None).ok() {
            Some(crate::cursor::ReadResult::Ok { records, .. }) => records,
            _ => Vec::new(),
        }
    };

    // invocation_id -> symbol_id for every record that carries both.
    let mut invocation_symbol: HashMap<chronos_domain::InvocationId, chronos_domain::SymbolId> =
        HashMap::with_capacity(snapshot.len());
    for r in &snapshot {
        if let (Some(inv), Some(sym)) = (r.invocation_id, r.symbol_id) {
            invocation_symbol.insert(inv, sym);
        }
    }

    // Aggregate edge counts. Key: (caller Option, callee).
    let mut edge_counts: HashMap<
        (Option<chronos_domain::SymbolId>, chronos_domain::SymbolId),
        u64,
    > = HashMap::new();
    for r in &snapshot {
        let Some(callee) = r.symbol_id else { continue };
        let caller = match r.parent_invocation_id {
            Some(parent) => invocation_symbol.get(&parent).copied(),
            None => None,
        };
        *edge_counts.entry((caller, callee)).or_insert(0) += 1;
    }

    let mut edges: Vec<CallGraphEdge> = edge_counts
        .into_iter()
        .map(|((caller, callee), calls)| CallGraphEdge {
            caller,
            callee,
            calls,
        })
        .collect();
    edges.sort_by_key(edge_key);

    CallGraph { edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{ExecutionKind, ExecutionPayload};
    use crate::seq::EventSeq;
    use chronos_domain::{InvocationId, Language, SymbolId};

    fn sid(name: &str) -> SymbolId {
        SymbolId::new(name, None, Language::Rust)
    }

    fn mk_record(
        session: &SessionId,
        seq: u64,
        invocation: Option<InvocationId>,
        parent: Option<InvocationId>,
        symbol: Option<SymbolId>,
    ) -> ExecutionRecord {
        ExecutionRecord {
            session_id: session.clone(),
            seq: EventSeq::new(seq),
            monotonic_ns: seq * 10,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(Vec::new(), "ev"),
            invocation_id: invocation,
            parent_invocation_id: parent,
            symbol_id: symbol,
        }
    }

    #[test]
    fn build_two_sibling_edges_and_root() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-siblings");
        let a = InvocationId::now();
        let b = InvocationId::now();
        let c = InvocationId::now();
        let sa = sid("a");
        let sb = sid("b");
        let sc = sid("c");
        // a is a root (no parent); b and c are children of a.
        log.replay_record(&mk_record(&s, 0, Some(a), None, Some(sa)))
            .unwrap();
        log.replay_record(&mk_record(&s, 1, Some(b), Some(a), Some(sb)))
            .unwrap();
        log.replay_record(&mk_record(&s, 2, Some(c), Some(a), Some(sc)))
            .unwrap();

        let g = call_graph(&log, &s);
        assert_eq!(g.edges().len(), 3);
        assert!(g.roots().contains(&sa));
        assert_eq!(g.callers_of(sb), vec![sa]);
        assert!(g.recursive().is_empty());
        assert_eq!(g.call_count(sb), 1);
        assert_eq!(g.call_count(sa), 1);
        // callees_of(sa) == {sb, sc} regardless of hash order.
        let mut callees = g.callees_of(sa);
        callees.sort_by_key(|s| symbol_key(*s));
        let mut expect = vec![sb, sc];
        expect.sort_by_key(|s| symbol_key(*s));
        assert_eq!(callees, expect);
    }

    #[test]
    fn weighted_duplicate_call_edge() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-weight");
        let sa = sid("a");
        let sb = sid("b");
        let ra = InvocationId::now();
        let rb1 = InvocationId::now();
        let rb2 = InvocationId::now();
        // One root `a` calls `b` twice (two distinct b invocations, both
        // from the same a frame) => a single a->b edge of weight 2.
        log.replay_record(&mk_record(&s, 0, Some(ra), None, Some(sa)))
            .unwrap();
        log.replay_record(&mk_record(&s, 1, Some(rb1), Some(ra), Some(sb)))
            .unwrap();
        log.replay_record(&mk_record(&s, 2, Some(rb2), Some(ra), Some(sb)))
            .unwrap();
        let g = call_graph(&log, &s);
        let ab: Vec<_> = g
            .edges()
            .iter()
            .filter(|e| e.caller == Some(sa) && e.callee == sb)
            .collect();
        assert_eq!(ab.len(), 1, "one a->b edge");
        assert_eq!(ab[0].calls, 2, "weight 2");
        assert_eq!(g.call_count(sb), 2);
    }

    #[test]
    fn direct_recursion_detected() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-rec");
        let outer = InvocationId::now();
        let inner = InvocationId::now();
        let sfac = sid("factorial");
        // factorial(root) -> factorial(recursive).
        log.replay_record(&mk_record(&s, 0, Some(outer), None, Some(sfac)))
            .unwrap();
        log.replay_record(&mk_record(&s, 1, Some(inner), Some(outer), Some(sfac)))
            .unwrap();
        let g = call_graph(&log, &s);
        assert!(g.recursive().contains(&sfac), "direct recursion edge a->a");
        let self_edge: Vec<_> = g
            .edges()
            .iter()
            .filter(|e| e.caller == Some(sfac) && e.callee == sfac)
            .collect();
        assert_eq!(self_edge.len(), 1);
        assert_eq!(self_edge[0].calls, 1);
    }

    #[test]
    fn dangling_parent_folds_to_root() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-dangling");
        // b's parent is a, but a's record is NOT in the session (or is
        // a v1 record with no symbol). b should fold to a root call.
        let b = InvocationId::now();
        let sb = sid("b");
        let dangling_a = InvocationId::now();
        log.replay_record(&mk_record(&s, 0, Some(b), Some(dangling_a), Some(sb)))
            .unwrap();
        let g = call_graph(&log, &s);
        assert!(g.roots().contains(&sb), "dangling parent -> root");
        let root_edge: Vec<_> = g.edges().iter().filter(|e| e.callee == sb).collect();
        assert_eq!(root_edge.len(), 1);
        assert!(root_edge[0].caller.is_none());
    }

    #[test]
    fn v1_records_contribute_nothing() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-v1");
        // Three v1 records: no invocation/symbol fields.
        for i in 0..3u64 {
            log.replay_record(&mk_record(&s, i, None, None, None))
                .unwrap();
        }
        let g = call_graph(&log, &s);
        assert!(g.edges().is_empty());
        assert!(g.nodes().is_empty());
        assert!(g.roots().is_empty());
        assert_eq!(g.call_count(sid("x")), 0);
    }

    #[test]
    fn deterministic_edge_ordering() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("cg-order");
        let ra = InvocationId::now();
        let rb = InvocationId::now();
        let rc = InvocationId::now();
        let sa = sid("a");
        let sb = sid("b");
        let sc = sid("c");
        // root a, root c, and a->b — root edges sort first.
        log.replay_record(&mk_record(&s, 0, Some(ra), None, Some(sa)))
            .unwrap();
        log.replay_record(&mk_record(&s, 1, Some(rb), Some(ra), Some(sb)))
            .unwrap();
        log.replay_record(&mk_record(&s, 2, Some(rc), None, Some(sc)))
            .unwrap();
        let g = call_graph(&log, &s);
        // Two calls twice to confirm stable iteration.
        assert_eq!(g, call_graph(&log, &s));
        let first = &g.edges()[0];
        // root edges (caller None) sort before the a->b edge.
        assert!(first.caller.is_none());
    }
}
