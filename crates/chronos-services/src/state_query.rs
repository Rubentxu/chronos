//! M6 — State Query dispatcher (m6-02).
//!
//! The v2 `state_query` tool is a single entry-point that supersedes
//! five overlapping v1 state-evidence tools:
//!
//! | v1 tool | kind |
//! |---|---|
//! | `state_diff` | `RegisterDiff` |
//! | `debug_get_memory` | `MemoryRead` |
//! | `debug_get_registers` | `RegisterSnapshot` |
//! | `debug_analyze_memory` | `MemoryAnalysis` |
//! | `evaluate_expression` | `ExpressionEval` |
//!
//! Each v1 tool's algorithm already lives in `chronos-services`:
//! - `DebugTraceService::state_diff`
//! - `DebugReadService::{get_memory, get_registers, analyze_memory, evaluate_expression}`
//!
//! This module is the v2 dispatcher that fans the v2 request out to one
//! of those five based on the `kind` discriminator, and wraps the inner
//! DTO in the v2 [`StateQueryOutput`](crate::output::StateQueryOutput)
//! envelope. The MCP wrapper at `crates/chronos-mcp/src/server.rs` only:
//!   1. reads params + builds `StateQueryContext`,
//!   2. dispatches to `ChronosStateQueryService::query`,
//!   3. maps `ServiceError` back to MCP error text.

use std::collections::HashMap;

