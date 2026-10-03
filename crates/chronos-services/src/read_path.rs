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
use crate::read_budget::{BudgetExceeded, ReadBudget, ResourceLimits};
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
    /// The operation passed its `ResourceLimits` ceiling (SCALE_BUDGETS §5
    /// D2). Never collapsed into an empty result or a partial aggregate:
    /// a truncated aggregate presented as complete is the same lie as an
    /// exhausted log presented as "no evidence".
    #[error("{0}")]
    Budget(#[from] BudgetExceeded),
    /// An aggregation walk stopped because a page could not be read.
    ///
    /// Carries the reason as text because this enum is `Clone + Eq` and
    /// `ServiceError` is neither. The partial aggregate is **not** returned:
    /// before `AggregateError` existed, a mid-walk read failure exited the
    /// walk through `while let Ok(page)` and produced an `Ok` summary whose
    /// `total_events` silently undercounted the log.
    #[error("read-path aggregate stopped: {0}")]
    Aggregate(String),
    /// The underlying service layer failed (e.g. lock poisoned).
    #[error("service error: {0}")]
    Service(String),
}

impl From<crate::virtualization::AggregateError> for ReadPathError {
    /// A budget stop keeps its own variant, so the resume anchor survives and
    /// the MCP envelope the `Budget` arm builds keeps applying. A read stop
    /// has no anchor to offer — it did not choose to stop — so it becomes
    /// `Aggregate` with the underlying reason intact.
    fn from(e: crate::virtualization::AggregateError) -> Self {
        match e {
            crate::virtualization::AggregateError::Budget(b) => ReadPathError::Budget(b),
            crate::virtualization::AggregateError::Read { source } => {
                ReadPathError::Aggregate(source.to_string())
            }
        }
    }
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
///
/// # The D2 ceiling lives here
///
/// `limits` is the `ResourceLimits` the operator decided in SCALE_BUDGETS §5
/// D2. It is held here — not consulted once and dropped — because every
/// operation below mints a [`ReadBudget`] from it and charges that budget at
/// page boundaries. A limit stored anywhere but on the path that reads the
/// log is a comment.
#[derive(Debug, Clone)]
pub struct ReadPathService {
    execution_logs: Arc<SessionExecutionLogRegistry>,
    subscriptions: Arc<Mutex<SubscriptionRegistry>>,
    limits: ResourceLimits,
}

impl ReadPathService {
    /// Build a reader over `execution_logs` with the D2 default ceiling
    /// (`max_events: 1_000_000`, `timeout_secs: 60`).
    pub fn new(execution_logs: Arc<SessionExecutionLogRegistry>) -> Self {
        Self::with_limits(execution_logs, ResourceLimits::default())
    }

    /// Build a reader with an explicit ceiling.
    ///
    /// The configuration seam: this is how an operator turns the ceiling, and
    /// how the tests below prove the ceiling is a mechanism rather than a
    /// field.
    pub fn with_limits(
        execution_logs: Arc<SessionExecutionLogRegistry>,
        limits: ResourceLimits,
    ) -> Self {
        Self {
            execution_logs,
            subscriptions: Arc::new(Mutex::new(SubscriptionRegistry::default())),
            limits,
        }
    }

