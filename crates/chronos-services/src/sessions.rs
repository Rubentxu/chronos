//! Session-lifecycle service — save, load, list, delete, and drop sessions.
//!
//! All 5 methods take a `SessionsContext<'_>` borrow struct rather than owning
//! state, because session lifecycle touches 4 separate pieces of state owned by
//! `ChronosServer` (engines, session_languages, connected_sessions, store).
//!
//! The service does NOT touch `connected_sessions` directly. The caller
//! (ChronosServer wrapper) calls `cleanup_session_memory` after the service
//! returns `Ok` for `delete_session` and `drop_session`.

use std::collections::{HashMap, HashSet};

use chronos_domain::Language;
use chronos_index::builder::IndexBuilder;
use chronos_query::QueryEngine;
use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::output::{DeleteResult, DropResult, ListResult, LoadResult, SaveResult, SessionSummary};
use chronos_store::{SessionMetadata, SessionStore};

/// Borrow struct holding all state needed by `SessionsService` methods.
///
/// All fields are references with `'a` lifetime, so the struct is `Copy`.
/// The `'static` bounds on the reference targets ensure the references
/// remain valid for the declared lifetime.
#[derive(Clone, Copy)]
pub struct SessionsContext<'a> {
    /// In-memory query engines, keyed by session_id.
    pub engines: &'a Mutex<HashMap<String, QueryEngine>>,
    /// Language tags for each session.
    pub session_languages: &'a Mutex<HashMap<String, Language>>,
    /// Sessions that are currently "connected" (active probes).
    pub connected_sessions: &'a std::sync::Mutex<HashSet<String>>,
    /// Persistent session store.
    pub store: &'a SessionStore,
}

impl std::fmt::Debug for SessionsContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionsContext").finish_non_exhaustive()
    }
}

/// A zero-sized service struct. All state is passed in via `SessionsContext`.
#[derive(Debug, Default)]
pub struct SessionsService;

impl SessionsService {
    /// Save an in-memory session to persistent storage.
    ///
    /// Reads events from the in-memory engine, computes duration from the
    /// first/last event timestamps, and delegates to `SessionStore::save_session`.
    ///
    /// # Errors
    /// - `SessionNotInMemory` if `session_id` is not in the engines map.
    /// - `EmptySession` if the engine has 0 events.
    /// - `SaveFailed` if the store write fails.
    pub async fn save_session(
        session_id: &str,
        language: Language,
        target: String,
        ctx: &SessionsContext<'_>,
    ) -> Result<SaveResult, ServiceError> {
        // Lock engines to get the engine
        let guard = ctx.engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotInMemory(session_id.to_string()))?;

        let events = engine.get_all_events();
        let event_count = events.len();

        if event_count == 0 {
            return Err(ServiceError::EmptySession(session_id.to_string()));
        }

        // Compute duration from first/last event timestamps
        let (duration_ms, created_at) =
            if let (Some(first), Some(last)) = (events.first(), events.last()) {
                let dur_ns = last.timestamp_ns.saturating_sub(first.timestamp_ns);
                (dur_ns / 1_000_000, last.timestamp_ns / 1_000_000)
            } else {
                (0, 0)
            };

        // Clone target so we can use it in both metadata and SaveResult
        let target_for_result = target.clone();

        let metadata = SessionMetadata {
            session_id: session_id.to_string(),
            created_at,
            language: language.to_string(),
            target: target.clone(),
            event_count,
            duration_ms,
        };

        // SessionStore methods are sync — drop the lock first
        drop(guard);

        let store_ref = ctx.store;
        let hashes = store_ref
            .save_session(metadata, &events)
            .map_err(|e| ServiceError::SaveFailed(e.to_string()))?;

