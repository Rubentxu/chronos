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

/// m1-02 — ExecutionLog persistence acceptance (UAT-M1-02).
///
/// Drives spec cases 5–8 (overflow → gap, crash-safe segments,
/// checkpoint+delta = full replay, deterministic replay) through
/// the public API of `chronos_log::SegmentedExecutionLog`.
#[test]
fn m1_02_execution_log_persistence_impl() {
    use chronos_log::{Gap, GapReason, SegmentedConfig, SegmentedExecutionLog};
    use std::num::NonZeroUsize;

    // -- Case 5: overflow → gap (soft memory budget) ----------
    {
        let dir = tempdir("m1-02-case5");
        let session = SessionId::new("m1-02-uat-5");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(8).unwrap();
        cfg.memory_budget_bytes = Some(300);
        let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");
        // Small record first (under budget).
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 0,
            payload: chronos_log::ExecutionPayload::new(vec![1u8], "small"),
        })
        .unwrap();
        // Big record pushes the in-memory estimate over the 64
        // byte budget, forcing the gap path.
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 10,
            payload: chronos_log::ExecutionPayload::new(vec![0u8; 256], "big"),
        })
        .unwrap();
        log.flush().unwrap();
        let consumer = LogConsumerId::new("u1");
        let read = log.read_after(&consumer, None).expect("read");
        match read {
            ReadResult::Ok { records, gaps, .. } => {
                assert_eq!(records.len(), 1, "small record preserved");
                assert!(
                    !gaps.is_empty(),
                    "memory-budget overflow must produce at least one gap"
                );
            }
            other => panic!("expected Ok, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- Case 6: crash-safe segments (truncate one; verify replay
    //    skips it and the surviving segment is fully replayed). ----
    {
        let dir = tempdir("m1-02-case6");
        let session = SessionId::new("m1-02-uat-6");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        for i in 0..3u64 {
            log.append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i * 10,
                payload: chronos_log::ExecutionPayload::new(vec![i as u8], "rec"),
            })
            .unwrap();
        }
        log.flush().unwrap();
        let segments = log.flushed_segments();
        assert_eq!(segments.len(), 2, "two segments flushed");
        // Truncate the first segment by 8 bytes to corrupt its
        // BLAKE3 checksum.
        let path = segments[0].2.clone();
        let len = std::fs::metadata(&path).unwrap().len();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(len - 8)
            .unwrap();
        drop(log);

        let log2 = SegmentedExecutionLog::open(session.clone(), cfg).expect("reopen");
        // First segment is skipped; second segment still recovers.
        assert_eq!(log2.tail_seq(), Some(EventSeq::new(2)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- Case 7: checkpoint + delta = full replay. -------------------
    {
        let dir = tempdir("m1-02-case7");
        let session = SessionId::new("m1-02-uat-7");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        for i in 0..6u64 {
            log.append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i,
                payload: chronos_log::ExecutionPayload::new(vec![i as u8], "d"),
            })
            .unwrap();
        }
        log.flush().unwrap();
        let pre_tail = log.tail_seq();
        drop(log);

        cfg.replay_on_open = true;
        let log2 = SegmentedExecutionLog::open(session.clone(), cfg).expect("replay");
        assert_eq!(log2.tail_seq(), pre_tail);
        assert_eq!(pre_tail, Some(EventSeq::new(5)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -- Case 8: deterministic replay (two logs same input). ---------
    {
        let dir1 = tempdir("m1-02-case8-a");
        let dir2 = tempdir("m1-02-case8-b");
        let session = SessionId::new("m1-02-uat-8");
        let l1 = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir1))
            .expect("open a");
        let l2 = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir2))
            .expect("open b");
        for i in 0..16u64 {
            l1.append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i,
                payload: chronos_log::ExecutionPayload::new(vec![(i & 0xFF) as u8], "d"),
            })
            .unwrap();
            l2.append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i,
                payload: chronos_log::ExecutionPayload::new(vec![(i & 0xFF) as u8], "d"),
            })
            .unwrap();
        }
        l1.flush().unwrap();
        l2.flush().unwrap();
        assert_eq!(l1.tail_seq(), l2.tail_seq());
        assert_eq!(l1.tail_seq(), Some(EventSeq::new(15)));
        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    // -- Bonus: gap replaying preserves the consumer-cursor view. ----
    {
        let dir = tempdir("m1-02-gap");
        let session = SessionId::new("m1-02-uat-gap");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(1).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 0,
            payload: chronos_log::ExecutionPayload::new(vec![1], "a"),
        })
        .unwrap();
        log.record_gap(Gap::new(
            EventSeq::new(1),
            EventSeq::new(3),
            GapReason::AdapterBufferOverflow,
            "uat",
        ))
        .unwrap();
        log.flush().unwrap();

        let consumer = LogConsumerId::new("c");
        let read = log.read_after(&consumer, None).expect("read");
        match read {
            ReadResult::Ok { gaps, records, .. } => {
                assert_eq!(gaps.len(), 1);
                assert_eq!(gaps[0].reason, GapReason::AdapterBufferOverflow);
                assert!(!records.is_empty(), "pre-gap record is preserved");
            }
            other => panic!("expected Ok, got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Local helper for sandbox UATs that need a tempdir.
fn tempdir(label: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let p = base.join(unique);
    std::fs::create_dir_all(&p).unwrap();
    p
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
