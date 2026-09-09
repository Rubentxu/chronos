//! Unit tests for the 7 debug-read MCP tools.
//!
//! These tests call `DebugReadService::*` methods directly, bypassing the MCP
//! transport layer. They verify error paths (SessionNotFound, etc.) without
//! requiring a running chronos-mcp binary.

use chronos_services::debug_read::DebugReadService;
use chronos_services::error::ServiceError;
use std::collections::HashMap;
use tokio::sync::Mutex;

fn make_engines() -> Mutex<HashMap<String, chronos_query::QueryEngine>> {
    Mutex::new(HashMap::new())
}

// ---------------------------------------------------------------------------
// get_variables
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_variables_session_not_found() {
    let engines = make_engines();
    let result = DebugReadService::get_variables("no-such-session", 0, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// get_registers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_registers_session_not_found() {
    let engines = make_engines();
    let result = DebugReadService::get_registers("no-such-session", 0, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// get_memory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_memory_session_not_found() {
    let engines = make_engines();
    let result = DebugReadService::get_memory("no-such-session", 0x1000, 0, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// evaluate_expression
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evaluate_expression_session_not_found() {
    let engines = make_engines();
    let result =
        DebugReadService::evaluate_expression("no-such-session", 0, "x + 1", &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

#[tokio::test]
async fn diff_session_not_found() {
    let engines = make_engines();
    let result = DebugReadService::diff("no-such-session", 0, 1, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// analyze_memory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn analyze_memory_session_not_found() {
    let engines = make_engines();
    let result =
        DebugReadService::analyze_memory("no-such-session", 0x1000, 0x2000, 0, 1_000_000, &engines)
            .await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}

// ---------------------------------------------------------------------------
// forensic_audit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn forensic_audit_session_not_found() {
    let engines = make_engines();
    let result = DebugReadService::forensic_audit("no-such-session", 0x1000, 10, &engines).await;
    assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
}
