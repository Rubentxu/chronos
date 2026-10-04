//! REC-C1.5.2 — strict, two-phase replay.
//!
//! ## The property this module establishes
//!
//! > A reopen may only publish an `ExecutionLog` if it can demonstrate that
//! > every segment of the retained region it reconstructs is intact and
//! > continuous. No unexplained hole may enter the registry.
//!
//! ## Two phases, never interleaved
//!
//! ```text
//! disk -> build_replay_plan() -> validate EVERYTHING
//!                                     |
//!                             ReplayPlan { segments, entries, tail }
//!                                     |
//!                             apply_replay_plan() -> fresh backend
//!                                     |
//!                                 publish
//! ```
//!
//! Applying segment-by-segment while reading would leave a partially
//! reconstructed backend when a later segment fails. Validation therefore
//! completes first and nothing is published on failure.
//!
//! ## What is checked beyond the checksum
//!
//! A valid BLAKE3 checksum only proves the bytes are the bytes that were
//! written. To support the C1.4 completeness guarantee we also validate the
//! SEMANTICS of the seq space:
//!
//! ```text
//! filename start_seq == header.start_seq
//! header.start_seq <= header.end_seq
//! first entry starts exactly at header.start_seq
//! each entry starts at previous_end + 1   (a Gap occupies its real range)
//! last entry ends exactly at header.end_seq
//! entry_count matches the persisted entries (records + Gaps)
//! every record.session_id == session_id
//! ```
//!
//! plus, across segments:
//!
//! ```text
//! first live segment starts exactly at retained_from
//! next.start == previous.end + 1        (else MissingRange / Overlap)
//! ```
//!
//! ## What this module does NOT do
//!
//! - It never turns corruption or a missing file into a `Gap`. A `Gap` is
//!   evidence produced during execution; later storage loss is a different kind
//!   of fact.
//! - It does not touch `.seg.tmp` or tail state: that is C1.5.3.
//! - Segments entirely below `retained_from` are reclaimable leftovers from
//!   C1.5.1 and stay out of the plan. A LIVE segment straddling the boundary is
//!   still a hard error (`SegmentCrossesRetention`), raised before this module.

use std::path::{Path, PathBuf};

use crate::gap::Gap;
use crate::record::SessionId;
use crate::segment::{read_segment, sanitize_session, SegmentEntry};
use crate::seq::EventSeq;

/// Why a replay was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplayIntegrityError {
    #[error("segment {path:?} could not be decoded: {reason}")]
    CorruptSegment { path: PathBuf, reason: String },

    #[error("segment {path:?} declares {start}..={end}, which is not a valid range")]
    HeaderRangeMismatch { path: PathBuf, start: u64, end: u64 },

    #[error(
        "segment {path:?} covers {first}..={last} but its header declares {header_start}..={header_end}"
    )]
    PayloadRangeMismatch {
        path: PathBuf,
        first: u64,
        last: u64,
        header_start: u64,
        header_end: u64,
    },

    #[error(
        "segment {path:?} filename says start {filename_start} but the header says {header_start}"
    )]
    FilenameHeaderMismatch {
        path: PathBuf,
        filename_start: u64,
        header_start: u64,
    },

    #[error("segment {path:?} holds a record for session {found}, expected {expected}")]
    RecordSessionMismatch {
        path: PathBuf,
        expected: String,
        found: String,
    },

    #[error("segment {path:?} declares {declared} entries but holds {actual}")]
    EntryCountMismatch {
        path: PathBuf,
        declared: u64,
        actual: u64,
    },

    #[error("missing range: expected a segment starting at {expected_from}, found one starting at {found_from}")]
    MissingRange { expected_from: u64, found_from: u64 },

    #[error("segment starting at {next_start} overlaps the previous segment ending at {prev_end}")]
    OverlappingSegments { prev_end: u64, next_start: u64 },

    #[error("segment {path:?} has no entries")]
    EmptySegment { path: PathBuf },
}

