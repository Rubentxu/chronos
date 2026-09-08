//! m1-04 — durable consumer cursor sidecar.
//!
//! These tests cover the JSON sidecar `<session>.cursors.json` that
//! `SegmentedExecutionLog` writes on every `commit_cursor` call and
//! replays on `open` to seed the inner `InMemoryExecutionLog`
//! cursor map.
//!
//! Scope (m1-04):
//!   - `commit_cursor` persists and `last_cursor` reflects it.
//!   - Two distinct consumers have independent cursors.
//!   - A stale `commit_cursor` (lower seq than what's already
//!     stored) is a no-op — the higher seq wins.
//!   - After `drop` + reopen, the sidecar's cursors survive and
//!     are visible via `cursors()`.
//!   - Reads via `read_after` after a `commit_cursor` skip
//!     records ≤ `last_seq`.

use chronos_log::{
    segment::sanitize_session, EventSeq, LogConsumerId, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use std::num::NonZeroUsize;
use std::path::PathBuf;

fn tempdir() -> PathBuf {
    let base = std::env::temp_dir();
    let unique = format!(
        "chronos-log-cursors-{}-{}",
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
fn commit_cursor_persists_and_last_cursor_reflects() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-c1");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");
    let consumer = LogConsumerId::new("agent-a");
    log.commit_cursor(&consumer, EventSeq::new(42))
        .expect("commit");

    // In-memory view reflects the commit.
    assert_eq!(log.last_cursor(&consumer), Some(EventSeq::new(42)));

    // The on-disk sidecar must exist next to the segments.
    let sidecar = dir.join(format!("{}.cursors.json", sanitize_session(&session)));
    assert!(sidecar.exists(), "sidecar file written: {:?}", sidecar);

    let raw = std::fs::read_to_string(&sidecar).expect("read sidecar");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(parsed["agent-a"], 42);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_commit_does_not_rollback_cursor() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-c2");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");
    let consumer = LogConsumerId::new("agent-b");

    log.commit_cursor(&consumer, EventSeq::new(10)).unwrap();
    log.commit_cursor(&consumer, EventSeq::new(7)).unwrap(); // stale
    log.commit_cursor(&consumer, EventSeq::new(15)).unwrap();

    assert_eq!(
        log.last_cursor(&consumer),
        Some(EventSeq::new(15)),
        "highest commit wins; stale commit ignored"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_consumers_have_independent_cursors() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-c3");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");

    log.commit_cursor(&LogConsumerId::new("agent-a"), EventSeq::new(3))
        .unwrap();
    log.commit_cursor(&LogConsumerId::new("agent-b"), EventSeq::new(8))
        .unwrap();
    log.commit_cursor(&LogConsumerId::new("agent-a"), EventSeq::new(5))
        .unwrap();

    assert_eq!(
        log.last_cursor(&LogConsumerId::new("agent-a")),
        Some(EventSeq::new(5))
    );
    assert_eq!(
        log.last_cursor(&LogConsumerId::new("agent-b")),
        Some(EventSeq::new(8))
    );

    let cursors = log.cursors();
    assert_eq!(cursors.len(), 2);
    assert_eq!(cursors["agent-a"], EventSeq::new(5));
    assert_eq!(cursors["agent-b"], EventSeq::new(8));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sidecar_survives_process_restart() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-c4");

    // Phase 1: open, commit, drop.
    {
        let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
            .expect("open");
        log.commit_cursor(&LogConsumerId::new("agent-a"), EventSeq::new(99))
            .expect("commit");
    }

    // Phase 2: reopen and confirm the cursor is restored.
    let log2 = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("reopen");
    assert_eq!(
        log2.last_cursor(&LogConsumerId::new("agent-a")),
        Some(EventSeq::new(99)),
        "cursor survives restart"
    );
    let cursors = log2.cursors();
    assert_eq!(cursors["agent-a"], EventSeq::new(99));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_after_commit_cursor_skips_processed_records() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-c5");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("open");

    // Append 6 records so the log crosses two flushes.
    for i in 0..6u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "x"),
        })
        .unwrap();
    }
    log.flush().unwrap();

    let consumer = LogConsumerId::new("agent-a");

    // Read everything from seq 0 with a fresh cursor.
    let initial = log.read_after(&consumer, None).expect("read all");
    let initial_records = match initial {
        chronos_log::ReadResult::Ok { records, .. } => records.len(),
        other => panic!("expected Ok, got {:?}", other),
    };
    assert_eq!(initial_records, 6);

    // Commit the cursor at the max seq returned.
    log.commit_cursor(&consumer, EventSeq::new(5)).unwrap();

    // Read again — the in-memory backend should now skip past
    // seq 5, returning empty.
    let followup = log
        .read_after(
            &consumer,
            Some(chronos_log::ConsumerCursor::at(
                consumer.clone(),
                EventSeq::new(5),
            )),
        )
        .expect("read with cursor");
    match followup {
        chronos_log::ReadResult::Ok { records, .. } => {
            assert!(
                records.is_empty(),
                "expected no records past cursor; got {}",
                records.len()
            );
        }
        other => panic!("expected Ok, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fresh_consumer_after_commit_still_returns_everything() {
    // The m1-02 contract says a `read_after(consumer, None)`
    // returns every record regardless of stored cursor — only
    // an *explicit* cursor (`Some(...)`) applies the high-water
    // mark. Make sure `commit_cursor` does not change that.
    let dir = tempdir();
    let session = SessionId::new("m1-04-c6");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");
    for i in 0..3u64 {
        log.append(chronos_log::NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i * 10,
            payload: chronos_log::ExecutionPayload::new(vec![i as u8], "y"),
        })
        .unwrap();
    }
    let consumer = LogConsumerId::new("agent-a");
    log.commit_cursor(&consumer, EventSeq::new(2)).unwrap();

    // Fresh read (None) → 3 records again.
    let read = log.read_after(&consumer, None).expect("fresh read");
    match read {
        chronos_log::ReadResult::Ok { records, .. } => {
            assert_eq!(records.len(), 3, "fresh read sees everything");
        }
        other => panic!("expected Ok, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_log_has_empty_cursor_snapshot() {
    let dir = tempdir();
    let session = SessionId::new("m1-04-empty");
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open");
    assert!(log.cursors().is_empty());
    assert_eq!(log.last_cursor(&LogConsumerId::new("nothing")), None);
    let _ = std::fs::remove_dir_all(&dir);
}
