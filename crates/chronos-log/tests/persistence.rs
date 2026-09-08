//! Spec cases 5-8 for `chronos-log::SegmentedExecutionLog`.
//!
//! All tests use a fresh tempdir under `/tmp/chronos-log-persistence-*`
//! so they can run in parallel without interfering.

use chronos_log::{
    EventSeq, ExecutionLogBackend, GapReason, SegmentedConfig, SegmentedExecutionLog, SessionId,
};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

fn tempdir() -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-log-persistence-{}-{}",
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

fn make_log(dir: &PathBuf, session: &SessionId, flush_threshold: usize) -> SegmentedExecutionLog {
    let mut cfg = SegmentedConfig::with_dir(dir);
    cfg.flush_threshold = NonZeroUsize::new(flush_threshold).unwrap();
    SegmentedExecutionLog::open(session.clone(), cfg).expect("open segmented log")
}

#[test]
fn spec_case_05_overflow_records_gap_not_record() {
    // Configure a tiny memory budget so the second append
    // triggers an overflow→gap.
    let dir = tempdir();
    let session = SessionId::new("case5");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(8).unwrap();
    cfg.memory_budget_bytes = Some(64);
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    // First, a small record that fits.
    let _ = log
        .append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 0,
            payload: chronos_log::ExecutionPayload::new(vec![1, 2, 3], "small"),
            ..Default::default()
        })
        .unwrap();

    // Big records overflow the 64-byte budget. Each should result
    // in a recorded gap (instead of a record).
    for i in 1..=4 {
        let seq = log
            .append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i * 10,
                payload: chronos_log::ExecutionPayload::new(vec![0u8; 256], "big"),
                ..Default::default()
            })
            .unwrap();
        assert!(seq.0 >= 1, "seqs must keep advancing");
    }

    // Flush, then read everything back via `read_after` from the
    // start and verify at least one gap is observed.
    log.flush().unwrap();
    let consumer = chronos_log::LogConsumerId::new("c1");
    let read = log.read_after(&consumer, None).unwrap();
    let total = match read {
        chronos_log::ReadResult::Ok { records, gaps, .. } => {
            assert!(
                !gaps.is_empty(),
                "expected at least one overflow gap (records={}, gaps={})",
                records.len(),
                gaps.len()
            );
            records.len() + gaps.len()
        }
        other => panic!("unexpected read result: {:?}", other),
    };
    assert!(total >= 4, "got {} entries (>=4)", total);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spec_case_06_crash_safe_segments() {
    // Simulate a crash mid-write by appending garbage to a
    // segment file after it has been written, then reopening.
    let dir = tempdir();
    let session = SessionId::new("case6");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = make_log(&dir, &session, 2);

    log.append(chronos_log::NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 0,
        payload: chronos_log::ExecutionPayload::new(vec![1], "a"),
        ..Default::default()
    })
    .unwrap();
    log.append(chronos_log::NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 10,
        payload: chronos_log::ExecutionPayload::new(vec![2], "b"),
        ..Default::default()
    })
    .unwrap();
    log.flush().unwrap();

    log.append(chronos_log::NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 20,
        payload: chronos_log::ExecutionPayload::new(vec![3], "c"),
        ..Default::default()
    })
    .unwrap();
    log.flush().unwrap();

    let segments = log.flushed_segments();
    assert_eq!(segments.len(), 2, "two segments flushed");

    // Truncate the first segment by 8 bytes. This guarantees the
    // payload's BLAKE3 checksum won't match the (now shorter)
    // payload, so the replay skips this segment.
    let path = segments[0].2.clone();
    let len = std::fs::metadata(&path).unwrap().len();
    assert!(len > 8, "segment is too small to truncate");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(len - 8)
        .unwrap();

    // Drop the first log (simulate process exit), then reopen.
    drop(log);

    cfg.replay_on_open = true;
    let log2 = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
    // The corrupt first segment is skipped; the second segment
    // (seq=2) is recovered. tail_seq should be Some(EventSeq(2)).
    assert_eq!(
        log2.tail_seq(),
        Some(EventSeq(2)),
        "second segment was recovered despite first being corrupt"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spec_case_07_checkpoint_plus_delta_equals_full_replay() {
    // Produce a stream across 3 flushes. The tail_seq *after* a
    // cold boot (no in-memory state, only disk) must match the
    // tail_seq before drop.
    let dir = tempdir();
    let session = SessionId::new("case7");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).unwrap();

    let expected_tail = {
        for i in 0..7 {
            log.append(chronos_log::NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i * 10,
                payload: chronos_log::ExecutionPayload::new(vec![i as u8], "r"),
                ..Default::default()
            })
            .unwrap();
        }
        log.flush().unwrap();
        let pre_tail = log.tail_seq();
        assert_eq!(pre_tail, Some(EventSeq(6)));
        pre_tail
    };
    drop(log);

    cfg.replay_on_open = true;
    let log2 = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
    assert_eq!(log2.tail_seq(), expected_tail);

    // Use `replay()` (full reconstruction) and verify the same
    // value matches.
    let replayed = log2.replay().unwrap();
    assert_eq!(replayed.tail_seq(&session), expected_tail);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spec_case_08_deterministic_replay() {
    // Two independent logs receiving the exact same inputs must
    // produce the same tail. Same inputs, same schema, same
    // config knobs.
    let dir1 = tempdir();
    let dir2 = tempdir();
    let session = SessionId::new("case8");
    let cfg1 = SegmentedConfig::with_dir(&dir1);
    let cfg2 = SegmentedConfig::with_dir(&dir2);
    let log1 = SegmentedExecutionLog::open(session.clone(), cfg1).unwrap();
    let log2 = SegmentedExecutionLog::open(session.clone(), cfg2).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let b1 = Arc::clone(&barrier);
    let b2 = Arc::clone(&barrier);
    let l1 = log1.clone();
    let l2 = log2.clone();
    let s1 = session.clone();
    let s2 = session.clone();
    let h1 = std::thread::spawn(move || {
        for i in 0..16u64 {
            l1.append(chronos_log::NewExecutionRecord {
                session_id: s1.clone(),
                monotonic_ns: i * 7,
                payload: chronos_log::ExecutionPayload::new(vec![(i & 0xFF) as u8], "t"),
                ..Default::default()
            })
            .unwrap();
        }
        l1.flush().unwrap();
        b1.wait();
        l1.tail_seq()
    });
    let h2 = std::thread::spawn(move || {
        for i in 0..16u64 {
            l2.append(chronos_log::NewExecutionRecord {
                session_id: s2.clone(),
                monotonic_ns: i * 7,
                payload: chronos_log::ExecutionPayload::new(vec![(i & 0xFF) as u8], "t"),
                ..Default::default()
            })
            .unwrap();
        }
        l2.flush().unwrap();
        b2.wait();
        l2.tail_seq()
    });
    let t1 = h1.join().unwrap();
    let t2 = h2.join().unwrap();
    assert_eq!(t1, t2, "two logs with identical input must match tail");
    assert_eq!(t1, Some(EventSeq(15)));

    let _ = std::fs::remove_dir_all(&dir1);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn checkpoint_method_flushes_even_when_buffer_is_partial() {
    // `checkpoint()` must produce a segment file regardless of
    // whether the buffer has reached `flush_threshold`.
    let dir = tempdir();
    let session = SessionId::new("ck");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(100).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    for i in 0..3 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![1], "z"),
            ..Default::default()
        })
        .unwrap();
    }
    assert!(
        log.flushed_segments().is_empty(),
        "no auto-flush expected with threshold=100"
    );
    let path = log.flush().unwrap();
    assert!(path.is_some(), "explicit flush must emit a segment file");
    assert_eq!(log.flushed_segments().len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gap_replaying_preserves_consumer_cursor_view() {
    // After a gap has been recorded on disk, replaying must
    // observe the gap when reading from the start.
    let dir = tempdir();
    let session = SessionId::new("gap-r");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(1).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();

    log.append(chronos_log::NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: 0,
        payload: chronos_log::ExecutionPayload::new(vec![1], "a"),
        ..Default::default()
    })
    .unwrap();
    let seq = log
        .append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 10,
            payload: chronos_log::ExecutionPayload::new(vec![2], "b"),
            ..Default::default()
        })
        .unwrap();
    log.record_gap(chronos_log::Gap::new(
        seq,
        seq,
        GapReason::ProcessDetached,
        "test",
    ))
    .unwrap();
    log.flush().unwrap();

    // Read everything from a fresh consumer and verify the gap
    // is in the response.
    let consumer = chronos_log::LogConsumerId::new("c");
    let read = log.read_after(&consumer, None).unwrap();
    if let chronos_log::ReadResult::Ok { gaps, .. } = read {
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].reason, GapReason::ProcessDetached);
    } else {
        panic!("expected Ok read result");
    }

    let _ = std::fs::remove_dir_all(&dir);
}
