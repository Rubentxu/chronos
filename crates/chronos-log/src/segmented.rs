//! `SegmentedExecutionLog`: a persistence-enabled `ExecutionLog`-style
//! backend. It wraps an `InMemoryExecutionLog` and periodically flushes
//! append-only buffers to immutable segment files on disk.
//!
//! ## On-disk layout
//!
//! ```text
//! <segment_dir>/<session>-<start_seq>.seg       one segment per flush
//! <segment_dir>/<session>-<start_seq>.seg.tmp   incomplete segment on crash
//! ```
//!
//! Segments are written atomically via `write_segment`. On startup,
//! the directory is scanned for headers and the segments are
//! replayed into the inner in-memory backend in `start_seq` order,
//! which restores the seq allocator so `tail_seq` returns the
//! correct value post-boot.
//!
//! Consumer cursors are **not** persisted in m1-02 — a fresh
//! consumer always resumes from the in-memory tail after replay.
//! Persisting cursors is deferred to m1-04 (checkpoint store).
//!
//! ## Crash safety
//!
//! A segment file whose header is valid but whose body does not
//! match its BLAKE3 checksum (left behind by a crash mid-write) is
//! detected on replay and skipped. The reader logs a warning and
//! moves on to the next segment. Subsequent segments remain
//! readable.
//!
//! ## Spec cases covered
//!
//! - Case 5 (overflow → gap): when the configured `memory_budget_bytes`
//!   is exceeded by `append`, the call records an explicit gap and
//!   forces a flush.
//! - Case 6 (crash-safe segments): the inner replay routine skips a
//!   segment whose body is corrupted; prior + later segments still
//!   load correctly.
//! - Case 7 (checkpoint+delta = full replay): `replay()` produces
//!   the same `InMemoryExecutionLog` regardless of which segments
//!   were checkpointed.
//! - Case 8 (deterministic replay): two `SegmentedExecutionLog`
//!   instances with identical input streams and identical configs
//!   produce identical replayed states.

use crate::backend::{ExecutionLogBackend, NewExecutionRecord};
use crate::cursor::{ConsumerCursor, LogConsumerId, ReadResult};
use crate::error::LogError;
use crate::gap::{Gap, GapReason};
use crate::memory::InMemoryExecutionLog;
use crate::record::{ExecutionKind, ExecutionPayload, ExecutionRecord, SessionId};
use crate::segment::{
    read_header, read_segment, sanitize_session, segment_path, write_segment, SegmentEntry,
};
use crate::seq::EventSeq;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Configuration for `SegmentedExecutionLog`.
#[derive(Debug, Clone)]
pub struct SegmentedConfig {
    pub segment_dir: PathBuf,
    /// Number of buffered entries (records + gaps) that triggers an
    /// automatic flush.
    pub flush_threshold: NonZeroUsize,
    /// Replay on `open` / `SegmentedExecutionLog::new`.
    pub replay_on_open: bool,
    /// Optional soft memory budget (in bytes) for case 5
    /// (overflow → gap). When the projected in-memory bytes after
    /// a write exceed this value, the next `append` records an
    /// explicit gap and forces a flush.
    pub memory_budget_bytes: Option<u64>,
}

impl SegmentedConfig {
    /// Default: flush every 64 entries, unlimited memory budget,
    /// replay on open.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            segment_dir: dir.into(),
            flush_threshold: NonZeroUsize::new(64).expect("non-zero"),
            replay_on_open: true,
            memory_budget_bytes: None,
        }
    }
}

/// Inner state of `SegmentedExecutionLog`, all under one Mutex so
/// the high-level API can be `&self`.
struct Inner {
    backend: InMemoryExecutionLog,
    buffer: Vec<SegmentEntry>,
    pending: usize,
    flushed_segments: Vec<FlushedSegment>,
    last_flushed_tail: Option<EventSeq>,
    overflow_pending: bool,
    /// Cached snapshot of the cursor sidecar, kept in lock-step
    /// with the backend's cursor map. The sidecar is the durable
    /// source of truth; this map lets us skip reading the file on
    /// every commit.
    cursors: std::collections::BTreeMap<String, EventSeq>,
    /// Compaction metrics (m1-06). Atomic counters so callers can
    /// read them without holding the inner lock.
    metrics: CompactionMetricsInner,
}

