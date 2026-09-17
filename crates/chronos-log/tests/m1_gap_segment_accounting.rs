//! M1 — characterization: segment header counts entries, replay counts records.
//!
//! FIND-C1.8-01, characterized. The on-disk segment payload is a sequence
//! of `SegmentEntry` items; each entry is either an `ExecutionRecord` or a
//! `Gap`. The header writes a count at offset 32. Today the two sides
//! disagree on what that count means:
//!
//! - **writer** (`segmented.rs::flush_inner`): `entry_count = buffer.len()`
//!   — every persisted `SegmentEntry` (records **and** gaps).
//! - **replay** (`replay.rs::build_replay_plan`): counts only
//!   `SegmentEntry::Record` and compares it to the header.
//!
//! So a segment that contains a `Gap` declared N entries but validated as
//! N−gaps records, and reopen failed with `RecordCountMismatch`
//! ("declares N records but holds N−gaps").
//!
//! The fix made all three agree on one unit: the persisted `SegmentEntry`
//! count. The header field is named `entry_count`, the writer emits
//! `buffer.len()`, and replay counts every entry. This test now documents
//! the invariant end to end.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use chronos_log::segment::{read_segment, SegmentEntry};
use chronos_log::{
    EventSeq, ExecutionPayload, Gap, GapReason, NewExecutionRecord, SegmentedConfig,
    SegmentedExecutionLog, SessionId,
};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-m1-gap-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn rec(session: &SessionId, ns: u64, tag: &str) -> NewExecutionRecord {
    NewExecutionRecord {
        session_id: session.clone(),
        monotonic_ns: ns,
        payload: ExecutionPayload::new(format!("payload-{tag}").into_bytes(), "m1_gap"),
        ..Default::default()
    }
}

fn only_segment_file(dir: &std::path::Path) -> PathBuf {
    let mut segs: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "seg").unwrap_or(false))
        .collect();
    segs.sort();
    assert_eq!(segs.len(), 1, "expected exactly one segment file: {segs:?}");
    segs.pop().unwrap()
}

#[test]
fn char_header_counts_entries_but_replay_counts_records() {
    let dir = tempdir("char");
    let session = SessionId::new("m1-gap-char");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    // Large threshold: force everything into a single segment.
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    // Factory: 2 records, 1 gap (one entry covering seqs 2..=5), 1 record.
    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        log.append(rec(&session, 0, "a")).expect("append a");
        log.append(rec(&session, 1, "b")).expect("append b");
        log.record_gap(Gap::new(
            EventSeq::new(2),
            EventSeq::new(5),
            GapReason::AdapterBufferOverflow,
            "m1-char",
        ))
        .expect("record_gap");
        log.append(rec(&session, 6, "c")).expect("append c");
        log.flush().expect("flush");
    }

    let seg = only_segment_file(&dir);
    let decoded = read_segment(&seg).expect("decode segment");

    let entry_count = decoded.entries.len() as u64;
    let record_only = decoded
        .entries
        .iter()
        .filter(|e| matches!(e, SegmentEntry::Record(_)))
        .count() as u64;

    // 4 persisted entries: Record(0), Record(1), Gap(2..=5), Record(6).
    assert_eq!(entry_count, 4, "persisted entries");
    assert_eq!(record_only, 3, "ExecutionRecord entries only");

    // The header carries the ENTRY count (writer uses buffer.len()).
    assert_eq!(
        decoded.metadata.entry_count, entry_count,
        "header counts every persisted entry (records + gaps)"
    );

    // Replay validates the header against the same unit, so a segment
    // containing a Gap reopens cleanly.
    let reopened = SegmentedExecutionLog::open(session.clone(), cfg.clone());
    assert!(
        reopened.is_ok(),
        "reopen must succeed: writer/header/replay share one unit; got: {:?}",
        reopened.err()
    );
}

// ---------------------------------------------------------------------------
// GAP-PERSIST-1..6 — the invariant this cycle establishes.
// ---------------------------------------------------------------------------

/// GAP-PERSIST-1: records + Gap + records → flush → reopen → the same
/// sequence and the same Gap survive.
#[test]
fn gap_persist_1_records_gap_records_survive_reopen() {
    let dir = tempdir("p1");
    let session = SessionId::new("m1-gap-p1");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        log.append(rec(&session, 10, "r0")).expect("append r0");
        log.append(rec(&session, 11, "r1")).expect("append r1");
        log.record_gap(Gap::new(
            EventSeq::new(2),
            EventSeq::new(4),
            GapReason::AdapterBufferOverflow,
            "p1",
        ))
        .expect("record_gap");
        log.append(rec(&session, 15, "r5")).expect("append r5");
        log.append(rec(&session, 16, "r6")).expect("append r6");
        log.flush().expect("flush");
    }

    let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("reopen");
    let page = log.read_from_seq(EventSeq::ZERO, 100).expect("read");
    let seqs: Vec<u64> = page.records.iter().map(|r| r.seq.0).collect();
    assert_eq!(seqs, vec![0, 1, 5, 6], "record seqs survive reopen");
    assert_eq!(page.gaps.len(), 1, "the Gap survives reopen");
    assert_eq!(page.gaps[0].first_missing, EventSeq::new(2));
    assert_eq!(page.gaps[0].last_missing, EventSeq::new(4));
}

/// GAP-PERSIST-2: a Gap created by `record_gap` produces a segment that
/// reopens valid.
#[test]
fn gap_persist_2_record_gap_reopens_valid() {
    let dir = tempdir("p2");
    let session = SessionId::new("m1-gap-p2");
    let cfg = SegmentedConfig::with_dir(&dir);

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        log.append(rec(&session, 0, "a")).expect("append a");
        log.record_gap(Gap::new(
            EventSeq::new(1),
            EventSeq::new(9),
            GapReason::AdapterBufferOverflow,
            "p2",
        ))
        .expect("record_gap");
        log.flush().expect("flush");
    }

    SegmentedExecutionLog::open(session.clone(), cfg).expect("reopen after record_gap");
}

