//! M6 — Trace Slice dispatcher (m6-01).
//!
//! The v2 `trace_slice` tool is a single entry-point that supersedes four
//! overlapping v1 tools:
//!
//! | v1 tool | slice_kind |
//! |---|---|
//! | `debug_find_variable_origin` | `VariableOrigin` |
//! | `debug_find_crash` | `Crash` |
//! | `inspect_causality` | `Causality` |
//! | `forensic_memory_audit` | `MemoryAudit` |
//!
//! Each v1 tool's algorithm already lives in `chronos-services`:
//! - `DebugTraceSpecializedService::{find_variable_origin, find_crash, inspect_causality}`
//! - `DebugReadService::forensic_audit`
//!
//! This module is the v2 dispatcher that fans the v2 request out to one of
//! those four based on the `slice_kind` discriminator, and wraps the inner
//! DTO in the v2 [`TraceSliceOutput`](crate::output::TraceSliceOutput)
//! envelope. The MCP wrapper at `crates/chronos-mcp/src/server.rs` only:
//!   1. reads params + builds `TraceSliceContext`,
//!   2. dispatches to `ChronosTraceSliceService::slice`,
//!   3. maps `ServiceError` back to MCP error text.

use std::collections::HashMap;

use chronos_query::QueryEngine;
use tokio::sync::Mutex as TokioMutex;

use crate::debug_read::DebugReadService;
use crate::debug_trace_specialized::DebugTraceSpecializedService;
use crate::error::ServiceError;
use crate::output::TraceSliceKind;
use crate::output::TraceSliceOutput;

/// Borrowed handle to the live engine map (shared with the MCP server).
///
/// Matches the established pattern in `chronos_services::sessions`,
/// `chronos_services::debug_trace`, `chronos_services::analysis` —
/// the value type is `QueryEngine` (no inner `Arc`); `Arc`-wrapping
/// happens at the call site.
pub struct TraceSliceContext<'a> {
    pub engines: &'a TokioMutex<HashMap<String, QueryEngine>>,
}

/// Input for [`ChronosTraceSliceService::slice`].
///
/// Carries the v2 `slice_kind` discriminator plus the variant-specific
/// target fields. The dispatcher validates that the target field is
/// present when required by the slice kind.
#[derive(Debug, Clone)]
pub struct TraceSliceInput {
    pub session_id: String,
    pub slice_kind: TraceSliceKind,
    pub variable_name: Option<String>,
    pub address: Option<u64>,
    pub limit: usize,
}

/// Stateless holder for the v2 trace_slice dispatcher.
///
/// The method is `async` only to acquire the engine-map lock; it releases
/// it as soon as the engine reference is in scope. Callers must drop the
/// returned outputs before any subsequent call that needs the map.
pub struct ChronosTraceSliceService;

