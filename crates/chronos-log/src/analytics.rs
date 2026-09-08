//! Analytics over the M2 identity-based read surface.
//!
//! These functions derive aggregate metrics from the records held by
//! an `InMemoryExecutionLog`. They reuse the m2-02 reads
//! (`get_by_invocation`, `children_of`, `in_range_by_symbol`) — no
//! new indexes needed.
//!
//! Concurrency: every function takes `&InMemoryExecutionLog` and
//! reads under the same `Mutex` the read methods use. The lock is
//! held briefly for `call_frequency` / `call_frequency_in_range` and
//! for the duration of tree reconstruction in
//! `reconstruct_call_tree` (the worst case is a tree with W^D nodes).
//!
//! Out of scope for m2-03: per-invocation duration (deferred until
//! exit detection lands in m2-04+).

use crate::backend::ExecutionLogBackend;
use crate::cursor::LogConsumerId;
use crate::memory::InMemoryExecutionLog;
use crate::record::{ExecutionRecord, SessionId};
use crate::seq::EventSeq;

use std::collections::HashSet;

/// A node in a reconstructed call tree. `children` is sorted by
/// `seq` ascending so callers can walk the tree deterministically
/// without re-sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTreeNode {
    pub invocation_id: chronos_domain::InvocationId,
    pub symbol_id: Option<chronos_domain::SymbolId>,
    pub parent_invocation_id: Option<chronos_domain::InvocationId>,
    pub monotonic_ns: u64,
    pub seq: EventSeq,
    pub children: Vec<CallTreeNode>,
}

/// Total number of records in `session` whose `symbol_id ==
/// Some(symbol)`. Records with `symbol_id == None` (v1) are not
/// counted. Unknown symbol or unknown session returns 0.
///
/// See REQ-CallFrequency.
pub fn call_frequency(
    log: &InMemoryExecutionLog,
    session: &SessionId,
    symbol: chronos_domain::SymbolId,
) -> u64 {
    log.in_range_by_symbol(session, symbol, 0, u64::MAX).len() as u64
}

/// Same as `call_frequency` but restricted to records whose
/// `monotonic_ns` lies in the half-open range `[start_ns, end_ns)`.
///
/// See REQ-CallFrequencyInRange.
pub fn call_frequency_in_range(
    log: &InMemoryExecutionLog,
    session: &SessionId,
    symbol: chronos_domain::SymbolId,
    start_ns: u64,
    end_ns: u64,
) -> u64 {
    log.in_range_by_symbol(session, symbol, start_ns, end_ns)
        .len() as u64
}

/// Maximum length of any `parent_invocation_id` chain across all
/// records in `session`.
///
/// The chain length is 1 for a root invocation (no parent), 2 for a
/// frame called directly from a root, etc. v1 records (None on every
/// identity field) contribute nothing to the depth. Returns 0 for an
/// empty session.
///
/// See REQ-RecursionDepth.
pub fn recursion_depth(log: &InMemoryExecutionLog, session: &SessionId) -> usize {
    // Snapshot every record once (under the records lock briefly),
    // then walk each one's parent chain outside the lock. This bounds
    // lock-hold time to O(N) for the snapshot regardless of chain
    // length.
    let snapshot: Vec<ExecutionRecord> = {
        // Accessing the records Vec directly would require a public
        // accessor that doesn't exist; we walk it via entry_count
        // and the identity indexes' records. The cheapest path is
        // "read every record that carries an invocation_id" via
        // get_by_invocation over a per-session scan... but we have no
        // per-session list. So instead we expose a private helper
        // through a back-door: ask each known invocation_index entry
        // for its records and union.
        //
        // Practical approach: read every record in the session by
        // walking the records map via the public read_after API.
        // The cheapest path is "give me every record" with a fresh
        // cursor.
        let consumer = LogConsumerId::new("__analytics_depth__");
        match log.read_after(session.clone(), consumer, None).ok() {
            Some(crate::cursor::ReadResult::Ok { records, .. }) => records,
            _ => Vec::new(),
        }
    };
    let mut max_depth = 0usize;
    let mut seen: HashSet<chronos_domain::InvocationId> = HashSet::new();
    for r in &snapshot {
        let Some(inv) = r.invocation_id else { continue };
        if seen.contains(&inv) {
            continue;
        }
        let depth = chain_depth(&snapshot, inv);
        if depth > max_depth {
            max_depth = depth;
        }
        seen.insert(inv);
    }
    max_depth
}