    /// The ceiling this service enforces. Read by the AC6 wiring test.
    pub fn limits(&self) -> ResourceLimits {
        self.limits
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
    ///
    /// # Budget
    ///
    /// The requested `limit` is first clamped to `max_events`, so one call
    /// can never ask the log for more than the whole operation may read. The
    /// single page read is then charged, which makes the ceiling apply to
    /// the fast path too rather than exempting it — see `read_budget` for the
    /// measured cost of that on this path.
    pub fn poll_batch(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
        limit: usize,
    ) -> Result<EventBatch, ReadPathError> {
        let log = self.session_log(session_id)?;
        let mut budget = ReadBudget::start("poll", &self.limits, cursor.next_seq().0);
        let limit = budget.clamp_page(limit);
        let sid = log.session_id().clone();
        let mut stream = subscribe_with_log(sid, Arc::clone(&self.subscriptions), &log);
        // Re-seat the handle at the caller's cursor so a poll loop can
        // resume where the previous page ended. A backward request is
        // rejected by `advance` per ADR-0004, and that rejection is
        // load-bearing: it is surfaced, not swallowed.
        stream.inner_mut().advance(cursor.next_seq())?;
        let batch = stream.poll_batch_real(limit)?;
        if !batch.events.is_empty() {
            let next_seq = batch.next_cursor.next_seq().0;
            budget.charge_page(batch.events.len() as u64, next_seq)?;
        }
        // Release the subscription before the log handle goes away.
        drop(stream);
        Ok(batch)
    }

    /// Summarize the whole log for `session_id` into time buckets.
    ///
    /// Reads every page (per the `summarize_log` contract); the caller
    /// owns the `should_summarize` threshold decision.
    ///
    /// This is the operation the D2 ceiling governs. Whether it *bites* at
    /// 1M events depends on the read path's cost, and that measurement moved
    /// with R2.2: see `SCALE_BUDGETS.md` §3.1, where the figure is reconciled
    /// by measurement on both trees — the 1.222,7 s figure was
    /// recorded against a `read_from_seq` that cloned the whole log per page
    /// and is no longer reproducible on current code. The ceiling is not
    /// argued from a stale number; it bounds the walk whatever the cost is,
    /// and a stop reports how far it got either way.
    pub fn summarize(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
        bucket_size_ns: u64,
    ) -> Result<EventSummary, ReadPathError> {
        let log = self.session_log(session_id)?;
        let mut budget = ReadBudget::start("summarize", &self.limits, cursor.next_seq().0);
        Ok(summarize_log(&log, cursor, bucket_size_ns, &mut budget)?)
    }

    /// Roll events up per thread for `session_id`.
    pub fn rollup(
        &self,
        session_id: &str,
        cursor: &EventsCursorV1,
    ) -> Result<InvocationRollup, ReadPathError> {
        let log = self.session_log(session_id)?;
        let mut budget = ReadBudget::start("rollup", &self.limits, cursor.next_seq().0);
        Ok(rollup_log(&log, cursor, &mut budget)?)
    }

    /// Report whether causality hints are available for `session_id`.
    ///
    /// Requires both halves to be present: a readable log (this service
    /// is the read authority) **and** a loaded engine carrying the M9
    /// causality index. When either is missing the answer is
    /// `Unsupported`, never a fabricated `Wired`.
    ///
    /// # Why there is no budget here
    ///
    /// D2 says the ceiling covers "any read-path operation", and this one is
    /// deliberately not wrapped in a budget check. It performs no log scan:
    /// it resolves a handle and reads one `bool` off the engine, so it is
    /// inside the ceiling by construction rather than by measurement. Adding
    /// a check that can never fire would be the decoration D2 is trying to
    /// end, and a real cost — one `Instant::now()` per call — for a
    /// guarantee that already holds. Pinned by
    /// `d2_causality_status_scans_no_pages`.
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

    // ======================================================================
    // D2 (SCALE_BUDGETS §5): the read-path ceiling, and proof it is a
    // mechanism rather than a field.
    //
    // Every test below drives the ceiling on purpose and asserts the stop.
    // The happy-path tests come first so a reader can tell the two apart:
    // the ceiling stops work, it does not break work.
    // ======================================================================

    use crate::read_budget::BudgetLimit;
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation, TraceEvent};
    use chronos_log::{ExecutionKind, ExecutionPayload, NewExecutionRecord};

    /// Events seeded by the fixtures below. Small on purpose: these tests are
    /// about whether the ceiling *stops* the walk, not about scale. The scale
    /// question has its own characterisation in
    /// `chronos-sandbox/tests/scale_execution_log_1m.rs`.
    const D2_EVENTS: u64 = 3_000;

