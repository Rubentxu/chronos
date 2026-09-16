//! REC-C1.2 — the session OWNS its `ExecutionLog`.
//!
//! ## Why this exists
//!
//! ```text
//! AgenticSession
//!      │ owns
//!      ▼
//! ExecutionLog        <-- introduced here (C1.2)
//!      ▲ reads
//!      │
//! events_read         <-- cut over in C1.3
//! ```
//!
//! Ownership must come before the reader. If `events_read` were changed first,
//! the read path would inevitably grow another fallback/transitional branch to
//! cope with "the log might not be attached", and that branch would then have
//! to be deleted again in REC-C2.
//!
//! ## What this type is
//!
//! [`SessionExecutionLog`] is the single authoritative handle to a session's
//! durable event log. It carries the three things a reader needs and nothing
//! else:
//!
//! * the [`SessionId`] the log belongs to (so a read can be checked against the
//!   session it was asked for),
//! * the directory the segments live in (for provenance/diagnostics),
//! * the open [`SegmentedExecutionLog`] handle.
//!
//! Writers (the native probe backend) receive a clone of the same handle; they
//! do not own it. Reads go through the session, never through the backend —
//! there is deliberately no backend fallback path.
//!
//! ## Scope (C1.2)
//!
//! Ownership + single-source reads. No `events_read` wiring (C1.3), no
//! EventBus deletion (REC-C2).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chronos_log::{SegmentedConfig, SegmentedExecutionLog, SessionId};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;

/// Authoritative handle to one session's durable execution log.
#[derive(Clone)]
pub struct SessionExecutionLog {
    session_id: SessionId,
    /// Where the segments live, when this handle knows. `None` when the session
    /// adopted a log that was opened elsewhere (e.g. by the native backend from
    /// its own configuration) — provenance is optional, ownership is not.
    dir: Option<PathBuf>,
    log: Arc<SegmentedExecutionLog>,
}

impl std::fmt::Debug for SessionExecutionLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionExecutionLog")
            .field("session_id", &self.session_id)
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl SessionExecutionLog {
    /// Open (creating if needed) the log for `session_id` under `dir`.
    ///
    /// Called once, when the session is created. After this the session is the
    /// owner; everybody else borrows.
    pub fn open(dir: impl AsRef<Path>, session_id: SessionId) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| {
            ServiceError::ProbeStartFailed(format!(
                "create ExecutionLog dir {}: {e}",
                dir.display()
            ))
        })?;
        let log =
            SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(dir.clone()))
                .map_err(|e| {
                    ServiceError::ProbeStartFailed(format!(
                        "open ExecutionLog for {}: {e:?}",
                        session_id.as_str()
                    ))
                })?;
        Ok(Self {
            session_id,
            dir: Some(dir),
            log: Arc::new(log),
        })
    }

    /// Take ownership of a log that was opened elsewhere.
    ///
    /// The session becomes the single read source; the previous holder keeps a
    /// clone for writing (`Arc::ptr_eq(handle(), original)` still holds, so
    /// writes and reads see the same log).
    pub fn adopt(
        dir: Option<PathBuf>,
        session_id: SessionId,
        handle: Arc<SegmentedExecutionLog>,
    ) -> Self {
        Self {
            session_id,
            dir,
            log: handle,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The shared handle. Writers clone this; they do not take ownership.
    pub fn handle(&self) -> Arc<SegmentedExecutionLog> {
        Arc::clone(&self.log)
    }

    /// Fresh read cursor for this log's session.
    ///
    /// This is the bridge between ownership (C1.2) and the authoritative cursor
    /// (C1.1): the cursor can only be minted from a session that owns a log,
    /// so it can never point at a session with no log behind it.
    pub fn cursor_start(&self) -> EventsCursorV1 {
        EventsCursorV1::start(self.session_id.clone())
    }

    /// Compaction counters (m1-07 surface).
    pub fn compaction_metrics(&self) -> chronos_log::CompactionMetrics {
        self.log.compaction_metrics()
    }

    /// Run one compaction pass; returns the removed segment paths.
    pub fn maybe_compact(&self) -> Result<Vec<PathBuf>, ServiceError> {
        self.log
            .maybe_compact()
            .map_err(|e| ServiceError::DrainFailed(format!("{e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rec-c1-2-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    #[test]
    fn open_owns_session_identity_and_dir() {
        let dir = tmpdir("open");
        let sid = SessionId::new("sess-c12");
        let owned = SessionExecutionLog::open(&dir, sid.clone()).expect("open");
        assert_eq!(owned.session_id(), &sid);
        assert_eq!(owned.dir(), Some(dir.as_path()));
        assert!(dir.exists(), "open must materialise the directory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cursor_is_minted_from_the_owned_session() {
        let dir = tmpdir("cursor");
        let sid = SessionId::new("sess-cursor");
        let owned = SessionExecutionLog::open(&dir, sid.clone()).expect("open");
        let cursor = owned.cursor_start();
        assert_eq!(cursor.session_id(), &sid);
        assert_eq!(cursor.next_seq(), chronos_log::EventSeq::ZERO);
        // A cursor minted here can only be used against this session.
        assert!(EventsCursorV1::decode_for_session(&cursor.encode(), &sid).is_ok());
        let other = SessionId::new("someone-else");
        assert!(EventsCursorV1::decode_for_session(&cursor.encode(), &other).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closed_handle_is_shared_with_writers_not_transferred() {
        let dir = tmpdir("shared");
        let owned = SessionExecutionLog::open(&dir, SessionId::new("sess-shared")).expect("open");
        let a = owned.handle();
        let b = owned.handle();
        assert!(
            Arc::ptr_eq(&a, &b),
            "clones must share one log, not fork it"
        );
        // owned + a + b (+ the clone created by `owned.handle()` in this call).
        assert_eq!(Arc::strong_count(&owned.handle()), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compaction_metrics_start_at_zero_and_compaction_is_safe() {
        let dir = tmpdir("compact");
        let owned = SessionExecutionLog::open(&dir, SessionId::new("sess-c")).expect("open");
        let m = owned.compaction_metrics();
        assert_eq!(m.segments_removed_total, 0);
        let removed = owned.maybe_compact().expect("compact");
        assert!(removed.is_empty(), "nothing to compact in a fresh log");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_does_not_leak_the_handle() {
        let dir = tmpdir("debug");
        let owned = SessionExecutionLog::open(&dir, SessionId::new("sess-d")).expect("open");
        let s = format!("{owned:?}");
        assert!(s.contains("sess-d"));
        assert!(s.contains("SessionExecutionLog"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
