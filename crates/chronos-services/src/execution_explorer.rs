//! M10.2 — Execution Explorer wire shape unified + permissions.
//!
//! ## Why this exists
//!
//! ADR-0029 §2.3 (M10.2) consolidates 3 separate read paths in `chronos-services`:
//!
//! - `ChronosEventsReadService::read_page` (events_cursor.rs:386 + events_log_read.rs:1507)
//! - `ChronosEventsReadService::read_canonical_drain_page` (canonical_drain.rs:737)
//! - `ChronosDebugReadService::*` (debug_read.rs:606, 7 read methods)
//!
//! into a single tool-agnostic unified contract that the MCP wrapper
//! `chronos-mcp::execution_explorer` will dispatch to. This module is the
//! **algorithm + permission model**; the MCP wrapper is a thin shim.
//!
//! ROADMAP §M10 §95: "Execution Explorer (M10) — read paths unificados +
//! permisos por operación + live streaming + virtualization + REC-C1/REC-C2
//! regression + UAT-M10-01/02. Consolidar 3 read paths en 1 contrato MCP".
//!
//! ## Permissions model
//!
//! Per ADR-0029 §3.3 (rejected ABAC) + ADR-0004 (no Silent Lies):
//!
//! - **Deny-by-default**: every request must declare its `Permissions`.
//!   Missing or empty permission → `PermissionDenied`.
//! - **Coarse-grained**: `ReadEvents` / `ReadDebug` / `ReadCompare`.
//!   Multi-tenant ABAC is scope futuro (M11+).
//! - **Honest**: if a request declares `ReadEvents` but the requested
//!   action needs `ReadDebug`, the service returns `PermissionDenied`
//!   with explicit reason — NOT silent truncation.
//!
//! ## What this is NOT
//!
//! - **NOT** a replacement for the existing read services (per ADR-0029
//!   §3.2). They are the underlying implementations.
//! - **NOT** a wire protocol implementation (JSON Schema is M10.5).
//! - **NOT** a streaming/aggregation layer (M10.3 / M10.4).
//! - **NOT** a multi-tenant permission model (M11+).
//!
//! ## Out-of-scope (per ADR-0029 §4)
//!
//! - Live streaming (M10.3).
//! - Causality wiring (M10.3 + M9 dependency; "Unsupported until M9 certified").
//! - Virtualization (M10.4).
//! - JSON Schema wire validation (M10.5).
//! - REC-C1/REC-C2 regression suite execution (M10.4 — these services are
//!   pre-existente foundation; M10.2 just wires them under one contract).
//! - UAT-M10-01/02 executors (M10.5).
//! - Multi-tenant ABAC (M11+).

use serde::{Deserialize, Serialize};

/// Coarse-grained permissions for Execution Explorer operations.
///
/// Per ADR-0029 §3.3 + ADR-0004: deny-by-default, coarse, honest.
/// Multi-tenant ABAC is out-of-scope (M11+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read events (canonical, cursor-paginated).
    ReadEvents,
    /// Read debug state (memory, registers, expressions).
    ReadDebug,
    /// Read session comparison + regression audit.
    ReadCompare,
}

impl Permission {
    /// Human-readable label for diagnostics / error messages.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ReadEvents => "read_events",
            Self::ReadDebug => "read_debug",
            Self::ReadCompare => "read_compare",
        }
    }
}

/// Set of permissions granted to a requester. The `Permissions::empty()`
/// variant is intentionally rejected by `check_required()` — deny-by-default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", transparent)]
pub struct Permissions {
    bits: u8,
}

impl Permissions {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_permissions(perms: &[Permission]) -> Self {
        let mut bits = 0u8;
        let mut i = 0;
        while i < perms.len() {
            bits |= match perms[i] {
                Permission::ReadEvents => 0b0000_0001,
                Permission::ReadDebug => 0b0000_0010,
                Permission::ReadCompare => 0b0000_0100,
            };
            i += 1;
        }
        Self { bits }
    }

    /// Check whether this permission set grants the required permission.
    pub fn grants(&self, required: Permission) -> bool {
        let bit = match required {
            Permission::ReadEvents => 0b0000_0001,
            Permission::ReadDebug => 0b0000_0010,
            Permission::ReadCompare => 0b0000_0100,
        };
        (self.bits & bit) != 0
    }

