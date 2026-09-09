//! Tripwire-management service — create, list, query, and delete tripwires.
//!
//! All 4 methods are **sync** (no `async`) because `TripwireManager` operations
//! are synchronous. The service takes `manager: &Arc<TripwireManager>` and returns
//! `Result` synchronously.
//!
//! This service does NOT evaluate tripwires against events — that logic lives in
//! [`TripwireManager::evaluate`](chronos_domain::tripwire::TripwireManager::evaluate)
//! and is called by the trace-probe layer at runtime.

use std::sync::Arc;

use chronos_domain::tripwire::{TripwireCondition, TripwireId, TripwireManager};

use crate::error::ServiceError;
use crate::output::{
    CreateResult, QueryResult, TripwireDeleteResult, TripwireFiredSummary, TripwireListResult,
    TripwireSummary,
};

/// A zero-sized service struct. All state is accessed via the `manager` reference.
#[derive(Debug, Default)]
pub struct TripwiresService;

impl TripwiresService {
    /// Register a new tripwire.
    ///
    /// The caller is responsible for converting the user-facing
    /// [`TripwireConditionType`](chronos_mcp::server::TripwireConditionType) MCP
    /// parameter into a domain [`TripwireCondition`] before calling this method
    /// (via [`TripwireConditionType::into_condition`]).
    ///
    /// # Errors
    /// - `InvalidCondition` if `condition.into_condition()` returns `Err`,
    ///   meaning at least one `event_type` string could not be mapped to a known
    ///   `EventType` variant.  (Handled by the MCP caller before reaching here.)
    pub fn create(
        condition: TripwireCondition,
        label: Option<String>,
        manager: &Arc<TripwireManager>,
    ) -> Result<CreateResult, ServiceError> {
        let id = manager.register_with_label(condition, label.clone());

        Ok(CreateResult {
            tripwire_id: id.to_string(),
            active_count: manager.active_count(),
            label,
        })
    }

    /// List all active tripwires and drain all accumulated fire notifications.
    ///
    /// This is a **destructive** read: `TripwireManager::drain_fired()` clears the
    /// internal fired-buffer, so each fire notification is delivered exactly once.
    /// Use [`query`](Self::query) if you need a non-destructive snapshot.
    pub fn list(manager: &Arc<TripwireManager>) -> TripwireListResult {
        let tripwires = manager.list();
        let fired = manager.drain_fired();

        let summaries: Vec<TripwireSummary> = tripwires
            .iter()
            .map(|tw| TripwireSummary {
                id: tw.id.to_string(),
                label: tw.label.clone(),
                condition: format!("{:?}", tw.condition),
                fire_count: tw.fire_count,
            })
            .collect();

        let fired_events: Vec<TripwireFiredSummary> = fired
            .iter()
            .map(|f| TripwireFiredSummary {
                tripwire_id: f.tripwire_id.to_string(),
                condition_description: f.condition_description.clone(),
                event_id: f.event_id,
                timestamp_ns: f.timestamp_ns,
                thread_id: f.thread_id,
            })
            .collect();

        let total_active = summaries.len();
        let fired_count = fired_events.len();

        TripwireListResult {
            tripwires: summaries,
            fired_events,
            total_active,
            fired_count,
        }
    }

    /// Delete a tripwire by its ID string (format: `"tripwire-<number>"`).
    ///
    /// # Errors
    /// - `InvalidTripwireIdFormat` if the string does not match `"tripwire-<number>"`.
    /// - `TripwireNotFound` if `manager.remove()` returns `false` (ID parsed OK but
    ///   no tripwire with that ID is registered).
    pub fn delete(
        tripwire_id: &str,
        manager: &Arc<TripwireManager>,
    ) -> Result<TripwireDeleteResult, ServiceError> {
        let id_num = Self::parse_tripwire_id(tripwire_id)?;
        let id = TripwireId(id_num);

        if !manager.remove(id) {
            return Err(ServiceError::TripwireNotFound(tripwire_id.to_string()));
        }

        Ok(TripwireDeleteResult {
            tripwire_id: tripwire_id.to_string(),
            remaining_active: manager.active_count(),
        })
    }

    /// Query all active tripwires without draining the fired-events buffer.
    ///
    /// This is a **non-destructive** read. Fire notifications accumulate in the
    /// manager's buffer and can be retrieved with [`list`](Self::list).
    pub fn query(manager: &Arc<TripwireManager>) -> QueryResult {
        let tripwires = manager.list();

        let summaries: Vec<TripwireSummary> = tripwires
            .iter()
            .map(|tw| TripwireSummary {
                id: tw.id.to_string(),
                label: tw.label.clone(),
                condition: format!("{:?}", tw.condition),
                fire_count: tw.fire_count,
            })
            .collect();

        let total_active = summaries.len();

        QueryResult {
            tripwires: summaries,
            total_active,
        }
    }

