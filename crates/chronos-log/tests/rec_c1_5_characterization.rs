//! REC-C1.5.0 — characterization of the restart/retention inconsistencies.
//!
//! These two behaviours are why retention and recovery cannot be treated as
//! "just another read": both make Chronos remember an execution differently
//! before and after a restart.
//!
//! As in C1.0, the tests assert the CURRENT (wrong) behaviour and are `#[ignore]`
//! by default. They are the executable statement of the bug, to be flipped by
//! C1.5.1 (retention watermark) and C1.5.2 (strict replay).
//!
//! Run with: `cargo test -p chronos-log --test rec_c1_5_characterization -- --ignored`

use std::num::NonZeroUsize;

use chronos_log::{
    segment_path, LogConsumerId, NewExecutionRecord, ReadResult, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};

fn tmpdir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rec-c1-5-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn config(dir: &std::path::Path, flush_every: usize) -> SegmentedConfig {
    let mut c = SegmentedConfig::with_dir(dir.to_path_buf());
    c.flush_threshold = NonZeroUsize::new(flush_every.max(1)).unwrap();
    c
}

fn append_n(log: &SegmentedExecutionLog, session: &SessionId, n: u64) {
    for i in 0..n {
        log.append(NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: i,
            payload: chronos_log::ExecutionPayload::new(format!("rec-{i}").into_bytes(), "t"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
        })
        .expect("append");
    }
}

/// CHAR-RET: retention has no durable boundary.
///
/// `compact_up_to(499)` deletes the segment files but leaves their records
/// readable in memory. A reader asking for `seq >= 0` therefore gets the OLD
/// answer until the process restarts, and the NEW (truncated) answer after.
/// Same request, two different truths.
#[test]
#[ignore = "REC-C1.5.0 characterization: retention boundary is not durable across restart"]
fn char_retention_answer_changes_after_restart() {
    let dir = tmpdir("retention");
    let session = SessionId::new("ret");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 1000);
    log.flush().expect("flush");

    // Before compaction: the whole history is readable.
    let before = log
        .read_from_seq(chronos_log::EventSeq::ZERO, 10)
        .expect("read before");
    let before_first = before.records.first().expect("records before").seq;
    assert_eq!(before_first, chronos_log::EventSeq::ZERO);

    // Retire the first 500 seqs. The files go away...
    let removed = log
        .compact_up_to(chronos_log::EventSeq::new(499))
        .expect("compact");
    assert!(!removed.is_empty(), "segments were removed from disk");

    // ...but the in-process reader still sees them.
    let in_process = log
        .read_from_seq(chronos_log::EventSeq::ZERO, 10)
        .expect("read after compaction, same process");
    assert_eq!(
        in_process.records.first().expect("records").seq,
        chronos_log::EventSeq::ZERO,
        "characterization: compaction does not change what this process reads"
    );

    // After a restart, the same request gets a different answer: the truncated
    // history. There is no watermark that says why.
    drop(log);
    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    let after = reopened
        .read_from_seq(chronos_log::EventSeq::ZERO, 10)
        .expect("read after restart");

    println!(
        "CHAR-RET before-restart first_seq={} after-restart first_seq={:?} records={}",
        before_first.0,
        after.records.first().map(|r| r.seq.0),
        after.records.len()
    );

    // Characterization of the inconsistency: the answer moved without any
    // explicit signal, and nothing reports the retention boundary.
    let after_first = after.records.first().map(|r| r.seq.0);
    assert_ne!(
        after_first,
        Some(before_first.0),
        "characterization changed: retention is now durable — retire this item"
    );
    assert!(
        after.gaps.is_empty(),
        "characterization: truncation is NOT reported as a gap either"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// CHAR-REPLAY: a corrupt segment is skipped and replay continues.
///
/// The skipped bytes vanish from the seq space with no `Gap`, which breaks the
/// C1.4 premise that inside the allocated range a missing seq can only come from
/// a recorded gap. After such a reopen, `Complete` could be claimed over a real
/// hole.
#[test]
#[ignore = "REC-C1.5.0 characterization: replay skips corrupt segments instead of failing closed"]
fn char_corrupt_segment_is_skipped_and_creates_a_silent_hole() {
    let dir = tmpdir("corrupt");
    let session = SessionId::new("corrupt");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 300);
    log.flush().expect("flush");
    drop(log);

    // Corrupt the BODY of the first segment while leaving its header intact:
    // a truncated file already fails closed at header read, which is why the
    // interesting case is "header fine, contents/checksum bad".
    let first_path = segment_path(&dir, &session, chronos_log::EventSeq::ZERO);
    assert!(
        first_path.exists(),
        "expected a segment at seq#0: {first_path:?}"
    );
    let mut bytes = std::fs::read(&first_path).expect("read seg");
    let flip_at = bytes.len() * 3 / 4;
    bytes[flip_at] ^= 0xFF;
    std::fs::write(&first_path, &bytes).expect("write corrupted seg");

    // Reopen: current behaviour is "warn + skip", so this SUCCEEDS.
    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1));
    assert!(
        reopened.is_ok(),
        "characterization: a corrupt segment is tolerated today; got {:?}",
        reopened.err()
    );
    let reopened = reopened.expect("reopen");

    let page = reopened
        .read_from_seq(chronos_log::EventSeq::ZERO, 5)
        .expect("read");
    println!(
        "CHAR-REPLAY reopened first_seq={:?} gaps={}",
        page.records.first().map(|r| r.seq.0),
        page.gaps.len()
    );

    // The hole is real: the seqs in the truncated segment are gone while the
    // surviving segment still answers, and NO gap records the loss.
    assert!(
        page.gaps.is_empty(),
        "characterization: the lost region is not recorded as a Gap"
    );
    let hole_is_real = page.records.first().map(|r| r.seq.0 > 0).unwrap_or(true);
    assert!(
        hole_is_real,
        "characterization changed: replay no longer silently drops records"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// CONTROL: with no corruption and no compaction, a restart preserves both the
/// SessionId and the seq space. This must keep passing after C1.5.1/C1.5.2.
#[test]
fn control_clean_restart_preserves_session_and_seqs() {
    let dir = tmpdir("clean");
    let session = SessionId::new("clean");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 50);
    log.flush().expect("flush");
    let before_tail = log.tail_seq();
    drop(log);

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    assert_eq!(
        reopened.session_id(),
        &session,
        "same SessionId after restart"
    );
    assert_eq!(reopened.tail_seq(), before_tail, "same EventSeq space");
    let page = reopened
        .read_from_seq(chronos_log::EventSeq::ZERO, 5)
        .expect("read");
    assert_eq!(
        page.records.first().map(|r| r.seq),
        Some(chronos_log::EventSeq::ZERO),
        "seq#0 survives a clean restart"
    );

    // Consumer cursors stay independent for a reader that re-anchors explicitly.
    let consumer = LogConsumerId::new("c");
    let read = reopened.read_after(&consumer, None).expect("read_after");
    assert!(matches!(read, ReadResult::Ok { .. }));

    let _ = std::fs::remove_dir_all(&dir);
}