use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::debug_read::DebugReadService;
use crate::debug_trace::DebugTraceService;
use crate::error::ServiceError;
use crate::output::{StateQueryKind, StateQueryOutput};

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::trace_slice`,
/// `chronos_services::sessions`, etc. — the value type is `QueryEngine`
/// (no inner `Arc`); `Arc`-wrapping happens at the call site.
pub struct StateQueryContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

/// Input for [`ChronosStateQueryService::query`].
///
/// Carries the v2 `kind` discriminator plus the variant-specific target
/// fields. The dispatcher validates that the target fields are present
/// when required by the kind.
#[derive(Debug, Clone)]
pub struct StateQueryInput {
    pub session_id: String,
    pub kind: StateQueryKind,
    pub timestamp_a: Option<u64>,
    pub timestamp_b: Option<u64>,
    pub event_id: Option<u64>,
    pub address: Option<u64>,
    pub timestamp_ns: Option<u64>,
    pub start_address: Option<u64>,
    pub end_address: Option<u64>,
    pub start_ts: Option<u64>,
    pub end_ts: Option<u64>,
    pub expression: Option<String>,
}

/// Stateless holder for the v2 state_query dispatcher.
///
/// The method is `async` only to acquire the engine-map lock; it
/// releases it as soon as the engine reference is in scope.
pub struct ChronosStateQueryService;

impl ChronosStateQueryService {
    /// Dispatch the v2 `state_query` request to the matching v1 algorithm.
    ///
    /// Required parameters per kind:
    /// - `RegisterDiff`     → `timestamp_a`, `timestamp_b`
    /// - `MemoryRead`       → `address`, `timestamp_ns`
    /// - `RegisterSnapshot` → `event_id`
    /// - `MemoryAnalysis`   → `start_address`, `end_address`, `start_ts`, `end_ts`
    /// - `ExpressionEval`   → `event_id`, `expression`
    ///
    /// Returns [`ServiceError::InvalidInput`] when a required target field
    /// is missing. The MCP wrapper forwards this verbatim to the agent.
    pub async fn query(
        ctx: &StateQueryContext<'_>,
        input: StateQueryInput,
    ) -> Result<StateQueryOutput, ServiceError> {
        match input.kind {
            StateQueryKind::RegisterDiff => {
                let ta = input.timestamp_a.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "timestamp_a required for kind=register_diff".into(),
                    )
                })?;
                let tb = input.timestamp_b.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "timestamp_b required for kind=register_diff".into(),
                    )
                })?;
                let r =
                    DebugTraceService::state_diff(&input.session_id, ta, tb, ctx.engines).await?;
                Ok(StateQueryOutput::RegisterDiff { result: r })
            }
            StateQueryKind::MemoryRead => {
                let addr = input.address.ok_or_else(|| {
                    ServiceError::InvalidInput("address required for kind=memory_read".into())
                })?;
                let ts = input.timestamp_ns.ok_or_else(|| {
                    ServiceError::InvalidInput("timestamp_ns required for kind=memory_read".into())
                })?;
                let r =
                    DebugReadService::get_memory(&input.session_id, addr, ts, ctx.engines).await?;
                Ok(StateQueryOutput::MemoryRead { result: r })
            }
            StateQueryKind::RegisterSnapshot => {
                let eid = input.event_id.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "event_id required for kind=register_snapshot".into(),
                    )
                })?;
                let r = DebugReadService::get_registers(&input.session_id, eid, ctx.engines).await?;
                Ok(StateQueryOutput::RegisterSnapshot { result: r })
            }
            StateQueryKind::MemoryAnalysis => {
                let sa = input.start_address.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "start_address required for kind=memory_analysis".into(),
                    )
                })?;
                let ea = input.end_address.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "end_address required for kind=memory_analysis".into(),
                    )
                })?;
                let st = input.start_ts.ok_or_else(|| {
                    ServiceError::InvalidInput("start_ts required for kind=memory_analysis".into())
                })?;
                let et = input.end_ts.ok_or_else(|| {
                    ServiceError::InvalidInput("end_ts required for kind=memory_analysis".into())
                })?;
                let r = DebugReadService::analyze_memory(
                    &input.session_id,
                    sa,
                    ea,
                    st,
                    et,
                    ctx.engines,
                )
                .await?;
                Ok(StateQueryOutput::MemoryAnalysis { result: r })
            }
            StateQueryKind::ExpressionEval => {
                let eid = input.event_id.ok_or_else(|| {
                    ServiceError::InvalidInput("event_id required for kind=expression_eval".into())
                })?;
                let expr = input.expression.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "expression required for kind=expression_eval".into(),
                    )
                })?;
                let r = DebugReadService::evaluate_expression(
                    &input.session_id,
                    eid,
                    &expr,
                    ctx.engines,
                )
                .await?;
                Ok(StateQueryOutput::ExpressionEval { result: r })
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
    async fn register_diff_missing_timestamp_a_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::RegisterDiff,
                timestamp_a: None,
                timestamp_b: Some(200),
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn register_diff_missing_timestamp_b_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::RegisterDiff,
                timestamp_a: Some(100),
                timestamp_b: None,
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn memory_read_missing_address_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::MemoryRead,
                address: None,
                timestamp_ns: Some(100),
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn register_snapshot_missing_event_id_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::RegisterSnapshot,
                event_id: None,
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn memory_analysis_missing_fields_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        // Missing end_ts.
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::MemoryAnalysis,
                start_address: Some(0),
                end_address: Some(100),
                start_ts: Some(100),
                end_ts: None,
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn expression_eval_missing_expression_returns_invalid_input() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "s1".into(),
                kind: StateQueryKind::ExpressionEval,
                event_id: Some(1),
                expression: None,
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn session_not_found_propagates() {
        let engines = empty_engines();
        let ctx = StateQueryContext { engines: &engines };
        // MemoryRead with all fields valid but no session in engine map.
        let r = ChronosStateQueryService::query(
            &ctx,
            StateQueryInput {
                session_id: "missing".into(),
                kind: StateQueryKind::MemoryRead,
                address: Some(0x1000),
                timestamp_ns: Some(100),
                ..base_minimal()
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    /// Returns a StateQueryInput with only `session_id` + `kind` set;
    /// each test fills the variant-specific optional fields it cares about.
    fn base_minimal() -> StateQueryInput {
        StateQueryInput {
            session_id: String::new(),
            kind: StateQueryKind::RegisterDiff,
            timestamp_a: None,
            timestamp_b: None,
            event_id: None,
            address: None,
            timestamp_ns: None,
            start_address: None,
            end_address: None,
            start_ts: None,
            end_ts: None,
            expression: None,
        }
    }
}