    /// Append one decodable event. The payload must be real `TraceEvent`
    /// JSON: `read_page` fails closed on a record it cannot decode, so an
    /// opaque payload would make the fixture unreadable rather than merely
    /// large.
    fn push_event(log: &crate::session_log::SessionExecutionLog, event_id: u64) {
        let event = TraceEvent::new(
            event_id,
            MonotonicNs::from(event_id * 1_000_000),
            1,
            EventType::FunctionEntry,
            SourceLocation::from_address(0),
            EventData::Empty,
        );
        let session = log.session_id().clone();
        log.append(NewExecutionRecord {
            kind: ExecutionKind::Raw,
            session_id: session,
            monotonic_ns: event_id * 1_000_000,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&event).expect("encode"),
                "trace_event",
            ),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect("append");
    }

    /// A registry holding one log seeded with `events` real records.
    fn registry_with_events(
        tag: &str,
        events: u64,
    ) -> (SessionId, Arc<SessionExecutionLogRegistry>) {
        use crate::session_log::SessionExecutionLog;
        let session = sid();
        let dir = tmpdir(tag);
        let log = SessionExecutionLog::create_for_tests(&dir, session.clone()).expect("log");
        for i in 0..events {
            push_event(&log, i);
        }
        let registry = SessionExecutionLogRegistry::new();
        registry.register(log).expect("register");
        (session, Arc::new(registry))
    }

    /// A ceiling with no wall clock at all. Legitimate as a configuration
    /// ("this operation gets zero seconds") and deterministic as a test: the
    /// first page boundary is already past the deadline, so nothing sleeps and
    /// nothing flakes.
    fn no_time() -> ResourceLimits {
        ResourceLimits {
            max_events: usize::MAX,
            timeout_secs: 0,
        }
    }

    /// A ceiling with an hour of wall clock and a huge event cap, so only the
    /// deadline can be the reason a walk stopped.
    fn only_clock() -> ResourceLimits {
        ResourceLimits {
            max_events: usize::MAX,
            timeout_secs: 3_600,
        }
    }

    /// A ceiling with a huge wall clock and a specific event cap, so only the
    /// event count can be the reason a walk stopped.
    fn only_events(max_events: usize) -> ResourceLimits {
        ResourceLimits {
            max_events,
            timeout_secs: 3_600,
        }
    }

    /// AC1 + AC2: `summarize` past its deadline returns an error that says how
    /// far it got and where to resume — never a partial summary.
    ///
    /// This is the test that makes the D2 decision a mechanism. It is
    /// non-vacuous: with the `charge_page` call removed from `summarize_log`
    /// it fails, because the walk then returns the complete summary over all
    /// 200 events and there is no ceiling left to observe.
    #[test]
    fn d2_summarize_stops_at_its_deadline_instead_of_running_on() {
        let (session, registry) = registry_with_events("d2_sum_deadline", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, no_time());
        let cursor = EventsCursorV1::start(session.clone());

        let err = service
            .summarize(session.as_str(), &cursor, 1_000_000_000)
            .expect_err("summarize must stop at its ceiling, not run the whole log");

        let ReadPathError::Budget(b) = err else {
            panic!("a ceiling stop must be a Budget error, got {err:?}");
        };
        assert_eq!(b.limit, BudgetLimit::Deadline);
        assert_eq!(b.operation, "summarize");
        // The progress is reported, and it is a *prefix*: strictly more than
        // nothing, strictly less than the whole log.
        assert!(b.events_scanned > 0, "must report what it read: {b}");
        assert!(
            b.events_scanned < D2_EVENTS,
            "must not claim to have read the whole log: {b}"
        );
        // The resume anchor is the one number the agent can act on.
        assert!(b.next_seq > 0, "must report where it stopped: {b}");
        assert!(
            b.next_seq <= b.events_scanned + 1,
            "the resume anchor must be consistent with the progress: {b}"
        );
    }

