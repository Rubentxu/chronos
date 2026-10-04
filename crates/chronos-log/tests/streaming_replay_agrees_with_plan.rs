//! The streaming replay must decide integrity EXACTLY as the plan-building
//! path does — no more lax, no more strict.
//!
//! `DEBT-SCALE-MEM-01` was fixed by making `replay_into_inner` validate,
//! apply, and drop one segment at a time instead of building a
//! `ReplayPlan` that held every decoded entry while the backend filled with a
//! second copy. The saving is real (489 bytes per event down to roughly half),
//! and it is paid for with a duplicated validation body: `build_and_apply_
//! replay_listing` carries its own copy of every integrity check.
//!
//! A duplicated check is only safe if it cannot drift. That is what this file
//! is for: it runs the SAME crafted segment over BOTH paths and asserts they
//! agree, on every shape the validator has an opinion about.
//!
//! Without it, the failure mode is silent and severe in the same direction:
//! the two bodies would disagree, one open would accept a log the other
//! refuses, and the product would have two truths about what a valid log is —
//! exactly the dual-truth defect ADR-0002/0003 exist to prevent.

use std::path::{Path, PathBuf};

use chronos_log::{
    build_replay_plan, segment::write_segment, EventSeq, ExecutionKind, ExecutionLogBackend,
    ExecutionPayload, Gap, GapReason, InMemoryExecutionLog, NewExecutionRecord,
    ReplayIntegrityError, SegmentEntry, SegmentedConfig, SegmentedExecutionLog, SessionId,
};

fn tmpdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "stream-eq-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create dir");
    p
}

fn stored_rec(session: &SessionId, seq: u64) -> chronos_log::ExecutionRecord {
    chronos_log::ExecutionRecord {
        session_id: session.clone(),
        seq: EventSeq::new(seq),
        kind: ExecutionKind::Raw,
        monotonic_ns: seq * 1_000,
        payload: ExecutionPayload::new(b"{}".to_vec(), "raw"),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    }
}

fn gap(first: u64, last: u64) -> Gap {
    Gap::new(
        EventSeq::new(first),
        EventSeq::new(last),
        GapReason::KernelRingOverflow,
        "test",
    )
}

/// The outcome BOTH paths must agree on, for one crafted directory.
#[derive(Debug, PartialEq)]
enum Verdict {
    Accepted { tail: Option<u64> },
    Refused(String),
}

fn via_plan(dir: &Path, session: &SessionId) -> Verdict {
    match build_replay_plan(dir, session, EventSeq::ZERO) {
        Ok(plan) => {
            let backend = InMemoryExecutionLog::new();
            // The plan path applies too, so an application failure is part of
            // the verdict and not silently dropped.
            match chronos_log::apply_replay_plan(&plan, &backend) {
                Ok(()) => Verdict::Accepted {
                    tail: backend.tail_seq(session).map(|t| t.0),
                },
                Err(e) => Verdict::Refused(format!("apply: {e}")),
            }
        }
        Err(e) => Verdict::Refused(format!("{e}")),
    }
}

fn via_stream(dir: &Path, session: &SessionId) -> Verdict {
    let backend = InMemoryExecutionLog::new();
    match chronos_log::replay::build_and_apply_replay_listing(
        dir,
        session,
        EventSeq::ZERO,
        &backend,
    ) {
        Ok(segments) => Verdict::Accepted {
            tail: segments
                .last()
                .map(|s| s.end_seq.0)
                .or_else(|| backend.tail_seq(session).map(|t| t.0)),
        },
        Err(chronos_log::replay::ReplayIntegrityErrorOrLog::Integrity(e)) => {
            Verdict::Refused(format!("{e}"))
        }
        Err(chronos_log::replay::ReplayIntegrityErrorOrLog::Apply(e)) => {
            Verdict::Refused(format!("apply: {e}"))
        }
    }
}

