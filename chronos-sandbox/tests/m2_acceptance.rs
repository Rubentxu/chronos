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