    /// Returns `Ok(())` if the requester holds all `required` permissions,
    /// or `Err(PermissionDenied)` with explicit reason if not.
    ///
    /// Deny-by-default: empty `required` always passes; empty `granted` with
    /// non-empty `required` always denies.
    pub fn check_required(
        granted: Permissions,
        required: &[Permission],
    ) -> Result<(), PermissionDenied> {
        for &perm in required {
            if !granted.grants(perm) {
                let labels: Vec<String> = required.iter().map(|p| p.label().to_string()).collect();
                return Err(PermissionDenied {
                    missing: perm,
                    granted_bits: granted.bits,
                    required: labels,
                });
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Returns the permissions present in this set, in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = Permission> + '_ {
        let mut result = Vec::new();
        if (self.bits & 0b0000_0001) != 0 {
            result.push(Permission::ReadEvents);
        }
        if (self.bits & 0b0000_0010) != 0 {
            result.push(Permission::ReadDebug);
        }
        if (self.bits & 0b0000_0100) != 0 {
            result.push(Permission::ReadCompare);
        }
        result.into_iter()
    }
}

/// Returned when a request lacks the permission required for an action.
///
/// Per ADR-0004 (no Silent Lies): the response carries explicit
/// `missing` permission, the `granted_bits`, and the full required set
/// so the caller (MCP layer, CI script, doc generator) can produce a
/// useful diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PermissionDenied {
    pub missing: Permission,
    pub granted_bits: u8,
    pub required: Vec<String>,
}

/// Action kind for an Execution Explorer request. The wire shape (MCP
/// v3 tool name + params) maps to one of these variants. Each variant
/// declares the permission required to perform it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionExplorerAction {
    /// Read events with cursor pagination. Requires `ReadEvents`.
    ReadEvents,
    /// Read memory snapshot. Requires `ReadDebug`.
    ReadMemory,
    /// Read register snapshot. Requires `ReadDebug`.
    ReadRegisters,
    /// Evaluate expression in session context. Requires `ReadDebug`.
    EvaluateExpression,
    /// Read session comparison / divergence / regression audit. Requires `ReadCompare`.
    ReadCompare,
}

impl ExecutionExplorerAction {
    /// Permissions required to perform this action.
    pub fn required_permissions(&self) -> &'static [Permission] {
        match self {
            Self::ReadEvents => &[Permission::ReadEvents],
            Self::ReadMemory => &[Permission::ReadDebug],
            Self::ReadRegisters => &[Permission::ReadDebug],
            Self::EvaluateExpression => &[Permission::ReadDebug],
            Self::ReadCompare => &[Permission::ReadCompare],
        }
    }

    /// Action label (snake_case for JSON Schema / logs).
    pub fn label(&self) -> &'static str {
        match self {
            Self::ReadEvents => "read_events",
            Self::ReadMemory => "read_memory",
            Self::ReadRegisters => "read_registers",
            Self::EvaluateExpression => "evaluate_expression",
            Self::ReadCompare => "read_compare",
        }
    }
}

/// Unified Execution Explorer request — the wire shape that consolidates
/// 3 read paths under 1 contract.
///
/// This struct is the **v3 unified contract**. The v1 (`query_events` /
/// `get_event` / `compare_sessions` / `performance_regression_audit` /
/// `get_memory` / `get_registers` / `evaluate_expression`) tools remain
/// as deprecated MCP shims that route through this service (per M7-01 +
/// M7-03 dispatcher pattern).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExecutionExplorerRequest {
    /// Action kind.
    pub action: ExecutionExplorerAction,
    /// Permissions granted to the caller (from auth context).
    pub granted_permissions: Permissions,
    /// Optional session identifier (None = current session).
    pub session_id: Option<String>,
    /// Optional cursor (for `ReadEvents`).
    pub cursor: Option<String>,
    /// Optional limit (for `ReadEvents`).
    pub limit: Option<usize>,
}

