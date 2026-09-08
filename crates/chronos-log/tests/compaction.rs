//! m1-05 — segment compaction.
//!
//! These tests cover the new `compact_up_to`, `compactable_segments_up_to`,
//! and `min_consumer_cursor` API on `SegmentedExecutionLog`. The
//! goal is to verify that:
//!   - Compaction removes only segment files whose `end_seq <= cutoff`
//!     (never deletes a still-needed segment).
//!   - The in-memory backend keeps the records until the next
//!     process restart, so reads continue to work after compaction.
//!   - The bookkeeping (`flushed_segments`, `last_flushed_tail`) is
//!     updated so subsequent `compactable_segments_up_to` calls
//!     don't re-emit the deleted paths.
//!   - `compact_up_to` is idempotent: calling it twice with the
//!     same cutoff is a no-op the second time.
//!   - Files that are already gone (concurrent delete) don't
//!     surface as an error.

use chronos_log::{
    CompactionMetrics, EventSeq, LogConsumerId, SegmentedConfig, SegmentedExecutionLog, SessionId,
};
use std::num::NonZeroUsize;
use std::path::PathBuf;

fn tempdir() -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-log-compact-{}-{}",
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

#[test]
fn compactable_lists_segments_below_cutoff() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-list");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    // Three flushes → three segments [0..1], [2..3], [4..5].
    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();
    assert_eq!(log.flushed_segments().len(), 3);

    // cutoff = seq 3 means segments [0..1] and [2..3] are below.
    let compactable = log.compactable_segments_up_to(EventSeq::new(3));
    assert_eq!(compactable.len(), 2);
    assert_eq!(compactable[0].0, EventSeq::new(0));
    assert_eq!(compactable[0].1, EventSeq::new(1));
    assert_eq!(compactable[1].0, EventSeq::new(2));
    assert_eq!(compactable[1].1, EventSeq::new(3));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compact_removes_segments_below_cutoff_and_updates_bookkeeping() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-compact");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();
    let pre = log.flushed_segments();
    assert_eq!(pre.len(), 3);

    let removed = log.compact_up_to(EventSeq::new(3)).expect("compact");
    assert_eq!(removed.len(), 2);
    // The paths returned must equal the first two segment paths.
    assert!(removed.contains(&pre[0].2));
    assert!(removed.contains(&pre[1].2));
    // Those files no longer exist on disk.
    for path in &removed {
        assert!(!path.exists(), "{:?} still on disk", path);
    }
    // The third segment still exists.
    assert!(pre[2].2.exists(), "third segment should survive");

    // Bookkeeping: flushed_segments now only has the third one.
    let post = log.flushed_segments();
    assert_eq!(post.len(), 1);
    assert_eq!(post[0].0, pre[2].0);
    assert_eq!(post[0].1, pre[2].1);

    // compactable_segments_up_to returns nothing for the same cutoff.
    let compactable = log.compactable_segments_up_to(EventSeq::new(3));
    assert!(compactable.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compact_is_idempotent() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-idem");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..4u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();

    // First call: deletes 1 segment (the [0..1] one).
    let removed_first = log.compact_up_to(EventSeq::new(1)).expect("compact 1");
    assert_eq!(removed_first.len(), 1);

    // Second call with same cutoff: nothing to do.
    let removed_second = log.compact_up_to(EventSeq::new(1)).expect("compact 2");
    assert!(removed_second.is_empty(), "idempotent compaction");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compact_keeps_in_memory_records_readable() {
    // The in-memory backend must keep all records even after
    // compaction — compaction only frees on-disk segments. Until
    // a process restart, reads still work via the in-memory view.
    let dir = tempdir();
    let session = SessionId::new("m1-05-mem");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();

    log.compact_up_to(EventSeq::new(5)).expect("compact");

    // In-memory reads still see every record.
    let consumer = LogConsumerId::new("agent-a");
    let read = log.read_after(&consumer, None).expect("read");
    let total = match read {
        chronos_log::ReadResult::Ok { records, .. } => records.len(),
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(total, 6, "in-memory backend survives compaction");

    // tail_seq is still 5.
    assert_eq!(log.tail_seq(), Some(EventSeq::new(5)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn min_consumer_cursor_returns_lowest_committed_seq() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-min");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");

    // No cursors yet — min is None.
    assert_eq!(log.min_consumer_cursor(), None);

    // Two cursors: the lowest is the answer.
    log.commit_cursor(&LogConsumerId::new("a"), EventSeq::new(7))
        .unwrap();
    log.commit_cursor(&LogConsumerId::new("b"), EventSeq::new(3))
        .unwrap();
    log.commit_cursor(&LogConsumerId::new("c"), EventSeq::new(10))
        .unwrap();
    assert_eq!(log.min_consumer_cursor(), Some(EventSeq::new(3)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compact_concurrent_delete_does_not_error() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-conc");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..4u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();

    // Manually delete one of the segment files to simulate
    // a concurrent compact or admin rm.
    let pre = log.flushed_segments();
    std::fs::remove_file(&pre[0].2).unwrap();

    // compact_up_to should succeed (treating NotFound as a no-op).
    let removed = log.compact_up_to(EventSeq::new(3)).expect("compact ok");
    // The manually-deleted file is gone (not in `removed`), the
    // other one is removed by compaction.
    assert_eq!(removed.len(), 1);
    assert!(!pre[0].2.exists());
    assert!(!pre[1].2.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compact_after_compact_keeps_survivors_intact() {
    let dir = tempdir();
    let session = SessionId::new("m1-05-twice");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..8u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();
    assert_eq!(log.flushed_segments().len(), 4);

    // Compact the first two.
    log.compact_up_to(EventSeq::new(3)).unwrap();
    assert_eq!(log.flushed_segments().len(), 2);

    // Compact the next one.
    log.compact_up_to(EventSeq::new(5)).unwrap();
    let post = log.flushed_segments();
    assert_eq!(post.len(), 1, "only the [6..7] segment remains");
    assert_eq!(post[0].0, EventSeq::new(6));
    assert_eq!(post[0].1, EventSeq::new(7));
    // The remaining segment file is on disk.
    assert!(post[0].2.exists());

    // last_flushed_tail tracks the survivor.
    assert_eq!(log.last_flushed_tail(), Some(EventSeq::new(7)));

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// m1-06 — compaction metrics + maybe_compact.
//
// These tests cover the new `compaction_metrics()` accessor and the
// `maybe_compact()` convenience wrapper.
// ---------------------------------------------------------------------------

/// Metrics are zero on a freshly opened log.
#[test]
fn compaction_metrics_initial_state_is_zero() {
    let dir = tempdir();
    let cfg = SegmentedConfig::with_dir(&dir);
    let log = SegmentedExecutionLog::open(SessionId::new("m1-06-metrics-zero"), cfg).unwrap();

    assert_eq!(log.compaction_metrics(), CompactionMetrics::default());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `compact_up_to` increments all three counters exactly once per
/// successful run. A second call with the same cutoff is a no-op and
/// must NOT re-increment.
#[test]
fn compaction_metrics_track_runs_segments_and_bytes() {
    let dir = tempdir();
    let session = SessionId::new("m1-06-metrics-track");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    // Three flushes ⇒ three segments [0..1], [2..3], [4..5].
    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();
    assert_eq!(log.flushed_segments().len(), 3);

    let consumer = LogConsumerId::new("c1");
    log.commit_cursor(&consumer, EventSeq::new(3)).unwrap();

    // Cutoff = seq 3 ⇒ segments [0..1] and [2..3] (end_seq=1 and
    // end_seq=3) both qualify.
    let removed = log.compact_up_to(EventSeq::new(3)).unwrap();
    assert_eq!(removed.len(), 2);
    let m1 = log.compaction_metrics();
    assert_eq!(m1.compaction_runs_total, 1);
    assert_eq!(m1.segments_removed_total, 2);
    assert!(m1.bytes_reclaimed_total > 0, "size should be >0");

    // Second call with the same cutoff is a no-op.
    let removed_again = log.compact_up_to(EventSeq::new(3)).unwrap();
    assert!(removed_again.is_empty());
    let m2 = log.compaction_metrics();
    assert_eq!(m2.compaction_runs_total, 1, "no new run on no-op");
    assert_eq!(m2.segments_removed_total, 2);
    assert_eq!(
        m2.bytes_reclaimed_total, m1.bytes_reclaimed_total,
        "bytes should not change"
    );

    // Third call with a stricter cutoff that nothing satisfies
    // (only one segment left, end_seq=5) ⇒ still no-op.
    let removed_3 = log.compact_up_to(EventSeq::new(0)).unwrap();
    assert!(removed_3.is_empty());
    let m3 = log.compaction_metrics();
    assert_eq!(m3.compaction_runs_total, 1);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `maybe_compact` uses `min_consumer_cursor()` automatically. Two
/// consumers with different positions ⇒ the slower one determines
/// the cutoff.
#[test]
fn maybe_compact_uses_min_consumer_cursor() {
    let dir = tempdir();
    let session = SessionId::new("m1-06-maybe");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();
    assert_eq!(log.flushed_segments().len(), 3);

    // The "slow" consumer has only read 1 record (seq 0). The
    // "fast" consumer has read through seq 4. Min cursor = 0 ⇒
    // nothing compactable (end_seq=1 > 0).
    let slow = LogConsumerId::new("slow");
    let fast = LogConsumerId::new("fast");
    log.commit_cursor(&slow, EventSeq::new(0)).unwrap();
    log.commit_cursor(&fast, EventSeq::new(4)).unwrap();

    let removed = log.maybe_compact().unwrap();
    assert!(
        removed.is_empty(),
        "slow consumer blocks compaction (min cursor too low)"
    );

    // Now advance slow to seq 3 — min cursor becomes min(3, 4) = 3.
    log.commit_cursor(&slow, EventSeq::new(3)).unwrap();
    let removed = log.maybe_compact().unwrap();
    assert_eq!(
        removed.len(),
        2,
        "segments with end_seq <= 3 are removed ([0..1] and [2..3])"
    );

    let m = log.compaction_metrics();
    assert_eq!(m.compaction_runs_total, 1);
    assert_eq!(m.segments_removed_total, 2);
    assert!(m.bytes_reclaimed_total > 0);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `maybe_compact` is a no-op when no consumer has committed a
/// cursor.
#[test]
fn maybe_compact_returns_empty_when_no_cursors() {
    let dir = tempdir();
    let session = SessionId::new("m1-06-no-cursor");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    for i in 0..4u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();

    let removed = log.maybe_compact().unwrap();
    assert!(removed.is_empty(), "no consumer ⇒ no compaction");
    let m = log.compaction_metrics();
    assert_eq!(m.compaction_runs_total, 0);
    assert_eq!(m.segments_removed_total, 0);

    let _ = std::fs::remove_dir_all(&dir);
}