/// GAP-PERSIST-3: a Gap created by the memory-budget overflow path
/// produces a segment that reopens valid.
#[test]
fn gap_persist_3_overflow_gap_reopens_valid() {
    let dir = tempdir("p3");
    let session = SessionId::new("m1-gap-p3");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    // Tiny budget: the oversized append trips the overflow gap path.
    cfg.memory_budget_bytes = Some(128);

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        log.append(rec(&session, 0, "small")).expect("append small");
        log.append(NewExecutionRecord {
            session_id: session.clone(),
            monotonic_ns: 10,
            payload: ExecutionPayload::new(vec![0u8; 4096], "oversize"),
            ..Default::default()
        })
        .expect("append oversize (triggers overflow gap)");
        log.flush().expect("flush");
    }

    let log =
        SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("reopen after overflow gap");
    let page = log.read_from_seq(EventSeq::ZERO, 100).expect("read");
    assert!(
        !page.gaps.is_empty(),
        "the overflow Gap must survive reopen; page: {:?}",
        page
    );
}

/// GAP-PERSIST-4: the header count equals exactly the number of
/// serialized entries.
#[test]
fn gap_persist_4_header_entry_count_equals_serialized_entries() {
    let dir = tempdir("p4");
    let session = SessionId::new("m1-gap-p4");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        log.append(rec(&session, 0, "a")).expect("append a");
        log.record_gap(Gap::new(
            EventSeq::new(1),
            EventSeq::new(3),
            GapReason::AdapterBufferOverflow,
            "p4",
        ))
        .expect("record_gap");
        log.append(rec(&session, 4, "b")).expect("append b");
        log.append(rec(&session, 5, "c")).expect("append c");
        log.flush().expect("flush");
    }

    let seg = only_segment_file(&dir);
    let decoded = read_segment(&seg).expect("decode");
    assert_eq!(
        decoded.metadata.entry_count as usize,
        decoded.entries.len(),
        "header entry_count must equal the serialized entries"
    );
    assert_eq!(decoded.entries.len(), 4, "1 record + 1 gap + 2 records");
}

/// GAP-PERSIST-5: a Gap 100..=199 occupies ONE entry, preserves its
/// range, and the next appended record gets seq 200.
#[test]
fn gap_persist_5_gap_occupies_one_entry_and_preserves_range() {
    let dir = tempdir("p5");
    let session = SessionId::new("m1-gap-p5");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    let next_seq;
    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        // Records 0..99.
        for i in 0..100u64 {
            log.append(rec(&session, i, "pre")).expect("append pre");
        }
        // Gap occupying 100..=199.
        log.record_gap(Gap::new(
            EventSeq::new(100),
            EventSeq::new(199),
            GapReason::AdapterBufferOverflow,
            "p5",
        ))
        .expect("record_gap");
        // Next append must land at 200.
        next_seq = log.append(rec(&session, 200, "post")).expect("append post");
        log.flush().expect("flush");
    }
    assert_eq!(next_seq, EventSeq::new(200), "gap consumes its whole range");

    let seg = only_segment_file(&dir);
    let decoded = read_segment(&seg).expect("decode");
    let gaps: Vec<_> = decoded
        .entries
        .iter()
        .filter_map(|e| match e {
            SegmentEntry::Gap(g) => Some(g),
            _ => None,
        })
        .collect();
    assert_eq!(gaps.len(), 1, "the Gap is exactly one entry");
    assert_eq!(gaps[0].first_missing, EventSeq::new(100));
    assert_eq!(gaps[0].last_missing, EventSeq::new(199));
    // 100 records + 1 gap + 1 record = 102 entries.
    assert_eq!(decoded.entries.len(), 102);
    assert_eq!(decoded.metadata.entry_count as usize, 102);

    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("reopen");
    let page = log.read_from_seq(EventSeq::ZERO, 1000).expect("read");
    assert_eq!(page.records.len(), 101, "100 pre + 1 post");
    assert_eq!(page.gaps.len(), 1);
}

/// GAP-PERSIST-6: after reopen, a read crossing the gap reports it and the
/// surviving seq space is contiguous — never a silent hole.
#[test]
fn gap_persist_6_read_crossing_gap_reports_it() {
    let dir = tempdir("p6");
    let session = SessionId::new("m1-gap-p6");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        for i in 0..10u64 {
            log.append(rec(&session, i, "pre")).expect("append pre");
        }
        log.record_gap(Gap::new(
            EventSeq::new(10),
            EventSeq::new(19),
            GapReason::AdapterBufferOverflow,
            "p6",
        ))
        .expect("record_gap");
        for i in 20..30u64 {
            log.append(rec(&session, i, "post")).expect("append post");
        }
        log.flush().expect("flush");
    }

    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("reopen");
    let page = log.read_from_seq(EventSeq::ZERO, 100).expect("read");
    assert_eq!(page.records.len(), 20);
    assert_eq!(page.gaps.len(), 1, "the crossing read sees the gap");
    assert_eq!(page.gaps[0].first_missing, EventSeq::new(10));
    assert_eq!(page.gaps[0].last_missing, EventSeq::new(19));

    // The surviving record seqs are exactly 0..=9 then 20..=29: the gap is
    // declared, not a silent hole.
    let seqs: Vec<u64> = page.records.iter().map(|r| r.seq.0).collect();
    let mut expected: Vec<u64> = (0..10).collect();
    expected.extend(20..30);
    assert_eq!(seqs, expected);
}
