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
use std::path::Path;

use chronos_domain::Language;
use chronos_index::builder::IndexBuilder;
use chronos_log::SessionId;
use chronos_query::QueryEngine;
use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::execution_log_bootstrap::delete_durable_execution_log;
use crate::output::{DeleteResult, DropResult, ListResult, LoadResult, SaveResult, SessionSummary};
use crate::session_log::SessionExecutionLogRegistry;
use chronos_domain::ports::session::SessionArchive;
use chronos_domain::SessionMetadata;

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
    /// Persistent session archive (REC-C3.3.3 Tren B — `SessionArchive` port).
    /// Replaces the previous `&SessionStore` field; the concrete `Arc<SessionStore>`
    /// still exists on `ChronosServer` and is wrapped via
    /// `SessionStoreBackedSessionArchive` at the composition root.
    pub archive: &'a dyn SessionArchive,
    /// Canonical durable ExecutionLog registry and root.
    pub execution_log_registry: &'a SessionExecutionLogRegistry,
    pub execution_log_root: &'a Path,
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
            tail_sealed: false,
            sealed_at: None,
        };

        // SessionStore methods are sync — drop the lock first
        drop(guard);

        let archive = ctx.archive;
        let hashes = archive
            .save(metadata, &events)
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
    /// CIH-F: the durable `ExecutionLog` is reopened and registered before the
    /// engine is published, so a subsequent canonical reader (e.g.
    /// `query_events`) finds the session in the registry. The registry's own
    /// rules decide what happens for each prior state:
    ///
    /// - `Available` — the existing handle is reused; no overwrite, so two
    ///   loads of the same session cannot produce an `ExecutionLogIdentityMismatch`.
    /// - `Unavailable { reason }` — the previously-recorded reason is preserved
    ///   verbatim. There is no implicit refresh; corruption or absence stays
    ///   explicit so `query_events` keeps reporting the typed unavailability.
    /// - `Absent` — the durable log under `execution_log_root.join(session_id)`
    ///   is validated via the injected factory (REC-C1.5.2 strict replay) and
    ///   registered. If the factory refuses (corruption, missing manifest,
    ///   unreadable), the reason is recorded as `Unavailable` instead of a
    ///   panic; no empty log is fabricated and no `Gap` is invented.
    ///
    /// The engine is published AFTER the registry decision, so the observable
    /// state is consistent: a session is either fully queryable (engine +
    /// ExecutionLog), or its engine is published with the registry explicitly
    /// carrying the unavailability reason.
    ///
    /// # Errors
    /// - `LoadFailed` if the store read fails.
    pub async fn load_session(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<LoadResult, ServiceError> {
        let archive = ctx.archive;
        let (metadata, events) = archive
            .load(session_id)
            .map_err(|e| ServiceError::LoadFailed(e.to_string()))?;

        // CIH-F: resolve the ExecutionLog entry FIRST, before publishing the
        // engine, so the registry state is consistent with what `query_events`
        // will see on its next read.
        Self::rehydrate_execution_log(session_id, ctx)?;

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

    /// CIH-F helper — bring the registry for `session_id` to a consistent
    /// state relative to the durable evidence on disk, without mutating any
    /// other session.
    ///
    /// The four registry states and their treatment:
    ///
    /// 1. `Available`  -> do nothing. The handle is the canonical in-memory
    ///    view; overwriting it with a freshly-reopened one would either be a
    ///    no-op (same Arc, ptr_eq) or an `ExecutionLogIdentityMismatch` (new
    ///    Arc from the factory), neither of which is what `load_session` wants.
    ///
    /// 2. `Unavailable` -> do nothing. The previously-recorded reason stays;
    ///    `query_events` will keep returning `ExecutionLogUnavailable { reason }`.
    ///    Auto-refreshing would mask corruption and contradict operator rule §3.3.
    ///
    /// 3. `Absent` + durable log present  -> reopen via the injected factory
    ///    (REC-C1.5.2 strict replay) and register the validated handle. The
    ///    factory's own validation owns the corruption case; on success the
    ///    handle is published, on failure the typed reason is published as
    ///    `Unavailable` so the next read reports it explicitly.
    ///
    /// 4. `Absent` + durable log absent   -> register `Unavailable` with the
    ///    factory's reason text. There is no Silent Lie path: an empty log
    ///    is never created, and no `Gap` is invented. Session metadata
    ///    remains queryable through the engine map; only the canonical
    ///    ExecutionLog reads surface the typed unavailability.
    fn rehydrate_execution_log(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<(), ServiceError> {
        let log_dir = ctx.execution_log_root.join(session_id);
        // CIH-F (operator review): atomic peek + try_reopen + register.
        // The non-atomic peek/try_reopen/register sequence could let two
        // concurrent load_session calls for the same session_id both
        // observe Absent, both call reopen_existing (yielding two distinct
        // Arcs from the factory), and have the second register fail with
        // ExecutionLogIdentityMismatch. rehydrate performs the whole
        // decision under a single critical section on the registry's logs
        // lock, so the first thread to enter the Absent branch wins and
        // any racers observe AlreadyAvailable / AlreadyUnavailable. A prior
        // Unavailable reason is preserved verbatim (operator rule §3.3).
        let _outcome = ctx
            .execution_log_registry
            .rehydrate(&log_dir, &SessionId::new(session_id))?;
        Ok(())
    }

    /// List all saved sessions from persistent storage.
    ///
    /// Returns metadata summaries for all sessions (no event data).
    ///
    /// "No sessions yet" is a **store** contract, not a caller concern:
    /// `SessionStore::list_sessions` returns an empty list for a database whose
    /// `sessions` table does not exist yet (fresh install, or a fresh in-memory
    /// store), so this method does not need to inspect or match on error text.
    /// The former substring match (`err_str.contains("does not exist")`) was
    /// removed in m9-71: it tolerated an error the store no longer produces, and
    /// it would have masked a genuine store failure whose message happened to
    /// contain those words.
    ///
    /// # Errors
    /// - `ListFailed` if the store read fails.
    pub async fn list_sessions(ctx: &SessionsContext<'_>) -> Result<ListResult, ServiceError> {
        let sessions = ctx
            .archive
            .list()
            .map_err(|e| ServiceError::ListFailed(e.to_string()))?;

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
    /// - `SessionStillActive` if the session is currently in
    ///   `ctx.connected_sessions` (REC-C1.6). No side effect is taken
    ///   in this case — the store row and the durable directory both
    ///   stay intact, and no implicit probe stop is performed.
    /// - `DeleteFailed` if the store delete fails.
    pub async fn delete_session(
        session_id: &str,
        ctx: &SessionsContext<'_>,
    ) -> Result<DeleteResult, ServiceError> {
        // REC-C1.6: refuse if the session still has a live probe attached.
        // `connected_sessions` is the canonical "live" set, already used by
        // save_session / list_sessions / cleanup_session_memory. We read it
        // BEFORE any destructive operation so a refusal cannot leave
        // partial state on disk.
        {
            let active = ctx
                .connected_sessions
                .lock()
                .map_err(|_| ServiceError::LockPoisoned)?;
            if active.contains(session_id) {
                return Err(ServiceError::SessionStillActive {
                    session_id: session_id.to_string(),
                    hint: "stop the probe (session_stop) and try again",
                });
            }
        }

        let archive = ctx.archive;
        archive
            .delete(session_id)
            .map_err(|e| ServiceError::DeleteFailed(e.to_string()))?;

        let paths_removed = delete_durable_execution_log(
            ctx.execution_log_registry,
            ctx.execution_log_root,
            session_id,
        )?;

        Ok(DeleteResult {
            session_id: session_id.to_string(),
            paths_removed,
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
    use crate::session_log::RegistrationState;
    use chronos_domain::ports::session::SessionArchive;
    use chronos_domain::{SourceLocation, TraceEvent};
    use chronos_store::SessionStore;
    use std::collections::HashSet;
    use std::sync::Arc;

    /// Helper: build a minimal in-memory SessionStore (still used by the
    /// `SessionStoreBackedSessionArchive` adapter in some tests).
    fn make_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    /// Helper: build a leaked-Arc `&'static dyn SessionArchive` backed by a
    /// fresh `SessionStore`. Mirrors the composition-root wiring.
    fn make_archive() -> &'static dyn SessionArchive {
        let store = Arc::new(make_store());
        let adapter = chronos_store::session_archive::SessionStoreBackedSessionArchive::new(store);
        Box::leak(Box::new(adapter)) as &'static dyn SessionArchive
    }

    /// Helper: build a leaked-Arc `&'static dyn SessionArchive` backed by
    /// the pure in-memory implementation (no SessionStore round-trip).
    /// Used by tests that want to exercise the port contract without
    /// touching the SQLite backend.
    #[allow(dead_code)] // available for future port-contract tests; not exercised yet
    fn make_in_memory_archive() -> &'static dyn SessionArchive {
        Box::leak(Box::new(
            chronos_domain::ports::session::InMemorySessionArchive::new(),
        )) as &'static dyn SessionArchive
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
        archive: &'a dyn SessionArchive,
    ) -> SessionsContext<'a> {
        SessionsContext {
            engines,
            session_languages: languages,
            connected_sessions: connected,
            archive,
            execution_log_registry: Box::leak(Box::new(SessionExecutionLogRegistry::new())),
            execution_log_root: Box::leak(Box::new(std::env::temp_dir())),
        }
    }

    // -------------------------------------------------------------------------
    // save_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn save_session_ok() {
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1"); // only s1 exists
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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

        let store = make_archive();
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

        let result = SessionsService::load_session("no-such-session", &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::LoadFailed(ref e)) if e.contains("not found")
        ));
    }

    // -------------------------------------------------------------------------
    // list_sessions — happy path
    // -------------------------------------------------------------------------

    /// m9-71 contract guard: a store with no `sessions` table yet (fresh
    /// install, fresh in-memory store) must list as **empty**, not as an error.
    ///
    /// This test is only load-bearing because `list_sessions` above propagates
    /// store errors instead of substring-matching them: with the old
    /// `contains("does not exist")` workaround in place it would pass even if
    /// the store returned `TableDoesNotExist` (verified by reverting the
    /// store-side absent-table handling in m9-70 — this test then fails).
    #[tokio::test]
    async fn list_sessions_empty() {
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

        let result = SessionsService::list_sessions(&ctx).await.unwrap();

        assert!(result.sessions.is_empty());
    }

    #[tokio::test]
    async fn list_sessions_one() {
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

        let result = SessionsService::delete_session("no-such", &ctx).await;

        assert!(matches!(
            result,
            Err(ServiceError::DeleteFailed(ref e)) if e.contains("not found")
        ));
    }

    // -------------------------------------------------------------------------
    // REC-C1.6: delete_session on a live session is refused
    // -------------------------------------------------------------------------

    /// DEL-LIVE-3 (unit form): when the target session is registered as
    /// "connected" (a live probe is attached), `delete_session` MUST
    /// refuse without performing any destructive side effect.
    ///
    /// This proves:
    ///   1. The error variant is `ServiceError::SessionStillActive`
    ///      (not a generic `DeleteFailed`).
    ///   2. The error text carries the session id and the action hint.
    ///   3. The store row is still present after the refusal — the
    ///      refusal happened BEFORE the store.delete_session call.
    #[tokio::test]
    async fn delete_session_refuses_live_session_unit() {
        let store = make_archive();
        let engines = make_engines("live-s1");
        let languages = make_languages();
        // Mark the session as live (probe writer attached).
        let mut active = HashSet::new();
        active.insert("live-s1".to_string());
        let connected = std::sync::Mutex::new(active);
        let ctx = make_context(&engines, &languages, &connected, store);

        // First save so the row exists; this simulates a "real" session
        // that the user might want to delete later.
        SessionsService::save_session("live-s1", Language::C, "main".to_string(), &ctx)
            .await
            .unwrap();

        let result = SessionsService::delete_session("live-s1", &ctx).await;

        match result {
            Err(ServiceError::SessionStillActive { session_id, hint }) => {
                assert_eq!(session_id, "live-s1");
                assert!(
                    hint.contains("session_stop"),
                    "hint must name the recovery action; got {hint:?}"
                );
                assert!(
                    hint.contains("probe"),
                    "hint must mention the probe; got {hint:?}"
                );
            }
            other => panic!(
                "expected SessionStillActive refusal, got {other:?}\n\
                 (live sessions must NEVER produce DeleteFailed nor Ok)"
            ),
        }

        // The archive row is still there — the refusal happened BEFORE
        // any destructive call.
        let still_there = store
            .list()
            .unwrap_or_default()
            .iter()
            .any(|m| m.session_id == "live-s1");
        assert!(
            still_there,
            "archive row for live-s1 must survive a refused delete"
        );
    }

    /// Sanity: a session that was live and is then explicitly removed
    /// from `connected_sessions` becomes deletable. This is what
    /// `session_stop` would do; we are testing only the service-side
    /// precondition, so the drop in `connected_sessions` is a manual
    /// stand-in.
    #[tokio::test]
    async fn delete_session_after_unconnected_succeeds() {
        let store = make_archive();
        let engines = make_engines("transitions-to-deletable");
        let languages = make_languages();
        let mut active = HashSet::new();
        active.insert("transitions-to-deletable".to_string());
        let connected = std::sync::Mutex::new(active);
        let ctx = make_context(&engines, &languages, &connected, store);

        SessionsService::save_session(
            "transitions-to-deletable",
            Language::C,
            "main".to_string(),
            &ctx,
        )
        .await
        .unwrap();

        // First call: refused (live).
        let refused = SessionsService::delete_session("transitions-to-deletable", &ctx).await;
        assert!(matches!(
            refused,
            Err(ServiceError::SessionStillActive { .. })
        ));

        // Simulate session_stop (in production that would remove from
        // connected_sessions AND seal the manifest). The service sees
        // only the connected_sessions set.
        {
            let mut g = connected.lock().unwrap();
            g.remove("transitions-to-deletable");
        }

        // Second call: succeeds.
        let ok = SessionsService::delete_session("transitions-to-deletable", &ctx).await;
        assert!(
            ok.is_ok(),
            "delete_session must succeed once the session is no longer live; got {ok:?}"
        );
    }

    // -------------------------------------------------------------------------
    // drop_session — happy path
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn drop_session_existed() {
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

        let result = SessionsService::drop_session("s1", &ctx).await.unwrap();

        assert_eq!(result.session_id, "s1");
        assert!(result.existed);
    }

    #[tokio::test]
    async fn drop_session_not_found_idempotent() {
        let store = make_archive();
        let engines = make_engines("s1"); // only s1 in engines
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let ctx = make_context(&engines, &languages, &connected, store);

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
        let store = make_archive();

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
        let ctx = make_context(&engines, &languages, &connected, store);

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

    // =====================================================================
    // CIH-F — load_session reopen tests
    //
    // The five mandatory acceptance cases (operator §3):
    //   1. Single-session: save -> drop -> load -> query the canonical log.
    //   2. Multi-session:  load one session must not modify another's
    //                      availability or events.
    //   3. Real restart:   a fresh registry bootstrapped from disk must
    //                      produce the same observable state as the
    //                      load_session path.
    //   4. Repeated load:  calling load_session twice on the same session
    //                      must NOT produce an ExecutionLogIdentityMismatch.
    //   5. Missing / corrupt durable log: session metadata stays in the
    //      engine map and the registry carries an explicit Unavailable
    //      reason. No empty log is fabricated.
    //
    // These tests live in the service crate because they exercise the
    // SessionsService::load_session contract directly. The end-to-end
    // behaviour over the real MCP wire is covered by the sandbox test
    // `multi_session::test_drop_session_not_in_load_list`, which is the
    // exact failure the operator authorised CIH-F to close.
    // =====================================================================

    /// Build a per-test ExecutionLog root (tempdir) and a registry wired
    /// to a segmented factory. Both are leaked so the returned context
    /// carries references with `'static` lifetime, matching the existing
    /// test convention (`make_context` above).
    ///
    /// The `store` is wrapped in a `SessionStoreBackedSessionArchive`
    /// adapter so the helper stays aligned with the production
    /// composition-root wiring (REC-C3.3.3 Tren B — `SessionsContext`
    /// consumes the port, not the concrete `SessionStore`).
    fn make_context_with_log_root<'a>(
        engines: &'a Mutex<HashMap<String, QueryEngine>>,
        languages: &'a Mutex<HashMap<String, Language>>,
        connected: &'a std::sync::Mutex<HashSet<String>>,
        store: &'a SessionStore,
    ) -> (SessionsContext<'a>, &'a Path) {
        let tmp = std::env::temp_dir().join(format!(
            "cih-f-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).expect("tempdir");
        let registry = Box::leak(Box::new(SessionExecutionLogRegistry::with_factory(
            std::sync::Arc::new(chronos_log::factory::SegmentedExecutionLogFactory::new()),
        )));
        let root = Box::leak(Box::new(tmp));
        let archive = chronos_store::session_archive::SessionStoreBackedSessionArchive::new(
            std::sync::Arc::new(SessionStore::in_memory().unwrap()),
        );
        let archive: &'a dyn chronos_domain::ports::session::SessionArchive =
            Box::leak(Box::new(archive));
        // NOTE: the `store` argument is intentionally retained to keep the
        // call-site pattern unchanged. The production SessionsContext reads
        // through the port (`archive`), not through the concrete store.
        let _ = store;
        let ctx = SessionsContext {
            engines,
            session_languages: languages,
            connected_sessions: connected,
            archive,
            execution_log_registry: registry,
            execution_log_root: root,
        };
        (ctx, root)
    }

    /// Helper: create a durable ExecutionLog on disk for `session_id` and
    /// register it in the supplied registry. Mirrors what `probe_start`
    /// does on the production path (minus the live probe).
    fn make_durable_log(
        registry: &SessionExecutionLogRegistry,
        root: &Path,
        session_id: &str,
        n: u64,
    ) {
        let dir = root.join(session_id);
        registry
            .register_create(&dir, chronos_log::SessionId::new(session_id))
            .expect("register_create");
        // Append n records directly through the handle so the on-disk log
        // has the same shape as a real capture.
        let handle = registry.get(session_id).expect("just-registered handle");
        for i in 0..n {
            handle
                .append(chronos_log::NewExecutionRecord {
                    kind: chronos_log::ExecutionKind::Raw,
                    session_id: chronos_log::SessionId::new(session_id),
                    monotonic_ns: i,
                    payload: chronos_log::ExecutionPayload::new(format!("r{i}").into_bytes(), "t"),
                    invocation_id: None,
                    parent_invocation_id: None,
                    symbol_id: None,
                    captured_at_unix_ns: None,
                })
                .expect("append");
        }
        handle.flush().expect("flush");
    }

    // -----------------------------------------------------------------
    // 1. Single-session: save -> drop -> load -> canonical read works.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_single_session_save_drop_load_query() {
        let store = make_store();
        let engines = make_engines("s1");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        // Probe + capture analogue: durable log on disk + registry handle.
        make_durable_log(ctx.execution_log_registry, root, "s1", 3);

        // Persist session metadata.
        SessionsService::save_session("s1", Language::Go, "./server".to_string(), &ctx)
            .await
            .unwrap();

        // drop_session analogue: forget the engine from memory and the
        // ExecutionLog from the registry, but keep the on-disk evidence.
        engines.lock().await.remove("s1");
        ctx.execution_log_registry.remove("s1");
        assert!(matches!(
            ctx.execution_log_registry.peek_registration("s1"),
            RegistrationState::Absent
        ));

        // load_session must (a) succeed with metadata, (b) repopulate the
        // engine, and (c) leave the registry with an Available entry so a
        // canonical read can find the evidence.
        let result = SessionsService::load_session("s1", &ctx).await.unwrap();
        assert_eq!(result.event_count, 2);
        assert!(engines.lock().await.contains_key("s1"));
        assert!(matches!(
            ctx.execution_log_registry.peek_registration("s1"),
            RegistrationState::Available
        ));

        let handle = ctx
            .execution_log_registry
            .get("s1")
            .expect("Available after load_session");
        let page = handle
            .handle()
            .read_from_seq(chronos_log::EventSeq::ZERO, 100)
            .expect("read");
        assert_eq!(
            page.records.len(),
            3,
            "all three raw records must survive reopen via load_session"
        );
    }

    // -----------------------------------------------------------------
    // 2. Multi-session: loading one session does not touch another.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_multi_session_load_one_does_not_disturb_another() {
        let store = make_store();
        let engines = make_engines("sA");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        // Build sA in-memory (engines map) and on-disk (registry + log).
        make_durable_log(ctx.execution_log_registry, root, "sA", 2);
        // Build sB ONLY on-disk (no engine map, no store entry yet).
        make_durable_log(ctx.execution_log_registry, root, "sB", 4);

        // Persist sA.
        SessionsService::save_session("sA", Language::Rust, "a.rs".to_string(), &ctx)
            .await
            .unwrap();

        // Snapshot the sB registry entry: must be untouched across load_session("sA").
        let s_b_provider_before = match ctx.execution_log_registry.peek_registration("sB") {
            RegistrationState::Available => Some(
                ctx.execution_log_registry
                    .get("sB")
                    .expect("sB Available")
                    .provider(),
            ),
            _ => None,
        };

        // Drop sA (engine + registry), then load_session("sA").
        engines.lock().await.remove("sA");
        ctx.execution_log_registry.remove("sA");
        SessionsService::load_session("sA", &ctx).await.unwrap();

        // sA is back in the registry.
        assert!(matches!(
            ctx.execution_log_registry.peek_registration("sA"),
            RegistrationState::Available
        ));

        // sB's identity (Arc pointer) is unchanged: load_session("sA") did
        // not register, remove, or refresh sB.
        let s_b_provider_after = match ctx.execution_log_registry.peek_registration("sB") {
            RegistrationState::Available => Some(
                ctx.execution_log_registry
                    .get("sB")
                    .expect("sB Available")
                    .provider(),
            ),
            _ => None,
        };
        assert_eq!(
            s_b_provider_before.as_ref().map(std::sync::Arc::as_ptr),
            s_b_provider_after.as_ref().map(std::sync::Arc::as_ptr),
            "sB's ExecutionLog provider Arc must not move across load_session('sA')"
        );
    }

    // -----------------------------------------------------------------
    // 3. Real restart: fresh registry, bootstrap from disk. The observable
    //    state must match what load_session produces for the same session.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_real_restart_bootstrap_equivalent_to_load_session() {
        let store = make_store();
        let engines = make_engines("sR");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        make_durable_log(ctx.execution_log_registry, root, "sR", 5);
        SessionsService::save_session("sR", Language::Cpp, "main.cpp".to_string(), &ctx)
            .await
            .unwrap();

        // Path A: load_session from a drop-equivalent state.
        engines.lock().await.remove("sR");
        ctx.execution_log_registry.remove("sR");
        SessionsService::load_session("sR", &ctx).await.unwrap();
        let page_load = ctx
            .execution_log_registry
            .get("sR")
            .expect("after load_session")
            .handle()
            .read_from_seq(chronos_log::EventSeq::ZERO, 100)
            .expect("read");
        assert_eq!(page_load.records.len(), 5);

        // Path B: fresh registry + bootstrap from the same on-disk root.
        let fresh_registry = SessionExecutionLogRegistry::with_factory(std::sync::Arc::new(
            chronos_log::factory::SegmentedExecutionLogFactory::new(),
        ));
        let plan = crate::execution_log_bootstrap::bootstrap_execution_logs(
            root,
            &fresh_registry,
            &(std::sync::Arc::new(chronos_log::factory::SegmentedExecutionLogFactory::new())
                as std::sync::Arc<dyn chronos_domain::ports::ExecutionLogFactory>),
        )
        .expect("bootstrap");
        assert_eq!(plan.available().count(), 1, "exactly one log on disk");
        let page_boot = fresh_registry
            .get("sR")
            .expect("after bootstrap")
            .handle()
            .read_from_seq(chronos_log::EventSeq::ZERO, 100)
            .expect("read");
        assert_eq!(
            page_boot.records.len(),
            page_load.records.len(),
            "bootstrap and load_session must surface the same number of records"
        );
        assert_eq!(
            page_boot.records.first().map(|r| r.monotonic_ns),
            page_load.records.first().map(|r| r.monotonic_ns),
            "first record's monotonic_ns must match"
        );
    }

    // -----------------------------------------------------------------
    // 4. Repeated load: a second load_session must NOT raise
    //    ExecutionLogIdentityMismatch; the existing handle is reused.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_repeated_load_does_not_overwrite_existing_handle() {
        let store = make_store();
        let engines = make_engines("s2");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        make_durable_log(ctx.execution_log_registry, root, "s2", 2);
        SessionsService::save_session("s2", Language::Go, "main.go".to_string(), &ctx)
            .await
            .unwrap();

        // First load: drops and reloads; the registry goes Absent -> Available.
        engines.lock().await.remove("s2");
        ctx.execution_log_registry.remove("s2");
        SessionsService::load_session("s2", &ctx).await.unwrap();
        let provider_first = ctx
            .execution_log_registry
            .get("s2")
            .expect("after first load")
            .provider();

        // Second load WITHOUT removing from the registry: the engine is
        // rebuilt from the store, but the registry state stays Available
        // and the original Arc is preserved.
        SessionsService::load_session("s2", &ctx).await.unwrap();
        let provider_second = ctx
            .execution_log_registry
            .get("s2")
            .expect("after second load")
            .provider();
        assert!(
            std::sync::Arc::ptr_eq(&provider_first, &provider_second),
            "second load_session must reuse the same Arc (no IdentityMismatch)"
        );
    }

    // -----------------------------------------------------------------
    // 5. Missing durable log: session metadata persists, registry
    //    carries an explicit Unavailable reason, no empty log fabricated.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_missing_durable_log_records_unavailable_keeps_metadata() {
        let store = make_store();
        let engines = make_engines("sM");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        // sM exists only in the engine map; NO ExecutionLog on disk.
        SessionsService::save_session("sM", Language::Python, "a.py".to_string(), &ctx)
            .await
            .unwrap();
        engines.lock().await.remove("sM");
        ctx.execution_log_registry.remove("sM");

        // Pre-condition: no directory on disk for sM.
        assert!(!root.join("sM").exists(), "fixture must be missing on disk");

        // load_session succeeds (metadata + engine are queryable), and
        // the registry carries an explicit Unavailable reason.
        let result = SessionsService::load_session("sM", &ctx).await.unwrap();
        assert_eq!(result.event_count, 2);

        match ctx.execution_log_registry.peek_registration("sM") {
            RegistrationState::Unavailable => {}
            other => panic!("expected Unavailable after missing-dir reopen, got {other:?}"),
        }
        let err = ctx
            .execution_log_registry
            .get("sM")
            .expect_err("Unavailable get");
        match err {
            ServiceError::ExecutionLogUnavailable { reason, .. } => {
                assert!(
                    !reason.is_empty(),
                    "Unavailable reason must carry the factory's typed error"
                );
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }

        // The on-disk directory must still NOT exist: no Silent Lie, no
        // empty-log fabrication, no Gap inserted.
        assert!(
            !root.join("sM").exists(),
            "load_session must not create an empty log dir for a missing session"
        );
    }

    // -----------------------------------------------------------------
    // 5b. Corrupt durable log: metadata persists, registry carries an
    //     Unavailable reason that mentions the strict-replay refusal.
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn cih_f_corrupt_durable_log_records_unavailable_keeps_metadata() {
        let store = make_store();
        let engines = make_engines("sC");
        let languages = make_languages();
        let connected = make_connected();
        let (ctx, root) = make_context_with_log_root(&engines, &languages, &connected, &store);

        make_durable_log(ctx.execution_log_registry, root, "sC", 3);
        SessionsService::save_session("sC", Language::C, "main.c".to_string(), &ctx)
            .await
            .unwrap();

        // Corrupt the only segment body so REC-C1.5.2 strict replay
        // refuses on reopen.
        let dir = root.join("sC");
        let seg_path = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().map(|e| e == "seg").unwrap_or(false))
            .expect("a segment file");
        let mut bytes = std::fs::read(&seg_path).unwrap();
        let n = bytes.len() * 3 / 4;
        bytes[n] ^= 0xFF;
        std::fs::write(&seg_path, &bytes).unwrap();

        // Drop from memory; the on-disk corrupt log is what load_session
        // will see.
        engines.lock().await.remove("sC");
        ctx.execution_log_registry.remove("sC");

        // load_session succeeds; registry carries Unavailable with the
        // typed rejection reason from REC-C1.5.2 strict replay.
        let result = SessionsService::load_session("sC", &ctx).await.unwrap();
        assert_eq!(result.event_count, 2, "metadata persists");
        match ctx.execution_log_registry.peek_registration("sC") {
            RegistrationState::Unavailable => {}
            other => panic!("expected Unavailable after corrupt reopen, got {other:?}"),
        }
        let err = ctx
            .execution_log_registry
            .get("sC")
            .expect_err("Unavailable get");
        match err {
            ServiceError::ExecutionLogUnavailable { reason, .. } => {
                assert!(
                    reason.contains("reopen")
                        || reason.contains("Integrity")
                        || reason.contains("Replay"),
                    "the typed reason must come from the factory's strict-replay \
                     refusal; got: {reason}"
                );
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }

        // The corrupted segment must NOT have been silently replaced or
        // truncated by load_session.
        let bytes_after = std::fs::read(&seg_path).unwrap();
        assert_eq!(
            bytes_after.len(),
            bytes.len(),
            "load_session must not rewrite the durable segment"
        );
    }
}