/// One validated segment.
#[derive(Debug, Clone)]
pub struct PlannedSegment {
    pub path: PathBuf,
    pub start_seq: EventSeq,
    pub end_seq: EventSeq,
}

/// A fully validated replay, ready to be applied in one step.
#[derive(Debug, Clone)]
pub struct ReplayPlan {
    pub session_id: SessionId,
    pub retained_from: EventSeq,
    pub segments: Vec<PlannedSegment>,
    /// Entries in seq order, already validated as contiguous.
    ///
    /// **Retaining these is what costs the memory, not what guarantees the
    /// validation.** Every integrity property this module enforces is decided
    /// from `segments` — continuity compares one segment's `start_seq` against
    /// the previous `end_seq` — plus per-segment checks that run while that
    /// segment is decoded. The entries were kept only so `apply_replay_plan`
    /// would not have to re-read the files, and that convenience means the plan
    /// holds the whole session while the backend it is filling holds a second
    /// copy of it. Measured on this host: opening a 200.000-event log costs
    /// **95.416 KB, 489 bytes per event**, and the two copies are the reason
    /// the figure is roughly double what one copy of the session needs.
    ///
    /// `build_and_apply_replay` does not build this field at all; it is kept
    /// for the callers that genuinely need a detached, re-appliable plan.
    pub entries: Vec<SegmentEntry>,
    /// Highest seq the plan reconstructs, if any.
    pub reconstructed_tail: Option<EventSeq>,
}

impl ReplayPlan {
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// List the live segments of `session_id` above `retained_from`, in seq order,
/// paired with the start seq their FILENAME declares.
///
/// Split out of [`build_replay_plan`] so the streaming variant below shares the
/// exact same selection — a second copy of this loop would be a second place for
/// the retention rule to drift.
fn live_segments(
    dir: &Path,
    session_id: &SessionId,
    retained_from: EventSeq,
) -> Result<Vec<(EventSeq, PathBuf)>, ReplayIntegrityError> {
    let safe = sanitize_session(session_id);
    let prefix = format!("{safe}-");

    // Gather candidate segments with their filename-declared start seq.
    let mut declared: Vec<(EventSeq, PathBuf)> = Vec::<(EventSeq, PathBuf)>::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(declared),
        Err(e) => {
            return Err(ReplayIntegrityError::CorruptSegment {
                path: dir.to_path_buf(),
                reason: format!("read_dir: {e}"),
            })
        }
    };
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // `.seg.tmp` and anything else is deliberately out of scope (C1.5.3).
        if !name.ends_with(".seg") || !name.starts_with(&prefix) {
            continue;
        }
        let Some(rest) = name
            .strip_prefix(&prefix)
            .and_then(|r| r.strip_suffix(".seg"))
        else {
            continue;
        };
        let Ok(seq) = rest.parse::<u64>() else {
            continue;
        };
        // Retired leftovers: present on disk, logically gone. Not evidence.
        if EventSeq::new(seq) < retained_from {
            continue;
        }
        declared.push((EventSeq::new(seq), entry.path()));
    }
    declared.sort_by_key(|(seq, _)| seq.0);
    Ok(declared)
}

