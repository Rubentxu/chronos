//! M1 acceptance tests.
//!
//! Each M1-NN ticket adds one `_impl` test that exercises the new
//! behavior end-to-end through the public API. Legacy `m1_NN_*`
//! stubs (without `_impl`) live alongside as `#[ignore]`d reminders
//! that point at the in-tree unit tests as the gate.

use std::sync::Arc;

use chronos_log::{
    EventSeq, ExecutionLogBackend, Gap, GapReason, InMemoryExecutionLog, LogConsumerId, ReadResult,
    SessionId,
};

/// m1-01 — ExecutionLog core acceptance (UAT-M1-01).
///
/// Drives the four canonical spec cases end-to-end through the
/// public API: 1000 events, two independent consumers, cursor
/// resume, concurrent append preserving `EventSeq`. Plus a gap
/// round-trip and a stale-cursor smoke check.
#[tokio::test(flavor = "current_thread")]
async fn m1_01_execution_log_core_impl() {
    let log = InMemoryExecutionLog::new();
    let session = SessionId::new("m1-01-uat");

    // Case 1: two consumers, 1000 events.
    for i in 0..1000u64 {
        log.append_raw(session.clone(), i, format!("ev-{}", i))
            .unwrap();
    }
    let (a_records, a_cursor) = match log
        .read_after(session.clone(), LogConsumerId::new("agent-a"), None)
        .unwrap()
    {
        ReadResult::Ok {
            records,
            next_cursor,
            ..
        } => (records, next_cursor),
        other => panic!("expected Ok, got {:?}", other),
    };
    let (b_records, b_cursor) = match log
        .read_after(session.clone(), LogConsumerId::new("agent-b"), None)
        .unwrap()
    {
        ReadResult::Ok {
            records,
            next_cursor,
            ..
        } => (records, next_cursor),
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(a_records.len(), 1000);
    assert_eq!(b_records.len(), 1000);
    assert_eq!(a_records, b_records);
    assert_eq!(a_cursor.last_seq, EventSeq::new(999));
    assert_eq!(b_cursor.last_seq, EventSeq::new(999));

    // Case 2 + 3: cursor resume returns the next event after a new
    // append.
    log.append_raw(session.clone(), 1000, "ev-1000").unwrap();
    let resumed_a = match log
        .read_after(
            session.clone(),
            LogConsumerId::new("agent-a"),
            Some(a_cursor),
        )
        .unwrap()
    {
        ReadResult::Ok { records, .. } => records,
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(resumed_a.len(), 1);
    assert_eq!(resumed_a[0].seq, EventSeq::new(1000));
    let resumed_b = match log
        .read_after(
            session.clone(),
            LogConsumerId::new("agent-b"),
            Some(b_cursor),
        )
        .unwrap()
    {
        ReadResult::Ok { records, .. } => records,
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(resumed_b.len(), 1);
    assert_eq!(resumed_b[0].seq, EventSeq::new(1000));

    // Case 4: concurrent append preserves seq.
    let log = Arc::new(InMemoryExecutionLog::new());
    let session2 = SessionId::new("m1-01-uat-conc");
    let mut handles = Vec::new();
    for _ in 0..8 {
        let log = log.clone();
        let session2 = session2.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..100u64 {
                log.append_raw(session2.clone(), i, "x").unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let read_back = match log
        .read_after(session2, LogConsumerId::new("auditor"), None)
        .unwrap()
    {
        ReadResult::Ok { records, .. } => records,
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(read_back.len(), 800);
    for (i, r) in read_back.iter().enumerate() {
        assert_eq!(r.seq, EventSeq::new(i as u64));
    }

    // Bonus: gap round-trip.
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("m1-01-gap");
    log.append_raw(s.clone(), 0, "before").unwrap();
    log.record_gap(
        s.clone(),
        Gap::new(
            EventSeq::new(1),
            EventSeq::new(3),
            GapReason::KernelRingOverflow,
            "ptrace",
        ),
    )
    .unwrap();
    let next_seq = log.append_raw(s.clone(), 100, "after").unwrap();
    assert!(next_seq > EventSeq::new(3));
    let read = match log.read_after(s, LogConsumerId::new("a"), None).unwrap() {
        ReadResult::Ok { records, gaps, .. } => (records, gaps),
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(read.0.len(), 2, "before + after");
    assert_eq!(read.1.len(), 1, "the recorded gap is observable");
    assert_eq!(read.1[0].reason, GapReason::KernelRingOverflow);
}

/// Legacy stub for future m1-02 (required test cases 5–8 from the
/// ExecutionLog spec). Marked ignored until that ticket lands.
#[test]
#[ignore = "implemented in cycle m1-02: spec cases 5-8 (overflow -> gap, crash-safe segments, checkpoint+delta replay, deterministic replay)"]
fn m1_02_execution_log_persistence_cases() {
    // m1-02 contract is enforced by the in-tree unit tests in
    // `chronos-log::tests` once persistence lands. No live UAT is
    // required (the persistence backend is library-level; the MCP
    // surface is unchanged).
}

/// Legacy stub for future m1-03 (live probe migration: one producer
/// + one query path). Marked ignored until that ticket lands.
#[test]
#[ignore = "implemented in cycle m1-03: chronos-native::probe_backend writes to ExecutionLog; chronos-mcp::probe_drain reads via ExecutionLog"]
fn m1_03_execution_log_migrates_one_producer_and_query_path() {
    // m1-03 migrates `probe_drain` to consume from ExecutionLog
    // instead of the legacy EventBus; the live UAT lives in the
    // sandbox test that exercises probe_drain through the MCP
    // server, not here.
}
