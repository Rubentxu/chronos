//! REC-C1.5.0 — characterization of the restart/retention inconsistencies.
//!
//! These two behaviours are why retention and recovery cannot be treated as
//! "just another read": both make Chronos remember an execution differently
//! before and after a restart.
//!
//! CHAR-RET was flipped by C1.5.1 into the positive RET-1..RET-10 suite below:
//! the durable retention watermark makes the same request answer identically
//! before and after a restart. CHAR-REPLAY remains a characterization, to be
//! flipped by C1.5.2 (strict replay).
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

/// RET-1: the retention boundary is durable. The SAME request answers the SAME
/// way before and after a restart, while physical reclamation may lag.
#[test]
fn ret_1_retention_boundary_is_durable_across_restart() {
    let dir = tmpdir("ret1");
    let session = SessionId::new("ret1");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 1000);
    log.flush().expect("flush");

    let outcome = log
        .retain_up_to(chronos_log::EventSeq::new(499))
        .expect("retain");
    println!(
        "RET-1 retained_from={} removed={}",
        outcome.retained_from.0,
        outcome.removed.len()
    );
    assert!(outcome.retained_from > chronos_log::EventSeq::ZERO);

    // Before restart: below the boundary is refused, not silently answered.
    let before = log.read_from_seq(chronos_log::EventSeq::ZERO, 10);
    match before {
        Err(chronos_log::LogError::PositionBeforeRetention {
            requested_next_seq,
            retained_from,
        }) => {
            assert_eq!(requested_next_seq, chronos_log::EventSeq::ZERO);
            assert_eq!(retained_from, outcome.retained_from);
        }
        other => panic!("expected PositionBeforeRetention before restart, got {other:?}"),
    }

    // At the boundary is valid.
    let at = log
        .read_from_seq(outcome.retained_from, 5)
        .expect("read at boundary");
    assert_eq!(
        at.records.first().map(|r| r.seq),
        Some(outcome.retained_from),
        "the boundary itself is queryable"
    );

    // After restart: EXACTLY the same answer.
    drop(log);
    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    assert_eq!(
        reopened.retained_from(),
        outcome.retained_from,
        "the watermark survives the restart"
    );
    match reopened.read_from_seq(chronos_log::EventSeq::ZERO, 10) {
        Err(chronos_log::LogError::PositionBeforeRetention { retained_from, .. }) => {
            assert_eq!(retained_from, outcome.retained_from)
        }
        other => panic!("expected the same PositionBeforeRetention after restart, got {other:?}"),
    }
    let at_after = reopened
        .read_from_seq(outcome.retained_from, 5)
        .expect("read at boundary after restart");
    assert_eq!(
        at_after.records.first().map(|r| r.seq),
        Some(outcome.retained_from)
    );

    // Rows: retention is never reported as a Gap.
    assert!(at_after.gaps.is_empty(), "retention is not evidence loss");

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-2 + RET-3: the persisted watermark is exact, and the boundary only
/// advances over segments that are WHOLLY retired.
#[test]
fn ret_2_3_watermark_is_exact_and_aligned_with_whole_segments() {
    let dir = tmpdir("ret23");
    let session = SessionId::new("ret23");
    // One record per segment: segments are exactly [n,n].
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 20);
    log.flush().expect("flush");

    let segs = log.flushed_segments();
    assert!(segs.len() >= 2, "expected several segments: {}", segs.len());

    // Cut inside a segment boundary region: the boundary may only land on a
    // whole-segment edge, never on `cutoff + 1` computed blindly.
    let cutoff = chronos_log::EventSeq::new(6);
    let outcome = log.retain_up_to(cutoff).expect("retain");
    println!(
        "RET-3 cutoff={} retained_from={}",
        cutoff.0, outcome.retained_from.0
    );
    assert_eq!(outcome.retained_from, chronos_log::EventSeq::new(7));
    assert!(outcome.retained_from > cutoff);

    // Persisted value is exactly the in-memory one.
    let manifest = chronos_log::segmented::read_manifest(&dir)
        .expect("read manifest")
        .expect("manifest present");
    assert_eq!(manifest.retained_from, outcome.retained_from.0);
    assert_eq!(manifest.session_id, session.as_str());
    assert_eq!(manifest.schema_version, 1);

    // A second pass does not move the boundary backwards, and a no-op pass
    // (nothing wholly retired) leaves it alone.
    let noop = log.retain_up_to(cutoff).expect("noop pass");
    assert_eq!(noop.retained_from, outcome.retained_from);

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-4: if the watermark cannot be persisted, NOTHING is reclaimed and the
/// previous boundary stands. Logical retention either happens or it does not.
#[test]
fn ret_4_failed_manifest_write_reclaims_nothing() {
    let dir = tmpdir("ret4");
    let session = SessionId::new("ret4");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 20);
    log.flush().expect("flush");
    let before_files = std::fs::read_dir(&dir).unwrap().count();

    // Make the manifest path unusable so the atomic write cannot succeed.
    let mpath = dir.join(chronos_log::segmented::MANIFEST_FILE_NAME);
    std::fs::remove_file(&mpath).expect("remove manifest");
    std::fs::create_dir(&mpath).expect("occupy manifest path with a directory");

    let err = log.retain_up_to(chronos_log::EventSeq::new(5)).unwrap_err();
    println!("RET-4 manifest failure: {err}");
    assert_eq!(log.retained_from(), chronos_log::EventSeq::ZERO);
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        before_files + 1,
        "no segment was reclaimed (the extra entry is the blocking directory)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-5: once committed, the NEW watermark stays authoritative even if physical