impl ChronosTraceSliceService {
    /// Dispatch the v2 `trace_slice` request to the matching v1 algorithm.
    ///
    /// Required parameters per slice_kind:
    /// - `VariableOrigin` → `variable_name` (required)
    /// - `Crash`          → (none — implicit target is "the crash")
    /// - `Causality`      → `address` (required)
    /// - `MemoryAudit`    → `address` (required)
    ///
    /// Returns [`ServiceError::InvalidInput`] when a required target field
    /// is missing. The MCP wrapper forwards this verbatim to the agent.
    pub async fn slice(
        ctx: &TraceSliceContext<'_>,
        input: TraceSliceInput,
    ) -> Result<TraceSliceOutput, ServiceError> {
        match input.slice_kind {
            TraceSliceKind::VariableOrigin => {
                let name = input.variable_name.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "variable_name required for slice_kind=variable_origin".into(),
                    )
                })?;
                let r = DebugTraceSpecializedService::find_variable_origin(
                    &input.session_id,
                    &name,
                    input.limit,
                    ctx.engines,
                )
                .await?;
                Ok(TraceSliceOutput::VariableOrigin {
                    session_id: input.session_id,
                    result: r,
                })
            }
            TraceSliceKind::Crash => {
                let r = DebugTraceSpecializedService::find_crash(&input.session_id, ctx.engines)
                    .await?;
                Ok(TraceSliceOutput::Crash {
                    session_id: input.session_id,
                    result: r,
                })
            }
            TraceSliceKind::Causality => {
                let addr = input.address.ok_or_else(|| {
                    ServiceError::InvalidInput("address required for slice_kind=causality".into())
                })?;
                let r = DebugTraceSpecializedService::inspect_causality(
                    &input.session_id,
                    addr,
                    input.limit,
                    ctx.engines,
                )
                .await?;
                Ok(TraceSliceOutput::Causality {
                    session_id: input.session_id,
                    result: r,
                })
            }
            TraceSliceKind::MemoryAudit => {
                let addr = input.address.ok_or_else(|| {
                    ServiceError::InvalidInput(
                        "address required for slice_kind=memory_audit".into(),
                    )
                })?;
                let r = DebugReadService::forensic_audit(
                    &input.session_id,
                    addr,
                    input.limit,
                    ctx.engines,
                )
                .await?;
                Ok(TraceSliceOutput::MemoryAudit {
                    session_id: input.session_id,
                    result: r,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{
        trace::{EventData, EventType, SourceLocation, TraceEvent},
        EventType as _,
    };
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    fn make_engine(events: Vec<TraceEvent>) -> QueryEngine {
        QueryEngine::new(events)
    }

    fn engines_map(
        session_id: &str,
        events: Vec<TraceEvent>,
    ) -> Mutex<HashMap<String, QueryEngine>> {
        let mut map = HashMap::new();
        map.insert(session_id.to_string(), make_engine(events));
        Mutex::new(map)
    }

    fn var_event(event_id: u64, ts: u64, tid: u64, name: &str, value: &str) -> TraceEvent {
        TraceEvent {
            event_id,
            timestamp_ns: ts,
            thread_id: tid,
            event_type: EventType::VariableWrite,
            location: SourceLocation::default(),
            data: EventData::Variable(chronos_domain::VariableInfo {
                name: name.to_string(),
                value: value.to_string(),
                type_name: "i32".to_string(),
                address: 0,
                scope: chronos_domain::VariableScope::Local,
            }),
        }
    }

    fn signal_event(event_id: u64, ts: u64, tid: u64, signum: i32, name: &str) -> TraceEvent {
        TraceEvent {
            event_id,
            timestamp_ns: ts,
            thread_id: tid,
            event_type: EventType::SignalDelivered,
            location: SourceLocation::default(),
            data: EventData::Signal {
                signal_number: signum,
                signal_name: name.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn slice_variable_origin_ok() {
        let engines = engines_map(
            "s1",
            vec![
                var_event(1, 100, 1, "x", "1"),
                var_event(2, 200, 1, "x", "2"),
            ],
        );
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::VariableOrigin,
                variable_name: Some("x".into()),
                address: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
        match r {
            TraceSliceOutput::VariableOrigin { session_id, result } => {
                assert_eq!(session_id, "s1");
                assert_eq!(result.variable_name, "x");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn slice_variable_origin_missing_name_returns_invalid_input() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::VariableOrigin,
                variable_name: None,
                address: None,
                limit: 10,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn slice_crash_ok() {
        let engines = engines_map("s1", vec![signal_event(1, 100, 1, 11, "SIGSEGV")]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::Crash,
                variable_name: None,
                address: None,
                limit: 0,
            },
        )
        .await
        .unwrap();
        match r {
            TraceSliceOutput::Crash { result, .. } => {
                assert!(result.crash_found);
                assert_eq!(result.signal, "SIGSEGV");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn slice_causality_ok() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::Causality,
                variable_name: None,
                address: Some(0x1000),
                limit: 10,
            },
        )
        .await
        .unwrap();
        match r {
            TraceSliceOutput::Causality { result, .. } => {
                assert_eq!(result.address, 0x1000);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn slice_causality_missing_address_returns_invalid_input() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::Causality,
                variable_name: None,
                address: None,
                limit: 10,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn slice_memory_audit_ok() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::MemoryAudit,
                variable_name: None,
                address: Some(0x2000),
                limit: 10,
            },
        )
        .await
        .unwrap();
        match r {
            TraceSliceOutput::MemoryAudit { result, .. } => {
                assert_eq!(result.address, "0x2000");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn slice_memory_audit_missing_address_returns_invalid_input() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "s1".into(),
                slice_kind: TraceSliceKind::MemoryAudit,
                variable_name: None,
                address: None,
                limit: 10,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn slice_session_not_found() {
        let engines = engines_map("s1", vec![]);
        let ctx = TraceSliceContext { engines: &engines };
        let r = ChronosTraceSliceService::slice(
            &ctx,
            TraceSliceInput {
                session_id: "missing".into(),
                slice_kind: TraceSliceKind::Crash,
                variable_name: None,
                address: None,
                limit: 0,
            },
        )
        .await;
        assert!(matches!(r, Err(ServiceError::SessionNotFound(_))));
    }

    // Touch EventType import to silence unused warning if no test uses it
    #[allow(dead_code)]
    fn _touch_event_type(_e: EventType) {}
}