/// Walk a parent_invocation_id chain starting at `start`, counting
/// the number of distinct invocation_ids in the chain (including
/// the start). Returns 0 if `start` isn't present in `records` at
/// all (which means a v1 record's parent pointed at a v1 record).
pub(crate) fn chain_depth(
    records: &[ExecutionRecord],
    start: chronos_domain::InvocationId,
) -> usize {
    // Build a local id → parent_invocation_id map for the records we
    // have in hand. O(N) once; reused by every chain walk.
    let mut parents: std::collections::HashMap<
        chronos_domain::InvocationId,
        Option<chronos_domain::InvocationId>,
    > = std::collections::HashMap::with_capacity(records.len());
    for r in records {
        if let Some(inv) = r.invocation_id {
            parents.insert(inv, r.parent_invocation_id);
        }
    }
    let mut current = Some(start);
    let mut depth = 0usize;
    let mut visited: HashSet<chronos_domain::InvocationId> = HashSet::new();
    while let Some(inv) = current {
        if !visited.insert(inv) {
            // Cycle (shouldn't happen in well-formed traces; the
            // tracker always links to a previously-emitted parent).
            break;
        }
        depth += 1;
        current = parents.get(&inv).copied().flatten();
    }
    depth
}

/// Reconstruct the call tree rooted at `root_id`. Returns `None` if
/// no record in `session` carries `invocation_id == Some(root_id)`.
///
/// Recursion depth is bounded by the tree depth (not the tree width
/// × depth), which is good. Tree width is unbounded — the caller is
/// responsible for not invoking this on adversarial inputs.
///
/// See REQ-ReconstructCallTree.
pub fn reconstruct_call_tree(
    log: &InMemoryExecutionLog,
    session: &SessionId,
    root_id: chronos_domain::InvocationId,
) -> Option<CallTreeNode> {
    let root_records = log.get_by_invocation(session, root_id);
    let root_record = root_records.into_iter().next()?;
    Some(build_node(log, session, root_record))
}

