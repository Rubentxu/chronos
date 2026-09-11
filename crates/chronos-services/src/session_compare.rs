//! M7-03 — `session_compare` v2 dispatcher.
//!
//! Unifies the v1 `compare_sessions` (set-diff) and
//! `performance_regression_audit` (per-function regression detection)
//! behind a single v2 endpoint with a `kind` discriminator. The v1
//! names are preserved as deprecated MCP shims that route through
//! this dispatcher.
//!
//! See `docs/milestones/m7-03-session-compare-explain-merge.md` for
//! the full spec + algorithm details.

use crate::diff::{
    ChronosDiffService, CompareSessionsInput, DiffContext, PerformanceRegressionAuditInput,
};
use crate::error::ServiceError;
use crate::output::{
    SessionCompareInput, SessionCompareKind, SessionCompareOutput, SessionCompareProvenance,
};

/// Borrowed handle to the live `SessionStore`.
///
/// The store is owned by `chronos-mcp::Server`; the service holds a reference
/// for the duration of the call. No locking past the await point.
pub struct SessionCompareContext<'a> {
    pub store: &'a chronos_store::SessionStore,
}

/// Stateless holder for the v2 `session_compare` dispatcher.
pub struct ChronosSessionCompareService;

impl ChronosSessionCompareService {
    /// v2 `session_compare` dispatcher entrypoint.
    ///
    /// Routes by `kind`:
    /// - `Divergence`  → wraps `ChronosDiffService::compare_sessions`
    /// - `Regression`  → wraps `ChronosDiffService::performance_regression_audit`
    ///
    /// Each variant returns the existing v1 result shape 1:1 plus a
    /// `SessionCompareProvenance` block. The dispatcher validates
    /// that kind-gated fields are populated and rejects mismatched
    /// input with `ServiceError::InvalidInput`.
    pub fn compare(
        ctx: &SessionCompareContext<'_>,
        input: SessionCompareInput,
    ) -> Result<SessionCompareOutput, ServiceError> {
        match input.kind {
            SessionCompareKind::Divergence => Self::divergence(ctx, input),
            SessionCompareKind::Regression => Self::regression(ctx, input),
        }
    }

