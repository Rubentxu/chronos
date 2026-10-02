//! M5 diff & compare algorithms extracted from `chronos-mcp::server`:
//! `performance_regression_audit` and `compare_sessions`.
//!
//! `compare_sessions` consumes the [`DiffEngine`] port from
//! `chronos_domain::ports::diff` (REC-C3.5-residual-inversion R.2).
//! The composition root (`chronos_mcp::composition::default_diff_engine`)
//! supplies the concrete `Blake3DiffEngine` adapter; this service
//! holds an `Arc<dyn DiffEngine>` for the duration of the call.
//!
//! This service owns the *audit* algorithm (per-function call-count
//! regression detection) and the LLM-readable `summary` formatter used
//! by both tools.
//!
//! The MCP wrappers at `crates/chronos-mcp/src/server.rs` only:
//!   1. read params + build `DiffContext`,
//!   2. dispatch to `ChronosDiffService::*`,
//!   3. map `ServiceError` back to MCP error text.

use std::collections::{HashMap, HashSet};

use chronos_domain::ports::diff::DiffEngine;
use chronos_domain::ports::session_reader::{SessionReader, SessionReaderError};
use chronos_query::QueryEngine;

use crate::error::ServiceError;
use crate::output::{
    CompareSessionsResult, FunctionRegressionEntry, PerformanceRegressionAuditResult,
};

/// Borrowed handle to the live `SessionReader` port + the live
/// `DiffEngine` port.
///
/// Both adapters behind the ports are owned by `chronos-mcp::Server`;
/// the service holds `Arc<dyn ...>` for the duration of the call. No
/// locking past the await point. The `DiffEngine` field was added by
/// REC-C3.5-residual-inversion R.2 to close the residual edge
/// `chronos-services -> chronos-store::TraceDiff`.
pub struct DiffContext {
    pub reader: std::sync::Arc<dyn SessionReader>,
    pub engine: std::sync::Arc<dyn DiffEngine>,
}