fn build_node(
    log: &InMemoryExecutionLog,
    session: &SessionId,
    record: ExecutionRecord,
) -> CallTreeNode {
    // Children can only exist if this record carries an
    // invocation_id — only then does parent_index have an entry for
    // it. v1 records (None invocation_id) are leaves by definition.
    let children = match record.invocation_id {
        Some(inv) => {
            let mut children_records = log.children_of(session, inv);
            // Sort by seq so the tree is deterministic.
            children_records.sort_by_key(|r| r.seq);
            children_records
                .into_iter()
                .map(|c| build_node(log, session, c))
                .collect()
        }
        None => Vec::new(),
    };
    CallTreeNode {
        // unwrap is safe for the root (filtered by get_by_invocation)
        // but intermediate v1 records can be leaves with no
        // invocation_id. Use a sentinel in that case.
        invocation_id: record
            .invocation_id
            .unwrap_or_else(chronos_domain::InvocationId::now),
        symbol_id: record.symbol_id,
        parent_invocation_id: record.parent_invocation_id,
        monotonic_ns: record.monotonic_ns,
        seq: record.seq,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{ExecutionKind, ExecutionPayload};

    fn mk_record(
        session_id: SessionId,
        seq: u64,
        monotonic_ns: u64,
        invocation: Option<chronos_domain::InvocationId>,
        parent: Option<chronos_domain::InvocationId>,
        symbol: Option<chronos_domain::SymbolId>,
    ) -> ExecutionRecord {
        ExecutionRecord {
            session_id,
            seq: EventSeq::new(seq),
            monotonic_ns,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(Vec::new(), "ev"),
            invocation_id: invocation,
            parent_invocation_id: parent,
            symbol_id: symbol,
        }
    }

    #[test]
    fn call_frequency_counts_only_matching_symbol() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("freq");
        let sym = chronos_domain::SymbolId::new("foo", None, chronos_domain::Language::Rust);
        let other = chronos_domain::SymbolId::new("bar", None, chronos_domain::Language::Rust);
        let inv = chronos_domain::InvocationId::now();
        for i in 0..5 {
            log.replay_record(&mk_record(
                s.clone(),
                i,
                i * 10,
                Some(inv),
                None,
                if i == 4 { Some(other) } else { Some(sym) },
            ))
            .unwrap();
        }
        assert_eq!(call_frequency(&log, &s, sym), 4);
        assert_eq!(call_frequency(&log, &s, other), 1);
        let unknown = chronos_domain::SymbolId::new("nope", None, chronos_domain::Language::Rust);
        assert_eq!(call_frequency(&log, &s, unknown), 0);
    }

    #[test]
    fn call_frequency_in_range_respects_half_open_window() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("range");
        let sym = chronos_domain::SymbolId::new("foo", None, chronos_domain::Language::Rust);
        let inv = chronos_domain::InvocationId::now();
        for ns in [0u64, 10, 20, 30, 40] {
            log.replay_record(&mk_record(s.clone(), ns, ns, Some(inv), None, Some(sym)))
                .unwrap();
        }
        // [10, 30) hits ns=10,20 (not 0, not 30).
        assert_eq!(call_frequency_in_range(&log, &s, sym, 10, 30), 2);
        // Empty range returns 0.
        assert_eq!(call_frequency_in_range(&log, &s, sym, 100, 200), 0);
    }

    #[test]
    fn recursion_depth_returns_max_chain_length() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("depth");
        // Build a chain a -> b -> c -> d (4 records, 3 of which have
        // parents). max chain = 4.
        let a = chronos_domain::InvocationId::now();
        let b = chronos_domain::InvocationId::now();
        let c = chronos_domain::InvocationId::now();
        let d = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);
        log.replay_record(&mk_record(s.clone(), 0, 0, Some(a), None, Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 1, 1, Some(b), Some(a), Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 2, 2, Some(c), Some(b), Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 3, 3, Some(d), Some(c), Some(sym)))
            .unwrap();
        assert_eq!(recursion_depth(&log, &s), 4);
        // Single root -> depth 1.
        let s2 = SessionId::new("depth2");
        let inv = chronos_domain::InvocationId::now();
        log.replay_record(&mk_record(s2.clone(), 0, 0, Some(inv), None, Some(sym)))
            .unwrap();
        assert_eq!(recursion_depth(&log, &s2), 1);
        // Empty session -> 0.
        let empty = SessionId::new("empty");
        assert_eq!(recursion_depth(&log, &empty), 0);
    }

    #[test]
    fn reconstruct_call_tree_returns_none_for_unknown_root() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("unknown-root");
        let unknown = chronos_domain::InvocationId::now();
        assert!(reconstruct_call_tree(&log, &s, unknown).is_none());
    }

    #[test]
    fn reconstruct_call_tree_builds_deep_chain() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("deep");
        let a = chronos_domain::InvocationId::now();
        let b = chronos_domain::InvocationId::now();
        let c = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);
        log.replay_record(&mk_record(s.clone(), 0, 0, Some(a), None, Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 1, 1, Some(b), Some(a), Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 2, 2, Some(c), Some(b), Some(sym)))
            .unwrap();
        let tree = reconstruct_call_tree(&log, &s, a).unwrap();
        assert_eq!(tree.invocation_id, a);
        assert_eq!(tree.children.len(), 1);
        let b_node = &tree.children[0];
        assert_eq!(b_node.invocation_id, b);
        assert_eq!(b_node.children.len(), 1);
        let c_node = &b_node.children[0];
        assert_eq!(c_node.invocation_id, c);
        assert!(c_node.children.is_empty());
    }

    #[test]
    fn reconstruct_call_tree_groups_siblings() {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("siblings");
        let root = chronos_domain::InvocationId::now();
        let a = chronos_domain::InvocationId::now();
        let b = chronos_domain::InvocationId::now();
        let sym = chronos_domain::SymbolId::new("f", None, chronos_domain::Language::Rust);
        log.replay_record(&mk_record(s.clone(), 0, 0, Some(root), None, Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 1, 1, Some(a), Some(root), Some(sym)))
            .unwrap();
        log.replay_record(&mk_record(s.clone(), 2, 2, Some(b), Some(root), Some(sym)))
            .unwrap();
        let tree = reconstruct_call_tree(&log, &s, root).unwrap();
        assert_eq!(tree.children.len(), 2);
        let names: Vec<chronos_domain::InvocationId> =
            tree.children.iter().map(|c| c.invocation_id).collect();
        assert!(names.contains(&a));
        assert!(names.contains(&b));
    }
}