/// Result of an Execution Explorer request. Permission denials are
/// returned as `Err(PermissionDenied)`; success returns `Ok(Ok(...))`.
///
/// We use a double-Result to distinguish between:
///
/// - `Ok(Ok(payload))` — action permitted + executed
/// - `Ok(Err(denial))` — action denied at permission check
/// - `Err(io_error)` — action permitted but execution failed
///
/// This separation makes it easy for the MCP wrapper to map denial to
/// the appropriate JSON-RPC error code (per ROADMAP §MCP error model).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionExplorerResult {
    /// Payload returned by the underlying read service.
    /// Wrapped as `String` here because the actual payload shape
    /// depends on the action (cursor page / memory snapshot / register
    /// snapshot / expression eval result / compare result). M10.5
    /// will define the JSON Schema per action.
    Payload(String),
    /// Permission denied — never returns data, never silently truncates.
    Denied(PermissionDenied),
}

/// Unified service entry point. Each method checks permissions
/// deny-by-default before delegating to the underlying service.
///
/// **NOTE**: In M10.2, the underlying service calls are STUBS that
/// return `Payload("<stub:M10.2 — actual delegation in M10.3/M10.4>")`.
/// The full dispatch logic lands in M10.3 (live streaming) and M10.4
/// (virtualization + REC regression). M10.2 delivers the **permission
/// model + wire shape contract** that those sub-cycles plug into.
pub struct ExecutionExplorerService;

impl ExecutionExplorerService {
    pub fn new() -> Self {
        Self
    }

    /// Process an Execution Explorer request. Performs deny-by-default
    /// permission check before any other work.
    pub fn process(
        &self,
        req: &ExecutionExplorerRequest,
    ) -> Result<ExecutionExplorerResult, ExecutionExplorerError> {
        // Step 1: deny-by-default permission check.
        let required = req.action.required_permissions();
        if let Err(denied) = Permissions::check_required(req.granted_permissions, required) {
            return Ok(ExecutionExplorerResult::Denied(denied));
        }

        // Step 2: dispatch (stub for M10.2; M10.3/M10.4 implement actual
        // delegation to events_cursor / debug_read / session_compare).
        Ok(ExecutionExplorerResult::Payload(format!(
            "stub:action={} session_id={:?} cursor={:?} limit={:?}",
            req.action.label(),
            req.session_id,
            req.cursor,
            req.limit,
        )))
    }
}

impl Default for ExecutionExplorerService {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level error for `ExecutionExplorerService::process`. Permission
/// denials are NOT errors (they are `Ok(Denied(...))`); this enum is
/// reserved for unexpected dispatch failures (M10.3+ scope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionExplorerError {
    /// Dispatch failed (placeholder for M10.3+).
    DispatchFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_labels_distinct() {
        assert_eq!(Permission::ReadEvents.label(), "read_events");
        assert_eq!(Permission::ReadDebug.label(), "read_debug");
        assert_eq!(Permission::ReadCompare.label(), "read_compare");
    }

    #[test]
    fn permissions_empty_has_no_bits() {
        let p = Permissions::empty();
        assert!(p.is_empty());
        assert!(!p.grants(Permission::ReadEvents));
        assert!(!p.grants(Permission::ReadDebug));
        assert!(!p.grants(Permission::ReadCompare));
    }

    #[test]
    fn permissions_from_slice_preserves_all_bits() {
        let p = Permissions::from_permissions(&[
            Permission::ReadEvents,
            Permission::ReadDebug,
            Permission::ReadCompare,
        ]);
        assert!(p.grants(Permission::ReadEvents));
        assert!(p.grants(Permission::ReadDebug));
        assert!(p.grants(Permission::ReadCompare));
    }

    #[test]
    fn permissions_from_single_bit() {
        let p = Permissions::from_permissions(&[Permission::ReadEvents]);
        assert!(p.grants(Permission::ReadEvents));
        assert!(!p.grants(Permission::ReadDebug));
        assert!(!p.grants(Permission::ReadCompare));
    }

    #[test]
    fn deny_by_default_empty_required_passes() {
        let granted = Permissions::empty();
        let required: &[Permission] = &[];
        assert!(Permissions::check_required(granted, required).is_ok());
    }