#[derive(Debug, Clone)]
pub struct PerformanceRegressionAuditInput {
    pub baseline_session_id: String,
    pub target_session_id: String,
    pub top_n: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CompareSessionsInput {
    pub session_a: String,
    pub session_b: String,
}

/// Stateless holder for diff & compare algorithms.
pub struct ChronosDiffService;

impl ChronosDiffService {
    /// Audit two saved sessions for performance regressions.
    ///
    /// Loads both sessions, computes `execution_summary().top_functions` for
    /// each, builds per-function call-count maps (top-N), and classifies:
    ///   - `delta > +50%` => `regressions`
    ///   - `delta < -50%` => `improvements`
    ///
    /// Both lists are sorted (descending / ascending by `call_delta_pct`).
    /// A `summary` string is produced for LLM consumers.
    ///
    /// The analyzed set is bounded by the hottest functions of each session
    /// (`execution_summary` keeps 20 per session, further capped by `top_n`),
    /// so every call total reported here is scoped to that set and never
    /// covers the whole session. The `summary` names the scope for that
    /// reason.
    pub fn performance_regression_audit(
        ctx: &DiffContext,
        input: PerformanceRegressionAuditInput,
    ) -> Result<PerformanceRegressionAuditResult, ServiceError> {
        let top_n = input.top_n.unwrap_or(20);

        let (_meta_a, events_a) = ctx
            .reader
            .load_session(&input.baseline_session_id)
            .map_err(|e| map_load_error(e, &input.baseline_session_id))?;
        let (_meta_b, events_b) = ctx
            .reader
            .load_session(&input.target_session_id)
            .map_err(|e| map_load_error(e, &input.target_session_id))?;

        let engine_a = QueryEngine::new(events_a);
        let engine_b = QueryEngine::new(events_b);
        let summary_a = engine_a.execution_summary(&input.baseline_session_id);
        let summary_b = engine_b.execution_summary(&input.target_session_id);

        let map_a: HashMap<&str, u64> = summary_a
            .top_functions
            .iter()
            .take(top_n)
            .map(|f| (f.name.as_str(), f.call_count))
            .collect();
        let map_b: HashMap<&str, u64> = summary_b
            .top_functions
            .iter()
            .take(top_n)
            .map(|f| (f.name.as_str(), f.call_count))
            .collect();

        let all: HashSet<&str> = map_a.keys().chain(map_b.keys()).copied().collect();
        let mut regressions: Vec<FunctionRegressionEntry> = Vec::new();
        let mut improvements: Vec<FunctionRegressionEntry> = Vec::new();
        let mut analyzed_calls_a: i64 = 0;
        let mut analyzed_calls_b: i64 = 0;

        for func in &all {
            let ca = map_a.get(func).copied().unwrap_or(0);
            let cb = map_b.get(func).copied().unwrap_or(0);
            analyzed_calls_a += ca as i64;
            analyzed_calls_b += cb as i64;

            if ca == 0 || cb == 0 {
                continue;
            }

            let delta_pct = ((cb as f64 - ca as f64) / ca as f64) * 100.0;
            let entry = FunctionRegressionEntry {
                function: func.to_string(),
                baseline_calls: ca,
                target_calls: cb,
                call_delta_pct: (delta_pct * 100.0).round() / 100.0,
            };

            if delta_pct > 50.0 {
                regressions.push(entry);
            } else if delta_pct < -50.0 {
                improvements.push(entry);
            }
        }

        regressions.sort_by(|a, b| {
            b.call_delta_pct
                .partial_cmp(&a.call_delta_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        improvements.sort_by(|a, b| {
            a.call_delta_pct
                .partial_cmp(&b.call_delta_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // The call totals below are sums over the analyzed set only, never
        // session totals: `execution_summary` keeps just the hottest 20
        // functions per session, so a session with more distinct functions
        // has an audit view strictly narrower than the session. The summary
        // therefore names the scope explicitly instead of calling these
        // numbers "total calls", which an LLM consumer would read as the
        // whole session and could invert a triage decision.
        let scope_note = format!(
            "Across the {} functions analyzed, target had {} calls vs {} in baseline.",
            all.len(),
            analyzed_calls_b,
            analyzed_calls_a
        );
        let summary = if regressions.is_empty() {
            format!("No significant regressions found. {}", scope_note)
        } else {
            let top = &regressions[0];
            format!(
                "Found {} significant regression(s). Top: '{}' increased by {:.0}%. {}",
                regressions.len(),
                top.function,
                top.call_delta_pct,
                scope_note
            )
        };

        Ok(PerformanceRegressionAuditResult {
            baseline_session_id: input.baseline_session_id,
            target_session_id: input.target_session_id,
            regressions,
            improvements,
            functions_analyzed: all.len(),
            total_call_delta: analyzed_calls_b - analyzed_calls_a,
            summary,
        })
    }

    /// Compare two saved sessions and produce a set-diff report.
    ///
    /// Delegates the heavy lifting to the [`DiffEngine`] port via
    /// `ctx.engine` (BLAKE3 hash-based set difference,
    /// similarity_pct). The composition root
    /// (`chronos_mcp::composition::default_diff_engine`) supplies the
    /// `Blake3DiffEngine` adapter. Formats a 3-tier LLM-readable
    /// `summary`:
    ///   - `>= 90%` similar   => "Sessions are highly similar..."
    ///   - `>= 50%` similar   => "Sessions differ in N events..."
    ///   - `< 50%` similar    => "Sessions are largely different..."
    pub fn compare_sessions(
        ctx: &DiffContext,
        input: CompareSessionsInput,
    ) -> Result<CompareSessionsResult, ServiceError> {
        let (meta_a, events_a) = ctx
            .reader
            .load_session(&input.session_a)
            .map_err(|e| map_load_error(e, &input.session_a))?;
        let (meta_b, events_b) = ctx
            .reader
            .load_session(&input.session_b)
            .map_err(|e| map_load_error(e, &input.session_b))?;

        let report = ctx.engine.compare(
            &input.session_a,
            &input.session_b,
            &events_a,
            &events_b,
            &meta_a,
            &meta_b,
        );

        let summary = if report.similarity_pct >= 90.0 {
            format!(
                "Sessions are highly similar ({}%). Most events match.",
                report.similarity_pct.round()
            )
        } else if report.similarity_pct >= 50.0 {
            format!(
                "Sessions differ in {} events (only_in_b) vs {} (only_in_a). {}% similar.",
                report.only_in_b.len(),
                report.only_in_a.len(),
                report.similarity_pct.round()
            )
        } else {
            format!(
                "Sessions are largely different. {}% similar with {} events only in B and {} only in A.",
                report.similarity_pct.round(),
                report.only_in_b.len(),
                report.only_in_a.len()
            )
        };

        Ok(CompareSessionsResult {
            session_a_id: input.session_a,
            session_b_id: input.session_b,
            only_in_a_count: report.only_in_a.len(),
            only_in_b_count: report.only_in_b.len(),
            total_a: events_a.len(),
            total_b: events_b.len(),
            common_count: report.common_count,
            similarity_pct: report.similarity_pct,
            timing_delta_ms: report.timing_delta.as_ref().map(|t| t.delta_ms),
            summary,
        })
    }
}

/// Map a `SessionReaderError` from `load_session` to a `ServiceError`.
///
/// `SessionReaderError::NotFound` is mapped to the canonical
/// `ServiceError::SessionNotFound` (preserves the session id).
/// `SessionReaderError::InvalidId` is mapped to `InvalidInput` because
/// the id was rejected by the port validator.
/// All other errors map to `ServiceError::LoadFailed`.
fn map_load_error(e: SessionReaderError, session_id: &str) -> ServiceError {
    match e {
        SessionReaderError::NotFound(_) => ServiceError::SessionNotFound(session_id.to_string()),
        SessionReaderError::InvalidId(_) => {
            ServiceError::InvalidInput(format!("invalid session id: '{}'", session_id))
        }
        other => ServiceError::LoadFailed(format!("diff: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::MonotonicNs;
    use chronos_domain::{EventData, EventType, SessionMetadata, SourceLocation, TraceEvent};
    use chronos_store::SessionStore;

    fn make_event(id: u64, func: &str) -> TraceEvent {
        let loc = SourceLocation::new("test.rs", 1, func, 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(id * 100),
            1,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    fn save_session(store: &SessionStore, id: &str, funcs: &[&str]) {
        let events: Vec<TraceEvent> = funcs
            .iter()
            .enumerate()
            .map(|(i, f)| make_event(i as u64, f))
            .collect();
        let meta = SessionMetadata {
            session_id: id.to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        store.save_session(meta, &events).unwrap();
    }

    /// Empty in-memory store wrapped in `Arc`, ready to seed and
    /// hand to the adapter.
    fn empty_arc_store() -> std::sync::Arc<SessionStore> {
        std::sync::Arc::new(SessionStore::in_memory().unwrap())
    }

    /// Seed `id` with 30 distinct functions named `f00`..`f29`, function `i`
    /// being called `count_for(i)` times.
    ///
    /// 30 distinct functions is the interesting width here:
    /// `QueryEngine::execution_summary` keeps only the hottest 20, so with
    /// more than 20 distinct functions the audit numbers are necessarily a
    /// partial view and the reported figures must say so. Counts must stay
    /// distinct within a session: the summary sorts by call count only, so
    /// ties would make top-20 membership depend on `HashMap` iteration order.
    fn save_wide_session(store: &SessionStore, id: &str, count_for: impl Fn(usize) -> usize) {
        let names: Vec<String> = (0..30).map(|i| format!("f{:02}", i)).collect();
        let funcs: Vec<&str> = names
            .iter()
            .enumerate()
            .flat_map(|(i, n)| (0..count_for(i)).map(move |_| n.as_str()))
            .collect();
        save_session(store, id, &funcs);
    }

    /// Build a `DiffContext` from a pre-seeded `Arc<SessionStore>`.
    /// Use this when the test seeds sessions into a specific store
    /// before exercising the service.
    fn make_ctx_with_arc(store: std::sync::Arc<SessionStore>) -> DiffContext {
        use chronos_domain::ports::diff::DiffEngine;
        use chronos_store::diff_engine_adapter::Blake3DiffEngine;
        use chronos_store::session_reader_adapter::SessionStoreBackedSessionReader;
        let adapter = std::sync::Arc::new(SessionStoreBackedSessionReader::new(store))
            as std::sync::Arc<dyn chronos_domain::ports::session_reader::SessionReader>;
        let engine = std::sync::Arc::new(Blake3DiffEngine) as std::sync::Arc<dyn DiffEngine>;
        DiffContext {
            reader: adapter,
            engine,
        }
    }

    #[test]
    fn performance_regression_audit_returns_session_not_found() {
        let store = empty_arc_store();
        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));

        let result = ChronosDiffService::performance_regression_audit(
            &ctx,
            PerformanceRegressionAuditInput {
                baseline_session_id: "absent-a".into(),
                target_session_id: "absent-b".into(),
                top_n: None,
            },
        );

        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[test]
    fn performance_regression_audit_classifies_regressions_and_improvements() {
        let store = empty_arc_store();
        // Baseline: 2 calls to "main" (id=0), 2 calls to "helper" (id=1).
        save_session(&store, "base", &["main", "main", "helper", "helper"]);
        // Target:   4 calls to "main" (id=0..3), 1 call to "helper" (id=4),
        //           1 call to "newcomer" (id=5).
        save_session(
            &store,
            "tgt",
            &["main", "main", "main", "main", "helper", "newcomer"],
        );

        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));
        let result = ChronosDiffService::performance_regression_audit(
            &ctx,
            PerformanceRegressionAuditInput {
                baseline_session_id: "base".into(),
                target_session_id: "tgt".into(),
                top_n: Some(10),
            },
        )
        .unwrap();

        // "main" grew from 2 -> 4 = +100% => regression.
        // "helper" shrank from 2 -> 1 = -50% (boundary, not < -50) => not included.
        // "newcomer" appears only in target => skipped (boundary check).
        assert_eq!(result.functions_analyzed, 3);
        assert_eq!(result.regressions.len(), 1);
        assert_eq!(result.regressions[0].function, "main");
        assert_eq!(result.regressions[0].baseline_calls, 2);
        assert_eq!(result.regressions[0].target_calls, 4);
        assert_eq!(result.regressions[0].call_delta_pct, 100.0);
        // "helper" is exactly -50%, which is NOT < -50, so no improvement.
        assert!(result.improvements.is_empty());
        assert!(result.summary.contains("Found 1 significant regression"));
    }

    /// Regression audit over 30 distinct functions (more than the 20 the
    /// execution summary keeps): the reported figures must be scoped to the
    /// analyzed set, and the summary must say so instead of calling them
    /// session totals.
    #[test]
    fn performance_regression_audit_scopes_call_totals_to_analyzed_functions() {
        let store = empty_arc_store();
        // Baseline: f00..f29 called 30..1 times. Session total 465;
        // hottest 20 (f00..f19, counts 30..11) sum to 410.
        save_wide_session(&store, "wide-base", |i| 30 - i);
        // Target: same functions one call lighter each, f29 never called.
        // Session total 435; hottest 20 (counts 29..10) sum to 390.
        save_wide_session(&store, "wide-tgt", |i| 29usize.saturating_sub(i));

        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));
        let result = ChronosDiffService::performance_regression_audit(
            &ctx,
            PerformanceRegressionAuditInput {
                baseline_session_id: "wide-base".into(),
                target_session_id: "wide-tgt".into(),
                top_n: None,
            },
        )
        .unwrap();

        // Each session holds 30 distinct functions, yet only the hottest 20
        // survive `execution_summary`, so 20 is the whole analyzed set here.
        assert_eq!(result.functions_analyzed, 20);
        // Every function shrank by a small relative amount, so neither list
        // is populated and the summary takes the no-regression branch.
        assert!(result.regressions.is_empty());
        assert!(result.improvements.is_empty());
        // The delta covers the analyzed functions only. The session-wide
        // delta would be -30 (465 - 435); the analyzed-set delta is -20.
        assert_eq!(result.total_call_delta, -20);
        assert_ne!(result.total_call_delta, -30);
        // The summary must state the scope and must not call these numbers
        // session totals.
        assert!(result.summary.contains(
            "Across the 20 functions analyzed, target had 390 calls vs 410 in baseline."
        ));
        assert!(!result.summary.contains("total calls"));
    }

    /// Same 30-function width, but with a real regression so the second
    /// summary branch is covered: its tail carries the same scoped totals.
    #[test]
    fn performance_regression_audit_scopes_totals_in_the_regression_summary() {
        let store = empty_arc_store();
        // Baseline: f00..f29 called 30..1 times (session total 465,
        // hottest 20 = f00..f19 sum to 410).
        save_wide_session(&store, "wide-base", |i| 30 - i);
        // Target: f18 grows from 12 to 40 calls (+233%, regression), and
        // f29 grows from 1 to 5 calls but stays outside the hottest 20.
        // Session total 497; hottest 20 still f00..f19, sum to 438.
        save_wide_session(&store, "wide-tgt", |i| match i {
            18 => 40,
            29 => 5,
            _ => 30 - i,
        });

        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));
        let result = ChronosDiffService::performance_regression_audit(
            &ctx,
            PerformanceRegressionAuditInput {
                baseline_session_id: "wide-base".into(),
                target_session_id: "wide-tgt".into(),
                top_n: None,
            },
        )
        .unwrap();

        // Both sessions yield the same hottest-20 set (f00..f19).
        assert_eq!(result.functions_analyzed, 20);
        assert_eq!(result.regressions.len(), 1);
        assert_eq!(result.regressions[0].function, "f18");
        assert_eq!(result.regressions[0].call_delta_pct, 233.33);
        // f29 is invisible to the audit: it is outside both hottest-20 sets.
        assert!(result.improvements.is_empty());
        // Analyzed-set delta is +28; the session-wide delta would be +32.
        assert_eq!(result.total_call_delta, 28);
        assert_ne!(result.total_call_delta, 32);
        assert!(result.summary.contains(
            "Found 1 significant regression(s). Top: 'f18' increased by 233%. \
             Across the 20 functions analyzed, target had 438 calls vs 410 in baseline."
        ));
        assert!(!result.summary.contains("total calls"));
    }

    #[test]
    fn compare_sessions_returns_session_not_found() {
        let store = empty_arc_store();
        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));

        let result = ChronosDiffService::compare_sessions(
            &ctx,
            CompareSessionsInput {
                session_a: "absent-a".into(),
                session_b: "absent-b".into(),
            },
        );

        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    #[test]
    fn compare_sessions_identical_sessions_have_high_similarity() {
        let store = empty_arc_store();
        save_session(&store, "a", &["main", "helper"]);
        save_session(&store, "b", &["main", "helper"]);

        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));
        let result = ChronosDiffService::compare_sessions(
            &ctx,
            CompareSessionsInput {
                session_a: "a".into(),
                session_b: "b".into(),
            },
        )
        .unwrap();

        assert_eq!(result.similarity_pct, 100.0);
        assert_eq!(result.common_count, 2);
        assert_eq!(result.only_in_a_count, 0);
        assert_eq!(result.only_in_b_count, 0);
        assert!(result.summary.contains("highly similar"));
    }

    #[test]
    fn compare_sessions_disjoint_sessions_have_low_similarity() {
        let store = empty_arc_store();
        save_session(&store, "a", &["main", "helper", "init", "main"]);
        save_session(&store, "b", &["worker", "render", "tick"]);

        let ctx = make_ctx_with_arc(std::sync::Arc::clone(&store));
        let result = ChronosDiffService::compare_sessions(
            &ctx,
            CompareSessionsInput {
                session_a: "a".into(),
                session_b: "b".into(),
            },
        )
        .unwrap();

        assert_eq!(result.similarity_pct, 0.0);
        assert_eq!(result.common_count, 0);
        assert!(result.summary.contains("largely different"));
    }
}
