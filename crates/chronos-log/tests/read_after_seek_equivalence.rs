//! The seek in `read_after` must return EXACTLY what the full scan returned.
//!
//! `read_after` now starts its filter loop at
//! `first_reachable(last_seq + 1)` instead of walking the session from index
//! 0. That is the same seek `read_from_seq` uses, and it is what turns a
//! caught-up poll from O(session) into O(new) — measured at 51.2 ms → 0.003
//! ms per call over a 1M-event session.
//!
//! A passing test suite is NOT that proof, because the seek and the scan
//! agree on every fixture the suite happens to contain. This states the
//! property directly: for the same session and the same cursor, the seeked
//! read must deliver the records and gaps a filter over the WHOLE session
//! would have delivered.
//!
//! The reference is a fresh read on a DIFFERENT consumer, which returns the
//! entire session in append order; the expectation is that read filtered
//! down. Isolating the consumer is what keeps the two reads independent —
//! `read_after` persists the cursor, so a shared consumer would make the
//! second read see the first one's state.
//!
//! The fixtures deliberately include the cases where a seek could go wrong:
//!
//!   * a cursor in the MIDDLE, so both a prefix and a suffix are live;
//!   * a cursor already at the tail, so the window is empty;
//!   * a cursor before the head, so everything is live;
//!   * gaps, whose `reach()` is `last_missing` and not `first_missing`, so the
//!     seek partitions on a different field than the oldest-seq walk does.
//!
//! Two states are NOT covered here, because the public API cannot build them
//! and pretending otherwise would test a fiction. Both are covered in-crate
//! instead, where `force_entry` bypasses the guard on purpose:
//!
//!   * a reach-`disordered` list, which `record_gap` can no longer produce;
//!   * a cursor genuinely behind the oldest retained seq, which needs a
//!     retained prefix to have been advanced.

use chronos_log::{
    ConsumerCursor, EventSeq, ExecutionKind, ExecutionLogBackend, ExecutionPayload, Gap, GapReason,
    InMemoryExecutionLog, LogConsumerId, NewExecutionRecord, ReadResult, SessionId,
};

fn gap(first: u64, last: u64) -> Gap {
    Gap::new(
        EventSeq::new(first),
        EventSeq::new(last),
        GapReason::KernelRingOverflow,
        "seek-equivalence",
    )
}

/// The consumer the read under test uses.
fn consumer() -> LogConsumerId {
    LogConsumerId::new("equiv")
}

/// A consumer reserved for the reference read, so persisting one read's
/// cursor cannot change what the other read sees.
fn reference_consumer() -> LogConsumerId {
    LogConsumerId::new("equiv-ref")
}

fn push(log: &InMemoryExecutionLog, s: &SessionId, i: u64) {
    log.append(NewExecutionRecord {
        session_id: s.clone(),
        kind: ExecutionKind::Raw,
        monotonic_ns: i,
        payload: ExecutionPayload::new(vec![i as u8; 16], "equiv"),
        ..Default::default()
    })
    .expect("append");
}

fn seed(n: u64) -> (InMemoryExecutionLog, SessionId) {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("equiv");
    for i in 0..n {
        push(&log, &s, i);
    }
    (log, s)
}

type Delivered = (Vec<u64>, Vec<String>);

fn gap_key(g: &Gap) -> String {
    format!("{}..{}", g.first_missing.0, g.last_missing.0)
}

/// The whole session, in append order, read fresh.
fn whole_session(log: &InMemoryExecutionLog, s: &SessionId) -> Delivered {
    match log
        .read_after(s.clone(), reference_consumer(), None)
        .expect("reference read")
    {
        ReadResult::Ok { records, gaps, .. } => (
            records.iter().map(|r| r.seq.0).collect(),
            gaps.iter().map(gap_key).collect(),
        ),
        other => panic!("a fresh read of a present session must be Ok, got {other:?}"),
    }
}

