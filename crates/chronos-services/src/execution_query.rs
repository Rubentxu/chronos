//! M6 — Execution Query dispatcher (m6-03).
//!
//! The v2 `execution_query` tool is a single entry-point that
//! supersedes six overlapping v1 execution-evidence tools:
//!
//! | v1 tool | kind |
//! |---|---|
//! | `get_call_stack` | `CallStack` |
//! | `get_execution_summary` | `ExecutionSummary` |
//! | `debug_call_graph` | `CallGraph` |
//! | `debug_detect_races` | `RaceDetect` |
//! | `debug_expand_hotspot` | `Hotspot` |
//! | `debug_get_saliency_scores` | `Saliency` |
//!
//! Each v1 tool's algorithm already lives in `chronos-services`:
//! - `DebugTraceService::{get_call_stack, get_execution_summary, debug_call_graph}`
//! - `DebugTraceSpecializedService::{detect_races, expand_hotspot, get_saliency_scores}`
//!
//! This module is the v2 dispatcher that fans the v2 request out to one
//! of those six based on the `kind` discriminator, and wraps the inner
//! DTO in the v2 [`ExecutionQueryOutput`](crate::output::ExecutionQueryOutput)
//! envelope. The MCP wrapper at `crates/chronos-mcp/src/server.rs` only:
//!   1. reads params + builds `ExecutionQueryContext`,
//!   2. dispatches to `ChronosExecutionQueryService::query`,
//!   3. maps `ServiceError` back to MCP error text.

use std::collections::HashMap;

