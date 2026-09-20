//! `SessionReader` — domain-side port for read-only session/event access.
//!
//! Services that need to load a saved session (metadata + trace events)
//! depend on this trait, not on `chronos_store::SessionStore` directly.
//! In production the composition root wires a thin adapter that
//! delegates to the SQLite-backed `SessionStore::load_session`; in
//! tests, an in-memory fake serves the same trait.
//!
//! Why one port and not three (`DiffSource`, `ExplainSource`,
//! `CompareSource`): all three services have identical load semantics
//! — fetch `(SessionMetadata, Vec<TraceEvent>)` by id, surface a
//! `StoreError`-shaped failure. A single capability port is the
//! minimal contract. The write side (`save_session`, `delete_session`)
//! is the separate `LifecycleStore` port (REC-C3.5-B.4) because write
//! semantics differ per service (seal-vs-append, lifecycle invariants).
//!
//! The trait is **synchronous** by design. Async adapters are wrapped
//! via `runtime::block_on` in the composition root. See
//! `docs/specs/SESSION.md` and `REC-C3.1` for the broader context.

use std::fmt;

use crate::session::SessionMetadata;
use crate::trace::TraceEvent;

/// Errors surfaced by `SessionReader`.
///
/// Implementations may translate `StoreError`/`io::Error` into these
/// variants. Services do pattern matching on `Reader` to keep
/// `chronos_store` types out of their public API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionReaderError {
    /// No session with the given id exists.
    NotFound(String),
    /// The id failed validation (e.g. path-separator injection).
    InvalidId(String),
    /// Underlying I/O or storage failure (corruption, redb panic, etc.).
    LoadFailed(String),
}

impl fmt::Display for SessionReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "session '{}' not found", id),
            Self::InvalidId(id) => write!(f, "invalid session id: '{}'", id),
            Self::LoadFailed(msg) => write!(f, "session load failed: {}", msg),
        }
    }
}

impl std::error::Error for SessionReaderError {}

/// Port — read-only access to saved sessions.
///
/// Implementations are expected to be thread-safe. The composition root
/// picks an adapter based on configuration (production: SQLite/Redis;
/// tests: in-memory fake).
///
/// `Send + Sync` because services may be called from multiple tokio
/// tasks (the composition root shares one adapter across them).
pub trait SessionReader: Send + Sync {
    /// Load the saved session for `session_id`.
    ///
    /// Returns `(metadata, events)` on success. On failure, surfaces
    /// the canonical reader error so services can map it to their
    /// own error types without depending on `StoreError`.
    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionReaderError>;
}

// =====================================================================
// In-memory implementation (AD-5: test composition)
// =====================================================================

/// `SessionReader` backed by an in-process map of sessions.
///
/// Used by unit tests; production wires the SQLite-backed adapter.
#[derive(Default, Debug, Clone)]
pub struct InMemorySessionReader {
    sessions: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, SessionEntry>>>,
}

#[derive(Debug, Clone)]
struct SessionEntry {
    metadata: SessionMetadata,
    events: Vec<TraceEvent>,
}

impl InMemorySessionReader {
    /// New empty reader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a session.
    pub fn put(&self, metadata: SessionMetadata, events: Vec<TraceEvent>) {
        let mut guard = self
            .sessions
            .write()
            .expect("InMemorySessionReader poisoned");
        guard.insert(
            metadata.session_id.clone(),
            SessionEntry { metadata, events },
        );
    }

    /// Number of sessions currently held (test helper).
    pub fn len(&self) -> usize {
        self.sessions
            .read()
            .expect("InMemorySessionReader poisoned")
            .len()
    }

    /// `len() == 0` (clippy `len_without_is_empty`).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SessionReader for InMemorySessionReader {
    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<(SessionMetadata, Vec<TraceEvent>), SessionReaderError> {
        // Mirror the production validator: no path separators.
        if session_id.contains('/') || session_id.contains('\\') {
            return Err(SessionReaderError::InvalidId(session_id.to_string()));
        }
        let guard = self
            .sessions
            .read()
            .expect("InMemorySessionReader poisoned");
        let entry = guard
            .get(session_id)
            .ok_or_else(|| SessionReaderError::NotFound(session_id.to_string()))?;
        Ok((entry.metadata.clone(), entry.events.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{EventData, EventType, SourceLocation};

    fn sample_metadata(id: &str) -> SessionMetadata {
        SessionMetadata {
            session_id: id.to_string(),
            created_at: 0,
            language: "rust".to_string(),
            target: "noop".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        }
    }

    fn sample_events() -> Vec<TraceEvent> {
        vec![TraceEvent::new(
            1,
            100,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("noop.rs", 1, "main", 0x1000),
            EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )]
    }

    #[test]
    fn empty_reader_returns_not_found() {
        let r = InMemorySessionReader::new();
        let err = r.load_session("missing").unwrap_err();
        assert_eq!(err, SessionReaderError::NotFound("missing".into()));
    }

    #[test]
    fn invalid_id_rejected() {
        let r = InMemorySessionReader::new();
        let err = r.load_session("bad/id").unwrap_err();
        assert!(matches!(err, SessionReaderError::InvalidId(_)));
    }

    #[test]
    fn round_trip_after_put() {
        let r = InMemorySessionReader::new();
        let meta = sample_metadata("s1");
        let events = sample_events();
        r.put(meta.clone(), events.clone());
        let (loaded_meta, loaded_events) = r.load_session("s1").unwrap();
        assert_eq!(loaded_meta.session_id, meta.session_id);
        assert_eq!(loaded_events.len(), events.len());
    }

    #[test]
    fn multiple_sessions_isolated() {
        let r = InMemorySessionReader::new();
        r.put(sample_metadata("a"), sample_events());
        r.put(sample_metadata("b"), vec![]);
        assert_eq!(r.len(), 2);
        assert!(r.load_session("a").is_ok());
        assert!(r.load_session("b").is_ok());
        assert!(r.load_session("c").is_err());
    }
}
