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
    /// Reopen an EXISTING durable log (REC-C1.5.4).
    ///
    /// Never creates, never infers. Bootstrap uses this exclusively, so a
    /// directory that disappears between discovery and reopen fails instead of
    /// silently becoming "a brand-new empty log with the same SessionId".
    pub fn reopen_existing(
        dir: impl AsRef<Path>,
        session_id: SessionId,
    ) -> Result<Self, ServiceError> {
        let dir = dir.as_ref().to_path_buf();
        let log = SegmentedExecutionLog::open_existing(
            session_id.clone(),
            SegmentedConfig::with_dir(dir.clone()),
        )
        .map_err(|e| ServiceError::ProbeStartFailed(format!("reopen {}: {e}", dir.display())))?;
        Ok(Self {
            session_id,
            dir: Some(dir),
            log: Arc::new(log),
        })
    }

    /// Create (or open) the log for a NEW session under `dir`.
    ///
    /// This is the `session_start` path only. Bootstrap must use
    /// [`SessionExecutionLog::reopen_existing`].
    pub fn create(dir: impl AsRef<Path>, session_id: SessionId) -> Result<Self, ServiceError> {
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

    /// Take ownership of a log that was opened elsewhere, *validating* that the
    /// caller's `session_id` really is the identity the log carries.
    ///
    /// REC-C1.2a: the pre-C1.2a `adopt()` accepted an external `SessionId`
    /// without checking `handle.session_id()`. That allowed this to exist:
    ///
    /// ```text
    /// SessionExecutionLog { session_id = <service uuid> }
    ///         └── handle { session_id = "native-1234" }
    /// ```
    ///
    /// and `cursor_start()` would then mint a cursor for an identity the log
    /// does not contain. Duplicated identity is exactly what must not become
    /// canonical, so the mismatch is a hard error.
    pub fn try_adopt(
        dir: Option<PathBuf>,
        session_id: SessionId,
        handle: Arc<SegmentedExecutionLog>,
    ) -> Result<Self, ServiceError> {
        let log_session = handle.session_id().clone();
        if log_session != session_id {
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: session_id.as_str().to_string(),
                actual: log_session.as_str().to_string(),
            });
        }
        Ok(Self {
            session_id,
            dir,
            log: handle,
        })
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Earliest queryable seq for this log (REC-C1.5.1).
    pub fn retained_from(&self) -> chronos_log::EventSeq {
        self.log.retained_from()
    }

    /// What is known about the end of the execution (REC-C1.5.3).
    pub fn tail_state(&self) -> chronos_log::TailState {
        self.log.tail_state()
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

/// Session-scoped registry of `ExecutionLog` handles (REC-C1.3).
///
/// ## Why a registry and not a second map of finalized logs
///
/// The log used to be reachable only through `live_probes`, so `probe_stop`
/// destroyed the only route to it and a read of a stopped session could only
/// answer `SessionNotFound`. Keeping a *second* `finalized_logs` map would mean
/// two places that can hold the log and a lifecycle transition to move it,
/// including a window ("removed from live, not yet inserted into finalized")
/// where a read fails.
///
/// The registry instead lives for the whole logical life of the session:
///
/// ```text
/// session_start -> registry.register(clone of the SAME handle)
/// probe_stop    -> live_probes.remove() ; registry untouched
/// drop/delete   -> cleanup_session_memory removes the entry
/// ```
///
/// Reads never change source when the session changes state, and there is no
/// alternative-source chain to maintain.
/// What the registry knows about a session's log.
///
/// One table, not two maps: a session whose log exists but could not be
/// validated is `Unavailable(reason)`, which is a different answer from "unknown
/// session" and never a fallback to another source.
#[derive(Clone)]
pub enum ExecutionLogRegistration {
    Available(SessionExecutionLog),
    Unavailable { reason: String },
}

impl std::fmt::Debug for ExecutionLogRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available(l) => f.debug_tuple("Available").field(l).finish(),
            Self::Unavailable { reason } => f
                .debug_struct("Unavailable")
                .field("reason", reason)
                .finish(),
        }
    }
}

