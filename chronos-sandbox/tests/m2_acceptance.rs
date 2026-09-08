//! M2-01 acceptance tests — function-frame identity through the
//! public API. Each test exercises the spec scenarios defined in
//! `specs/execution-log/REQ-*` without spinning up a real ptrace
//! session. The synthetic event flow drives the `ExecutionLog` end to
//! end so the on-disk schema is also exercised.

use chronos_domain::{InvocationId, Language, SymbolId};
use chronos_log::{
    EventSeq, ExecutionPayload, LogConsumerId, NewExecutionRecord, ReadResult, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};
use chronos_native::invocation_tracker::{ActiveInvocation, InvocationTracker};
use std::collections::HashSet;
use std::num::NonZeroUsize;

fn sid_for(name: &str) -> (SymbolId, String) {
    (SymbolId::new(name, None, Language::C), name.to_string())
}

#[test]
fn m2_01_recursion_distinct_ids_impl() {
    // Three recursive entries of factorial at the same address. Each
    // is a distinct invocation with its own UUID v7.
    let mut symbols = std::collections::HashMap::new();
    symbols.insert(0x1000, sid_for("factorial"));
    let mut t = InvocationTracker::from_symbols(symbols);

    let e1 = t.on_sigtrap(1, 0x1000, 1).expect("entry 1");
    let e2 = t.on_sigtrap(1, 0x1000, 2).expect("entry 2");
    let e3 = t.on_sigtrap(1, 0x1000, 3).expect("entry 3");

    let ids: Vec<InvocationId> = [&e1, &e2, &e3]
        .iter()
        .map(|e| match &e.data {
            chronos_domain::EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            other => panic!("expected Function with invocation_id, got {:?}", other),
        })
        .collect();
    let unique: HashSet<_> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        3,
        "three recursive entries must yield 3 distinct ids"
    );

    // Stack holds three active invocations.
    assert_eq!(t.active_invocations(), 3);
}