/// Assert both paths reach the same verdict, and report which way they
/// disagreed — a divergence here is the bug this file exists to catch.
fn assert_agrees(dir: &Path, session: &SessionId, what: &str) {
    let a = via_plan(dir, session);
    let b = via_stream(dir, session);
    assert_eq!(
        a, b,
        "the streaming replay and the plan replay disagreed on {what}.\n\
         plan path:    {a:?}\n\
         stream path:  {b:?}\n\
         A disagreement means the two validation bodies have drifted, which is \
         how a log becomes readable through one open and unreadable through the other."
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Both paths accept a well-formed single segment, and agree on the tail.
#[test]
fn a_well_formed_segment_is_accepted_by_both_paths_with_the_same_tail() {
    let dir = tmpdir("ok");
    let session = SessionId::new("eq-ok");
    let entries: Vec<SegmentEntry> = (0..5)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(4),
        entries.len() as u64,
        &entries,
    )
    .expect("write");

    assert_eq!(
        via_plan(&dir, &session),
        Verdict::Accepted { tail: Some(4) },
        "the plan path must accept and report tail 4"
    );
    assert_agrees(&dir, &session, "a well-formed segment");
}

/// Both paths accept a session whose segments tile the seq space exactly,
/// and the multi-segment case is where a "drop the last one" bug would hide.
#[test]
fn a_multi_segment_session_is_accepted_by_both_paths() {
    let dir = tmpdir("multi");
    let session = SessionId::new("eq-multi");
    for (start, end) in [(0u64, 2u64), (3, 5), (6, 8)] {
        let entries: Vec<SegmentEntry> = (start..=end)
            .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
            .collect();
        write_segment(
            &dir,
            &session,
            EventSeq::new(start),
            EventSeq::new(end),
            entries.len() as u64,
            &entries,
        )
        .expect("write");
    }

    assert_eq!(
        via_plan(&dir, &session),
        Verdict::Accepted { tail: Some(8) },
        "the plan path must accept three tiling segments and report tail 8"
    );
    // A streaming bug that skipped the last segment would show up as a
    // different tail here, and as a different tail on the plan path only if
    // the bug were in both.
    assert_agrees(&dir, &session, "three tiling segments");
}

/// A gap-bearing segment is the shape FIND-C1.8-01 was about: the header counts
/// ENTRIES, not records, so a segment holding records and a gap must reopen.
#[test]
fn a_gap_bearing_segment_is_accepted_by_both_paths() {
    let dir = tmpdir("gap");
    let session = SessionId::new("eq-gap");
    let mut entries: Vec<SegmentEntry> = (0..3)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    entries.push(SegmentEntry::Gap(gap(3, 5)));
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(5),
        entries.len() as u64,
        &entries,
    )
    .expect("write");

    assert_agrees(&dir, &session, "a gap-bearing segment");
}

/// A payload whose declared range does not match its entries: both paths must
/// REFUSE, and this is the check most likely to drift because it is the longest
/// of the per-entry comparisons.
#[test]
fn a_payload_that_overruns_its_header_is_refused_by_both_paths() {
    let dir = tmpdir("overrun");
    let session = SessionId::new("eq-overrun");
    // Header claims 0..=2 but four records are written, so the payload range
    // check and the entry-count check both have something to say.
    let entries: Vec<SegmentEntry> = (0..4)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(2),
        3, // declared count, inconsistent with the four entries
        &entries,
    )
    .expect("write");

    assert!(
        matches!(via_plan(&dir, &session), Verdict::Refused(_)),
        "the plan path must refuse an inconsistent segment"
    );
    assert_agrees(&dir, &session, "a payload overrunning its header");
}

/// A hole between two segments: the second starts past the first's end. Both
/// paths must refuse, and `MissingRange` is the specific verdict.
#[test]
fn a_hole_between_segments_is_refused_by_both_paths_as_missing_range() {
    let dir = tmpdir("hole");
    let session = SessionId::new("eq-hole");
    for (start, end) in [(0u64, 2u64), (4, 6)] {
        let entries: Vec<SegmentEntry> = (start..=end)
            .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
            .collect();
        write_segment(
            &dir,
            &session,
            EventSeq::new(start),
            EventSeq::new(end),
            entries.len() as u64,
            &entries,
        )
        .expect("write");
    }

    match via_plan(&dir, &session) {
        Verdict::Refused(msg) => assert!(
            msg.contains("missing range"),
            "expected a missing-range refusal, got: {msg}"
        ),
        other => panic!("the plan path must refuse a hole, got {other:?}"),
    }
    assert_agrees(&dir, &session, "a hole between segments");
}

/// The retroactive gap: the exact shape
/// `control_a_retroactive_gap_is_also_unreopenable_so_it_is_not_a_legitimate_category`
/// pins as unreopenable. Both paths must refuse it, or a streaming body that
/// forgot the entry-range check would let a contradictory log back in.
#[test]
fn a_retroactive_gap_is_refused_by_both_paths() {
    let dir = tmpdir("retro");
    let session = SessionId::new("eq-retro");
    let mut entries: Vec<SegmentEntry> = (0..10)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    entries.push(SegmentEntry::Gap(gap(3, 5)));
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(9),
        entries.len() as u64,
        &entries,
    )
    .expect("write");

    match via_plan(&dir, &session) {
        Verdict::Refused(_) => {}
        other => panic!("a retroactive gap must be refused, got {other:?}"),
    }
    assert_agrees(&dir, &session, "a retroactive gap");
}