/// What a filter over the WHOLE session delivers for a cursor at `last_seq`.
fn expected_from_whole(log: &InMemoryExecutionLog, s: &SessionId, last_seq: u64) -> Delivered {
    let (all_records, all_gaps) = whole_session(log, s);
    (
        all_records
            .into_iter()
            .filter(|seq| *seq > last_seq)
            .collect(),
        all_gaps
            .into_iter()
            .filter(|key| {
                let last: u64 = key
                    .split("..")
                    .nth(1)
                    .expect("gap key is `first..last`")
                    .parse()
                    .expect("gap key is numeric");
                last > last_seq
            })
            .collect(),
    )
}

fn assert_matches_whole_session(
    log: &InMemoryExecutionLog,
    s: &SessionId,
    label: &str,
    last_seq: u64,
) {
    let cursor = ConsumerCursor::at(consumer(), EventSeq::new(last_seq));
    let (records, gaps) = match log
        .read_after(s.clone(), consumer(), Some(cursor))
        .expect("read_after")
    {
        ReadResult::Ok { records, gaps, .. } => (
            records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
            gaps.iter().map(gap_key).collect::<Vec<_>>(),
        ),
        other => panic!("{label}: expected an Ok result, got {other:?}"),
    };
    let (want_records, want_gaps) = expected_from_whole(log, s, last_seq);
    assert_eq!(
        records, want_records,
        "{label}: records differ from the whole session"
    );
    assert_eq!(
        gaps, want_gaps,
        "{label}: gaps differ from the whole session"
    );
}

#[test]
fn the_seeked_read_matches_the_whole_session_on_every_cursor_position() {
    let (log, s) = seed(50);

    // At the head: everything after seq 0 is live.
    assert_matches_whole_session(&log, &s, "cursor at head", 0);
    // Middle: a prefix is skipped and a suffix is live.
    assert_matches_whole_session(&log, &s, "cursor mid-session", 10);
    // Exactly at the last seq: the window is empty.
    assert_matches_whole_session(&log, &s, "cursor at tail", 49);
    // One before the tail: exactly one record is live.
    assert_matches_whole_session(&log, &s, "cursor one before tail", 48);
    // Past the tail: still empty, and must not read as stale.
    assert_matches_whole_session(&log, &s, "cursor past tail", 500);
}

#[test]
fn a_fresh_cursor_still_delivers_the_whole_session() {
    let (log, s) = seed(50);
    match log
        .read_after(s.clone(), consumer(), None)
        .expect("fresh read")
    {
        ReadResult::Ok { records, gaps, .. } => {
            let (want_records, want_gaps) = whole_session(&log, &s);
            assert_eq!(
                records.iter().map(|r| r.seq.0).collect::<Vec<_>>(),
                want_records,
                "a fresh cursor must deliver every record"
            );
            assert_eq!(
                gaps.iter().map(gap_key).collect::<Vec<_>>(),
                want_gaps,
                "a fresh cursor must deliver every gap"
            );
        }
        other => panic!("a fresh read must be Ok, got {other:?}"),
    }
}

#[test]
fn the_seeked_read_matches_the_whole_session_when_the_session_has_gaps() {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("equiv-gaps");
    // Records 0..=9, a gap over 10..=19, then records 20..=29. The gap's
    // `reach()` is 19 while its `first_missing` is 10, so a seek that
    // partitions on the wrong field lands in the wrong place.
    for i in 0..10 {
        push(&log, &s, i);
    }
    log.record_gap(s.clone(), gap(10, 19)).expect("gap");
    for i in 20..30 {
        push(&log, &s, i);
    }

    assert_matches_whole_session(&log, &s, "gaps: cursor at head", 0);
    assert_matches_whole_session(&log, &s, "gaps: cursor inside leading records", 5);
    // A cursor that lands INSIDE the gap range: the gap is past the cursor and
    // must be delivered, which is what a `first_missing`-based seek drops.
    assert_matches_whole_session(&log, &s, "gaps: cursor inside the gap", 14);
    assert_matches_whole_session(&log, &s, "gaps: cursor just past the gap", 19);
    assert_matches_whole_session(&log, &s, "gaps: cursor at tail", 29);
}
