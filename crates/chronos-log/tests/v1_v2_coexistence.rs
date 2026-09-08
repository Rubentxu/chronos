//! Coexistence tests: v1 records (no invocation/symbol fields) and v2
//! records (with invocation/symbol fields) must round-trip through
//! the same `ExecutionLog` without error and surface the v1 fields
//! as `None` on read.

use chronos_domain::{InvocationId, Language, SymbolId};
use chronos_log::{
    ExecutionPayload, LogConsumerId, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use std::num::NonZeroUsize;
use uuid::Uuid;

#[test]
fn v1_records_surface_none_on_new_fields() {
    // Build a v1 record by serializing without the new fields,
    // simulating an m1-08 producer writing to a segment.
    let v1_json = serde_json::json!({
        "session_id": SessionId::new("s-v1"),
        "seq": chronos_log::EventSeq::new(1),
        "monotonic_ns": 100u64,
        "kind": chronos_log::ExecutionKind::Raw,
        "payload": {
            "bytes": [1, 2, 3],
            "tag": "raw",
        },
    });

    let v1: chronos_log::ExecutionRecord =
        serde_json::from_value(v1_json).expect("v1 record must deserialize without the new fields");
    assert!(v1.invocation_id.is_none());
    assert!(v1.parent_invocation_id.is_none());
    assert!(v1.symbol_id.is_none());
    assert_eq!(v1.schema_version(), "chronos_exec_v1");
}

#[test]
fn v2_records_round_trip_with_populated_fields() {
    let inv_id = InvocationId::now();
    let parent_id = InvocationId::now();
    let sym_id = SymbolId::new("factorial", None, Language::C);

    let session = SessionId::new("s-v2");
    let record = NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 200,
        payload: ExecutionPayload::new(vec![4, 5, 6], "raw"),
        invocation_id: Some(inv_id),
        parent_invocation_id: Some(parent_id),
        symbol_id: Some(sym_id),
    };

    let dir = std::env::temp_dir().join(format!("chronos-log-v2-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(8).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
    log.append(record).unwrap();
    log.flush().ok();

    let consumer = LogConsumerId::new("c1");
    let read = log.read_after(&consumer, None).unwrap();

    let records: Vec<chronos_log::ExecutionRecord> = match read {
        chronos_log::ReadResult::Ok { records, .. } => records,
        other => panic!("expected ReadResult::Ok, got {:?}", other),
    };
    assert_eq!(records.len(), 1, "expected exactly one record on read");
    let r = &records[0];
    assert_eq!(r.invocation_id, Some(inv_id));
    assert_eq!(r.parent_invocation_id, Some(parent_id));
    assert_eq!(r.symbol_id, Some(sym_id));
    assert_eq!(r.schema_version(), "chronos_exec_v2");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn symbol_id_is_stable_across_calls() {
    let a = SymbolId::new("factorial", Some("(I)I"), Language::Java);
    let b = SymbolId::new("factorial", Some("(I)I"), Language::Java);
    assert_eq!(a, b);

    let c = SymbolId::new("factorial", Some("(II)I"), Language::Java);
    assert_ne!(a, c);

    let d = SymbolId::new("factorial", None, Language::C);
    assert_ne!(a, d);
}

#[test]
fn invocation_ids_are_unique_across_calls() {
    let mut ids = Vec::new();
    for _ in 0..32 {
        ids.push(InvocationId::now());
    }
    let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "32 calls must yield 32 distinct ids"
    );
}