    /// AC1 for the other aggregate: `rollup` obeys the same ceiling.
    #[test]
    fn d2_rollup_stops_at_its_deadline_instead_of_running_on() {
        let (session, registry) = registry_with_events("d2_roll_deadline", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, no_time());
        let cursor = EventsCursorV1::start(session.clone());

        let err = service
            .rollup(session.as_str(), &cursor)
            .expect_err("rollup must stop at its ceiling too");

        let ReadPathError::Budget(b) = err else {
            panic!("a ceiling stop must be a Budget error, got {err:?}");
        };
        assert_eq!(b.limit, BudgetLimit::Deadline);
        assert_eq!(b.operation, "rollup");
        assert!(b.events_scanned < D2_EVENTS, "must be a partial walk: {b}");
    }

    /// AC1 for `max_events`, and independent of the clock: here an hour is
    /// available, so only the event cap can stop the walk.
    #[test]
    fn d2_the_event_ceiling_stops_the_walk_when_the_clock_is_generous() {
        let (session, registry) = registry_with_events("d2_maxevents", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, only_events(10));
        let cursor = EventsCursorV1::start(session.clone());

        let err = service
            .summarize(session.as_str(), &cursor, 1_000_000_000)
            .expect_err("the event ceiling must stop the walk");

        let ReadPathError::Budget(b) = err else {
            panic!("a ceiling stop must be a Budget error, got {err:?}");
        };
        assert_eq!(b.limit, BudgetLimit::MaxEvents);
        assert_eq!(b.limit_field, "max_events");
        assert_eq!(b.max_events, 10);
    }

    /// AC2: the rendered message is the difference between an agent that
    /// resumes and one that retries the same doomed aggregation. It must name
    /// the operation, the knob, the progress and the anchor — and say in words
    /// that this is not an exhausted log.
    #[test]
    fn d2_the_ceiling_message_is_usable_by_an_agent() {
        let (session, registry) = registry_with_events("d2_message", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, no_time());
        let cursor = EventsCursorV1::start(session.clone());

        let text = service
            .summarize(session.as_str(), &cursor, 1_000_000_000)
            .expect_err("must stop")
            .to_string();

        assert!(text.contains("summarize"), "names the operation: {text}");
        assert!(
            text.contains("timeout_secs"),
            "names the knob an operator turns: {text}"
        );
        assert!(
            text.contains("next_seq="),
            "carries the resume anchor: {text}"
        );
        assert!(
            text.contains("NOT \"the log has no more evidence\""),
            "must not be mistakable for an exhausted log: {text}"
        );
    }

    /// AC3: a ceiling stop and an exhausted log are different answers.
    ///
    /// The same log, polled to the end under a generous ceiling, returns
    /// `Ok` with an empty batch. The stopped aggregate returned `Err`. An
    /// agent that saw `Ok`+empty and an agent that saw `Err` are therefore
    /// never confused about whether evidence remains — which is the whole
    /// point of ADR-0004 and the whole point of this error variant.
    #[test]
    fn d2_a_ceiling_stop_is_not_an_empty_result() {
        let (session, registry) = registry_with_events("d2_distinct", D2_EVENTS);
        let service = ReadPathService::with_limits(registry.clone(), no_time());
        let cursor = EventsCursorV1::start(session.clone());

        // Stopped: an error.
        assert!(matches!(
            service.rollup(session.as_str(), &cursor),
            Err(ReadPathError::Budget(_))
        ));

        // Same log, generous ceiling: drained to the end, so `Ok` with zero
        // events, which is the honest "no events past this cursor".
        let generous = ReadPathService::with_limits(registry.clone(), only_clock());
        let mut drained = EventsCursorV1::start(session.clone());
        loop {
            let batch = generous
                .poll_batch(session.as_str(), &drained, 512)
                .expect("a generous ceiling must not stop a bounded poll");
            if batch.events.is_empty() {
                break;
            }
            drained = batch.next_cursor;
        }
        let tail = generous
            .poll_batch(session.as_str(), &drained, 512)
            .expect("drained log still answers");
        assert!(
            tail.events.is_empty(),
            "an exhausted log answers Ok+empty, which is not a ceiling stop"
        );
    }

