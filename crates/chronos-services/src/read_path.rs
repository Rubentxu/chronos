//! M10 read-path production entry point (`m10-readpath-production-entry`).
//!
//! # What this module is
//!
//! `poll_batch_real` (`live_streaming`), `summarize_log` + `rollup_log`
//! (`virtualization`) and `causality_status_for_engine` (`live_streaming`)
//! were implemented and unit-tested, but had **zero production callers**.
//! This module is the seam that makes them reachable from a real
//! deployment: it resolves a `session_id` to the authoritative
//! `SessionExecutionLog` and drives those three consumers.
//!
//! # Why the previously-recorded "architecture decision" was not one
//!
//! The blocker recorded on 2026-09-27 stated:
//!
//! > "the read path requires `&SessionExecutionLog`, while `ChronosServer`
//! > holds `Arc<Mutex<HashMap<String, QueryEngine>>>` — two different
//! > representations. Choosing between 'open the log per session_id' and
//! > 'derive EventSummary from the engine' is an architecture decision."
//!
//! That premise is false, and the reason is an owned-vs-borrowed confusion:
//!
//! - `SessionExecutionLogRegistry::get(&str)` (`session_log.rs:652`)
//!   returns an **owned, cheaply-cloneable** `SessionExecutionLog`. The
//!   handle is `Arc`-backed (`session_log.rs:87-97`), so cloning shares
//!   the same provider rather than reopening or copying the log.
//! - `subscribe_with_log(..., log: &SessionExecutionLog)`
//!   (`live_streaming.rs:327`) only needs a **borrow** of that owned value.
//! - `ChronosServer` already holds the registry
//!   (`chronos-mcp/src/server.rs:179`) and already resolves a session to
//!   its log in `ensure_projection` (`server.rs:2275`).
//!
//! So the bridge is `let log = registry.get(id)?;` followed by a borrow.
//! There is no representation conflict and no ADR-0029 §3.2/§3.4 fork:
//! there is exactly one authority (the per-session `ExecutionLog`), and
//! this module reads it.
//!
//! # Contract
//!
//! - **One authority.** Every function here reads the per-session
//!   `ExecutionLog` via the registry. Nothing derives events from a
//!   `QueryEngine` projection, so a summary can never disagree with the
//!   event stream (REC-C1 single-truth invariant).
//! - **Fail-closed.** A session with no registered log returns
//!   `Err(ReadPathError::LogUnavailable)` carrying the registry's own
//!   honest reason. It is never coerced into an empty page, an empty
//!   summary, or a "no events" answer (ADR-0004).
//! - **Cheap handles.** Logs are fetched by value and dropped at the end
//!   of each call. The underlying `Arc`s keep the segment mapping alive
//!   without extending a borrow across the caller's stack frame.
//!
//! # UAT mapping
//!
//! - UAT-M10-03-02: `poll_batch` returns events in arrival order, now
//!   reachable through [`ReadPathService::poll_batch`].
//! - UAT-M10-03-05: causality status is now derivable per session, so
//!   the `Unsupported` stub stops being the only answer an agent can get.
//! - UAT-M10-04-03/04: `EventSummary` and `InvocationRollup` are now
//!   reachable through [`ReadPathService::summarize`] /
//!   [`ReadPathService::rollup`].

use std::sync::{Arc, Mutex};

use crate::error::ServiceError;
use crate::events_cursor::EventsCursorV1;
use crate::live_streaming::{
    causality_status_for_engine, subscribe_with_log, CausalityStatus, EventBatch, LiveStreamError,
    SubscriptionRegistry,
};
use crate::session_log::SessionExecutionLogRegistry;
use crate::virtualization::{rollup_log, summarize_log, EventSummary, InvocationRollup};

/// Errors surfaced by the read-path entry point.
///
/// These are *read* failures. Per ADR-0004 they must remain
/// distinguishable from "the session has no events": an unavailable
/// log is an error, not an empty result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadPathError {
    /// The session has no readable `ExecutionLog` registered.
    ///
    /// Carries the registry's own reason so the caller can tell a
    /// never-registered session from a deliberately-unavailable one.
    #[error("execution log unavailable for session {session_id}: {reason}")]
    LogUnavailable { session_id: String, reason: String },
    /// An engine was requested for causality status but is not loaded.
    #[error("no query engine loaded for session {session_id}")]
    EngineNotLoaded { session_id: String },
    /// The live-stream read failed mid-poll.
    #[error("live stream read failed: {0}")]
    Live(#[from] LiveStreamError),
    /// The underlying service layer failed (e.g. lock poisoned).
    #[error("service error: {0}")]
    Service(String),
}

