//! M1 — characterization: segment header counts entries, replay counts records.
//!
//! FIND-C1.8-01, characterized. The on-disk segment payload is a sequence
//! of `SegmentEntry` items; each entry is either an `ExecutionRecord` or a
//! `Gap`. The header writes a count at offset 32. Today the two sides
//! disagree on what that count means:
//!
//! - **writer** (`segmented.rs::flush_inner`): `record_count = buffer.len()`
//!   — every persisted `SegmentEntry` (records **and** gaps).
//! - **replay** (`replay.rs::build_replay_plan`): counts only
//!   `SegmentEntry::Record` and compares it to the header.
//!
//! So a segment that contains a `Gap` declares N entries but validates as
//! N−gaps records, and reopen fails with `RecordCountMismatch`
//! ("declares N records but holds N−gaps").
//!
//! This test pins the arithmetic. The `reopen` assertion is RED until the
//! writer/header/replay agree on one unit (the follow-up fix renames the
//! header field to `entry_count` and makes replay count every entry).
//!
//! It is a characterization, not a specification: the *first* two
//! assertions describe today's reality, the *last* one describes the
//! invariant the fix establishes.

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

    // Today the header carries the ENTRY count (writer uses buffer.len()).
    assert_eq!(
        decoded.metadata.record_count, entry_count,
        "characterization: header currently counts every persisted entry"
    );

    // ...but replay validates the header against the RECORD-only count, so
    // a segment containing a Gap cannot be reopened.
    let reopened = SegmentedExecutionLog::open(session.clone(), cfg.clone());
    assert!(
        reopened.is_ok(),
        "reopen must succeed once writer/header/replay share one unit; \
         today it fails with: {:?}",
        reopened.err()
    );
}