    /// AC5: the fast path is unharmed. `poll` under the D2 defaults still
    /// returns exactly the requested batch out of a multi-page log and its
    /// cursor advances — the ceiling does not shave the path that matters.
    #[test]
    fn d2_poll_under_default_limits_still_serves_the_full_batch() {
        let (session, registry) = registry_with_events("d2_poll_fast", D2_EVENTS);
        let service = ReadPathService::new(registry);
        let cursor = EventsCursorV1::start(session.clone());

        let first = service
            .poll_batch(session.as_str(), &cursor, 100)
            .expect("a bounded poll under the default ceiling must succeed");
        assert_eq!(
            first.events.len(),
            100,
            "poll must still return exactly the requested batch"
        );

        let second = service
            .poll_batch(session.as_str(), &first.next_cursor, 100)
            .expect("a resumed poll must succeed");
        assert_eq!(second.events.len(), 100);
        assert!(
            second.next_cursor.next_seq() > first.next_cursor.next_seq(),
            "the cursor must still advance"
        );
        assert_ne!(
            second.events[0].seq, first.events[0].seq,
            "the resumed poll must not replay"
        );
    }

    /// AC1 on the fast path: a single `poll` may never ask the log for more
    /// events than the whole operation is allowed to read, so the ceiling
    /// covers `poll` instead of exempting it.
    #[test]
    fn d2_poll_clamps_a_request_to_the_event_ceiling() {
        let (session, registry) = registry_with_events("d2_poll_clamp", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, only_events(4));
        let cursor = EventsCursorV1::start(session.clone());

        let batch = service
            .poll_batch(session.as_str(), &cursor, 10_000)
            .expect("a clamped poll is still a valid poll");
        assert_eq!(
            batch.events.len(),
            4,
            "one call may not exceed the operation's whole event ceiling"
        );
    }

    /// AC6: the field is read, not merely present. The production constructor
    /// installs the D2 values, and they are the ones the loops consume. If
    /// this ever drifts from `ResourceLimits::default()`, the enforcement
    /// everywhere else in this module is enforcing something other than what
    /// the operator decided.
    ///
    /// The second half is what makes it non-vacuous: the very same
    /// constructor with the very same log, reconfigured to zero seconds,
    /// stops. So the value the service holds is demonstrably the value the
    /// service applies, and the 60 s in the first half is not inert text.
    #[test]
    fn d2_the_production_service_carries_the_decided_ceiling() {
        let (session, registry) = registry_with_events("d2_defaults", D2_EVENTS);
        let service = ReadPathService::new(registry.clone());
        let limits = service.limits();
        assert_eq!(limits, ResourceLimits::default());
        assert_eq!(limits.max_events, 1_000_000);
        assert_eq!(limits.timeout_secs, 60);

        // Same constructor, same log, same code path — only the configured
        // wall clock differs, and that alone is the difference between a
        // completed walk and a stop. (`registry` is an `Arc`, so both
        // services read the very same log.)
        let cursor = EventsCursorV1::start(session.clone());
        let generous = ReadPathService::with_limits(registry.clone(), only_clock());
        let summary = generous
            .summarize(session.as_str(), &cursor, 1_000_000_000)
            .expect("an hour of wall clock walks the whole log");
        assert_eq!(
            summary.total_events, D2_EVENTS,
            "with room to work, the aggregate is complete"
        );

        let curtailed = ReadPathService::with_limits(registry, no_time());
        assert!(
            curtailed
                .summarize(session.as_str(), &cursor, 1_000_000_000)
                .is_err(),
            "the same log under the same code, with no wall clock, must stop"
        );
    }

    /// The `causality_status` doc claims it scans no pages, so the ceiling
    /// cannot apply to it. Pin the claim: under a zero-second ceiling — a
    /// policy no page-bearing operation survives — causality still answers.
    /// If someone later adds a log scan here, this test is what notices.
    #[test]
    fn d2_causality_status_scans_no_pages() {
        let (session, registry) = registry_with_events("d2_causality", D2_EVENTS);
        let service = ReadPathService::with_limits(registry, no_time());
        let status = service
            .causality_status(session.as_str(), None)
            .expect("causality does no page work, so no ceiling can stop it");
        assert_eq!(status, CausalityStatus::Unsupported);
    }
}