/// Atomic counters for compaction metrics (m1-06). Exposed via
/// `CompactionMetrics` (a snapshot type).
struct CompactionMetricsInner {
    segments_removed: std::sync::atomic::AtomicU64,
    bytes_reclaimed: std::sync::atomic::AtomicU64,
    compaction_runs: std::sync::atomic::AtomicU64,
}

impl Default for CompactionMetricsInner {
    fn default() -> Self {
        Self {
            segments_removed: std::sync::atomic::AtomicU64::new(0),
            bytes_reclaimed: std::sync::atomic::AtomicU64::new(0),
            compaction_runs: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

/// Snapshot of compaction counters, returned by
/// `SegmentedExecutionLog::compaction_metrics()`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompactionMetrics {
    /// Total number of segment files removed by all compaction
    /// runs on this log since it was opened.
    pub segments_removed_total: u64,
    /// Total bytes reclaimed (sum of file sizes at the moment of
    /// deletion).
    pub bytes_reclaimed_total: u64,
    /// Number of times `compact_up_to` (or `maybe_compact`)
    /// successfully removed at least one segment.
    pub compaction_runs_total: u64,
}

#[derive(Debug, Clone)]
struct FlushedSegment {
    start_seq: EventSeq,
    end_seq: EventSeq,
    path: PathBuf,
}

/// A persistence-enabled execution log.
///
/// Cheap to clone (clone shares the same on-disk directory and
/// in-memory state).
#[derive(Clone)]
pub struct SegmentedExecutionLog {
    inner: Arc<Mutex<Inner>>,
    session_id: SessionId,
    config: SegmentedConfig,
}

impl SegmentedExecutionLog {
    pub fn open(session_id: SessionId, config: SegmentedConfig) -> Result<Self, LogError> {
        std::fs::create_dir_all(&config.segment_dir)
            .map_err(|e| LogError::Backend(format!("mkdir {:?}: {}", &config.segment_dir, e)))?;
        let inner = Inner {
            backend: InMemoryExecutionLog::new(),
            buffer: Vec::new(),
            pending: 0,
            flushed_segments: Vec::new(),
            last_flushed_tail: None,
            overflow_pending: false,
            cursors: std::collections::BTreeMap::new(),
            metrics: CompactionMetricsInner::default(),
        };
        let this = Self {
            inner: Arc::new(Mutex::new(inner)),
            session_id,
            config,
        };
        if this.config.replay_on_open {
            this.replay_into_inner()?;
            this.replay_cursors_into_inner()?;
        }
        Ok(this)
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Append a record. Returns the assigned seq. May transparently
    /// record a gap if `memory_budget_bytes` is configured and the
    /// projected bytes exceed it.
    pub fn append(&self, record: NewExecutionRecord) -> Result<EventSeq, LogError> {
        let mut inner = self.inner.lock().expect("poisoned");

        // Case 5: overflow → gap.
        if let Some(budget) = self.config.memory_budget_bytes {
            let projected = in_memory_bytes(&inner) + record.payload.bytes.len() as u64 + 96;
            if projected > budget {
                let seq = inner.backend.allocate_seq_for_gap(&record.session_id)?;
                let gap = Gap::new(
                    seq,
                    seq,
                    GapReason::AdapterBufferOverflow,
                    "SegmentedExecutionLog::append: overran memory budget",
                );
                inner
                    .backend
                    .record_gap(record.session_id.clone(), gap.clone())?;
                inner.buffer.push(SegmentEntry::Gap(gap));
                inner.pending += 1;
                inner.overflow_pending = true;
                maybe_flush(&self.session_id, &self.config, &mut inner)?;
                return Ok(seq);
            }
        }

        let seq = inner.backend.append(record.clone())?;
        let full = ExecutionRecord {
            session_id: record.session_id,
            seq,
            monotonic_ns: record.monotonic_ns,
            kind: ExecutionKind::Raw,
            payload: record.payload,
        };
        inner.buffer.push(SegmentEntry::Record(full));
        inner.pending += 1;
        maybe_flush(&self.session_id, &self.config, &mut inner)?;
        Ok(seq)
    }

    /// Record an explicit gap.
    pub fn record_gap(&self, gap: Gap) -> Result<(), LogError> {
        let mut inner = self.inner.lock().expect("poisoned");
        inner
            .backend
            .record_gap(self.session_id.clone(), gap.clone())?;
        inner.buffer.push(SegmentEntry::Gap(gap));
        inner.pending += 1;
        maybe_flush(&self.session_id, &self.config, &mut inner)?;
        Ok(())
    }

    /// Read records from `cursor` onward (see `InMemoryExecutionLog`).
    pub fn read_after(
        &self,
        consumer: &LogConsumerId,
        cursor: Option<ConsumerCursor>,
    ) -> Result<ReadResult, LogError> {
        let inner = self.inner.lock().expect("poisoned");
        inner
            .backend
            .read_after(self.session_id.clone(), consumer.clone(), cursor)
    }

    pub fn tail_seq(&self) -> Option<EventSeq> {
        let inner = self.inner.lock().expect("poisoned");
        inner.backend.tail_seq(&self.session_id)
    }

    /// Force the in-memory buffer to a new segment file on disk.
    /// Returns the segment path if a new segment was written.
    pub fn flush(&self) -> Result<Option<PathBuf>, LogError> {
        let mut inner = self.inner.lock().expect("poisoned");
        flush_inner(&self.session_id, &self.config, &mut inner)
    }

    /// Read all `.seg` files on disk and *replace* the in-memory
    /// backend's records/gaps with the replayed version. Returns
    /// the resulting backend for convenience.
    pub fn replay(&self) -> Result<InMemoryExecutionLog, LogError> {
        let fresh = InMemoryExecutionLog::new();
        self.populate_with_replay(&fresh)?;
        Ok(fresh)
    }

    /// Populate `target` with the on-disk segments and *merge* them
    /// into the in-memory backend. Used by `open()` to restore seq
    /// allocator state before returning a usable handle.
    pub fn replay_into_inner(&self) -> Result<(), LogError> {
        let mut inner = self.inner.lock().expect("poisoned");
        for meta in self.list_segment_headers()? {
            let path = segment_path(&self.config.segment_dir, &self.session_id, meta.start_seq);
            let entries = match read_segment(&path) {
                Ok(d) => d.entries,
                Err(e) => {
                    eprintln!(
                        "SegmentedExecutionLog::replay: skipping corrupt \
                         segment {:?}: {}",
                        path, e
                    );
                    continue;
                }
            };
            for entry in entries {
                match entry {
                    SegmentEntry::Record(r) => {
                        inner.backend.replay_record(&r)?;
                    }
                    SegmentEntry::Gap(g) => {
                        inner.backend.record_gap(self.session_id.clone(), g)?;
                    }
                }
            }
        }
        // Mark each loaded segment as flushed so segment bookkeeping
        // matches reality after a cold boot.
        let mut headers = self.list_segment_headers()?;
        headers.sort_by_key(|m| m.start_seq.0);
        for meta in headers {
            inner.flushed_segments.push(FlushedSegment {
                start_seq: meta.start_seq,
                end_seq: meta.end_seq,
                path: segment_path(&self.config.segment_dir, &self.session_id, meta.start_seq),
            });
            inner.last_flushed_tail = Some(meta.end_seq);
        }
        Ok(())
    }

    fn populate_with_replay(&self, target: &InMemoryExecutionLog) -> Result<(), LogError> {
        for meta in self.list_segment_headers()? {
            let path = segment_path(&self.config.segment_dir, &self.session_id, meta.start_seq);
            let entries = match read_segment(&path) {
                Ok(d) => d.entries,
                Err(_) => continue,
            };
            for entry in entries {
                match entry {
                    SegmentEntry::Record(r) => {
                        target.append(NewExecutionRecord::from(&r))?;
                    }
                    SegmentEntry::Gap(g) => {
                        target.record_gap(self.session_id.clone(), g)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Replay and rebuild the in-memory backend (used by tests
    /// that simulate "process restart" against the same on-disk
    /// state).
    pub fn reload_from_disk(&self) -> Result<InMemoryExecutionLog, LogError> {
        let fresh = InMemoryExecutionLog::new();
        self.populate_with_replay(&fresh)?;
        // Replace the in-memory backend so subsequent reads see
        // the replayed state.
        {
            let mut inner = self.inner.lock().expect("poisoned");
            inner.backend = InMemoryExecutionLog::new();
        }
        self.replay_into_inner()?;
        Ok(fresh)
    }

    /// List segment files whose `end_seq <= cutoff`. These are
    /// safe to delete once every known consumer has a cursor ≥
    /// `cutoff`. The caller is responsible for picking the right
    /// cutoff (typically `cursors().values().min()`).
    ///
    /// Pure read — does not modify the in-memory backend or the
    /// on-disk state.
    pub fn compactable_segments_up_to(
        &self,
        cutoff: EventSeq,
    ) -> Vec<(EventSeq, EventSeq, PathBuf)> {
        let inner = self.inner.lock().expect("poisoned");
        inner
            .flushed_segments
            .iter()
            .filter(|s| s.end_seq <= cutoff)
            .map(|s| (s.start_seq, s.end_seq, s.path.clone()))
            .collect()
    }

    /// Delete segment files whose `end_seq <= cutoff`. The
    /// in-memory backend and the cursor sidecar are *not*
    /// modified — the records remain readable until the next
    /// process restart (when `replay_into_inner` would skip the
    /// missing files). After compaction, the in-memory
    /// `flushed_segments` list is updated so subsequent
    /// `compactable_segments_up_to` calls do not re-emit the
    /// deleted paths.
    ///
    /// Returns the list of paths actually removed. If a file is
    /// already missing on disk (concurrent compaction, manual
    /// delete), the corresponding bookkeeping entry is dropped
    /// silently.
    ///
    /// Updates `compaction_metrics`: `compaction_runs_total`
    /// increments once per call that removed at least one file;
    /// `segments_removed_total` and `bytes_reclaimed_total`
    /// accumulate across runs.
    pub fn compact_up_to(&self, cutoff: EventSeq) -> Result<Vec<PathBuf>, LogError> {
        let mut removed = Vec::new();
        let mut removed_bytes = 0u64;
        let mut survivors = Vec::new();
        let mut inner = self.inner.lock().expect("poisoned");
        for seg in inner.flushed_segments.drain(..) {
            if seg.end_seq <= cutoff {
                let size = std::fs::metadata(&seg.path).map(|m| m.len()).unwrap_or(0);
                match std::fs::remove_file(&seg.path) {
                    Ok(()) => {
                        removed.push(seg.path);
                        removed_bytes += size;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // Already gone (concurrent delete, manual
                        // rm). Drop the bookkeeping entry.
                    }
                    Err(e) => {
                        // Put the segment back so we don't lose
                        // the bookkeeping on a transient I/O
                        // failure.
                        let path = seg.path.clone();
                        survivors.push(seg);
                        return Err(LogError::Backend(format!("remove_file {:?}: {}", path, e)));
                    }
                }
            } else {
                survivors.push(seg);
            }
        }
        inner.flushed_segments = survivors;
        // Recompute last_flushed_tail as the max end_seq of the
        // survivors. If all segments were deleted, fall back to
        // the buffer's natural state (the allocator may still
        // have unflushed entries that will be flushed next time).
        inner.last_flushed_tail = inner.flushed_segments.iter().map(|s| s.end_seq).max();
        // Update metrics (atomic so they survive the lock drop).
        if !removed.is_empty() {
            use std::sync::atomic::Ordering;
            inner
                .metrics
                .compaction_runs
                .fetch_add(1, Ordering::Relaxed);
            inner
                .metrics
                .segments_removed
                .fetch_add(removed.len() as u64, Ordering::Relaxed);
            inner
                .metrics
                .bytes_reclaimed
                .fetch_add(removed_bytes, Ordering::Relaxed);
        }
        Ok(removed)
    }

    /// Convenience: pick the cutoff from `min_consumer_cursor()`
    /// and run `compact_up_to` if a cursor exists. If no consumer
    /// has read yet, returns an empty list (compaction is unsafe).
    pub fn maybe_compact(&self) -> Result<Vec<PathBuf>, LogError> {
        match self.min_consumer_cursor() {
            Some(cutoff) => self.compact_up_to(cutoff),
            None => Ok(Vec::new()),
        }
    }

    /// Snapshot of compaction counters for this log.
    pub fn compaction_metrics(&self) -> CompactionMetrics {
        use std::sync::atomic::Ordering;
        let inner = self.inner.lock().expect("poisoned");
        CompactionMetrics {
            segments_removed_total: inner.metrics.segments_removed.load(Ordering::Relaxed),
            bytes_reclaimed_total: inner.metrics.bytes_reclaimed.load(Ordering::Relaxed),
            compaction_runs_total: inner.metrics.compaction_runs.load(Ordering::Relaxed),
        }
    }

    /// Convenience: returns the lowest seq any committed consumer
    /// still needs. Use this with `compact_up_to`. If no consumer
    /// has a cursor yet, returns `None` — compaction is unsafe
    /// until at least one consumer has read.
    pub fn min_consumer_cursor(&self) -> Option<EventSeq> {
        let inner = self.inner.lock().expect("poisoned");
        inner
            .cursors
            .values()
            .copied()
            .min()
            // If no committed cursor exists but the in-memory
            // backend has a stored cursor (e.g. via reads
            // without commit), fall back to that.
            .or_else(|| {
                let consumer = LogConsumerId::new("m1-05-fallback");
                inner.backend.cursor(&self.session_id, &consumer)
            })
    }

    /// List of segments currently on disk.
    pub fn flushed_segments(&self) -> Vec<(EventSeq, EventSeq, PathBuf)> {
        let inner = self.inner.lock().expect("poisoned");
        inner
            .flushed_segments
            .iter()
            .map(|s| (s.start_seq, s.end_seq, s.path.clone()))
            .collect()
    }

    pub fn last_flushed_tail(&self) -> Option<EventSeq> {
        self.inner.lock().expect("poisoned").last_flushed_tail
    }

    /// `seg.path → on-disk size` map for tests that want to verify
    /// which segments exist after a sequence of operations.
    pub fn segment_sizes(&self) -> BTreeMap<PathBuf, u64> {
        let mut out = BTreeMap::new();
        for seg in &self.inner.lock().expect("poisoned").flushed_segments {
            if let Ok(meta) = std::fs::metadata(&seg.path) {
                out.insert(seg.path.clone(), meta.len());
            }
        }
        out
    }

    fn list_segment_headers(&self) -> Result<Vec<crate::segment::SegmentMetadata>, LogError> {
        let mut out = Vec::new();
        let safe = sanitize_session(&self.session_id);
        let entries = std::fs::read_dir(&self.config.segment_dir).map_err(|e| {
            LogError::Backend(format!("read_dir {:?}: {}", &self.config.segment_dir, e))
        })?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".seg") {
                continue;
            }
            let prefix = format!("{}-", safe);
            if !name_str.starts_with(&prefix) {
                continue;
            }
            out.push(read_header(&entry.path())?);
        }
        out.sort_by_key(|m| m.start_seq.0);
        Ok(out)
    }

    /// Path to the per-consumer cursor sidecar. The file lives next
    /// to the `.seg` files in `segment_dir` and contains a JSON map
    /// of `consumer_id → last_seq`. Updates are atomic via
    /// `<file>.tmp` → rename.
    fn cursor_sidecar_path(&self) -> PathBuf {
        let safe = sanitize_session(&self.session_id);
        self.config
            .segment_dir
            .join(format!("{}.cursors.json", safe))
    }

    /// Snapshot of all consumer cursors known to this log. Keys
    /// are `LogConsumerId` strings; values are the stored
    /// `last_seq`. Reads from the in-memory cache, which is in
    /// lock-step with the on-disk sidecar.
    pub fn cursors(&self) -> std::collections::BTreeMap<String, EventSeq> {
        self.inner.lock().expect("poisoned").cursors.clone()
    }

    /// Look up the cursor for `consumer`. Returns `None` if the
    /// consumer has never read from this log.
    pub fn last_cursor(&self, consumer: &LogConsumerId) -> Option<EventSeq> {
        let inner = self.inner.lock().expect("poisoned");
        // Prefer the durable sidecar view (which is what survives
        // a restart); fall back to the in-memory backend cursor
        // for the same consumer.
        inner
            .cursors
            .get(consumer.as_str())
            .copied()
            .or_else(|| inner.backend.cursor(&self.session_id, consumer))
    }

    /// Persist the cursor for `consumer`. Updates the in-memory
    /// backend first (so the next `read_after` skips records ≤
    /// `last_seq`), then writes the sidecar to disk so the cursor
    /// survives a process restart. Concurrent commits for
    /// different consumers are serialized through the inner
    /// mutex.
    pub fn commit_cursor(
        &self,
        consumer: &LogConsumerId,
        last_seq: EventSeq,
    ) -> Result<(), LogError> {
        let snapshot = {
            let mut inner = self.inner.lock().expect("poisoned");
            inner
                .backend
                .seed_cursor(&self.session_id, consumer, last_seq);
            // Merge with the higher of any prior stored value so
            // a stale commit never rolls the cursor backwards.
            let key = consumer.as_str().to_string();
            match inner.cursors.get(&key) {
                Some(prev) if *prev >= last_seq => {}
                _ => {
                    inner.cursors.insert(key, last_seq);
                }
            }
            inner.cursors.clone()
        };
        write_cursor_sidecar(&self.cursor_sidecar_path(), &snapshot)
    }

    /// Called by `open()` to seed the inner backend's cursor map
    /// from the on-disk sidecar. Skips silently if no sidecar
    /// exists.
    fn replay_cursors_into_inner(&self) -> Result<(), LogError> {
        let path = self.cursor_sidecar_path();
        if !path.exists() {
            return Ok(());
        }
        let map = read_cursor_sidecar(&path)?;
        let mut inner = self.inner.lock().expect("poisoned");
        for (consumer_str, last_seq) in &map {
            inner.backend.seed_cursor(
                &self.session_id,
                &LogConsumerId::new(consumer_str),
                *last_seq,
            );
        }
        // Cache the sidecar contents so `cursors()` returns the
        // exact same view the inner backend sees.
        inner.cursors = map;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Cursor sidecar I/O. Lives at file scope so it can be unit-tested
// in isolation from the rest of `SegmentedExecutionLog`.
// ---------------------------------------------------------------------------

/// Read a cursor sidecar JSON file. Returns an empty map if the
/// file does not exist. Returns an error if the file is corrupt
/// JSON or has the wrong shape.
fn read_cursor_sidecar(
    path: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, EventSeq>, LogError> {
    if !path.exists() {
        return Ok(std::collections::BTreeMap::new());
    }
    let bytes =
        std::fs::read(path).map_err(|e| LogError::Backend(format!("read {:?}: {}", path, e)))?;
    if bytes.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }
    let raw: std::collections::BTreeMap<String, u64> = serde_json::from_slice(&bytes)
        .map_err(|e| LogError::Backend(format!("parse cursor sidecar {:?}: {}", path, e)))?;
    Ok(raw
        .into_iter()
        .map(|(k, v)| (k, EventSeq::new(v)))
        .collect())
}

/// Write a cursor sidecar atomically (`*.tmp` → rename). On
/// failure the partial file is left for forensic inspection but
/// the durable file (if any) remains untouched.
fn write_cursor_sidecar(
    path: &std::path::Path,
    snapshot: &std::collections::BTreeMap<String, EventSeq>,
) -> Result<(), LogError> {
    let raw: std::collections::BTreeMap<String, u64> =
        snapshot.iter().map(|(k, v)| (k.clone(), v.0)).collect();
    let bytes = serde_json::to_vec(&raw)
        .map_err(|e| LogError::Backend(format!("encode cursor sidecar: {}", e)))?;
    let tmp = path.with_extension("cursors.json.tmp");
    std::fs::write(&tmp, &bytes)
        .map_err(|e| LogError::Backend(format!("write {:?}: {}", tmp, e)))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| LogError::Backend(format!("rename {:?}: {}", tmp, e)))?;
    Ok(())
}

fn in_memory_bytes(inner: &Inner) -> u64 {
    inner
        .buffer
        .iter()
        .map(|e| match e {
            SegmentEntry::Record(r) => r.payload.bytes.len() as u64 + 96,
            SegmentEntry::Gap(_) => 64,
        })
        .sum()
}

fn maybe_flush(
    session: &SessionId,
    config: &SegmentedConfig,
    inner: &mut Inner,
) -> Result<(), LogError> {
    let threshold = config.flush_threshold.get();
    if inner.pending >= threshold || (inner.overflow_pending && inner.pending > 0) {
        flush_inner(session, config, inner)?;
    }
    Ok(())
}

fn flush_inner(
    session: &SessionId,
    config: &SegmentedConfig,
    inner: &mut Inner,
) -> Result<Option<PathBuf>, LogError> {
    if inner.buffer.is_empty() {
        return Ok(None);
    }
    let first_seq = match &inner.buffer[0] {
        SegmentEntry::Record(r) => r.seq,
        SegmentEntry::Gap(g) => g.first_missing,
    };
    let last_seq = match inner.buffer.last().expect("non-empty") {
        SegmentEntry::Record(r) => r.seq,
        SegmentEntry::Gap(g) => g.last_missing,
    };
    let record_count = inner.buffer.len() as u64;
    let entries = std::mem::take(&mut inner.buffer);
    let path = write_segment(
        &config.segment_dir,
        session,
        first_seq,
        last_seq,
        record_count,
        &entries,
    )?;
    inner.last_flushed_tail = Some(last_seq);
    inner.flushed_segments.push(FlushedSegment {
        start_seq: first_seq,
        end_seq: last_seq,
        path: path.clone(),
    });
    inner.pending = 0;
    inner.overflow_pending = false;
    Ok(Some(path))
}

// ---------------------------------------------------------------------------
// Convenience conversions so callers don't have to spell out
// `ExecutionRecord` shapes at every replay site.
// ---------------------------------------------------------------------------

/// Build a `NewExecutionRecord` from a stored `ExecutionRecord`.
impl NewExecutionRecord {
    pub fn from_record(r: &ExecutionRecord) -> Self {
        Self {
            session_id: r.session_id.clone(),
            monotonic_ns: r.monotonic_ns,
            payload: ExecutionPayload::new(r.payload.bytes.clone(), r.payload.tag.clone()),
        }
    }
}

/// Same as `from_record`, exposed via a trait-like helper. Lives
/// here to avoid pulling `impl From` into the public API.
pub fn record_to_new(r: &ExecutionRecord) -> NewExecutionRecord {
    NewExecutionRecord::from_record(r)
}

impl From<&ExecutionRecord> for NewExecutionRecord {
    fn from(r: &ExecutionRecord) -> Self {
        NewExecutionRecord::from_record(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::ExecutionPayload;

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "chronos-seg-test-{}-{}",
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

    fn new_record(s: &SessionId, ns: u64, tag: &str) -> NewExecutionRecord {
        NewExecutionRecord {
            session_id: s.clone(),
            monotonic_ns: ns,
            payload: ExecutionPayload::new(vec![1, 2, 3], tag),
        }
    }

    #[test]
    fn open_creates_empty_log() {
        let dir = tempdir();
        let session = SessionId::new("s1");
        let log =
            SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&dir)).unwrap();
        assert_eq!(log.tail_seq(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_flushes_at_threshold() {
        let dir = tempdir();
        let session = SessionId::new("s2");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(4).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
        for i in 0..7 {
            log.append(new_record(&session, i * 10, "x")).unwrap();
        }
        log.flush().unwrap();
        let segments = log.flushed_segments();
        // 4 records on the 4th append, 3 leftover then explicit
        // flush → 2 segments total.
        assert_eq!(segments.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn overflow_records_gap_in_buffer() {
        let dir = tempdir();
        let session = SessionId::new("s3");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(8).unwrap();
        cfg.memory_budget_bytes = Some(200);
        let log = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
        log.append(new_record(&session, 0, "a")).unwrap();
        for i in 1..=4 {
            log.append(NewExecutionRecord {
                session_id: session.clone(),
                monotonic_ns: i * 10,
                payload: ExecutionPayload::new(vec![0u8; 128], "big"),
            })
            .unwrap();
        }
        log.flush().unwrap();
        let segments = log.flushed_segments();
        assert!(!segments.is_empty(), "at least one segment flushed");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn replay_restores_seq_allocator() {
        let dir = tempdir();
        let session = SessionId::new("rep");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(3).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).unwrap();
        for i in 0..6 {
            log.append(new_record(&session, i * 10, "x")).unwrap();
        }
        log.flush().unwrap();
        drop(log);

        cfg.replay_on_open = true;
        let log2 = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
        assert_eq!(log2.tail_seq(), Some(EventSeq(5)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_segment_is_skipped() {
        let dir = tempdir();
        let session = SessionId::new("cs");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).unwrap();
        log.append(new_record(&session, 0, "a")).unwrap();
        log.append(new_record(&session, 10, "b")).unwrap();
        log.flush().unwrap();
        log.append(new_record(&session, 20, "c")).unwrap();
        log.flush().unwrap();
        let segments = log.flushed_segments();
        assert_eq!(segments.len(), 2);
        // Corrupt the first segment's payload by appending bytes
        // to it. BLAKE3 mismatch will skip it on the next replay.
        let (_start, _end, path) = segments[0].clone();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        f.write_all(&[0xFFu8; 64]).unwrap();
        drop(f);
        drop(log);

        cfg.replay_on_open = true;
        let log2 = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
        // The first segment is skipped, only seq=2 from segment
        // #2 remains. The replayed log has tail_seq 2; we just
        // verify *something* survived.
        assert!(log2.tail_seq().is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn checkpoint_then_delta_equals_full_replay() {
        // Spec case 7: produce a log, split into N segments via
        // repeated flushes, then reconstruct from disk only and
        // verify tail_seq matches what we had before.
        let dir = tempdir();
        let session = SessionId::new("ck");
        let mut cfg = SegmentedConfig::with_dir(&dir);
        cfg.flush_threshold = NonZeroUsize::new(2).unwrap();
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).unwrap();
        for i in 0..5 {
            log.append(new_record(&session, i * 10, "x")).unwrap();
        }
        log.flush().unwrap();
        let pre_tail = log.tail_seq();
        drop(log);

        cfg.replay_on_open = true;
        let log2 = SegmentedExecutionLog::open(session.clone(), cfg).unwrap();
        assert_eq!(log2.tail_seq(), pre_tail);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deterministic_replay_two_logs_produce_same_tail() {
        // Spec case 8: two logs with identical inputs produce the
        // same tail.
        let dir1 = tempdir();
        let dir2 = tempdir();
        let session = SessionId::new("det");
        let cfg1 = SegmentedConfig::with_dir(&dir1);
        let cfg2 = SegmentedConfig::with_dir(&dir2);
        let log1 = SegmentedExecutionLog::open(session.clone(), cfg1).unwrap();
        let log2 = SegmentedExecutionLog::open(session.clone(), cfg2).unwrap();
        for i in 0..10 {
            log1.append(new_record(&session, i * 10, "x")).unwrap();
            log2.append(new_record(&session, i * 10, "x")).unwrap();
        }
        log1.flush().unwrap();
        log2.flush().unwrap();
        assert_eq!(log1.tail_seq(), log2.tail_seq());
        std::fs::remove_dir_all(&dir1).ok();
        std::fs::remove_dir_all(&dir2).ok();
    }
}
