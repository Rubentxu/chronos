//! Integration tests for the 4 tripwire MCP tools.
//!
//! These tests call `TripwiresService::*` methods directly, bypassing the MCP
//! transport layer. They verify the service logic without requiring a running
//! chronos-mcp binary or any probe infrastructure.
//!
//! All `TripwiresService` methods are **sync**, so plain `#[test]` is sufficient.

use std::sync::Arc;

use chronos_domain::tripwire::TripwireManager;
use chronos_services::error::ServiceError;
use chronos_services::tripwires::TripwiresService;

/// `TripwireConditionType`-equivalent for tests. Mirrors the real MCP-layer enum
/// (chronos-mcp/src/server.rs) so integration tests exercise the same code paths
/// that the MCP tool handler does before calling `TripwiresService::create`.
#[derive(Debug, Clone)]
pub enum TripwireConditionType {
    EventType { event_types: Vec<String> },
    FunctionName { pattern: String },
    ExceptionType { exc_type: String },
    MemoryAddress { start: u64, end: u64 },
    SyscallNumber { numbers: Vec<u64> },
    VariableName { name: String },
    Signal { numbers: Vec<i32> },
}

impl TripwireConditionType {
    /// Convert to domain [`TripwireCondition`].
    ///
    /// Returns the bad `event_type` string on `Err`, matching the behaviour of
    /// `ChronosServer::TripwireConditionType::into_condition`.
    pub fn into_condition(self) -> Result<chronos_domain::tripwire::TripwireCondition, String> {
        match self {
            Self::EventType { event_types } => {
                let mut types = Vec::with_capacity(event_types.len());
                for s in &event_types {
                    match parse_event_type(s) {
                        Some(t) => types.push(t),
                        None => return Err(s.clone()),
                    }
                }
                Ok(chronos_domain::tripwire::TripwireCondition::EventType(
                    types,
                ))
            }
            Self::FunctionName { pattern } => {
                Ok(chronos_domain::tripwire::TripwireCondition::FunctionName { pattern })
            }
            Self::ExceptionType { exc_type } => {
                Ok(chronos_domain::tripwire::TripwireCondition::ExceptionType { exc_type })
            }
            Self::MemoryAddress { start, end } => {
                Ok(chronos_domain::tripwire::TripwireCondition::MemoryAddress { start, end })
            }
            Self::SyscallNumber { numbers } => {
                Ok(chronos_domain::tripwire::TripwireCondition::SyscallNumber { numbers })
            }
            Self::VariableName { name } => {
                Ok(chronos_domain::tripwire::TripwireCondition::VariableName { name })
            }
            Self::Signal { numbers } => {
                Ok(chronos_domain::tripwire::TripwireCondition::Signal { numbers })
            }
        }
    }
}

/// Mirror of `ChronosServer::parse_event_type`.
fn parse_event_type(name: &str) -> Option<chronos_domain::EventType> {
    match name {
        "syscall_enter" => Some(chronos_domain::EventType::SyscallEnter),
        "syscall_exit" => Some(chronos_domain::EventType::SyscallExit),
        "function_entry" => Some(chronos_domain::EventType::FunctionEntry),
        "function_exit" => Some(chronos_domain::EventType::FunctionExit),
        "variable_write" => Some(chronos_domain::EventType::VariableWrite),
        "memory_write" => Some(chronos_domain::EventType::MemoryWrite),
        "signal_delivered" => Some(chronos_domain::EventType::SignalDelivered),
        "breakpoint_hit" => Some(chronos_domain::EventType::BreakpointHit),
        "thread_create" => Some(chronos_domain::EventType::ThreadCreate),
        "thread_exit" => Some(chronos_domain::EventType::ThreadExit),
        "exception_thrown" => Some(chronos_domain::EventType::ExceptionThrown),
        _ => None,
    }
}

/// Helper: build a fresh Arc-wrapped TripwireManager.
fn fresh_manager() -> Arc<TripwireManager> {
    Arc::new(TripwireManager::new())
}

/// Reset the global tripwire ID counter before each test.
/// Without this, IDs accumulate across the workspace test suite.
fn reset() {
    chronos_domain::tripwire::reset_tripwire_ids_for_testing();
}

// ---------------------------------------------------------------------------
// tripwire_create
// ---------------------------------------------------------------------------

#[test]
fn tripwire_create_ok() {
    reset();
    let manager = fresh_manager();
    let condition = TripwireConditionType::EventType {
        event_types: vec!["function_entry".into(), "function_exit".into()],
    };
    let cond = condition.into_condition().unwrap();

    let result = TripwiresService::create(cond, Some("entry/exit".into()), &manager).unwrap();

    assert_eq!(result.tripwire_id, "tripwire-1");
    assert_eq!(result.active_count, 1);
    assert_eq!(result.label.as_deref(), Some("entry/exit"));
}

#[test]
fn tripwire_create_function_name() {
    reset();
    let manager = fresh_manager();
    let cond = TripwireConditionType::FunctionName {
        pattern: "UserService.*".into(),
    }
    .into_condition()
    .unwrap();

    let result = TripwiresService::create(cond, None, &manager).unwrap();

    assert_eq!(result.tripwire_id, "tripwire-1");
    assert!(result.label.is_none());
}