        Ok(SaveResult {
            event_count,
            hash_count: hashes.len(),
            language: language.to_string(),
            target: target_for_result,
            duration_ms,
        })
    }

    /// Load a session from persistent storage into a new in-memory query engine.
    ///
    /// Reconstructs all 4 indices (shadow, temporal, causality, performance)
    /// from the loaded events, then inserts the engine into the engines map.
    /// Does NOT update `session_languages` or `connected_sessions`.
    ///
    /// # Errors
    /// - `LoadFailed` if the store read fails.
    pub async fn load_session(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<LoadResult, ServiceError> {
        let store_ref = ctx.store;
        let (metadata, events) = store_ref
            .load_session(session_id)
            .map_err(|e| ServiceError::LoadFailed(e.to_string()))?;

        // Build engine from loaded events with all 4 indices
        let mut builder = IndexBuilder::new();
        builder.push_all(&events);
        let indices = builder.finalize();

        let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
            .with_causality(indices.causality)
            .with_performance(indices.performance);

        // Insert into engines map
        let mut guard = ctx.engines.lock().await;
        guard.insert(session_id.to_string(), engine);

        Ok(LoadResult {
            language: metadata.language,
            target: metadata.target,
            event_count: metadata.event_count,
            duration_ms: metadata.duration_ms,
            created_at: metadata.created_at,
        })
    }

    /// List all saved sessions from persistent storage.
    ///
    /// Returns metadata summaries for all sessions (no event data).
    ///
    /// # Errors
    /// - `ListFailed` if the store read fails for a reason other than
    ///   a missing table (which returns an empty list, suitable for empty stores).
    pub async fn list_sessions(ctx: &SessionsContext<'_>) -> Result<ListResult, ServiceError> {
        let store_ref = ctx.store;
        let sessions = match store_ref.list_sessions() {
            Ok(s) => s,
            Err(e) => {
                // If the table doesn't exist yet (empty store), return empty list.
                // Any other store error is propagated.
                let err_str = e.to_string();
                if err_str.contains("does not exist") || err_str.contains("not exist") {
                    return Ok(ListResult { sessions: vec![] });
                }
                return Err(ServiceError::ListFailed(err_str));
            }
        };

        let summaries: Vec<SessionSummary> = sessions
            .into_iter()
            .map(|m| SessionSummary {
                session_id: m.session_id,
                language: m.language,
                target: m.target,
                event_count: m.event_count,
                duration_ms: m.duration_ms,
                created_at: m.created_at,
            })
            .collect();

        Ok(ListResult {
            sessions: summaries,
        })
    }

    /// Delete a session from persistent storage.
    ///
    /// The caller is responsible for calling `cleanup_session_memory` after
    /// this returns `Ok`.
    ///
    /// # Errors
    /// - `DeleteFailed` if the store delete fails.
    pub async fn delete_session(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<DeleteResult, ServiceError> {
        let store_ref = ctx.store;
        store_ref
            .delete_session(session_id)
            .map_err(|e| ServiceError::DeleteFailed(e.to_string()))?;

        Ok(DeleteResult {
            session_id: session_id.to_string(),
        })
    }

    /// Drop a session from in-memory state WITHOUT touching persistent storage.
    ///
    /// Checks if the session exists in the engines map before removing.
    /// Returns `existed: true` if the session was present, `false` if not found
    /// (idempotent — no error is raised for a missing session).
    ///
    /// The caller is responsible for calling `cleanup_session_memory` after
    /// this returns, if `existed` is true.
    pub async fn drop_session(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<DropResult, ServiceError> {
        // Check existence
        let existed = ctx.engines.lock().await.contains_key(session_id);

        Ok(DropResult {
            session_id: session_id.to_string(),
            existed,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{SourceLocation, TraceEvent};
    use std::collections::HashSet;

    /// Helper: build a minimal in-memory SessionStore.
    fn make_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    /// Helper: build a HashMap with one engine containing two trace events
    /// (different timestamps so duration > 0).
    fn make_engine_with_two_events(session_id: &str) -> (String, QueryEngine) {
        let events = vec![
            TraceEvent {
                event_id: 1,
                timestamp_ns: 1_000_000_000, // 1 second
                thread_id: 1,
                event_type: chronos_domain::EventType::FunctionEntry,
                location: SourceLocation::default(),
                data: chronos_domain::EventData::Function {
                    name: "main".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            },
            TraceEvent {
                event_id: 2,
                timestamp_ns: 2_001_000_000, // 2 seconds + 1ms
                thread_id: 1,
                event_type: chronos_domain::EventType::FunctionExit,
                location: SourceLocation::default(),
                data: chronos_domain::EventData::Function {
                    name: "main".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            },
        ];
        let engine = QueryEngine::new(events);
        (session_id.to_string(), engine)
    }

    /// Helper: build a HashMap with one engine containing two trace events.
    fn make_engines(session_id: &str) -> Mutex<HashMap<String, QueryEngine>> {
        let (id, engine) = make_engine_with_two_events(session_id);
        let mut map = HashMap::new();
        map.insert(id, engine);
        Mutex::new(map)
    }

    /// Helper: empty languages map.
    fn make_languages() -> Mutex<HashMap<String, Language>> {
        Mutex::new(HashMap::new())
    }

    /// Helper: empty connected_sessions set.
    fn make_connected() -> std::sync::Mutex<HashSet<String>> {
        std::sync::Mutex::new(HashSet::new())
    }

    /// Helper: build a SessionsContext from its parts.
    fn make_context<'a>(
        engines: &'a Mutex<HashMap<String, QueryEngine>>,
        languages: &'a Mutex<HashMap<String, Language>>,
        connected: &'a std::sync::Mutex<HashSet<String>>,
        store: &'a SessionStore,
    ) -> SessionsContext<'a> {
        SessionsContext {
            engines,
            session_languages: languages,
            connected_sessions: connected,
            store,
        }
    }

    // -------------------------------------------------------------------------
    // save_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn save_session_ok() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result = SessionsService::save_session(
            "s1",
            Language::Python,
            "/usr/bin/python3".to_string(),
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(result.event_count, 2);
        assert_eq!(result.hash_count, 2);
        assert_eq!(result.language, "python");
        assert_eq!(result.target, "/usr/bin/python3");
        assert_eq!(result.duration_ms, 1001);
    }

    // -------------------------------------------------------------------------
    // save_session — errors
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn save_session_not_in_memory() {
        let store = make_store();
        let engines = make_engines("s1"); // only s1 exists
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result =
            SessionsService::save_session("s2", Language::C, "main".to_string(), &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::SessionNotInMemory(ref s)) if s == "s2"
        ));
    }

    #[tokio::test]
    async fn save_session_empty() {
        // Empty engines map — session not in memory
        let engines = Mutex::new(HashMap::new());

        let store = make_store();
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result =
            SessionsService::save_session("s1", Language::C, "main".to_string(), &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::SessionNotInMemory(ref s)) if s == "s1"
        ));
    }

    // -------------------------------------------------------------------------
    // load_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn load_session_ok() {
        // First save a session so we have something to load
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        // Save
        SessionsService::save_session("s1", Language::Go, "./server".to_string(), &ctx)
            .await
            .unwrap();

        // Drop it from memory
        engines.lock().await.remove("s1");

        // Load it back
        let result = SessionsService::load_session("s1", &ctx).await.unwrap();

        assert_eq!(result.language, "go");
        assert_eq!(result.target, "./server");
        assert_eq!(result.event_count, 2);
        assert_eq!(result.duration_ms, 1001);
        assert_eq!(result.created_at, 2001);

        // Verify engine was inserted into memory
        assert!(engines.lock().await.contains_key("s1"));
    }

    // -------------------------------------------------------------------------
    // load_session — errors
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn load_session_not_found() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result = SessionsService::load_session("no-such-session", &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::LoadFailed(ref e)) if e.contains("not found")
        ));
    }

    // -------------------------------------------------------------------------
    // list_sessions — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn list_sessions_empty() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result = SessionsService::list_sessions(&ctx).await.unwrap();

        assert!(result.sessions.is_empty());
    }

    #[tokio::test]
    async fn list_sessions_one() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        SessionsService::save_session("s1", Language::Python, "script.py".to_string(), &ctx)
            .await
            .unwrap();

        let result = SessionsService::list_sessions(&ctx).await.unwrap();

        assert_eq!(result.sessions.len(), 1);
        let s = &result.sessions[0];
        assert_eq!(s.session_id, "s1");
        assert_eq!(s.language, "python");
        assert_eq!(s.target, "script.py");
    }

    // -------------------------------------------------------------------------
    // list_sessions — errors
    // -------------------------------------------------------------------------

    // NOTE: StoreError variants are internal. Since we control the store
    // via in_memory(), the only realistic error path is a corrupted store,
    // which is not testable without mocking. ListFailed is still present
    // in the type signature for completeness.

    // -------------------------------------------------------------------------
    // delete_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn delete_session_ok() {
        // First save a session
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        SessionsService::save_session("s1", Language::C, "main".to_string(), &ctx)
            .await
            .unwrap();

        // Delete it
        let result = SessionsService::delete_session("s1", &ctx).await.unwrap();

        assert_eq!(result.session_id, "s1");
        // NOTE: cleanup_session_memory is called by the wrapper after this returns Ok.
    }

    // -------------------------------------------------------------------------
    // delete_session — errors
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn delete_session_not_found() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result = SessionsService::delete_session("no-such", &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::DeleteFailed(ref e)) if e.contains("not found")
        ));
    }

    // -------------------------------------------------------------------------
    // drop_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn drop_session_existed() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        let result = SessionsService::drop_session("s1", &ctx).await.unwrap();

        assert_eq!(result.session_id, "s1");
        assert!(result.existed);
    }

    #[tokio::test]
    async fn drop_session_not_found_idempotent() {
        let store = make_store();
        let engines = make_engines("s1"); // only s1 in engines
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        // Dropping a non-existent session must NOT return an error (idempotent)
        let result = SessionsService::drop_session("s2", &ctx).await.unwrap();

        assert_eq!(result.session_id, "s2");
        assert!(!result.existed);
    }

    // -------------------------------------------------------------------------
    // Additional: verify save+load round-trip
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn save_load_roundtrip() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        // Save with known values
        let save_result = SessionsService::save_session(
            "s1",
            Language::Rust,
            "target/debug/app".to_string(),
            &ctx,
        )
        .await
        .unwrap();

        assert_eq!(save_result.event_count, 2);
        assert_eq!(save_result.language, "rust");

        // Remove from memory
        engines.lock().await.remove("s1");

        // Load back
        let load_result = SessionsService::load_session("s1", &ctx).await.unwrap();

        assert_eq!(load_result.language, "rust");
        assert_eq!(load_result.target, "target/debug/app");
        assert_eq!(load_result.event_count, 2);
    }

    // -------------------------------------------------------------------------
    // Additional: multiple sessions in list
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn list_sessions_multiple() {
        let store = make_store();

        // Create two engines
        let make_engine = |id: &str| -> (String, QueryEngine) {
            let events = vec![
                TraceEvent {
                    event_id: 1,
                    timestamp_ns: 1_000_000_000,
                    thread_id: 1,
                    event_type: chronos_domain::EventType::FunctionEntry,
                    location: SourceLocation::default(),
                    data: chronos_domain::EventData::Function {
                        name: id.to_string(),
                        signature: None,
                        symbol_id: None,
                        invocation_id: None,
                        parent_invocation_id: None,
                    },
                },
                TraceEvent {
                    event_id: 2,
                    timestamp_ns: 2_001_000_000,
                    thread_id: 1,
                    event_type: chronos_domain::EventType::FunctionExit,
                    location: SourceLocation::default(),
                    data: chronos_domain::EventData::Function {
                        name: id.to_string(),
                        signature: None,
                        symbol_id: None,
                        invocation_id: None,
                        parent_invocation_id: None,
                    },
                },
            ];
            let engine = QueryEngine::new(events);
            (id.to_string(), engine)
        };

        let mut map = HashMap::new();
        let (k1, e1) = make_engine("s1");
        let (k2, e2) = make_engine("s2");
        map.insert(k1, e1);
        map.insert(k2, e2);
        let engines = Mutex::new(map);

        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, &store);

        // Save both
        SessionsService::save_session("s1", Language::Python, "a.py".to_string(), &ctx)
            .await
            .unwrap();
        SessionsService::save_session("s2", Language::Go, "b.go".to_string(), &ctx)
            .await
            .unwrap();

        let result = SessionsService::list_sessions(&ctx).await.unwrap();

        assert_eq!(result.sessions.len(), 2);
    }
}