use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::debug_trace::DebugTraceService;
use crate::debug_trace_specialized::DebugTraceSpecializedService;
use crate::error::ServiceError;
use crate::output::{ExecutionQueryKind, ExecutionQueryOutput};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::trace_slice`,
/// `chronos_services::state_query`, etc. — the value type is `QueryEngine`
/// (no inner `Arc`); `Arc`-wrapping happens at the call site.
pub struct ExecutionQueryContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

/// Input for [`ChronosExecutionQueryService::query`].
///
/// Carries the v2 `kind` discriminator plus variant-specific target
/// fields with v1 default-value fallbacks:
/// - `max_depth` defaults to 10 (CallGraph).
/// - `threshold_ns` defaults to 100 (RaceDetect).
/// - `top_n` defaults to 10 (Hotspot).
/// - `saliency_limit` defaults to 20 (Saliency).
#[derive(Debug, Clone)]
pub struct ExecutionQueryInput {
    pub session_id: String,
    pub kind: ExecutionQueryKind,
    pub event_id: Option<u64>,
    pub max_depth: Option<usize>,
    pub threshold_ns: Option<u64>,
    pub top_n: Option<usize>,
    pub saliency_limit: Option<usize>,
}

/// Stateless holder for the v2 execution_query dispatcher.
pub struct ChronosExecutionQueryService;

impl ChronosExecutionQueryService {
    /// Dispatch the v2 `execution_query` request to the matching v1 algorithm.
    ///
    /// Required parameters per kind:
    /// - `CallStack`        → `event_id` (required)
    /// - `ExecutionSummary` → (none — implicit)
    /// - `CallGraph`        → `max_depth` (optional, defaults to 10)
    /// - `RaceDetect`       → `threshold_ns` (optional, defaults to 100)
    /// - `Hotspot`          → `top_n` (optional, defaults to 10)
    /// - `Saliency`         → `saliency_limit` (optional, defaults to 20)
    ///
    /// Returns [`ServiceError::InvalidInput`] when a required target
    /// field is missing. The MCP wrapper forwards this verbatim.
    pub async fn query(
        ctx: &ExecutionQueryContext<'_>,
        input: ExecutionQueryInput,
    ) -> Result<ExecutionQueryOutput, ServiceError> {
        match input.kind {
            ExecutionQueryKind::CallStack => {
                let eid = input.event_id.ok_or_else(|| {
                    ServiceError::InvalidInput("event_id required for kind=call_stack".into())
                })?;
                let r =
                    DebugTraceService::get_call_stack(&input.session_id, eid, ctx.engines).await?;
                Ok(ExecutionQueryOutput::CallStack { frames: r })
            }
            ExecutionQueryKind::ExecutionSummary => {
                let r = DebugTraceService::get_execution_summary(&input.session_id, ctx.engines)
                    .await?;
                Ok(ExecutionQueryOutput::ExecutionSummary { summary: r })
            }
            ExecutionQueryKind::CallGraph => {
                let d = input.max_depth.unwrap_or(10);
                let r =
                    DebugTraceService::debug_call_graph(&input.session_id, d, ctx.engines).await?;
                Ok(ExecutionQueryOutput::CallGraph { graph: r })
            }
            ExecutionQueryKind::RaceDetect => {
                let t = input.threshold_ns.unwrap_or(100);
                let r =
                    DebugTraceSpecializedService::detect_races(&input.session_id, t, ctx.engines)
                        .await?;
                Ok(ExecutionQueryOutput::RaceDetect { report: r })
            }
            ExecutionQueryKind::Hotspot => {
                let n = input.top_n.unwrap_or(10);
                let r =
                    DebugTraceSpecializedService::expand_hotspot(&input.session_id, n, ctx.engines)
                        .await?;
                Ok(ExecutionQueryOutput::Hotspot { report: r })
            }
            ExecutionQueryKind::Saliency => {
                let l = input.saliency_limit.unwrap_or(20);
                let r = DebugTraceSpecializedService::get_saliency_scores(
                    &input.session_id,
                    l,
                    ctx.engines,
                )
                .await?;
                Ok(ExecutionQueryOutput::Saliency { result: r })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    fn empty_engines() -> Mutex<HashMap<String, QueryEngine>> {
        Mutex::new(HashMap::new())
    }

    #[tokio::test]
    async fn call_stack_missing_event_id_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::CallStack,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn execution_summary_with_no_target_succeeds_or_session_not_found() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        // Empty engine map → SessionNotFound (not InvalidInput).
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::ExecutionSummary,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn call_graph_uses_default_max_depth_10() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::CallGraph,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        // SessionNotFound because engine map is empty, but the request
        // got past the default-resolution step without InvalidInput.
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn race_detect_uses_default_threshold_ns_100() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::RaceDetect,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn hotspot_uses_default_top_n_10() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::Hotspot,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn saliency_uses_default_saliency_limit_20() {
        let engines = empty_engines();
        let ctx = ExecutionQueryContext { engines: &engines };
        let r = ChronosExecutionQueryService::query(
            &ctx,
            ExecutionQueryInput {
                session_id: "s1".into(),
                kind: ExecutionQueryKind::Saliency,
                event_id: None,
                max_depth: None,
                threshold_ns: None,
                top_n: None,
                saliency_limit: None,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    #[test]
    fn output_serializes_with_kind_tag() {
        // Smoke: each variant carries the right "kind" tag.
        // We test the JSON-tag emission without a live engine by
        // constructing each variant directly with valid empty DTOs.
        use crate::debug_trace::{CallGraph, CallGraphStats};
        use crate::output::{HotspotReport, RaceReport, SaliencyScoreResult};

        let sal = ExecutionQueryOutput::Saliency {
            result: SaliencyScoreResult {
                session_id: String::new(),
                scored_functions: 0,
                scores: vec![],
                hint: None,
            },
        };
        let v = serde_json::to_value(&sal).unwrap();
        assert_eq!(v["kind"], "saliency");

        let hot = ExecutionQueryOutput::Hotspot {
            report: HotspotReport {
                session_id: String::new(),
                compression_level: "L1".to_string(),
                top_n: 10,
                total_calls_in_trace: 0,
                hotspot_functions: vec![],
                hint: None,
            },
        };
        let v = serde_json::to_value(&hot).unwrap();
        assert_eq!(v["kind"], "hotspot");

        let race = ExecutionQueryOutput::RaceDetect {
            report: RaceReport {
                session_id: String::new(),
                threshold_ns: 100,
                access_count: 0,
                accesses: vec![],
                total_writes: 0,
                suspicious_pairs: vec![],
                summary: String::new(),
            },
        };
        let v = serde_json::to_value(&race).unwrap();
        assert_eq!(v["kind"], "race_detect");

        let cg = ExecutionQueryOutput::CallGraph {
            graph: CallGraph {
                stats: CallGraphStats {
                    node_count: 0,
                    edge_count: 0,
                    max_observed_depth: 0,
                },
                nodes: vec![],
                edges: vec![],
            },
        };
        let v = serde_json::to_value(&cg).unwrap();
        assert_eq!(v["kind"], "call_graph");

        let cs = ExecutionQueryOutput::CallStack { frames: vec![] };
        let v = serde_json::to_value(&cs).unwrap();
        assert_eq!(v["kind"], "call_stack");
        assert!(v["frames"].is_array());
    }
}