    #[test]
    fn deny_by_default_empty_granted_with_required_denies() {
        let granted = Permissions::empty();
        let required = &[Permission::ReadEvents];
        let result = Permissions::check_required(granted, required);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.missing, Permission::ReadEvents);
        assert_eq!(err.granted_bits, 0);
    }

    #[test]
    fn deny_by_default_wrong_permission_denies() {
        // Granted ReadEvents, requested ReadDebug → denied.
        let granted = Permissions::from_permissions(&[Permission::ReadEvents]);
        let required = &[Permission::ReadDebug];
        let result = Permissions::check_required(granted, required);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().missing, Permission::ReadDebug);
    }

    #[test]
    fn read_events_action_requires_read_events() {
        let req = ExecutionExplorerRequest {
            action: ExecutionExplorerAction::ReadEvents,
            granted_permissions: Permissions::from_permissions(&[Permission::ReadEvents]),
            session_id: None,
            cursor: None,
            limit: None,
        };
        let svc = ExecutionExplorerService::new();
        let result = svc.process(&req).expect("dispatch ok");
        match result {
            ExecutionExplorerResult::Payload(p) => {
                assert!(p.contains("read_events"));
            }
            _ => panic!("expected Payload, got {:?}", result),
        }
    }

    #[test]
    fn read_memory_without_read_debug_is_denied() {
        let req = ExecutionExplorerRequest {
            action: ExecutionExplorerAction::ReadMemory,
            granted_permissions: Permissions::from_permissions(&[Permission::ReadEvents]),
            session_id: Some("session-1".to_string()),
            cursor: None,
            limit: None,
        };
        let svc = ExecutionExplorerService::new();
        let result = svc.process(&req).expect("dispatch ok (denial is Ok)");
        match result {
            ExecutionExplorerResult::Denied(denied) => {
                assert_eq!(denied.missing, Permission::ReadDebug);
            }
            _ => panic!("expected Denied, got {:?}", result),
        }
    }

    #[test]
    fn read_memory_with_read_debug_is_permitted() {
        let req = ExecutionExplorerRequest {
            action: ExecutionExplorerAction::ReadMemory,
            granted_permissions: Permissions::from_permissions(&[Permission::ReadDebug]),
            session_id: Some("session-1".to_string()),
            cursor: None,
            limit: None,
        };
        let svc = ExecutionExplorerService::new();
        let result = svc.process(&req).expect("dispatch ok");
        assert!(matches!(result, ExecutionExplorerResult::Payload(_)));
    }

    #[test]
    fn read_compare_requires_read_compare() {
        let req = ExecutionExplorerRequest {
            action: ExecutionExplorerAction::ReadCompare,
            granted_permissions: Permissions::from_permissions(&[
                Permission::ReadEvents,
                Permission::ReadDebug,
            ]),
            session_id: None,
            cursor: None,
            limit: None,
        };
        let svc = ExecutionExplorerService::new();
        let result = svc.process(&req).expect("dispatch ok");
        // Even with ReadEvents + ReadDebug, ReadCompare is denied.
        assert!(matches!(result, ExecutionExplorerResult::Denied(_)));
    }

    #[test]
    fn request_serde_round_trip() {
        let req = ExecutionExplorerRequest {
            action: ExecutionExplorerAction::ReadEvents,
            granted_permissions: Permissions::from_permissions(&[Permission::ReadEvents]),
            session_id: Some("session-1".to_string()),
            cursor: Some("cursor-abc".to_string()),
            limit: Some(100),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let parsed: ExecutionExplorerRequest =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, req);
    }

    #[test]
    fn action_required_permissions_table() {
        // Every action declares its required permissions; verify table
        // is consistent (no action accidentally has empty requirements,
        // which would be a deny-by-default bypass).
        for action in [
            ExecutionExplorerAction::ReadEvents,
            ExecutionExplorerAction::ReadMemory,
            ExecutionExplorerAction::ReadRegisters,
            ExecutionExplorerAction::EvaluateExpression,
            ExecutionExplorerAction::ReadCompare,
        ] {
            assert!(
                !action.required_permissions().is_empty(),
                "{:?} must require at least one permission (deny-by-default)",
                action
            );
        }
    }

    #[test]
    fn permissions_iter_returns_canonical_order() {
        let p = Permissions::from_permissions(&[
            Permission::ReadCompare,
            Permission::ReadEvents,
            Permission::ReadDebug,
        ]);
        let collected: Vec<Permission> = p.iter().collect();
        assert_eq!(
            collected,
            vec![
                Permission::ReadEvents,
                Permission::ReadDebug,
                Permission::ReadCompare,
            ]
        );
    }
}