/// reclamation fails, and the leftovers are inert after a reopen.
#[test]
fn ret_5_committed_watermark_wins_over_leftover_segments() {
    let dir = tmpdir("ret5");
    let session = SessionId::new("ret5");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 30);
    log.flush().expect("flush");

    // Keep a faithful copy of the earliest segment BEFORE retention, so the
    // leftover we plant afterwards is exactly the file that should have been
    // removed (a real "delete failed" outcome, not a fabricated one).
    let retired_path = segment_path(&dir, &session, chronos_log::EventSeq::ZERO);
    let retired_bytes = std::fs::read(&retired_path).expect("read earliest segment");

    let retained = log
        .retain_up_to(chronos_log::EventSeq::new(9))
        .expect("retain")
        .retained_from;
    assert!(retained > chronos_log::EventSeq::ZERO);
    assert!(!retired_path.exists(), "reclamation removed it in this run");

    // Simulate the failure mode: watermark committed, file never removed.
    std::fs::write(&retired_path, &retired_bytes).expect("plant leftover");
    drop(log);

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    assert_eq!(reopened.retained_from(), retained, "watermark wins");
    assert!(
        matches!(
            reopened.read_from_seq(chronos_log::EventSeq::ZERO, 5),
            Err(chronos_log::LogError::PositionBeforeRetention { .. })
        ),
        "leftovers below the watermark are not live evidence"
    );
    // And they did not corrupt the surviving range.
    let ok = reopened
        .read_from_seq(retained, 5)
        .expect("surviving range still readable");
    assert_eq!(ok.records.first().map(|r| r.seq), Some(retained));

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-6/7/8: boundary arithmetic and the gap-at-the-boundary case.
#[test]
fn ret_6_7_8_boundary_edges_and_gap_at_boundary() {
    use chronos_log::{Gap, GapReason};
    let dir = tmpdir("ret678");
    let session = SessionId::new("ret678");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 12);
    // An explicit gap starting exactly where retention will start.
    log.record_gap(Gap::new(
        chronos_log::EventSeq::new(12),
        chronos_log::EventSeq::new(14),
        GapReason::AdapterBufferOverflow,
        "t",
    ))
    .expect("gap");
    log.flush().expect("flush");

    let retained = log
        .retain_up_to(chronos_log::EventSeq::new(5))
        .expect("retain")
        .retained_from;
    assert_eq!(retained, chronos_log::EventSeq::new(6));

    // RET-6: exactly at the boundary is valid.
    assert!(log.read_from_seq(retained, 1).is_ok());
    // RET-7: below it is stale.
    assert!(matches!(
        log.read_from_seq(chronos_log::EventSeq::new(5), 1),
        Err(chronos_log::LogError::PositionBeforeRetention { .. })
    ));
    // RET-8: an explicit gap at/after the boundary is reported normally, not as
    // retention.
    let page = log
        .read_from_seq(chronos_log::EventSeq::new(11), 10)
        .expect("read across the gap");
    assert_eq!(
        page.gaps.len(),
        1,
        "the gap is still a gap: {:?}",
        page.gaps
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-9: one identity, checked. The manifest is authoritative.
#[test]
fn ret_9_manifest_identity_mismatch_is_a_hard_error() {
    let dir = tmpdir("ret9");
    let session = SessionId::new("ret9-original");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 3);
    log.flush().expect("flush");
    drop(log);

    let err = match SegmentedExecutionLog::open(SessionId::new("someone-else"), config(&dir, 1)) {
        Ok(_) => panic!("identity mismatch must fail"),
        Err(e) => e,
    };
    match err {
        chronos_log::LogError::IdentityMismatch {
            requested,
            manifest,
        } => {
            assert_eq!(requested, "someone-else");
            assert_eq!(manifest, "ret9-original");
        }
        other => panic!("expected IdentityMismatch, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// RET-10: a legacy directory without a manifest cannot have its retention
/// inferred, so it refuses instead of fabricating truth from absence.
#[test]
fn ret_10_legacy_dir_without_manifest_is_not_inferred() {
    let dir = tmpdir("ret10");
    let session = SessionId::new("ret10");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 10);
    log.flush().expect("flush");
    drop(log);

    // Simulate a legacy layout: no manifest, and history that does not start at 0.
    std::fs::remove_file(dir.join(chronos_log::segmented::MANIFEST_FILE_NAME))
        .expect("drop manifest");
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = entry.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with(".seg") && name.contains("-0.seg") {
            std::fs::remove_file(&p).expect("remove earliest segment");
        }
    }

    let err = match SegmentedExecutionLog::open(session.clone(), config(&dir, 1)) {
        Ok(_) => panic!("cannot infer retention from absence"),
        Err(e) => e,
    };
    assert!(
        matches!(err, chronos_log::LogError::RetentionMetadataMissing { .. }),
        "expected RetentionMetadataMissing, got {err:?}"
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
