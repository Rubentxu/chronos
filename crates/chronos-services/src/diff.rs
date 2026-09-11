//! M5 diff & compare algorithms extracted from `chronos-mcp::server`:
//! `performance_regression_audit` and `compare_sessions`.
//!
//! The heavy lifting for `compare_sessions` is already done by
//! `chronos_store::TraceDiff::compare` (BLAKE3 hash-based set diff,
//! similarity_pct, address normalization with feature flag). This service
//! owns the *audit* algorithm (per-function call-count regression detection)
//! and the LLM-readable `summary` formatter used by both tools.
//!
//! The MCP wrappers at `crates/chronos-mcp/src/server.rs` only:
//!   1. read params + build `DiffContext`,
//!   2. dispatch to `ChronosDiffService::*`,
//!   3. map `ServiceError` back to MCP error text.

use std::collections::{HashMap, HashSet};

use chronos_query::QueryEngine;
use chronos_store::{SessionStore, StoreError, TraceDiff};

use crate::error::ServiceError;
use crate::output::{
    CompareSessionsResult, FunctionRegressionEntry, PerformanceRegressionAuditResult,
};

/// Borrowed handle to the live `SessionStore`.
///
/// The store is owned by `chronos-mcp::Server`; the service holds a reference
/// for the duration of the call. No locking past the await point.
pub struct DiffContext<'a> {
    pub store: &'a SessionStore,
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
    pub fn performance_regression_audit(
        ctx: &DiffContext<'_>,
        input: PerformanceRegressionAuditInput,
    ) -> Result<PerformanceRegressionAuditResult, ServiceError> {
        let top_n = input.top_n.unwrap_or(20);

        let (_meta_a, events_a) = ctx
            .store
            .load_session(&input.baseline_session_id)
            .map_err(|e| map_load_error(e, &input.baseline_session_id))?;
        let (_meta_b, events_b) = ctx
            .store
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
        let mut total_a: i64 = 0;
        let mut total_b: i64 = 0;

        for func in &all {
            let ca = map_a.get(func).copied().unwrap_or(0);
            let cb = map_b.get(func).copied().unwrap_or(0);
            total_a += ca as i64;
            total_b += cb as i64;

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

        let summary = if regressions.is_empty() {
            format!(
                "No significant regressions found. Target had {} total calls vs {} in baseline.",
                total_b, total_a
            )
        } else {
            let top = &regressions[0];
            format!(
                "Found {} significant regression(s). Top: '{}' increased by {:.0}%. \
                 Target: {} total calls vs baseline: {}.",
                regressions.len(),
                top.function,
                top.call_delta_pct,
                total_b,
                total_a
            )
        };

        Ok(PerformanceRegressionAuditResult {
            baseline_session_id: input.baseline_session_id,
            target_session_id: input.target_session_id,
            regressions,
            improvements,
            functions_analyzed: all.len(),
            total_call_delta: total_b - total_a,
            summary,
        })
    }

    /// Compare two saved sessions and produce a set-diff report.
    ///
    /// Delegates the heavy lifting to [`chronos_store::TraceDiff::compare`]
    /// (BLAKE3 hash-based set difference, similarity_pct, address
    /// normalization). Formats a 3-tier LLM-readable `summary`:
    ///   - `>= 90%` similar   => "Sessions are highly similar..."
    ///   - `>= 50%` similar   => "Sessions differ in N events..."
    ///   - `< 50%` similar    => "Sessions are largely different..."
    pub fn compare_sessions(
        ctx: &DiffContext<'_>,
        input: CompareSessionsInput,
    ) -> Result<CompareSessionsResult, ServiceError> {
        let (meta_a, events_a) = ctx
            .store
            .load_session(&input.session_a)
            .map_err(|e| map_load_error(e, &input.session_a))?;
        let (meta_b, events_b) = ctx
            .store
            .load_session(&input.session_b)
            .map_err(|e| map_load_error(e, &input.session_b))?;

        let report = TraceDiff::compare(
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

/// Map a `StoreError` from `load_session` to a `ServiceError`.
///
/// `SessionNotFound` is mapped to the canonical `ServiceError::SessionNotFound`
/// (preserves the session id; the MCP wrapper reconstructs the legacy literal
/// error string `"session '{}' not found: {}"`).
/// All other errors map to `ServiceError::LoadFailed`.
fn map_load_error(e: StoreError, session_id: &str) -> ServiceError {
    match e {
        StoreError::SessionNotFound(_) => ServiceError::SessionNotFound(session_id.to_string()),
        other => ServiceError::LoadFailed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
    use chronos_store::SessionMetadata;

    fn make_event(id: u64, func: &str) -> TraceEvent {
        let loc = SourceLocation::new("test.rs", 1, func, 0x1000 + id);
        TraceEvent::new(
            id,
            id * 100,
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

    fn empty_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    fn make_ctx<'a>(store: &'a SessionStore) -> DiffContext<'a> {
        DiffContext { store }
    }

    #[test]
    fn performance_regression_audit_returns_session_not_found() {
        let store = empty_store();
        let ctx = make_ctx(&store);

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
        let store = empty_store();
        // Baseline: 2 calls to "main" (id=0), 2 calls to "helper" (id=1).
        save_session(&store, "base", &["main", "main", "helper", "helper"]);
        // Target:   4 calls to "main" (id=0..3), 1 call to "helper" (id=4),
        //           1 call to "newcomer" (id=5).
        save_session(
            &store,
            "tgt",
            &["main", "main", "main", "main", "helper", "newcomer"],
        );

        let ctx = make_ctx(&store);
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

    #[test]
    fn compare_sessions_returns_session_not_found() {
        let store = empty_store();
        let ctx = make_ctx(&store);

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
        let store = empty_store();
        save_session(&store, "a", &["main", "helper"]);
        save_session(&store, "b", &["main", "helper"]);

        let ctx = make_ctx(&store);
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
        let store = empty_store();
        save_session(&store, "a", &["main", "helper", "init", "main"]);
        save_session(&store, "b", &["worker", "render", "tick"]);

        let ctx = make_ctx(&store);
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