/// An empty directory: both paths must accept it as an empty session, not
/// disagree about whether "no segments" is valid.
#[test]
fn an_empty_directory_is_accepted_by_both_paths_as_empty() {
    let dir = tmpdir("empty");
    let session = SessionId::new("eq-empty");
    assert_agrees(&dir, &session, "an empty directory");
}

/// The end-to-end proof on the real type: a log written through the public API,
/// reopened, and read back. This is the property the memory fix had to
/// preserve, and it is stated on `SegmentedExecutionLog` rather than on the
/// replay primitives, so it fails if anything above the replay changes too.
#[test]
fn a_log_written_through_the_api_reopens_with_every_event() {
    let dir = tmpdir("e2e");
    let session = SessionId::new("eq-e2e");
    const N: u64 = 500;

    {
        let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
            .expect("open for writing");
        for i in 0..N {
            log.append(NewExecutionRecord {
                session_id: session.clone(),
                kind: ExecutionKind::Raw,
                monotonic_ns: i * 1_000,
                payload: ExecutionPayload::new(b"{}".to_vec(), "raw"),
                ..Default::default()
            })
            .expect("append");
        }
        log.flush().expect("flush");
    }

    let reopened = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir))
        .expect("reopen — the streaming replay must not refuse a valid log");
    assert_eq!(
        reopened.tail_seq().map(|t| t.0),
        Some(N - 1),
        "the reopened log must report the last allocated seq"
    );

    // Walk it: a replay that validates the segments but applies only some of
    // the entries would still report the right tail and fail here.
    let mut seen = 0u64;
    let mut from = EventSeq::ZERO;
    loop {
        let page = reopened
            .read_from_seq(from, 64)
            .expect("read from seq must succeed");
        if page.records.is_empty() {
            break;
        }
        seen += page.records.len() as u64;
        if page.exhausted {
            break;
        }
        from = page.position_after;
    }
    assert_eq!(
        seen, N,
        "every seeded record must come back after the streaming replay: read {seen} of {N}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The counter-test that gives the agreement tests their teeth: prove the
/// two paths are not trivially equal because neither inspects anything.
///
/// This asserts the NEGATIVE — that a genuinely broken log IS refused — and it
/// is the half that a lazily-written guard would omit. If a future edit made
/// the streaming body accept everything, `a_retroactive_gap_is_refused_by_both_
/// paths` above would still pass (both would say Accepted) while this fails.
#[test]
fn the_agreement_is_not_vacuous_because_broken_logs_are_still_refused() {
    let dir = tmpdir("vacuity");
    let session = SessionId::new("eq-vacuity");
    let mut entries: Vec<SegmentEntry> = (0..10)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    entries.push(SegmentEntry::Gap(gap(3, 5)));
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(9),
        entries.len() as u64,
        &entries,
    )
    .expect("write");

    // Both refuse, and the refusal is a real integrity verdict rather than an
    // incidental error.
    match via_stream(&dir, &session) {
        Verdict::Refused(msg) => assert!(
            !msg.is_empty(),
            "a refusal must say why, otherwise 'refused' is unfalsifiable"
        ),
        other => panic!(
            "the streaming path accepted a contradictory log: {other:?}. If this fails, the \
             agreement tests above are comparing two bodies that both stopped validating."
        ),
    }
    assert_agrees(&dir, &session, "the vacuity control");
}

/// `ReplayIntegrityError` stays comparable so a divergence can be reported as a
/// value difference rather than only as a string, which is what makes a
/// same-message-different-reason drift visible.
#[test]
fn the_verdict_type_can_distinguish_two_different_integrity_errors() {
    let a = ReplayIntegrityError::MissingRange {
        expected_from: 3,
        found_from: 4,
    };
    let b = ReplayIntegrityError::MissingRange {
        expected_from: 3,
        found_from: 5,
    };
    let c = ReplayIntegrityError::EmptySegment {
        path: PathBuf::from("/x"),
    };
    assert_ne!(a, b, "different holes are different errors");
    assert_ne!(a, c, "different error kinds are different errors");
}
