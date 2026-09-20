//! `SessionRepository` — domain-side port for session metadata storage.
//!
//! Sessions in Chronos are long-lived: capture creates them, probes may
//! stop and restart, queries stream over them, and lifecycle ops open
//! and close them. Services today keep them inline in `ServiceContext`.
//! The port here lets services talk to "any" session store — in tests
//! we hand an `InMemorySessionRepository`, in production the
//! composition root wires a SQLite-backed adapter (REC-C3.3 territory).
//!
//! The port is **synchronous** by design; repository implementations
//! may use async internally. The composition root bridges via
//! `runtime::block_on` or `spawn_blocking` where blocking I/O is
//! unavoidable.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::TraceError;
use crate::session::SessionMetadata;
use crate::session_id::SessionId;
use crate::TraceEvent;

/// Session record as seen by the application layer.
///
/// This is intentionally minimal: just the ids and state. Backends
/// (REC-C3.3 SQLite) attach additional columns such as timestamps and
/// process ids, but the application contract here is only what the
/// use-cases (`AttachSession`, `DetachSession`, `ListActiveSessions`)
/// actually need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    pub session_id: SessionId,
    pub state: SessionState,
}

/// Lifecycle states for a session. Mirrors the
/// `SessionLifecycleState` documented in `docs/specs/SESSION.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    Active,
    Paused,
    Closed,
}

/// Port — owning a map of session ids to handles.
///
/// Implementations are expected to be thread-safe and process-safe.
/// The composition root picks an adapter based on configuration.
pub trait SessionRepository: Send + Sync {
    /// Fetch the handle for `session_id`. Returns
    /// `TraceError::SessionNotFound` if no record exists.
    fn get(&self, session_id: &SessionId) -> Result<SessionHandle, TraceError>;

    /// Insert or replace the handle for `session_id`. Returns the
    /// prior handle if one existed (useful for lifecycle-aware code
    /// that needs to compare).
    fn upsert(&self, handle: SessionHandle) -> Result<Option<SessionHandle>, TraceError>;

    /// Remove and return the handle for `session_id`. Returns
    /// `TraceError::SessionNotFound` if the session is unknown.
    fn remove(&self, session_id: &SessionId) -> Result<SessionHandle, TraceError>;

    /// Snapshot list of session ids. Cheap on the in-memory impl;
    /// adapters may need to be careful with this call (full scan).
    fn list(&self) -> Vec<SessionId>;

    /// Number of currently registered sessions.
    fn len(&self) -> usize;

    /// `len() == 0` — provided so the trait conforms to clippy's
    /// `len_without_is_empty` convention.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// =====================================================================
// In-memory implementation (AD-5: test composition)
// =====================================================================

/// `SessionRepository` backed by a `RwLock<HashMap>`. Used by unit
/// tests and by the composition root as the fallback when no adapter
/// is configured.
#[derive(Debug, Default)]
pub struct InMemorySessionRepository {
    inner: RwLock<HashMap<SessionId, SessionHandle>>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionRepository for InMemorySessionRepository {
    fn get(&self, session_id: &SessionId) -> Result<SessionHandle, TraceError> {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionRepository read-lock poisoned");
        guard
            .get(session_id)
            .cloned()
            .ok_or_else(|| TraceError::session_not_found(session_id.as_str()))
    }

    fn upsert(&self, handle: SessionHandle) -> Result<Option<SessionHandle>, TraceError> {
        let mut guard = self
            .inner
            .write()
            .expect("InMemorySessionRepository write-lock poisoned");
        Ok(guard.insert(handle.session_id.clone(), handle))
    }

    fn remove(&self, session_id: &SessionId) -> Result<SessionHandle, TraceError> {
        let mut guard = self
            .inner
            .write()
            .expect("InMemorySessionRepository write-lock poisoned");
        guard
            .remove(session_id)
            .ok_or_else(|| TraceError::session_not_found(session_id.as_str()))
    }

    fn list(&self) -> Vec<SessionId> {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionRepository read-lock poisoned");
        guard.keys().cloned().collect()
    }

    fn len(&self) -> usize {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionRepository read-lock poisoned");
        guard.len()
    }
}

impl Clone for InMemorySessionRepository {
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(
                self.inner
                    .read()
                    .expect("InMemorySessionRepository read-lock poisoned")
                    .clone(),
            ),
        }
    }
}

// Make it dyn-friendly by exposing an `Arc` helper.
impl InMemorySessionRepository {
    /// Wrap in `Arc<dyn SessionRepository>` for composition-root use.
    pub fn into_arc(self) -> Arc<dyn SessionRepository> {
        Arc::new(self)
    }
}

// =====================================================================
// SessionArchive port (REC-C3.3.3 / Tren B slice C)
// =====================================================================