/// Read and validate every live segment. Nothing is applied here.
///
/// `retained_from` is the C1.5.1 logical boundary: segments entirely below it
/// are expected leftovers and are excluded. Segments at or above it are
/// validated; a failure anywhere means NO plan.
pub fn build_replay_plan(
    dir: &Path,
    session_id: &SessionId,
    retained_from: EventSeq,
) -> Result<ReplayPlan, ReplayIntegrityError> {
    let declared = live_segments(dir, session_id, retained_from)?;
    if declared.is_empty() {
        return Ok(ReplayPlan {
            session_id: session_id.clone(),
            retained_from,
            segments: Vec::new(),
            entries: Vec::new(),
            reconstructed_tail: None,
        });
    }
    let mut segments: Vec<PlannedSegment> = Vec::new();
    let mut entries: Vec<SegmentEntry> = Vec::new();
    let mut reconstructed_tail: Option<EventSeq> = None;

    for (filename_start, path) in declared {
        let decoded = read_segment(&path).map_err(|e| ReplayIntegrityError::CorruptSegment {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        let header = decoded.metadata;

        if header.start_seq != filename_start {
            return Err(ReplayIntegrityError::FilenameHeaderMismatch {
                path,
                filename_start: filename_start.0,
                header_start: header.start_seq.0,
            });
        }
        if header.start_seq > header.end_seq {
            return Err(ReplayIntegrityError::HeaderRangeMismatch {
                path,
                start: header.start_seq.0,
                end: header.end_seq.0,
            });
        }
        if decoded.entries.is_empty() {
            return Err(ReplayIntegrityError::EmptySegment { path });
        }

        // Seq-space semantics inside the segment.
        let mut cursor = header.start_seq;
        // The header count is in *entries*: one unit for both an
        // `ExecutionRecord` and a `Gap`. Counting only records here is what
        // produced FIND-C1.8-01 (a gap-bearing segment could not reopen).
        let mut entry_total = 0u64;
        for entry in &decoded.entries {
            entry_total += 1;
            let (entry_start, entry_end) = match entry {
                SegmentEntry::Record(r) => {
                    if r.session_id != *session_id {
                        return Err(ReplayIntegrityError::RecordSessionMismatch {
                            path: path.clone(),
                            expected: session_id.as_str().to_string(),
                            found: r.session_id.as_str().to_string(),
                        });
                    }
                    (r.seq, r.seq)
                }
                SegmentEntry::Gap(g) => (g.first_missing, g.last_missing),
            };
            if entry_start != cursor {
                return Err(ReplayIntegrityError::PayloadRangeMismatch {
                    path: path.clone(),
                    first: entry_start.0,
                    last: entry_end.0,
                    header_start: cursor.0,
                    header_end: header.end_seq.0,
                });
            }
            cursor = EventSeq::new(entry_end.0 + 1);
        }
        if cursor != EventSeq::new(header.end_seq.0 + 1) {
            return Err(ReplayIntegrityError::PayloadRangeMismatch {
                path: path.clone(),
                first: header.start_seq.0,
                last: cursor.0.saturating_sub(1),
                header_start: header.start_seq.0,
                header_end: header.end_seq.0,
            });
        }
        if entry_total != header.entry_count {
            return Err(ReplayIntegrityError::EntryCountMismatch {
                path,
                declared: header.entry_count,
                actual: entry_total,
            });
        }

        // Continuity with the previous live segment.
        if let Some(prev) = segments.last() {
            let expected = EventSeq::new(prev.end_seq.0 + 1);
            if header.start_seq < expected {
                return Err(ReplayIntegrityError::OverlappingSegments {
                    prev_end: prev.end_seq.0,
                    next_start: header.start_seq.0,
                });
            }
            if header.start_seq > expected {
                return Err(ReplayIntegrityError::MissingRange {
                    expected_from: expected.0,
                    found_from: header.start_seq.0,
                });
            }
        } else if header.start_seq != retained_from {
            // The first live segment must begin exactly where retention ends;
            // anything else is an unexplained hole at the head of the region.
            return Err(ReplayIntegrityError::MissingRange {
                expected_from: retained_from.0,
                found_from: header.start_seq.0,
            });
        }

        segments.push(PlannedSegment {
            path,
            start_seq: header.start_seq,
            end_seq: header.end_seq,
        });
        reconstructed_tail = Some(header.end_seq);
        entries.extend(decoded.entries);
    }

    Ok(ReplayPlan {
        session_id: session_id.clone(),
        retained_from,
        segments,
        entries,
        reconstructed_tail,
    })
}

/// Apply a validated plan to `backend`, in one step.
pub fn apply_replay_plan(
    plan: &ReplayPlan,
    backend: &crate::memory::InMemoryExecutionLog,
) -> Result<(), crate::error::LogError> {
    for entry in &plan.entries {
        match entry {
            SegmentEntry::Record(r) => backend.replay_record(r)?,
            SegmentEntry::Gap(g) => {
                crate::backend::ExecutionLogBackend::record_gap(
                    backend,
                    plan.session_id.clone(),
                    g.clone(),
                )?;
            }
        }
    }
    Ok(())
}

/// Validate every live segment and apply it to `backend` in ONE pass, without
/// ever holding the whole session in the plan.
///
/// This is the memory fix for `DEBT-SCALE-MEM-01`. [`build_replay_plan`] keeps
/// every decoded entry in [`ReplayPlan::entries`] so that
/// [`apply_replay_plan`] does not have to re-read the files, and because the
/// plan is retained while the backend fills up, the live set holds the session
/// twice: measured at **489 bytes per event** on this host for a 200.000-event
/// log, where one copy of the session is roughly half that.
///
/// Nothing about the *validation* changes. Every integrity decision in this
/// module is made from segment headers and the per-segment entry loop, and both
/// run here exactly as they run in the plan-building path; the only difference
/// is that each segment is handed to the backend and then dropped, so peak
/// memory is one segment plus the growing backend rather than the whole session
/// plus the whole session.
///
/// # Atomicity, deliberately unchanged
///
/// `apply_plan` in the segmented log builds a FRESH backend and swaps it in only
/// on success, so a failure cannot publish a half-reconstructed log. This
/// function applies into the backend it is given, so **the caller must still
/// apply into a fresh one and swap on success** — that is why the two lenient
/// replay paths that skipped segments were deleted, and this function does not
/// reintroduce them. `SegmentedExecutionLog::apply_plan` keeps doing the swap.
///
/// # Errors
///
/// An integrity error is returned *before* any of that segment's entries reach
/// the backend, but segments already applied stay applied. That is why the
/// caller needs the fresh-backend discipline rather than a partial rollback.
pub fn build_and_apply_replay(
    dir: &Path,
    session_id: &SessionId,
    retained_from: EventSeq,
    backend: &crate::memory::InMemoryExecutionLog,
) -> Result<Option<EventSeq>, ReplayIntegrityErrorOrLog> {
    build_and_apply_replay_listing(dir, session_id, retained_from, backend)
        .map(|segments| segments.last().map(|s| s.end_seq))
}

/// The segments the replay covered, in seq order.
///
/// Returning the list is what lets the caller keep its `flushed_segments`
/// bookkeeping without a SECOND pass over the directory — a second
/// `build_replay_plan` here would rebuild the very plan this function exists to
/// avoid, and pay the memory again at exactly the moment the fix is trying to
/// save it.
pub type AppliedSegments = Vec<PlannedSegment>;

/// Validate and apply in one pass, returning the segments applied.
///
/// The body of [`build_and_apply_replay`]; this is the form the segmented log
/// uses, because it needs the segment list as well as the tail.
pub fn build_and_apply_replay_listing(
    dir: &Path,
    session_id: &SessionId,
    retained_from: EventSeq,
    backend: &crate::memory::InMemoryExecutionLog,
) -> Result<AppliedSegments, ReplayIntegrityErrorOrLog> {
    let declared = live_segments(dir, session_id, retained_from)?;
    let mut segments: Vec<PlannedSegment> = Vec::new();

    for (filename_start, path) in declared {
        let decoded = read_segment(&path).map_err(|e| ReplayIntegrityError::CorruptSegment {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        let header = decoded.metadata;

        if header.start_seq != filename_start {
            return Err(ReplayIntegrityError::FilenameHeaderMismatch {
                path,
                filename_start: filename_start.0,
                header_start: header.start_seq.0,
            }
            .into());
        }
        if header.start_seq > header.end_seq {
            return Err(ReplayIntegrityError::HeaderRangeMismatch {
                path,
                start: header.start_seq.0,
                end: header.end_seq.0,
            }
            .into());
        }
        if decoded.entries.is_empty() {
            return Err(ReplayIntegrityError::EmptySegment { path }.into());
        }

        // Seq-space semantics inside the segment — identical checks to the
        // plan-building path, including the entry-count-in-entries rule that
        // fixed FIND-C1.8-01 (a gap-bearing segment could not reopen).
        let mut cursor = header.start_seq;
        let mut entry_total = 0u64;
        for entry in &decoded.entries {
            entry_total += 1;
            let (entry_start, entry_end) = match entry {
                SegmentEntry::Record(r) => {
                    if r.session_id != *session_id {
                        return Err(ReplayIntegrityError::RecordSessionMismatch {
                            path: path.clone(),
                            expected: session_id.as_str().to_string(),
                            found: r.session_id.as_str().to_string(),
                        }
                        .into());
                    }
                    (r.seq, r.seq)
                }
                SegmentEntry::Gap(g) => (g.first_missing, g.last_missing),
            };
            if entry_start != cursor {
                return Err(ReplayIntegrityError::PayloadRangeMismatch {
                    path: path.clone(),
                    first: entry_start.0,
                    last: entry_end.0,
                    // `cursor`, not the header's start: this is the seq the
                    // payload SHOULD have continued from, and reporting the
                    // header start instead makes the message say the segment
                    // "covers 3..=5 but its header declares 0..=9" when the
                    // real disagreement is at 10. The equivalence guard
                    // (`streaming_replay_agrees_with_plan`) caught exactly
                    // that: this arm was transcribed with the wrong field.
                    header_start: cursor.0,
                    header_end: header.end_seq.0,
                }
                .into());
            }
            cursor = EventSeq::new(entry_end.0 + 1);
        }
        if cursor != EventSeq::new(header.end_seq.0 + 1) {
            return Err(ReplayIntegrityError::PayloadRangeMismatch {
                path: path.clone(),
                first: header.start_seq.0,
                last: cursor.0.saturating_sub(1),
                header_start: header.start_seq.0,
                header_end: header.end_seq.0,
            }
            .into());
        }
        if entry_total != header.entry_count {
            return Err(ReplayIntegrityError::EntryCountMismatch {
                path: path.clone(),
                declared: header.entry_count,
                actual: entry_total,
            }
            .into());
        }

        // Continuity with the previous live segment — the same rule, decided
        // from `segments`, which is why retaining entries was never required to
        // guarantee it.
        if let Some(prev) = segments.last() {
            let expected = EventSeq::new(prev.end_seq.0 + 1);
            if header.start_seq < expected {
                return Err(ReplayIntegrityError::OverlappingSegments {
                    prev_end: prev.end_seq.0,
                    next_start: header.start_seq.0,
                }
                .into());
            }
            if header.start_seq > expected {
                return Err(ReplayIntegrityError::MissingRange {
                    expected_from: expected.0,
                    found_from: header.start_seq.0,
                }
                .into());
            }
        } else if header.start_seq != retained_from {
            return Err(ReplayIntegrityError::MissingRange {
                expected_from: retained_from.0,
                found_from: header.start_seq.0,
            }
            .into());
        }

        // The segment passed every check: hand it to the backend and let it go.
        // This is the line that removes the second copy of the session.
        for entry in &decoded.entries {
            match entry {
                SegmentEntry::Record(r) => backend
                    .replay_record(r)
                    .map_err(ReplayIntegrityErrorOrLog::from)?,
                SegmentEntry::Gap(g) => crate::backend::ExecutionLogBackend::record_gap(
                    backend,
                    session_id.clone(),
                    g.clone(),
                )
                .map_err(ReplayIntegrityErrorOrLog::from)?,
            }
        }

        segments.push(PlannedSegment {
            path,
            start_seq: header.start_seq,
            end_seq: header.end_seq,
        });
    }

    Ok(segments)
}

/// Either a rejected replay or a failure while applying an already-validated
/// segment.
///
/// Both matter and they are not the same: the first means the on-disk evidence
/// is inconsistent and nothing about this session may be published, the second
/// means the evidence was fine and the in-memory reconstruction failed.
#[derive(Debug)]
pub enum ReplayIntegrityErrorOrLog {
    Integrity(ReplayIntegrityError),
    Apply(crate::error::LogError),
}

impl From<ReplayIntegrityError> for ReplayIntegrityErrorOrLog {
    fn from(e: ReplayIntegrityError) -> Self {
        Self::Integrity(e)
    }
}

impl From<crate::error::LogError> for ReplayIntegrityErrorOrLog {
    fn from(e: crate::error::LogError) -> Self {
        Self::Apply(e)
    }
}

/// Convenience: the gaps in a plan (used by callers that need them without
/// re-reading).
pub fn plan_gaps(plan: &ReplayPlan) -> Vec<Gap> {
    plan.entries
        .iter()
        .filter_map(|e| match e {
            SegmentEntry::Gap(g) => Some(g.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod rep_tests {
    //! Segment-level REP cases that need crafted files rather than a live log.
    use super::*;
    use crate::record::{ExecutionKind, ExecutionPayload, ExecutionRecord};
    use crate::segment::{segment_path, write_segment};

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rec-c1-5-2-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn rec(session: &SessionId, seq: u64) -> ExecutionRecord {
        ExecutionRecord {
            session_id: session.clone(),
            seq: EventSeq::new(seq),
            monotonic_ns: seq,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(vec![seq as u8], "t"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        }
    }

    /// REP-5: a segment whose checksum is fine but whose seq space has a hole
    /// (record 0 then record 2, no Gap for 1) must be rejected.
    #[test]
    fn rep_5_internal_hole_in_payload_is_rejected() {
        let dir = tmpdir("rep5");
        let session = SessionId::new("rep5");
        // Claim the segment covers 0..=2 with 2 records, but omit seq 1.
        let entries = vec![
            SegmentEntry::Record(rec(&session, 0)),
            SegmentEntry::Record(rec(&session, 2)),
        ];
        write_segment(
            &dir,
            &session,
            EventSeq::ZERO,
            EventSeq::new(2),
            2,
            &entries,
        )
        .expect("write");

        let err = build_replay_plan(&dir, &session, EventSeq::ZERO)
            .expect_err("an internal hole must be rejected");
        assert!(
            matches!(err, ReplayIntegrityError::PayloadRangeMismatch { .. }),
            "expected PayloadRangeMismatch, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REP-6: the same shape WITH the gap materialised is valid.
    #[test]
    fn rep_6_materialised_gap_is_valid() {
        let dir = tmpdir("rep6");
        let session = SessionId::new("rep6");
        let entries = vec![
            SegmentEntry::Record(rec(&session, 0)),
            SegmentEntry::Gap(Gap::new(
                EventSeq::new(1),
                EventSeq::new(1),
                crate::gap::GapReason::AdapterBufferOverflow,
                "t",
            )),
            SegmentEntry::Record(rec(&session, 2)),
        ];
        write_segment(
            &dir,
            &session,
            EventSeq::ZERO,
            EventSeq::new(2),
            // Entry count, not record count: the payload is
            // [Record(0), Gap(1..=1), Record(2)] = 3 entries. (This test
            // used to pass the record-only count, mirroring the validator's
            // old unit — see FIND-C1.8-01.)
            3,
            &entries,
        )
        .expect("write");

        let plan = build_replay_plan(&dir, &session, EventSeq::ZERO).expect("valid plan");
        assert_eq!(plan.entries.len(), 3);
        assert_eq!(plan_gaps(&plan).len(), 1);
        assert_eq!(plan.reconstructed_tail, Some(EventSeq::new(2)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REP-3: two segments that overlap must be rejected, never deduplicated.
    #[test]
    fn rep_3_overlapping_segments_are_rejected() {
        let dir = tmpdir("rep3");
        let session = SessionId::new("rep3");
        write_segment(
            &dir,
            &session,
            EventSeq::ZERO,
            EventSeq::new(5),
            6,
            &(0..6)
                .map(|s| SegmentEntry::Record(rec(&session, s)))
                .collect::<Vec<_>>(),
        )
        .expect("write 0..5");
        write_segment(
            &dir,
            &session,
            EventSeq::new(3),
            EventSeq::new(4),
            2,
            &(3..5)
                .map(|s| SegmentEntry::Record(rec(&session, s)))
                .collect::<Vec<_>>(),
        )
        .expect("write 3..4");

        let err = build_replay_plan(&dir, &session, EventSeq::ZERO).expect_err("overlap");
        assert!(
            matches!(err, ReplayIntegrityError::OverlappingSegments { .. }),
            "expected OverlappingSegments, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REP-9: the filename's start_seq must match the header's.
    #[test]
    fn rep_9_filename_header_mismatch_is_rejected() {
        let dir = tmpdir("rep9");
        let session = SessionId::new("rep9");
        write_segment(
            &dir,
            &session,
            EventSeq::new(4),
            EventSeq::new(4),
            1,
            &[SegmentEntry::Record(rec(&session, 4))],
        )
        .expect("write");
        // Rename the file so the filename claims a different start.
        let real = segment_path(&dir, &session, EventSeq::new(4));
        let lie = segment_path(&dir, &session, EventSeq::ZERO);
        std::fs::rename(&real, &lie).expect("rename");

        let err = build_replay_plan(&dir, &session, EventSeq::ZERO).expect_err("mismatch");
        assert!(
            matches!(err, ReplayIntegrityError::FilenameHeaderMismatch { .. }),
            "expected FilenameHeaderMismatch, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// REP-8: a record carrying a foreign session id is rejected.
    #[test]
    fn rep_8_record_session_mismatch_is_rejected() {
        let dir = tmpdir("rep8");
        let session = SessionId::new("rep8");
        let other = SessionId::new("someone-else");
        write_segment(
            &dir,
            &session,
            EventSeq::ZERO,
            EventSeq::ZERO,
            1,
            &[SegmentEntry::Record(rec(&other, 0))],
        )
        .expect("write");

        let err = build_replay_plan(&dir, &session, EventSeq::ZERO).expect_err("session mismatch");
        assert!(
            matches!(err, ReplayIntegrityError::RecordSessionMismatch { .. }),
            "expected RecordSessionMismatch, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A valid two-segment layout is accepted and continuous, including when the
    /// boundary between them is a materialised gap.
    #[test]
    fn rep_7_gap_between_segments_is_continuous() {
        let dir = tmpdir("rep7");
        let session = SessionId::new("rep7");
        write_segment(
            &dir,
            &session,
            EventSeq::ZERO,
            EventSeq::new(1),
            2,
            &[
                SegmentEntry::Record(rec(&session, 0)),
                SegmentEntry::Record(rec(&session, 1)),
            ],
        )
        .expect("seg A");
        // Segment B starts with a gap occupying 2..=3, then a record at 4.
        write_segment(
            &dir,
            &session,
            EventSeq::new(2),
            EventSeq::new(4),
            // Entry count: [Gap(2..=3), Record(4)] = 2 entries
            // (was 1 under the old record-only unit; see FIND-C1.8-01).
            2,
            &[
                SegmentEntry::Gap(Gap::new(
                    EventSeq::new(2),
                    EventSeq::new(3),
                    crate::gap::GapReason::KernelRingOverflow,
                    "t",
                )),
                SegmentEntry::Record(rec(&session, 4)),
            ],
        )
        .expect("seg B");

        let plan = build_replay_plan(&dir, &session, EventSeq::ZERO)
            .expect("continuous layout with a materialised gap");
        assert_eq!(plan.segments.len(), 2);
        assert_eq!(plan.reconstructed_tail, Some(EventSeq::new(4)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