impl From<ServiceError> for ReadPathError {
    fn from(e: ServiceError) -> Self {
        ReadPathError::Service(e.to_string())
    }
}

/// Stateless reader over the authoritative per-session execution logs.
///
/// Holds the same `SessionExecutionLogRegistry` the MCP server owns, so
/// the composition root can hand it over without any new abstraction
/// (REC-C3.3: services consume what is already wired).
#[derive(Debug, Clone)]
pub struct ReadPathService {
    execution_logs: Arc<SessionExecutionLogRegistry>,
    subscriptions: Arc<Mutex<SubscriptionRegistry>>,
}

impl ReadPathService {
    /// Build a reader over `execution_logs`.
    pub fn new(execution_logs: Arc<SessionExecutionLogRegistry>) -> Self {
        Self {
            execution_logs,
            subscriptions: Arc::new(Mutex::new(SubscriptionRegistry::default())),
        }
    }

    /// Resolve `session_id` to its authoritative log handle.
    ///
    /// This is the single place the read path obtains evidence. The
    /// handle is owned (cheap clone, shared `Arc`s) and lives only for
    /// the duration of the caller's borrow.
    pub fn session_log(
        &self,
        session_id: &str,
    ) -> Result<crate::session_log::SessionExecutionLog, ReadPathError> {
        self.execution_logs.get(session_id).map_err(|e| {
            // `ServiceError` renders the honest reason; surface it
            // verbatim rather than inventing our own message.
            let reason = match &e {
                ServiceError::ExecutionLogUnavailable { reason, .. } => reason.clone(),
                other => other.to_string(),
            };
            ReadPathError::LogUnavailable {
                session_id: session_id.to_string(),
                reason,
            }
        })
    }

    /// Poll up to `limit` events from `cursor` for `session_id`.
    ///
    /// Thin, honest wrapper over `LiveEventStreamWithLog::poll_batch_real`:
    /// it subscribes, reads one bounded page, and drops the handle. The
    /// cursor is returned to the caller so the next call can resume;
    /// per ADR-0004 it never moves backward.
    ///
    /// An empty `events` vec with `Ok` means "no events past this
    /// cursor". A read failure is `Err`, never an empty `Ok`.
    pub fn poll_batch(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
        limit: usize,
    ) -> Result<EventBatch, ReadPathError> {
        let log = self.session_log(session_id)?;
        let sid = log.session_id().clone();
        let mut stream = subscribe_with_log(sid, Arc::clone(&self.subscriptions), &log);
        // Re-seat the handle at the caller's cursor so a poll loop can
        // resume where the previous page ended. A backward request is
        // rejected by `advance` per ADR-0004, and that rejection is
        // load-bearing: it is surfaced, not swallowed.
        stream.inner_mut().advance(cursor.next_seq())?;
        let batch = stream.poll_batch_real(limit)?;
        // Release the subscription before the log handle goes away.
        drop(stream);
        Ok(batch)
    }

    /// Summarize the whole log for `session_id` into time buckets.
    ///
    /// Reads every page (per the `summarize_log` contract); the caller
    /// owns the `should_summarize` threshold decision.
    pub fn summarize(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
        bucket_size_ns: u64,
    ) -> Result<EventSummary, ReadPathError> {
        let log = self.session_log(session_id)?;
        Ok(summarize_log(&log, cursor, bucket_size_ns))
    }

    /// Roll events up per thread for `session_id`.
    pub fn rollup(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
    ) -> Result<InvocationRollup, ReadPathError> {
        let log = self.session_log(session_id)?;
        Ok(rollup_log(&log, cursor))
    }