    fn divergence(
        ctx: &SessionCompareContext<'_>,
        input: SessionCompareInput,
    ) -> Result<SessionCompareOutput, ServiceError> {
        let session_a = input.session_a.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_compare{kind=divergence} requires `session_a`".to_string(),
            )
        })?;
        let session_b = input.session_b.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_compare{kind=divergence} requires `session_b`".to_string(),
            )
        })?;
        let result = ChronosDiffService::compare_sessions(
            &DiffContext { store: ctx.store },
            CompareSessionsInput {
                session_a,
                session_b,
            },
        )?;
        Ok(SessionCompareOutput::Divergence {
            result,
            provenance: SessionCompareProvenance {
                engine_version: "chronos-0.1.0".to_string(),
                source: "session_compare:divergence".to_string(),
            },
        })
    }

    fn regression(
        ctx: &SessionCompareContext<'_>,
        input: SessionCompareInput,
    ) -> Result<SessionCompareOutput, ServiceError> {
        let baseline_session_id = input.baseline_session_id.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_compare{kind=regression} requires `baseline_session_id`".to_string(),
            )
        })?;
        let target_session_id = input.target_session_id.ok_or_else(|| {
            ServiceError::InvalidInput(
                "session_compare{kind=regression} requires `target_session_id`".to_string(),
            )
        })?;
        let result = ChronosDiffService::performance_regression_audit(
            &DiffContext { store: ctx.store },
            PerformanceRegressionAuditInput {
                baseline_session_id,
                target_session_id,
                top_n: input.top_n,
            },
        )?;
        Ok(SessionCompareOutput::Regression {
            result,
            provenance: SessionCompareProvenance {
                engine_version: "chronos-0.1.0".to_string(),
                source: "session_compare:regression".to_string(),
            },
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
    use chronos_store::{SessionMetadata, SessionStore};

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
        };
        store.save_session(meta, &events).unwrap();
    }

    fn empty_store() -> SessionStore {
        SessionStore::in_memory().unwrap()
    }

    fn populate_two_sessions() -> (SessionStore, String, String) {
        let store = empty_store();
        save_session(&store, "sess-a", &["main", "helper", "main"]);
        save_session(&store, "sess-b", &["main", "helper", "helper"]);
        (store, "sess-a".to_string(), "sess-b".to_string())
    }

    #[test]
    fn compare_divergence_returns_divergence_variant() {
        let (store, a, b) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Divergence,
            session_a: Some(a),
            session_b: Some(b),
            baseline_session_id: None,
            target_session_id: None,
            top_n: None,
        };
        let out = ChronosSessionCompareService::compare(&ctx, input).unwrap();
        match out {
            SessionCompareOutput::Divergence { result, provenance } => {
                assert!(result.total_a > 0, "session a should have events");
                assert_eq!(provenance.source, "session_compare:divergence");
                assert!(!provenance.engine_version.is_empty());
            }
            other => panic!("expected Divergence, got {:?}", other),
        }
    }

    #[test]
    fn compare_regression_returns_regression_variant() {
        let (store, a, b) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Regression,
            session_a: None,
            session_b: None,
            baseline_session_id: Some(a),
            target_session_id: Some(b),
            top_n: Some(5),
        };
        let out = ChronosSessionCompareService::compare(&ctx, input).unwrap();
        match out {
            SessionCompareOutput::Regression {
                result: _,
                provenance,
            } => {
                assert_eq!(provenance.source, "session_compare:regression");
                assert!(!provenance.engine_version.is_empty());
            }
            other => panic!("expected Regression, got {:?}", other),
        }
    }

    #[test]
    fn compare_divergence_session_not_found() {
        let (store, a, _) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Divergence,
            session_a: Some(a),
            session_b: Some("nonexistent-session".to_string()),
            baseline_session_id: None,
            target_session_id: None,
            top_n: None,
        };
        let err = ChronosSessionCompareService::compare(&ctx, input).unwrap_err();
        assert!(matches!(err, ServiceError::SessionNotFound(_)));
    }

    #[test]
    fn compare_regression_session_not_found() {
        let (store, a, _) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Regression,
            session_a: None,
            session_b: None,
            baseline_session_id: Some(a),
            target_session_id: Some("nonexistent".to_string()),
            top_n: None,
        };
        let err = ChronosSessionCompareService::compare(&ctx, input).unwrap_err();
        assert!(matches!(err, ServiceError::SessionNotFound(_)));
    }

    #[test]
    fn compare_divergence_missing_session_a_returns_invalid_input() {
        let store = empty_store();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Divergence,
            session_a: None,
            session_b: Some("b".to_string()),
            baseline_session_id: None,
            target_session_id: None,
            top_n: None,
        };
        let err = ChronosSessionCompareService::compare(&ctx, input).unwrap_err();
        assert!(
            matches!(err, ServiceError::InvalidInput(_)),
            "expected InvalidInput, got {:?}",
            err
        );
    }

    #[test]
    fn compare_regression_missing_target_returns_invalid_input() {
        let store = empty_store();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Regression,
            session_a: None,
            session_b: None,
            baseline_session_id: Some("baseline".to_string()),
            target_session_id: None,
            top_n: None,
        };
        let err = ChronosSessionCompareService::compare(&ctx, input).unwrap_err();
        assert!(
            matches!(err, ServiceError::InvalidInput(_)),
            "expected InvalidInput, got {:?}",
            err
        );
    }

    #[test]
    fn compare_divergence_preserves_summary_string() {
        let (store, a, b) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Divergence,
            session_a: Some(a),
            session_b: Some(b),
            baseline_session_id: None,
            target_session_id: None,
            top_n: None,
        };
        let out = ChronosSessionCompareService::compare(&ctx, input).unwrap();
        if let SessionCompareOutput::Divergence { result, .. } = out {
            assert!(
                !result.summary.is_empty(),
                "summary string should be populated"
            );
        } else {
            panic!("expected Divergence");
        }
    }

    #[test]
    fn compare_regression_classifies_threshold() {
        // Baseline: 2 calls to "main". Target: 10 calls to "main" => +400% regression.
        let store = empty_store();
        save_session(&store, "b", &["main", "main"]);
        save_session(
            &store,
            "t",
            &[
                "main", "main", "main", "main", "main", "main", "main", "main", "main", "main",
            ],
        );
        let ctx = SessionCompareContext { store: &store };
        let input = SessionCompareInput {
            kind: SessionCompareKind::Regression,
            session_a: None,
            session_b: None,
            baseline_session_id: Some("b".to_string()),
            target_session_id: Some("t".to_string()),
            top_n: None,
        };
        let out = ChronosSessionCompareService::compare(&ctx, input).unwrap();
        if let SessionCompareOutput::Regression { result, .. } = out {
            assert!(
                !result.regressions.is_empty(),
                "expected at least one regression for 5x call growth"
            );
            assert_eq!(result.regressions[0].function, "main");
            assert_eq!(result.regressions[0].baseline_calls, 2);
            assert_eq!(result.regressions[0].target_calls, 10);
            assert!(result.regressions[0].call_delta_pct > 50.0);
        } else {
            panic!("expected Regression");
        }
    }

    #[test]
    fn compare_provenance_present_on_both_kinds() {
        let (store, a, b) = populate_two_sessions();
        let ctx = SessionCompareContext { store: &store };

        let div_in = SessionCompareInput {
            kind: SessionCompareKind::Divergence,
            session_a: Some(a.clone()),
            session_b: Some(b.clone()),
            baseline_session_id: None,
            target_session_id: None,
            top_n: None,
        };
        let div_out = ChronosSessionCompareService::compare(&ctx, div_in).unwrap();
        if let SessionCompareOutput::Divergence { provenance, .. } = div_out {
            assert_eq!(provenance.source, "session_compare:divergence");
        } else {
            panic!("expected Divergence");
        }

        let reg_in = SessionCompareInput {
            kind: SessionCompareKind::Regression,
            session_a: None,
            session_b: None,
            baseline_session_id: Some(a),
            target_session_id: Some(b),
            top_n: None,
        };
        let reg_out = ChronosSessionCompareService::compare(&ctx, reg_in).unwrap();
        if let SessionCompareOutput::Regression { provenance, .. } = reg_out {
            assert_eq!(provenance.source, "session_compare:regression");
        } else {
            panic!("expected Regression");
        }
    }
}
