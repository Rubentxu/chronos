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
use crate::session_id::SessionId;

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
// Tests live under `crates/chronos-domain/tests/ports/session.rs`.
// =====================================================================