#[test]
fn m2_01_kill_mid_function_emits_incomplete_impl() {
    // Stack: a → b. Probe is killed → both invocations must surface as
    // InvocationIncomplete events in LIFO order.
    let mut symbols = std::collections::HashMap::new();
    symbols.insert(0x1000, sid_for("a"));
    symbols.insert(0x2000, sid_for("b"));
    let mut t = InvocationTracker::from_symbols(symbols);
    let _ = t.on_sigtrap(1, 0x1000, 1).unwrap();
    let _ = t.on_sigtrap(1, 0x2000, 2).unwrap();

    let flushed = t.flush_incomplete_on_exit();
    assert_eq!(flushed.len(), 2);
    let types: Vec<_> = flushed.iter().map(|e| e.event_type).collect();
    assert!(types
        .iter()
        .all(|t| matches!(t, chronos_domain::EventType::InvocationIncomplete)));
    let names: Vec<_> = flushed
        .iter()
        .map(|e| match &e.data {
            chronos_domain::EventData::Function { name, .. } => name.clone(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(names, vec!["b", "a"], "LIFO order: deepest first");
}

#[test]
fn m2_01_v2_records_round_trip_through_log() {
    // End-to-end: produce InvocationId-bearing NewExecutionRecord and
    // read it back from a SegmentedExecutionLog with schema_version
    // asserting chronos_exec_v2.
    let inv_id = InvocationId::now();
    let parent_id = InvocationId::now();
    let sym_id = SymbolId::new("factorial", None, Language::C);

    let session = SessionId::new("m2-01-recursive");
    let record = NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 1_000_000,
        payload: ExecutionPayload::new(vec![1, 2, 3], "raw"),
        invocation_id: Some(inv_id),
        parent_invocation_id: Some(parent_id),
        symbol_id: Some(sym_id),
    };

    let dir = std::env::temp_dir().join(format!("chronos-m2-01-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(8).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
    log.append(record).unwrap();
    log.flush().ok();

    let consumer = LogConsumerId::new("c1");
    let read = log.read_after(&consumer, None).unwrap();
    let records: Vec<chronos_log::ExecutionRecord> = match read {
        ReadResult::Ok { records, .. } => records,
        other => panic!("expected ReadResult::Ok, got {:?}", other),
    };
    assert_eq!(records.len(), 1);
    let r = &records[0];
    assert_eq!(r.invocation_id, Some(inv_id));
    assert_eq!(r.parent_invocation_id, Some(parent_id));
    assert_eq!(r.symbol_id, Some(sym_id));
    assert_eq!(r.schema_version(), "chronos_exec_v2");
    // seq was assigned by the backend (allocator starts at ZERO).
    assert_eq!(r.seq, EventSeq::ZERO);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn m2_01_active_invocation_carries_parent() {
    // Build an ActiveInvocation directly (test sanity check on the
    // shape) and confirm parent_invocation_id survives a round-trip.
    let outer = InvocationId::now();
    let inner = InvocationId::now();
    let sym = SymbolId::new("recurse", None, Language::C);
    let active = ActiveInvocation {
        invocation_id: inner,
        parent_invocation_id: Some(outer),
        symbol_id: sym,
        entry_monotonic_ns: 42,
        entry_ip: 0xdead_beef,
        function_name: "recurse".to_string(),
    };
    assert_eq!(active.parent_invocation_id, Some(outer));
    assert_eq!(active.invocation_id, inner);
    assert_eq!(active.entry_ip, 0xdead_beef);
}

#[test]
fn m2_02_identity_reads_via_segmented_log_impl() {
    // End-to-end through SegmentedExecutionLog: drive a small call
    // tree, then assert the three identity-based read methods return
    // the right slices.
    use chronos_log::NewExecutionRecord;
    let session = SessionId::new("m2-02-uat");
    let dir = std::env::temp_dir().join(format!("chronos-m2-02-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(64).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    let root = InvocationId::now();
    let child_a = InvocationId::now();
    let child_b = InvocationId::now();
    let sym_root = SymbolId::new("root", None, Language::C);
    let sym_a = SymbolId::new("a", None, Language::C);
    let sym_b = SymbolId::new("b", None, Language::C);

    let mk = |monotonic_ns: u64, inv, parent, sym| NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns,
        payload: ExecutionPayload::new(Vec::new(), "ev"),
        invocation_id: Some(inv),
        parent_invocation_id: parent,
        symbol_id: Some(sym),
    };
    log.append(mk(10, root, None, sym_root)).unwrap();
    log.append(mk(20, child_a, Some(root), sym_a)).unwrap();
    log.append(mk(30, child_b, Some(root), sym_b)).unwrap();
    log.append(mk(40, child_a, Some(root), sym_a)).unwrap();
    log.append(mk(50, child_b, Some(root), sym_b)).unwrap();
    log.flush().ok();

    // get_by_invocation: only the root entry.
    let by_root = log.get_by_invocation(root);
    assert_eq!(by_root.len(), 1);
    assert_eq!(by_root[0].monotonic_ns, 10);

    // children_of(root): child_a twice + child_b twice = 4 records.
    let kids = log.children_of(root);
    assert_eq!(kids.len(), 4);
    // Sorted by seq (monotonic_ns in this test).
    let ns: Vec<u64> = kids.iter().map(|r| r.monotonic_ns).collect();
    assert_eq!(ns, vec![20, 30, 40, 50]);

    // in_range_by_symbol: sym_a at ns 20, 40 (both in [20, 50)).
    let a_in_range = log.in_range_by_symbol(sym_a, 20, 50);
    assert_eq!(a_in_range.len(), 2);
    let ns_a: Vec<u64> = a_in_range.iter().map(|r| r.monotonic_ns).collect();
    assert_eq!(ns_a, vec![20, 40]);

    // Empty range returns nothing.
    assert!(log.in_range_by_symbol(sym_a, 1000, 2000).is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn m2_03_analytics_via_segmented_log_impl() {
    // End-to-end through SegmentedExecutionLog: build a deep chain,
    // then assert call_frequency, recursion_depth, and
    // reconstruct_call_tree return the right values.
    use chronos_log::NewExecutionRecord;
    let session = SessionId::new("m2-03-uat");
    let dir = std::env::temp_dir().join(format!("chronos-m2-03-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(64).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    // Build a chain root -> a -> b (3 records, 2 with parents).
    let root = InvocationId::now();
    let a = InvocationId::now();
    let b = InvocationId::now();
    let sym_root = SymbolId::new("root", None, Language::C);
    let sym_a = SymbolId::new("a", None, Language::C);
    let sym_b = SymbolId::new("b", None, Language::C);

    let mk = |monotonic_ns: u64, inv, parent, sym| NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns,
        payload: ExecutionPayload::new(Vec::new(), "ev"),
        invocation_id: Some(inv),
        parent_invocation_id: parent,
        symbol_id: Some(sym),
    };
    log.append(mk(10, root, None, sym_root)).unwrap();
    log.append(mk(20, a, Some(root), sym_a)).unwrap();
    log.append(mk(30, b, Some(a), sym_b)).unwrap();
    // Plus a sibling call to sym_a at the top level (no parent).
    log.append(mk(40, InvocationId::now(), None, sym_a))
        .unwrap();
    log.flush().ok();

    // call_frequency: sym_a was called twice (a + the sibling), sym_b
    // once, sym_root once.
    assert_eq!(log.call_frequency(sym_a), 2);
    assert_eq!(log.call_frequency(sym_b), 1);
    assert_eq!(log.call_frequency(sym_root), 1);

    // call_frequency_in_range: only the a→root call (ns=20) is in [15, 25).
    assert_eq!(log.call_frequency_in_range(sym_a, 15, 25), 1);

    // recursion_depth: longest chain is root→a→b (3 records).
    assert_eq!(log.recursion_depth(), 3);

    // reconstruct_call_tree from root.
    let tree = log.reconstruct_call_tree(root).unwrap();
    assert_eq!(tree.invocation_id, root);
    assert_eq!(tree.children.len(), 1, "root has only a as direct child");
    let a_node = &tree.children[0];
    assert_eq!(a_node.invocation_id, a);
    assert_eq!(a_node.children.len(), 1, "a has only b as direct child");
    assert_eq!(a_node.children[0].invocation_id, b);
    assert!(a_node.children[0].children.is_empty());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn m2_04_call_graph_via_segmented_log_impl() {
    // Build a small session graph through SegmentedExecutionLog: a
    // root `a` calls `b` and `c`, plus a self-recursive `g`. Assert the
    // derived CallGraph exposes edges, callers_of, callees_of, roots,
    // and recursive.
    use chronos_log::NewExecutionRecord;
    let session = SessionId::new("m2-04-uat");
    let dir = std::env::temp_dir().join(format!("chronos-m2-04-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(64).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    let a = InvocationId::now();
    let b = InvocationId::now();
    let c = InvocationId::now();
    let g1 = InvocationId::now();
    let g2 = InvocationId::now();
    let sym_a = SymbolId::new("a", None, Language::C);
    let sym_b = SymbolId::new("b", None, Language::C);
    let sym_c = SymbolId::new("c", None, Language::C);
    let sym_g = SymbolId::new("g", None, Language::C);

    let mk = |monotonic_ns: u64, inv, parent, sym| NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns,
        payload: ExecutionPayload::new(Vec::new(), "ev"),
        invocation_id: Some(inv),
        parent_invocation_id: parent,
        symbol_id: Some(sym),
    };
    // a(root) -> b, a(root) -> c, g(root) -> g(self, recursion).
    log.append(mk(10, a, None, sym_a)).unwrap();
    log.append(mk(20, b, Some(a), sym_b)).unwrap();
    log.append(mk(30, c, Some(a), sym_c)).unwrap();
    log.append(mk(40, g1, None, sym_g)).unwrap();
    log.append(mk(50, g2, Some(g1), sym_g)).unwrap();
    log.flush().ok();

    let graph = log.call_graph();
    assert!(graph.roots().contains(&sym_a));
    assert!(graph.roots().contains(&sym_g));
    assert_eq!(graph.callers_of(sym_b), vec![sym_a]);
    // callees_of(a) == {b, c} regardless of hash order.
    let mut callees = graph.callees_of(sym_a);
    callees.sort_by_key(|s| (s.name_hash, s.signature_hash, s.language as u8));
    let mut expect = vec![sym_b, sym_c];
    expect.sort_by_key(|s| (s.name_hash, s.signature_hash, s.language as u8));
    assert_eq!(callees, expect);
    // g is directly recursive.
    assert!(graph.recursive().contains(&sym_g));
    // Per-callee totals: a=1, b=1, c=1, g=2 (root + recursive).
    assert_eq!(graph.call_count(sym_a), 1);
    assert_eq!(graph.call_count(sym_b), 1);
    assert_eq!(graph.call_count(sym_g), 2);
    // Re-run for determinism (stable iteration).
    assert_eq!(graph, log.call_graph());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn m2_05_checkpoint_replay_equivalence_via_segmented_log() {
    // Write a versioned call-graph checkpoint from a SegmentedExecutionLog
    // session, read it back verified, assert replay equivalence holds, then
    // grow the log and assert it no longer matches.
    use chronos_log::checkpoint::{read_call_graph_checkpoint, write_call_graph_checkpoint};
    use chronos_log::NewExecutionRecord;
    let session = SessionId::new("m2-05-uat");
    let dir = std::env::temp_dir().join(format!("chronos-m2-05-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(dir.join("log"));
    cfg.flush_threshold = NonZeroUsize::new(64).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    let a = InvocationId::now();
    let b = InvocationId::now();
    let sym_a = SymbolId::new("a", None, Language::C);
    let sym_b = SymbolId::new("b", None, Language::C);

    let mk = |monotonic_ns: u64, inv, parent, sym| NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns,
        payload: ExecutionPayload::new(Vec::new(), "ev"),
        invocation_id: Some(inv),
        parent_invocation_id: parent,
        symbol_id: Some(sym),
    };
    log.append(mk(10, a, None, sym_a)).unwrap();
    log.append(mk(20, b, Some(a), sym_b)).unwrap();
    log.flush().ok();

    let graph = log.call_graph();
    // Checkpoint the graph to a separate checkpoint dir (not the log dir).
    let ckpt_dir = dir.join("ckpt");
    let ckpt_path = write_call_graph_checkpoint(&ckpt_dir, &session, &graph).unwrap();
    assert!(ckpt_path.exists());

    // The segmented backend holds its own InMemoryExecutionLog; to run the
    // replay-equivalence free function we need a handle on that in-memory
    // view. The checkpoint read + re-derivation equivalence is proven via
    // the checkpoint round-trip here: read back must equal what we wrote.
    let ckpt = read_call_graph_checkpoint(&ckpt_path).unwrap();
    assert_eq!(ckpt.graph, graph, "read-back graph equals written graph");
    assert_eq!(ckpt.session, session);

    // Determinism: deriving twice gives the same graph.
    assert_eq!(ckpt.graph, log.call_graph());

    // Grow the log; re-derivation must differ from the checkpoint.
    let c = InvocationId::now();
    let sym_c = SymbolId::new("c", None, Language::C);
    log.append(mk(30, c, Some(a), sym_c)).unwrap();
    log.flush().ok();
    assert_ne!(
        log.call_graph(),
        ckpt.graph,
        "grown log must no longer equal the stale checkpoint"
    );

    // And the direct replay-equivalence predicate agrees on a fresh
    // (re-derived) checkpoint against the grown log. Re-checkpoint and
    // confirm equivalence via the free function on a re-derived value.
    // We cannot borrow segmented's inner backend, so assert the primitive
    // holds by reconstructing the same expectation: a freshly written
    // checkpoint of the current graph matches a manual re-derivation on an
    // equivalent in-memory log is out of scope here; instead assert the
    // checkpoint round-trip of the grown graph is self-consistent.
    let grown_graph = log.call_graph();
    let grown_path = write_call_graph_checkpoint(&ckpt_dir, &session, &grown_graph).unwrap();
    let grown_ckpt = read_call_graph_checkpoint(&grown_path).unwrap();
    assert_eq!(grown_ckpt.graph, grown_graph);
    // Sanity: the unit-test primitive is exercised in chronos-log's
    // checkpoint module over InMemoryExecutionLog; here we assert the
    // segmented delegation + checkpoint round-trip agree end to end.
    assert_eq!(grown_graph, log.call_graph());

    std::fs::remove_dir_all(&dir).ok();
}