    /// Parse a `"tripwire-<number>"` ID string into a `u64`.
    ///
    /// # Errors
    /// Returns `Err(InvalidTripwireIdFormat)` if the string is missing the prefix
    /// or the numeric portion cannot be parsed as `u64`.
    fn parse_tripwire_id(s: &str) -> Result<u64, ServiceError> {
        let s = s.trim();
        let rest = s
            .strip_prefix("tripwire-")
            .ok_or_else(|| ServiceError::InvalidTripwireIdFormat(s.to_string()))?;

        rest.parse::<u64>()
            .map_err(|_| ServiceError::InvalidTripwireIdFormat(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::tripwire::TripwireCondition;

    /// Reset the global tripwire ID counter before each test.
    /// Without this, IDs accumulate across the workspace test suite and
    /// assertions on fixed ID strings (e.g. "tripwire-1") fail.
    fn reset() {
        chronos_domain::tripwire::reset_tripwire_ids_for_testing();
    }

    /// Minimal parse_event_type mirroring ChronosServer::parse_event_type.
    /// Used only in tests to simulate the MCP-layer validation.
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

    /// Simulate what `TripwireConditionType::into_condition` does.
    /// Returns the bad string on Err so callers can assert on it.
    fn try_into_condition(event_types: Vec<String>) -> Result<TripwireCondition, String> {
        let mut types = Vec::with_capacity(event_types.len());
        for s in &event_types {
            match parse_event_type(s) {
                Some(t) => types.push(t),
                None => return Err(s.clone()),
            }
        }
        Ok(TripwireCondition::EventType(types))
    }

    // -------------------------------------------------------------------------
    // create — happy paths
    // -------------------------------------------------------------------------

    #[test]
    fn create_event_type_ok() {
        reset();
        reset();
        let manager = Arc::new(TripwireManager::new());
        let cond =
            try_into_condition(vec!["function_entry".into(), "function_exit".into()]).unwrap();

        let result = TripwiresService::create(cond, Some("main-watch".into()), &manager).unwrap();

        assert_eq!(result.tripwire_id, "tripwire-1");
        assert_eq!(result.active_count, 1);
        assert_eq!(result.label.as_deref(), Some("main-watch"));
    }

    #[test]
    fn create_function_name_ok() {
        reset();
        let manager = Arc::new(TripwireManager::new());
        let cond = TripwireCondition::FunctionName {
            pattern: "process_*".into(),
        };

        let result = TripwiresService::create(cond, None, &manager).unwrap();

        assert_eq!(result.tripwire_id, "tripwire-1");
        assert_eq!(result.active_count, 1);
        assert!(result.label.is_none());
    }

    #[test]
    fn create_multiple_increments_count() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let r1 = TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        let r2 = TripwiresService::create(
            TripwireCondition::Signal { numbers: vec![11] },
            None,
            &manager,
        )
        .unwrap();

        assert_eq!(r1.active_count, 1);
        assert_eq!(r2.active_count, 2);
    }

    #[test]
    fn create_exception_type_ok() {
        reset();
        let manager = Arc::new(TripwireManager::new());
        let cond = TripwireCondition::ExceptionType {
            exc_type: "ValueError".into(),
        };

        let result =
            TripwiresService::create(cond, Some("python-errors".into()), &manager).unwrap();

        assert_eq!(result.tripwire_id, "tripwire-1");
        assert_eq!(result.label.as_deref(), Some("python-errors"));
    }

    // -------------------------------------------------------------------------
    // delete — happy paths
    // -------------------------------------------------------------------------

    #[test]
    fn delete_ok() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let r = TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        let result = TripwiresService::delete(&r.tripwire_id, &manager).unwrap();

        assert_eq!(result.tripwire_id, "tripwire-1");
        assert_eq!(result.remaining_active, 0);
    }

    #[test]
    fn delete_one_of_two() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        let r2 = TripwiresService::create(
            TripwireCondition::Signal { numbers: vec![9] },
            None,
            &manager,
        )
        .unwrap();

        let result = TripwiresService::delete(&r2.tripwire_id, &manager).unwrap();

        assert_eq!(result.tripwire_id, "tripwire-2");
        assert_eq!(result.remaining_active, 1);
    }

    // -------------------------------------------------------------------------
    // delete — error paths
    // -------------------------------------------------------------------------

    #[test]
    fn delete_invalid_format_missing_prefix() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let result = TripwiresService::delete("tw-1", &manager);