#[derive(Default)]
pub struct SessionExecutionLogRegistry {
    logs: std::sync::Mutex<std::collections::HashMap<String, ExecutionLogRegistration>>,
}

impl std::fmt::Debug for SessionExecutionLogRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<String> = self
            .logs
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        f.debug_struct("SessionExecutionLogRegistry")
            .field("sessions", &ids)
            .finish()
    }
}

impl SessionExecutionLogRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `log` under its own session id.
    ///
    /// The key is always `log.session_id()`, so a handle can never be indexed
    /// under an identity it does not carry. Re-registering the SAME handle is
    /// idempotent; registering a DIFFERENT handle for an existing session is an
    /// error rather than a silent replacement, because that would swap the
    /// evidence a reader is watching.
    pub fn register(&self, log: SessionExecutionLog) -> Result<(), ServiceError> {
        let key = log.session_id().as_str().to_string();
        let mut map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        if let Some(existing) = map.get(&key) {
            if let ExecutionLogRegistration::Available(existing) = existing {
                if std::sync::Arc::ptr_eq(&existing.handle(), &log.handle()) {
                    return Ok(());
                }
            }
            return Err(ServiceError::ExecutionLogIdentityMismatch {
                expected: key.clone(),
                actual: format!("a different ExecutionLog handle for {key}"),
            });
        }
        map.insert(key, ExecutionLogRegistration::Available(log));
        Ok(())
    }

    /// The session's log, or a typed error.
    ///
    /// Deliberately NOT `SessionNotFound`: a session may be perfectly known to
    /// other surfaces while having no `ExecutionLog` here (a session loaded from
    /// the store today rebuilds a `QueryEngine`, not a log). Reopening that log
    /// is REC-C1.5, and until then the honest answer is "unavailable", never a
    /// fallback to another source.
    pub fn get(&self, session_id: &str) -> Result<SessionExecutionLog, ServiceError> {
        let map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        match map.get(session_id) {
            Some(ExecutionLogRegistration::Available(log)) => Ok(log.clone()),
            Some(ExecutionLogRegistration::Unavailable { reason }) => {
                Err(ServiceError::ExecutionLogUnavailable {
                    session_id: session_id.to_string(),
                    reason: reason.clone(),
                })
            }
            None => Err(ServiceError::ExecutionLogUnavailable {
                session_id: session_id.to_string(),
                reason: "no ExecutionLog registered for this session (a session loaded \
                         from the session store has none yet; reopen belongs to REC-C1.5)"
                    .to_string(),
            }),
        }
    }

    /// Record that a durable log exists but could not be validated.
    ///
    /// Keeps ONE table: `events_read` on that session reports
    /// `ExecutionLogUnavailable(reason)` instead of pretending the session is
    /// unknown.
    pub fn register_unavailable(
        &self,
        session_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), ServiceError> {
        let mut map = self.logs.lock().map_err(|_| ServiceError::LockPoisoned)?;
        map.insert(
            session_id.to_string(),
            ExecutionLogRegistration::Unavailable {
                reason: reason.into(),
            },
        );
        Ok(())
    }

    /// Remove and return the session's log (drop/delete cleanup).
    pub fn remove(&self, session_id: &str) -> Option<ExecutionLogRegistration> {
        self.logs
            .lock()
            .ok()
            .and_then(|mut map| map.remove(session_id))
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.logs
            .lock()
            .map(|map| map.contains_key(session_id))
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.logs.lock().map(|map| map.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
        let owned = SessionExecutionLog::create(&dir, sid.clone()).expect("open");
        assert_eq!(owned.session_id(), &sid);
        assert_eq!(owned.dir(), Some(dir.as_path()));
        assert!(dir.exists(), "open must materialise the directory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cursor_is_minted_from_the_owned_session() {
        let dir = tmpdir("cursor");
        let sid = SessionId::new("sess-cursor");
        let owned = SessionExecutionLog::create(&dir, sid.clone()).expect("open");
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
    fn registry_keys_by_the_logs_own_identity_and_is_idempotent() {
        let dir = tmpdir("reg");
        let log = SessionExecutionLog::create(&dir, SessionId::new("sess-reg")).expect("log");
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log.clone()).expect("register");
        assert!(registry.contains("sess-reg"));
        // Same handle again: idempotent, not an error.
        registry
            .register(log.clone())
            .expect("idempotent re-register");
        let got = registry.get("sess-reg").expect("get");
        assert!(Arc::ptr_eq(&got.handle(), &log.handle()), "same Arc");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_refuses_to_swap_the_handle_for_an_existing_session() {
        let dir_a = tmpdir("swap-a");
        let dir_b = tmpdir("swap-b");
        let first = SessionExecutionLog::create(&dir_a, SessionId::new("dup")).expect("a");
        let second = SessionExecutionLog::create(&dir_b, SessionId::new("dup")).expect("b");
        let registry = SessionExecutionLogRegistry::new();
        registry.register(first.clone()).expect("first");
        let err = registry.register(second).unwrap_err();
        assert!(
            matches!(err, ServiceError::ExecutionLogIdentityMismatch { .. }),
            "{err:?}"
        );
        // The original is still the registered one.
        assert!(Arc::ptr_eq(
            &registry.get("dup").unwrap().handle(),
            &first.handle()
        ));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn registry_reports_unavailable_distinctly_from_unknown() {
        let registry = SessionExecutionLogRegistry::new();
        let err = registry.get("never-existed").unwrap_err();
        match err {
            ServiceError::ExecutionLogUnavailable { session_id, reason } => {
                assert_eq!(session_id, "never-existed");
                assert!(reason.contains("REC-C1.5"), "{reason}");
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn registry_remove_drops_the_entry() {
        let dir = tmpdir("rm");
        let log = SessionExecutionLog::create(&dir, SessionId::new("gone")).expect("log");
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log).expect("register");
        assert!(registry.remove("gone").is_some());
        assert!(!registry.contains("gone"));
        assert!(registry.get("gone").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn try_adopt_rejects_identity_mismatch() {
        let dir = tmpdir("mismatch");
        let real = SessionId::new("native-1234");
        let handle = Arc::new(
            SegmentedExecutionLog::open(real.clone(), SegmentedConfig::with_dir(&dir)).unwrap(),
        );
        let err =
            SessionExecutionLog::try_adopt(None, SessionId::new("service-uuid"), handle.clone())
                .unwrap_err();
        match err {
            ServiceError::ExecutionLogIdentityMismatch { expected, actual } => {
                assert_eq!(expected, "service-uuid");
                assert_eq!(actual, "native-1234");
            }
            other => panic!("expected identity mismatch, got {other:?}"),
        }
        // The matching identity is accepted, and it is the log's own.
        let ok = SessionExecutionLog::try_adopt(None, real, handle).expect("matching identity");
        assert_eq!(ok.session_id().as_str(), "native-1234");
        assert_eq!(ok.cursor_start().session_id().as_str(), "native-1234");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closed_handle_is_shared_with_writers_not_transferred() {
        let dir = tmpdir("shared");
        let owned = SessionExecutionLog::create(&dir, SessionId::new("sess-shared")).expect("open");
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
        let owned = SessionExecutionLog::create(&dir, SessionId::new("sess-c")).expect("open");
        let m = owned.compaction_metrics();
        assert_eq!(m.segments_removed_total, 0);
        let removed = owned.maybe_compact().expect("compact");
        assert!(removed.is_empty(), "nothing to compact in a fresh log");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_does_not_leak_the_handle() {
        let dir = tmpdir("debug");
        let owned = SessionExecutionLog::create(&dir, SessionId::new("sess-d")).expect("open");
        let s = format!("{owned:?}");
        assert!(s.contains("sess-d"));
        assert!(s.contains("SessionExecutionLog"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
