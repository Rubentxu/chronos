//! REC-C1.5.3 — tail state: clean, abnormal, or unknown.
//!
//! Closure criterion: Chronos can distinguish a clean end, an abnormal end and
//! an unknown end WITHOUT degrading or falsifying the integrity of the validated
//! durable region.
//!
//! Run with `cargo test -p chronos-log --test rec_c1_5_3_tail_state`.

use std::num::NonZeroUsize;

use chronos_log::{
    EventSeq, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog, SessionId, TailState,
};

fn tmpdir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rec-c1-5-3-{tag}-{}-{}",
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
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session.clone(),
            monotonic_ns: i,
            payload: chronos_log::ExecutionPayload::new(format!("r{i}").into_bytes(), "t"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect("append");
    }
}

fn manifest_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join(chronos_log::segmented::MANIFEST_FILE_NAME)
}

/// TAIL-1: a brand-new log is Open.
#[test]
fn tail_1_new_log_is_open() {
    let dir = tmpdir("t1");
    let log = SegmentedExecutionLog::open(SessionId::new("t1"), config(&dir, 1)).expect("open");
    assert_eq!(log.tail_state(), TailState::Open);
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-2 + TAIL-13: a clean seal records the exact tail, including for an empty
/// session.
#[test]
fn tail_2_13_clean_seal_records_the_exact_tail() {
    let dir = tmpdir("t2");
    let session = SessionId::new("t2");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");

    // TAIL-13: sealing an empty session is legitimate.
    let empty_seal = log.seal().expect("seal empty");
    assert_eq!(empty_seal.tail_seq, None);
    assert!(matches!(
        log.tail_state(),
        TailState::Sealed { tail_seq: None, .. }
    ));
    drop(log);

    // A fresh dir for the non-empty case.
    let dir2 = tmpdir("t2b");
    let session2 = SessionId::new("t2b");
    let log = SegmentedExecutionLog::open(session2.clone(), config(&dir2, 1)).expect("open");
    append_n(&log, &session2, 10);
    let sealed = log.seal().expect("seal");
    assert_eq!(sealed.tail_seq, Some(chronos_log::EventSeq::new(9)));
    match log.tail_state() {
        TailState::Sealed { tail_seq, .. } => assert_eq!(tail_seq, Some(EventSeq::new(9))),
        other => panic!("expected Sealed, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

/// TAIL-3: a sealed run reopens as sealed when replay rebuilds exactly that tail.
#[test]
fn tail_3_sealed_reopen_preserves_the_seal() {
    let dir = tmpdir("t3");
    let session = SessionId::new("t3");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 6);
    log.seal().expect("seal");
    drop(log);

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    match reopened.tail_state() {
        TailState::Sealed { tail_seq, .. } => assert_eq!(tail_seq, Some(EventSeq::new(5))),
        other => panic!("expected Sealed, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-4: deleting the LAST segment of a sealed run cannot be detected by
/// C1.5.2 (nothing follows it), but the seal witnesses it.
#[test]
fn tail_4_missing_last_segment_is_a_tail_integrity_mismatch() {
    let dir = tmpdir("t4");
    let session = SessionId::new("t4");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 6);
    log.seal().expect("seal");
    let last = log
        .flushed_segments()
        .into_iter()
        .max_by_key(|(start, _, _)| start.0)
        .expect("a last segment")
        .2;
    drop(log);
    std::fs::remove_file(&last).expect("remove last segment");

    let err = match SegmentedExecutionLog::open(session.clone(), config(&dir, 1)) {
        Ok(_) => panic!("a missing sealed tail must not be published"),
        Err(e) => e,
    };
    match err {
        chronos_log::LogError::TailIntegrityMismatch {
            expected, actual, ..
        } => {
            assert_eq!(expected, Some(5));
            assert_eq!(actual, Some(4), "replay rebuilt one record less");
        }
        other => panic!("expected TailIntegrityMismatch, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-5 + TAIL-14: a persisted Open that was never sealed reopens as Unclean,
/// never as Sealed, and the validated prefix stays readable.
#[test]
fn tail_5_14_persisted_open_becomes_unclean_with_readable_prefix() {
    let dir = tmpdir("t5");
    let session = SessionId::new("t5");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 4);
    log.flush().expect("flush");
    drop(log); // crash: no seal

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    match reopened.tail_state() {
        TailState::Unclean {
            last_durable_seq,
            reason,
        } => {
            assert_eq!(last_durable_seq, Some(EventSeq::new(3)));
            assert!(reason.contains("not sealed"), "{reason}");
        }
        other => panic!("expected Unclean, got {other:?}"),
    }
    // The validated prefix is still fully usable.
    let page = reopened
        .read_from_seq(chronos_log::EventSeq::ZERO, 10)
        .expect("prefix readable");
    assert_eq!(page.records.len(), 4);
    assert_eq!(reopened.tail_seq(), Some(chronos_log::EventSeq::new(3)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-6: a live segment temp after a crash marks the tail Unclean and is never
/// replayed or turned into a Gap.
#[test]
fn tail_6_live_segment_temp_marks_unclean_and_is_not_evidence() {
    let dir = tmpdir("t6");
    let session = SessionId::new("t6");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 4);
    log.flush().expect("flush");
    drop(log);

    // Simulate an interrupted write: a temp at the canonical position.
    let tmp = dir.join("t6-4.tmp");
    std::fs::write(&tmp, b"partial").expect("write temp");

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    match reopened.tail_state() {
        TailState::Unclean { reason, .. } => assert!(reason.contains("temp"), "{reason}"),
        other => panic!("expected Unclean, got {other:?}"),
    }
    // The temp is not evidence: it never appears as records or as a Gap.
    let page = reopened
        .read_from_seq(chronos_log::EventSeq::ZERO, 100)
        .expect("read");
    assert_eq!(page.records.len(), 4, "only durable records");
    assert!(page.gaps.is_empty(), "a temp is not a Gap");
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-7: a temp entirely below the retention boundary says nothing about the
/// live tail.
#[test]
fn tail_7_retired_temp_does_not_contaminate_the_tail() {
    let dir = tmpdir("t7");
    let session = SessionId::new("t7");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 10);
    log.flush().expect("flush");
    let retained = log
        .retain_up_to(chronos_log::EventSeq::new(4))
        .expect("retain")
        .retained_from;
    assert!(retained > chronos_log::EventSeq::ZERO);
    // A clean seal gives a known-good baseline, so anything Unclean afterwards
    // can only come from the temp.
    log.seal().expect("seal");
    drop(log);

    // A retired temp (below the boundary) is reclaimable garbage.
    std::fs::write(dir.join("t7-1.tmp"), b"old").expect("write retired temp");

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    assert!(
        matches!(reopened.tail_state(), TailState::Sealed { .. }),
        "a retired temp must not accuse the tail: {:?}",
        reopened.tail_state()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-8: a legacy v1 manifest yields Unknown, never a guess.
#[test]
fn tail_8_legacy_manifest_is_unknown() {
    use std::io::Write;
    let dir = tmpdir("t8");
    let session = SessionId::new("t8");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 3);
    log.flush().expect("flush");
    drop(log);

    // Rewrite the manifest as v1 (no tail_state at all).
    let legacy = serde_json::json!({
        "schema_version": 1,
        "session_id": "t8",
        "retained_from": 0,
        "created_at_unix_ms": 1234
    });
    let mut f = std::fs::File::create(manifest_path(&dir)).expect("open manifest");
    f.write_all(legacy.to_string().as_bytes()).expect("write");
    drop(f);

    let reopened = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("reopen");
    match reopened.tail_state() {
        TailState::Unknown { reason } => assert!(reason.contains("legacy"), "{reason}"),
        other => panic!("a legacy manifest must be Unknown, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-9: sealing with a live temp in flight fails and does not write Sealed.
#[test]
fn tail_9_seal_with_live_temp_fails() {
    let dir = tmpdir("t9");
    let session = SessionId::new("t9");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 3);
    log.flush().expect("flush");
    // A temp at the next canonical position, as an interrupted write would leave.
    std::fs::write(dir.join("t9-3.tmp"), b"inflight").expect("write temp");

    let err = log.seal().expect_err("seal must refuse");
    assert!(
        err.to_string().contains("temp") || err.to_string().contains("LiveSegmentTemp"),
        "{err}"
    );
    assert!(
        !matches!(log.tail_state(), TailState::Sealed { .. }),
        "a refused seal must not be recorded"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-10: a sealed log accepts no further writes.
#[test]
fn tail_10_writes_after_seal_are_refused() {
    use chronos_log::{Gap, GapReason};
    let dir = tmpdir("t10");
    let session = SessionId::new("t10");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 2);
    log.seal().expect("seal");

    let append_err = log
        .append(NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,

            session_id: session.clone(),
            monotonic_ns: 99,
            payload: chronos_log::ExecutionPayload::new(b"late".to_vec(), "t"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect_err("append after seal must fail");
    assert!(
        matches!(append_err, chronos_log::LogError::LogSealed(_)),
        "{append_err:?}"
    );

    let gap_err = log
        .record_gap(Gap::new(
            chronos_log::EventSeq::new(50),
            chronos_log::EventSeq::new(51),
            GapReason::ProcessDetached,
            "t",
        ))
        .expect_err("record_gap after seal must fail");
    assert!(
        matches!(gap_err, chronos_log::LogError::LogSealed(_)),
        "{gap_err:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// TAIL-11 + TAIL-12: retention only changes `retained_from`; the seal, the tail
/// and `created_at` are preserved.
#[test]
fn tail_11_12_retention_preserves_tail_state_and_created_at() {
    // Sealed + retention keeps Sealed.
    let dir = tmpdir("t11");
    let session = SessionId::new("t11");
    let log = SegmentedExecutionLog::open(session.clone(), config(&dir, 1)).expect("open");
    append_n(&log, &session, 12);
    log.seal().expect("seal");
    let before = chronos_log::segmented::read_manifest(&dir)
        .unwrap()
        .expect("manifest");
    log.retain_up_to(chronos_log::EventSeq::new(5))
        .expect("retention after seal");
    let after = chronos_log::segmented::read_manifest(&dir)
        .unwrap()
        .expect("manifest");
    assert!(
        after.retained_from > before.retained_from,
        "retention moved"
    );
    assert_eq!(after.created_at_unix_ms, before.created_at_unix_ms);
    assert_eq!(after.tail_state, before.tail_state, "seal preserved");
    drop(log);

    // Open + retention keeps Open.
    let dir2 = tmpdir("t12");
    let session2 = SessionId::new("t12");
    let log2 = SegmentedExecutionLog::open(session2.clone(), config(&dir2, 1)).expect("open");
    append_n(&log2, &session2, 12);
    log2.flush().expect("flush");
    log2.retain_up_to(chronos_log::EventSeq::new(5))
        .expect("retention while open");
    let m2 = chronos_log::segmented::read_manifest(&dir2)
        .unwrap()
        .expect("manifest");
    assert_eq!(m2.tail_state, Some(TailState::Open), "Open preserved");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}