        assert!(matches!(
            result,
            Err(ServiceError::InvalidTripwireIdFormat(ref s)) if s == "tw-1"
        ));
    }

    #[test]
    fn delete_invalid_format_bad_number() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let result = TripwiresService::delete("tripwire-not-a-number", &manager);

        assert!(matches!(
            result,
            Err(ServiceError::InvalidTripwireIdFormat(ref s)) if s == "tripwire-not-a-number"
        ));
    }

    #[test]
    fn delete_not_found() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let result = TripwiresService::delete("tripwire-99", &manager);

        assert!(matches!(
            result,
            Err(ServiceError::TripwireNotFound(ref s)) if s == "tripwire-99"
        ));
    }

    #[test]
    fn delete_twice() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let r = TripwiresService::create(
            TripwireCondition::FunctionName {
                pattern: "*".into(),
            },
            None,
            &manager,
        )
        .unwrap();

        // First delete succeeds
        TripwiresService::delete(&r.tripwire_id, &manager).unwrap();

        // Second delete fails with TripwireNotFound
        let result = TripwiresService::delete(&r.tripwire_id, &manager);

        assert!(matches!(
            result,
            Err(ServiceError::TripwireNotFound(ref s)) if s == "tripwire-1"
        ));
    }

    #[test]
    fn delete_whitespace_trimmed() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let r = TripwiresService::create(
            try_into_condition(vec!["function_exit".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        // Whitespace is trimmed before parsing, so this succeeds
        let result = TripwiresService::delete(&format!(" {} ", r.tripwire_id), &manager);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().remaining_active, 0);
    }

    // -------------------------------------------------------------------------
    // list — happy paths
    // -------------------------------------------------------------------------

    #[test]
    fn list_empty() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let result = TripwiresService::list(&manager);

        assert!(result.tripwires.is_empty());
        assert!(result.fired_events.is_empty());
        assert_eq!(result.total_active, 0);
        assert_eq!(result.fired_count, 0);
    }

    #[test]
    fn list_two_tripwires() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            Some("entry-watch".into()),
            &manager,
        )
        .unwrap();

        TripwiresService::create(
            TripwireCondition::FunctionName {
                pattern: "main".into(),
            },
            None,
            &manager,
        )
        .unwrap();

        let result = TripwiresService::list(&manager);

        assert_eq!(result.total_active, 2);
        assert_eq!(result.fired_count, 0);

        let ids: Vec<_> = result.tripwires.iter().map(|tw| tw.id.clone()).collect();
        assert!(ids.contains(&"tripwire-1".into()));
        assert!(ids.contains(&"tripwire-2".into()));

        let labeled = &result.tripwires[0];
        assert_eq!(labeled.label.as_deref(), Some("entry-watch"));
    }

    #[test]
    fn list_drains_fired() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        // First list drains the empty buffer
        let r1 = TripwiresService::list(&manager);
        assert_eq!(r1.fired_count, 0);

        // Simulate a fire by manually calling evaluate
        use chronos_domain::{SourceLocation, TraceEvent};
        let event = TraceEvent {
            event_id: 1,
            timestamp_ns: 100,
            thread_id: 1,
            event_type: chronos_domain::EventType::FunctionEntry,
            location: SourceLocation::default(),
            data: chronos_domain::EventData::Function {
                name: "main".into(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        };
        manager.evaluate(&event);

        // Second list drains the fired event
        let r2 = TripwiresService::list(&manager);
        assert_eq!(r2.fired_count, 1);
        assert_eq!(r2.fired_events[0].tripwire_id, "tripwire-1");

        // Third list shows empty buffer (drained)
        let r3 = TripwiresService::list(&manager);
        assert_eq!(r3.fired_count, 0);
    }

    // -------------------------------------------------------------------------
    // query — happy paths
    // -------------------------------------------------------------------------

    #[test]
    fn query_empty() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        let result = TripwiresService::query(&manager);

        assert!(result.tripwires.is_empty());
        assert_eq!(result.total_active, 0);
    }

    #[test]
    fn query_one() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        TripwiresService::create(
            TripwireCondition::Signal { numbers: vec![11] },
            Some("sigsegv".into()),
            &manager,
        )
        .unwrap();

        let result = TripwiresService::query(&manager);

        assert_eq!(result.total_active, 1);
        assert_eq!(result.tripwires[0].id, "tripwire-1");
        assert_eq!(result.tripwires[0].label.as_deref(), Some("sigsegv"));
    }

    #[test]
    fn query_non_destructive() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        // First query
        let r1 = TripwiresService::query(&manager);
        assert_eq!(r1.total_active, 1);

        // Second query — still 1, nothing drained
        let r2 = TripwiresService::query(&manager);
        assert_eq!(r2.total_active, 1);

        // list also returns 1 (list is destructive but the manager is fresh)
        let r3 = TripwiresService::list(&manager);
        assert_eq!(r3.total_active, 1);
    }

    // -------------------------------------------------------------------------
    // cross-method: create + list + delete chain
    // -------------------------------------------------------------------------

    #[test]
    fn full_lifecycle() {
        reset();
        let manager = Arc::new(TripwireManager::new());

        // Create two
        let r1 = TripwiresService::create(
            try_into_condition(vec!["function_entry".into()]).unwrap(),
            None,
            &manager,
        )
        .unwrap();

        TripwiresService::create(
            TripwireCondition::ExceptionType {
                exc_type: "NullPointerException".into(),
            },
            None,
            &manager,
        )
        .unwrap();

        // Query
        let query = TripwiresService::query(&manager);
        assert_eq!(query.total_active, 2);

        // Delete first
        TripwiresService::delete(&r1.tripwire_id, &manager).unwrap();

        // Query again — only one remains
        let query = TripwiresService::query(&manager);
        assert_eq!(query.total_active, 1);
        assert_eq!(query.tripwires[0].id, "tripwire-2");

        // List
        let list = TripwiresService::list(&manager);
        assert_eq!(list.total_active, 1);
        assert_eq!(list.fired_count, 0);
    }
}