/// Errors produced by `SessionArchive` operations.
///
/// `Store` carries the underlying infrastructure error from the
/// driven adapter (e.g. `chronos_store::StoreError`); `SessionNotFound`
/// is a typed variant so services can pattern-match without parsing
/// strings.
#[derive(Debug, thiserror::Error)]
pub enum SessionArchiveError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("session archive store error: {0}")]
    Store(String),
}

impl SessionArchiveError {
    /// Convert any error into the `Store` variant, preserving its `Display`.
    pub fn from_store<E: std::fmt::Display>(e: E) -> Self {
        Self::Store(e.to_string())
    }
}

/// `SessionArchive` — domain-side port for persisting and reloading session
/// snapshots (metadata + raw events).
///
/// This is the second port alongside `SessionRepository` (which stays
/// lifecycle/registry-only per REC-C3.3.3 B2 constraint). The two are
/// deliberately separated so the persistence concern does not leak back
/// into the registry, and so `delete` semantics can live at the service
/// layer (the port carries no `Force` flag — see Q2 decision).
///
/// The port is **synchronous** by design; the composition root bridges
/// to async runtimes where needed (see `composition.rs`).
pub trait SessionArchive: Send + Sync {
    /// Persist `metadata` and `events` for a session, returning the
    /// CAS hashes of the stored events (mirrors
    /// `SessionStore::save_session`'s return shape so callers can
    /// audit what was written).
    fn save(
        &self,
        metadata: SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<Vec<String>, SessionArchiveError>;

    /// Load a previously-saved session (metadata + events).
    fn load(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionArchiveError>;

    /// List metadata for every saved session.
    fn list(&self) -> Result<Vec<SessionMetadata>, SessionArchiveError>;

    /// Delete a session. The refusal of "live session delete" lives
    /// at the service layer (per Q2 decision); the port carries no
    /// Force flag.
    fn delete(&self, session_id: &str) -> Result<(), SessionArchiveError>;

    /// Number of events stored for a session.
    fn count_events(&self, session_id: &str) -> Result<usize, SessionArchiveError>;

    /// Whether the underlying backing store survives process restart.
    /// Mirrors `SessionStore::is_persistent` (per Q1 decision — the
    /// single bool query lives on this port rather than a separate
    /// capability port).
    fn is_persistent(&self) -> bool;
}

/// `SessionArchive` backed by an in-memory `RwLock<HashMap>`. Used by
/// unit tests and as the composition-root fallback when no persistent
/// store is configured.
#[derive(Debug, Default)]
pub struct InMemorySessionArchive {
    inner: RwLock<HashMap<String, (SessionMetadata, Vec<TraceEvent>)>>,
}

impl InMemorySessionArchive {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wrap in `Arc<dyn SessionArchive>` for composition-root use.
    pub fn into_arc(self) -> Arc<dyn SessionArchive> {
        Arc::new(self)
    }
}

impl SessionArchive for InMemorySessionArchive {
    fn save(
        &self,
        metadata: SessionMetadata,
        events: &[TraceEvent],
    ) -> Result<Vec<String>, SessionArchiveError> {
        let mut guard = self
            .inner
            .write()
            .expect("InMemorySessionArchive write-lock poisoned");
        let hashes: Vec<String> = events
            .iter()
            .enumerate()
            .map(|(idx, _)| format!("mem://{}#{}", metadata.session_id, idx))
            .collect();
        guard.insert(metadata.session_id.clone(), (metadata, events.to_vec()));
        Ok(hashes)
    }

    fn load(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionArchiveError> {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionArchive read-lock poisoned");
        guard
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionArchiveError::SessionNotFound(session_id.to_string()))
    }

    fn list(&self) -> Result<Vec<SessionMetadata>, SessionArchiveError> {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionArchive read-lock poisoned");
        Ok(guard.values().map(|(meta, _)| meta.clone()).collect())
    }

    fn delete(&self, session_id: &str) -> Result<(), SessionArchiveError> {
        let mut guard = self
            .inner
            .write()
            .expect("InMemorySessionArchive write-lock poisoned");
        guard
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| SessionArchiveError::SessionNotFound(session_id.to_string()))
    }

    fn count_events(&self, session_id: &str) -> Result<usize, SessionArchiveError> {
        let guard = self
            .inner
            .read()
            .expect("InMemorySessionArchive read-lock poisoned");
        guard
            .get(session_id)
            .map(|(_, events)| events.len())
            .ok_or_else(|| SessionArchiveError::SessionNotFound(session_id.to_string()))
    }

    fn is_persistent(&self) -> bool {
        false
    }
}

impl Clone for InMemorySessionArchive {
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(
                self.inner
                    .read()
                    .expect("InMemorySessionArchive read-lock poisoned")
                    .clone(),
            ),
        }
    }
}

// =====================================================================
// Tests live under `crates/chronos-domain/tests/ports/session.rs`.
// =====================================================================