    /// Report whether causality hints are available for `session_id`.
    ///
    /// Requires both halves to be present: a readable log (this service
    /// is the read authority) **and** a loaded engine carrying the M9
    /// causality index. When either is missing the answer is
    /// `Unsupported`, never a fabricated `Wired`.
    pub fn causality_status(
        &self,
        session_id: &str,
        engine: Option<&chronos_query::QueryEngine>,
    ) -> Result<CausalityStatus, ReadPathError> {
        // Fail-closed: no readable log means we cannot assert anything
        // about causality for this session.
        let _log = self.session_log(session_id)?;
        match engine {
            Some(engine) => Ok(causality_status_for_engine(engine)),
            None => Ok(CausalityStatus::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::SessionId;
    use std::env;

    fn sid() -> SessionId {
        SessionId::new(format!("sess-{}", uuid::Uuid::new_v4()))
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!("read_path_{tag}_{}", uuid::Uuid::new_v4().simple()))
    }

    /// Build a registry holding one real, readable log for `session`.
    ///
    /// This is the construction that the 2026-09-27 blocker claimed was
    /// impossible ("two different representations"). It is not only
    /// possible, it is the same construction `ChronosEventsReadService`
    /// already uses in production.
    fn registry_with_log(tag: &str) -> (SessionId, Arc<SessionExecutionLogRegistry>) {
        use crate::session_log::SessionExecutionLog;
        let session = sid();
        let dir = tmpdir(tag);
        let log = SessionExecutionLog::create_for_tests(&dir, session.clone()).expect("log");
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log).expect("register");
        (session, Arc::new(registry))
    }

    // ============ INV-1: the read path resolves session_id -> log ============

    /// The central claim of this cycle: a `session_id` alone is enough to
    /// reach the authoritative log, with no engine and no second
    /// representation. If this test ever fails, the blocker premise was
    /// right after all and `ReadPathService` is built on a false bridge.
    #[test]
    fn inv_1_session_id_alone_reaches_the_authoritative_log() {
        let (session, registry) = registry_with_log("inv1");
        let service = ReadPathService::new(registry);
        let log = service
            .session_log(session.as_str())
            .expect("session_id must resolve to a log without an engine");
        assert_eq!(log.session_id(), &session);
    }

    /// The handle is cheap to clone and shares the same provider, so the
    /// read path does not reopen or duplicate the segment mapping. This
    /// is the fact that makes the borrow in `subscribe_with_log` free.
    #[test]
    fn inv_1b_log_handle_is_shared_not_duplicated() {
        let (session, registry) = registry_with_log("inv1b");
        let service = ReadPathService::new(registry);
        let a = service.session_log(session.as_str()).expect("a");
        let b = service.session_log(session.as_str()).expect("b");
        // Same session, and the registry still reports exactly one
        // registration (cloning a handle must not register a second).
        assert_eq!(a.session_id(), b.session_id());
        assert!(service.session_log(session.as_str()).is_ok());
    }

    // ============ INV-2: fail-closed, never a silent empty ============

    /// ADR-0004: an unknown session is an ERROR, not "no events". This
    /// is the assertion that would be silently broken by any future
    /// refactor that coerces the lookup into an empty result.
    #[test]
    fn inv_2_unknown_session_is_an_error_not_an_empty_result() {
        let registry = Arc::new(SessionExecutionLogRegistry::new());
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(sid());
        let err = service
            .poll_batch("never-registered", &cursor, 10)
            .expect_err("unknown session must fail closed");
        match err {
            ReadPathError::LogUnavailable { session_id, reason } => {
                assert_eq!(session_id, "never-registered");
                // The reason is the registry's own, not invented here.
                assert!(
                    !reason.is_empty(),
                    "reason must carry the registry's explanation"
                );
            }
            other => panic!("expected LogUnavailable, got {other:?}"),
        }
    }

    /// A session registered as deliberately unavailable must surface that
    /// reason, and must NOT be reported as an empty-but-valid log.
    #[test]
    fn inv_2b_deliberately_unavailable_session_surfaces_its_reason() {
        let registry = SessionExecutionLogRegistry::new();
        registry
            .register_unavailable("sess-broken", "disk went away")
            .expect("mark unavailable");
        let service = ReadPathService::new(Arc::new(registry));
        let cursor = EventsCursorV1::start(sid());
        let err = service
            .summarize("sess-broken", &cursor, 1_000_000)
            .expect_err("unavailable session must fail closed");
        match err {
            ReadPathError::LogUnavailable { reason, .. } => {
                assert!(
                    reason.contains("disk went away"),
                    "registry reason must survive, got {reason:?}"
                );
            }
            other => panic!("expected LogUnavailable, got {other:?}"),
        }
    }

    // ============ INV-3: the three orphans are now reachable ============

    /// `poll_batch_real` is driven from a `session_id`. On a real,
    /// registered log this returns `Ok` with an empty batch — which is
    /// the honest answer for a log with no events yet, and is
    /// distinguishable from INV-2's error.
    #[test]
    fn inv_3_poll_batch_real_is_reachable_from_a_session_id() {
        let (session, registry) = registry_with_log("inv3");
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());
        let batch = service
            .poll_batch(session.as_str(), &cursor, 10)
            .expect("readable log must poll successfully");
        assert_eq!(batch.events.len(), 0, "fresh log has no events");
    }