#[test]
fn tripwire_create_invalid_event_type() {
    reset();
    // MCP tool would return early with the error message before calling the service.
    // The integration test verifies the conversion path works correctly.
    let condition = TripwireConditionType::EventType {
        event_types: vec!["function_entry".into(), "made_up_type".into()],
    };
    let conv_result = condition.into_condition();
    assert!(conv_result.is_err());
    assert_eq!(conv_result.unwrap_err(), "made_up_type");
}

// ---------------------------------------------------------------------------
// tripwire_list
// ---------------------------------------------------------------------------

#[test]
fn tripwire_list_empty() {
    reset();
    let manager = fresh_manager();
    let result = TripwiresService::list(&manager);

    assert!(result.tripwires.is_empty());
    assert!(result.fired_events.is_empty());
    assert_eq!(result.total_active, 0);
    assert_eq!(result.fired_count, 0);
}

#[test]
fn tripwire_list_two() {
    reset();
    let manager = fresh_manager();

    let c1 = TripwireConditionType::EventType {
        event_types: vec!["function_entry".into()],
    }
    .into_condition()
    .unwrap();
    TripwiresService::create(c1, Some("entry".into()), &manager).unwrap();

    let c2 = TripwireConditionType::Signal { numbers: vec![11] }
        .into_condition()
        .unwrap();
    TripwiresService::create(c2, None, &manager).unwrap();

    let result = TripwiresService::list(&manager);

    assert_eq!(result.total_active, 2);
    assert_eq!(result.fired_count, 0);
    assert_eq!(result.tripwires.len(), 2);

    let ids: Vec<_> = result.tripwires.iter().map(|tw| tw.id.clone()).collect();
    assert!(ids.contains(&"tripwire-1".into()));
    assert!(ids.contains(&"tripwire-2".into()));
}

// ---------------------------------------------------------------------------
// tripwire_query
// ---------------------------------------------------------------------------

#[test]
fn tripwire_query_empty() {
    reset();
    let manager = fresh_manager();
    let result = TripwiresService::query(&manager);

    assert!(result.tripwires.is_empty());
    assert_eq!(result.total_active, 0);
}

#[test]
fn tripwire_query_non_destructive() {
    reset();
    let manager = fresh_manager();

    let c = TripwireConditionType::EventType {
        event_types: vec!["function_entry".into()],
    }
    .into_condition()
    .unwrap();
    TripwiresService::create(c, None, &manager).unwrap();

    let r1 = TripwiresService::query(&manager);
    let r2 = TripwiresService::query(&manager);

    assert_eq!(r1.total_active, 1);
    assert_eq!(r2.total_active, 1);
}

// ---------------------------------------------------------------------------
// tripwire_delete
// ---------------------------------------------------------------------------

#[test]
fn tripwire_delete_ok() {
    reset();
    let manager = fresh_manager();

    let c = TripwireConditionType::ExceptionType {
        exc_type: "RuntimeError".into(),
    }
    .into_condition()
    .unwrap();
    let r = TripwiresService::create(c, Some("runtime".into()), &manager).unwrap();

    let result = TripwiresService::delete(&r.tripwire_id, &manager).unwrap();

    assert_eq!(result.tripwire_id, "tripwire-1");
    assert_eq!(result.remaining_active, 0);
}

#[test]
fn tripwire_delete_not_found() {
    reset();
    let manager = fresh_manager();
    let result = TripwiresService::delete("tripwire-99", &manager);

    assert!(matches!(
        result,
        Err(ServiceError::TripwireNotFound(ref s)) if s == "tripwire-99"
    ));
}

#[test]
fn tripwire_delete_invalid_format() {
    reset();
    let manager = fresh_manager();
    let result = TripwiresService::delete("not-a-tripwire-id", &manager);

    assert!(matches!(
        result,
        Err(ServiceError::InvalidTripwireIdFormat(ref s)) if s == "not-a-tripwire-id"
    ));
}

// ---------------------------------------------------------------------------
// Full chain: create → list → query → delete
// ---------------------------------------------------------------------------

#[test]
fn full_tripwire_lifecycle() {
    let manager = fresh_manager();

    // Create three tripwires
    let c1 = TripwireConditionType::EventType {
        event_types: vec!["function_entry".into()],
    }
    .into_condition()
    .unwrap();
    let r1 = TripwiresService::create(c1, Some("entry".into()), &manager).unwrap();

    let c2 = TripwireConditionType::Signal { numbers: vec![9] }
        .into_condition()
        .unwrap();
    TripwiresService::create(c2, None, &manager).unwrap();

    let c3 = TripwireConditionType::FunctionName {
        pattern: "main".into(),
    }
    .into_condition()
    .unwrap();
    TripwiresService::create(c3, Some("main-watch".into()), &manager).unwrap();

    // list shows 3
    let list = TripwiresService::list(&manager);
    assert_eq!(list.total_active, 3);

    // query confirms 3
    let query = TripwiresService::query(&manager);
    assert_eq!(query.total_active, 3);

    // Delete the first
    TripwiresService::delete(&r1.tripwire_id, &manager).unwrap();

    // Now 2 remain
    let query = TripwiresService::query(&manager);
    assert_eq!(query.total_active, 2);

    // list also shows 2
    let list = TripwiresService::list(&manager);
    assert_eq!(list.total_active, 2);
}