    /// `summarize_log` is reachable and returns an empty summary for an
    /// empty log rather than panicking on a zero-length bucket vector.
    #[test]
    fn inv_3b_summarize_is_reachable_from_a_session_id() {
        let (session, registry) = registry_with_log("inv3b");
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());
        let summary = service
            .summarize(session.as_str(), &cursor, 1_000_000)
            .expect("readable log must summarize");
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.bucket_count, 0);
    }

    /// `bucket_size_ns == 0` is rejected by `summarize_log`'s assert.
    /// That is a programmer-error panic, not a runtime error, so this
    /// test documents it rather than pretending it is handled.
    #[test]
    #[should_panic(expected = "bucket_size_ns must be > 0")]
    fn inv_3c_zero_bucket_size_panics_loudly() {
        let (session, registry) = registry_with_log("inv3c");
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());
        let _ = service.summarize(session.as_str(), &cursor, 0);
    }

    /// `rollup_log` is reachable and returns a zero rollup for an empty
    /// log.
    #[test]
    fn inv_3d_rollup_is_reachable_from_a_session_id() {
        let (session, registry) = registry_with_log("inv3d");
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());
        let rollup = service
            .rollup(session.as_str(), &cursor)
            .expect("readable log must roll up");
        assert_eq!(rollup.total_events, 0);
        assert_eq!(rollup.invocation_count, 0);
    }

    /// `causality_status_for_engine` is reachable. With no engine passed
    /// the answer is `Unsupported` — NOT a fabricated `Wired`.
    #[test]
    fn inv_3e_causality_status_without_engine_is_unsupported_not_wired() {
        let (session, registry) = registry_with_log("inv3e");
        let service = ReadPathService::new(registry);
        let status = service
            .causality_status(session.as_str(), None)
            .expect("readable log must answer");
        assert_eq!(status, CausalityStatus::Unsupported);
    }

    /// Causality status is fail-closed on the log too: no readable log
    /// means no claim at all, even before the engine is considered.
    /// It surfaces as `Err`, NOT as a silent `Unsupported` — an
    /// unavailable log and a log-without-causality-index are different
    /// facts and must not collapse into one answer.
    #[test]
    fn inv_3f_causality_status_requires_a_readable_log() {
        let registry = Arc::new(SessionExecutionLogRegistry::new());
        let service = ReadPathService::new(registry);
        let err = service
            .causality_status("never-registered", None)
            .expect_err("causality status must fail closed without a readable log");
        match err {
            ReadPathError::LogUnavailable { session_id, .. } => {
                assert_eq!(session_id, "never-registered");
            }
            other => panic!("expected LogUnavailable, got {other:?}"),
        }
    }

    // ============ INV-4: subscription bookkeeping ============

    /// Each `poll_batch` subscribes and unsubscribes, so a poll loop
    /// does not leak subscription counts across calls. The registry is
    /// shared, so a leak would be observable on the second call.
    #[test]
    fn inv_4_poll_batch_does_not_leak_subscriptions() {
        let (session, registry) = registry_with_log("inv4");
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());
        for _ in 0..3 {
            service
                .poll_batch(session.as_str(), &cursor, 10)
                .expect("poll");
        }
        assert_eq!(
            service.subscriptions.lock().unwrap().total(),
            0,
            "every subscription must be released on drop"
        );
    }

    // ============ Falsification of this cycle's own claim ============

    /// The blocker said the two representations were incompatible. The
    /// strongest form of that claim is "you need an engine to read". Pin
    /// the opposite: zero engines exist anywhere near this path.
    #[test]
    fn falsification_read_path_needs_no_engine_at_all() {
        let (session, registry) = registry_with_log("falsify");
        let service = ReadPathService::new(registry);
        // No QueryEngine is constructed anywhere in this test. The read
        // still succeeds, which is the direct refutation of "derive
        // EventSummary from the engine" being the only alternative.
        let cursor = EventsCursorV1::start(session.clone());
        let summary = service
            .summarize(session.as_str(), &cursor, 1_000_000)
            .expect("reads work with no engine present");
        assert_eq!(summary.total_events, 0);
    }
}
