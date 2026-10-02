//! Debug-read service — read-only inspection operations on a trace session.
//!
//! All 7 methods follow the same pattern:
//! 1. Lock the session map mutex.
//! 2. Look up the engine by `session_id`, return `Err(SessionNotFound)` if missing.
//! 3. Delegate to a `QueryEngine` method (all are sync).
//! 4. Map engine-level errors to `ServiceError` variants.
//!
//! The mutex is held only for the duration of the sync call, keeping latency
//! and contention low.

use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::output::{
    AuditEntry, AuditStackFrame, EvalResult, MemoryAccess, MemoryAnalysis, MemoryAudit, MemoryRead,
    RegisterChange, RegisterRead, RegisterSet, StateDiffSnapshot, VariableChange,
};
use chronos_domain::EventData;
use chronos_domain::MonotonicNs;
use chronos_query::QueryEngine;

/// A zero-sized service struct. All state is passed in as arguments.
#[derive(Debug, Default)]
pub struct DebugReadService;

impl DebugReadService {
    /// Evaluate an arithmetic expression using local variables captured at a frame event.
    ///
    /// Supports `+`, `-`, `*`, `/`, parentheses, and variable names.
    pub async fn evaluate_expression(
        session_id: &str,
        event_id: u64,
        expression: &str,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<EvalResult, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        match engine.evaluate_expression(event_id, expression) {
            Ok(value) => Ok(EvalResult::Value(value)),
            Err(e) => Ok(EvalResult::Error(format!("{:?}", e))),
        }
    }

    /// Get all variables in scope at a specific event.
    ///
    /// Returns [`ServiceError::EventNotFound`] when the event does not exist,
    /// and an empty list when it does exist but carries no frame data. Those
    /// are different answers: "this event is not in the trace" cannot be
    /// reported as "this event has no variables", because a caller reading an
    /// empty list has no way to tell a real absence from a typo in the id.
    /// `get_registers` above draws the same distinction, with
    /// [`ServiceError::NoRegisterState`] for the second case.
    pub async fn get_variables(
        session_id: &str,
        event_id: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<Vec<chronos_domain::VariableInfo>, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        // Exact-id lookup, not a range: an id that was never captured belongs to
        // no frame and must not fall through as an empty variable set.
        engine
            .get_event_by_id(event_id)
            .ok_or(ServiceError::EventNotFound { event_id })?;

        Ok(engine.get_variables_at_event(event_id))
    }

    /// Read raw memory at an address as of a specific timestamp.
    ///
    /// Returns the most recent `MemoryWrite` event at or before `timestamp_ns`.
    pub async fn get_memory(
        session_id: &str,
        address: u64,
        timestamp_ns: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<MemoryRead, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let mem = engine
            .get_memory_at(address, MonotonicNs::from(timestamp_ns))
            .ok_or(ServiceError::MemoryNotFound {
                address,
                timestamp_ns,
            })?;

        let hex = mem
            .data
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");

        Ok(MemoryRead {
            address: mem.address,
            timestamp_ns: mem.timestamp_ns,
            event_id: mem.event_id,
            size: mem.size,
            data: mem.data,
            hex,
        })
    }

    /// Get CPU register values at a specific event.
    ///
    /// Fails if the event does not exist, or if no register state is available
    /// at that event.
    pub async fn get_registers(
        session_id: &str,
        event_id: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<RegisterRead, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let _ = engine
            .get_event_by_id(event_id)
            .ok_or(ServiceError::EventNotFound { event_id })?;

        let regs = engine
            .find_registers_at_event(event_id)
            .ok_or(ServiceError::NoRegisterState { event_id })?;

        Ok(RegisterRead {
            event_id,
            registers: RegisterSet {
                rax: regs.rax,
                rbx: regs.rbx,
                rcx: regs.rcx,
                rdx: regs.rdx,
                rsi: regs.rsi,
                rdi: regs.rdi,
                rbp: regs.rbp,
                rsp: regs.rsp,
                r8: regs.r8,
                r9: regs.r9,
                r10: regs.r10,
                r11: regs.r11,
                r12: regs.r12,
                r13: regs.r13,
                r14: regs.r14,
                r15: regs.r15,
                rip: regs.rip,
                rflags: regs.rflags,
            },
        })
    }

    /// Compare process state between two event IDs — variables, registers, memory.
    ///
    /// Returns [`ServiceError::EventNotFound`] when either side names an event
    /// that does not exist, and the id in the error is the offending one, so a
    /// caller holding two ids can tell which was mistyped; `event_id_a` is
    /// checked first, so when both are missing the error names it. A zero delta
    /// is reserved for the answer it actually means: both events exist and
    /// nothing changed between them. Those are different answers — reporting a
    /// bad id as an unchanged comparison would claim to have compared two
    /// events when at most one was ever captured, and would report every
    /// variable on the real side as removed against a side that does not
    /// exist. `get_variables` and `get_registers` already draw this distinction
    /// for a single event; a two-sided comparison must not lose it.
    pub async fn diff(
        session_id: &str,
        event_id_a: u64,
        event_id_b: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<StateDiffSnapshot, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        // Exact-id lookup on both sides, not a range: an id that was never
        // captured belongs to no frame and must not stand in for an empty
        // side, or every variable on the other side reads as removed.
        engine
            .get_event_by_id(event_id_a)
            .ok_or(ServiceError::EventNotFound {
                event_id: event_id_a,
            })?;
        engine
            .get_event_by_id(event_id_b)
            .ok_or(ServiceError::EventNotFound {
                event_id: event_id_b,
            })?;

        // Get variables at both events
        let vars_a = engine.get_variables_at_event(event_id_a);
        let vars_b = engine.get_variables_at_event(event_id_b);

        let names_a: std::collections::HashSet<_> = vars_a.iter().map(|v| v.name.clone()).collect();
        let names_b: std::collections::HashSet<_> = vars_b.iter().map(|v| v.name.clone()).collect();

        let variables_added: Vec<String> = names_b.difference(&names_a).cloned().collect();
        let variables_removed: Vec<String> = names_a.difference(&names_b).cloned().collect();

        let mut variables_changed = Vec::new();
        for name in names_a.intersection(&names_b) {
            let val_a = vars_a
                .iter()
                .find(|v| &v.name == name)
                .map(|v| v.value.clone());
            let val_b = vars_b
                .iter()
                .find(|v| &v.name == name)
                .map(|v| v.value.clone());
            if val_a != val_b {
                variables_changed.push(VariableChange {
                    name: name.clone(),
                    before: val_a,
                    after: val_b,
                });
            }
        }

        // Get registers at both events
        let regs_a = engine.find_registers_at_event(event_id_a);
        let regs_b = engine.find_registers_at_event(event_id_b);

        let mut registers_changed = std::collections::HashMap::new();
        if let (Some(ra), Some(rb)) = (&regs_a, &regs_b) {
            let reg_fields = [
                ("rax", ra.rax, rb.rax),
                ("rbx", ra.rbx, rb.rbx),
                ("rcx", ra.rcx, rb.rcx),
                ("rdx", ra.rdx, rb.rdx),
                ("rsi", ra.rsi, rb.rsi),
                ("rdi", ra.rdi, rb.rdi),
                ("rbp", ra.rbp, rb.rbp),
                ("rsp", ra.rsp, rb.rsp),
                ("r8", ra.r8, rb.r8),
                ("r9", ra.r9, rb.r9),
                ("r10", ra.r10, rb.r10),
                ("r11", ra.r11, rb.r11),
                ("r12", ra.r12, rb.r12),
                ("r13", ra.r13, rb.r13),
                ("r14", ra.r14, rb.r14),
                ("r15", ra.r15, rb.r15),
                ("rip", ra.rip, rb.rip),
                ("rflags", ra.rflags, rb.rflags),
            ];
            for (name, val_a, val_b) in reg_fields {
                if val_a != val_b {
                    registers_changed.insert(
                        name.to_string(),
                        RegisterChange {
                            before: format!("0x{:x}", val_a),
                            after: format!("0x{:x}", val_b),
                        },
                    );
                }
            }
        }

        // Get timestamps for delta
        let event_a = engine.get_event_by_id(event_id_a);
        let event_b = engine.get_event_by_id(event_id_b);
        let timestamp_delta_ns = match (event_a, event_b) {
            (Some(ea), Some(eb)) => eb.timestamp_ns.get().saturating_sub(ea.timestamp_ns.get()),
            _ => 0,
        };

        Ok(StateDiffSnapshot {
            event_id_a,
            event_id_b,
            variables_added,
            variables_removed,
            variables_changed,
            registers_changed,
            timestamp_delta_ns,
        })
    }

    /// Analyze all memory accesses to an address range within a time window.
    pub async fn analyze_memory(
        session_id: &str,
        start_address: u64,
        end_address: u64,
        start_ts: u64,
        end_ts: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<MemoryAnalysis, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let all_events = engine.get_all_events();
        let mut accesses = Vec::new();
        let mut total_writes = 0u64;

        for event in all_events {
            if event.timestamp_ns.get() < start_ts || event.timestamp_ns.get() > end_ts {
                continue;
            }

            if let EventData::Memory {
                address,
                size,
                data,
            } = &event.data
            {
                if *address >= start_address && *address <= end_address {
                    let hex = data
                        .as_ref()
                        .map(|d| {
                            d.iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default();
                    accesses.push(MemoryAccess {
                        address: format!("0x{:x}", address),
                        timestamp_ns: event.timestamp_ns.get(),
                        data_hex: hex,
                        event_id: event.event_id,
                        size: *size,
                    });
                    total_writes += 1;
                }
            }
        }

        Ok(MemoryAnalysis {
            start_address: format!("0x{:x}", start_address),
            end_address: format!("0x{:x}", end_address),
            start_ts,
            end_ts,
            total_writes,
            accesses,
        })
    }

    /// Full audit trail for a specific address — all writes with calling context.
    ///
    /// Results are sorted by timestamp, newest first (the order declared by
    /// [`MemoryAudit::writes`]), and truncated to `limit`. Because the sort is
    /// descending, the truncation keeps the `limit` most recent writes, not the
    /// oldest ones.
    pub async fn forensic_audit(
        session_id: &str,
        address: u64,
        limit: usize,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<MemoryAudit, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

        let all_events = engine.get_all_events();
        let mut writes = Vec::new();

        for event in &all_events {
            if let EventData::Memory {
                address: evt_addr,
                data,
                ..
            } = &event.data
            {
                if *evt_addr == address {
                    let stack = engine.reconstruct_call_stack(event.event_id);
                    let hex = data
                        .as_ref()
                        .map(|d| {
                            d.iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default();
                    writes.push(AuditEntry {
                        timestamp_ns: event.timestamp_ns.get(),
                        event_id: event.event_id,
                        data_hex: hex,
                        call_stack: stack
                            .into_iter()
                            .map(|f| AuditStackFrame {
                                depth: f.depth,
                                function: f.function,
                                file: f.file,
                                line: f.line,
                            })
                            .collect(),
                    });
                }
            }
        }

        writes.sort_by_key(|w| std::cmp::Reverse(w.timestamp_ns));
        writes.truncate(limit);

        Ok(MemoryAudit {
            address: format!("0x{:x}", address),
            write_count: writes.len(),
            writes,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation, VariableInfo};
    use std::collections::HashMap;

    fn make_engine(events: Vec<chronos_domain::TraceEvent>) -> QueryEngine {
        QueryEngine::new(events)
    }

    fn trace_event(
        event_id: u64,
        timestamp_ns: u64,
        thread_id: u64,
        event_type: EventType,
        data: EventData,
    ) -> chronos_domain::TraceEvent {
        chronos_domain::TraceEvent {
            event_id,
            timestamp_ns: MonotonicNs::from(timestamp_ns),
            thread_id,
            event_type,
            location: SourceLocation::default(),
            data,
        }
    }

    // --- Happy-path helpers ----------------------------------------------------

    fn vars_engine() -> HashMap<String, QueryEngine> {
        let events = vec![trace_event(
            0,
            0,
            1,
            EventType::FunctionEntry,
            EventData::Variable(VariableInfo::new(
                "x",
                "10",
                "i32",
                0x1000,
                chronos_domain::value::VariableScope::Local,
            )),
        )];
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert("s1".to_string(), engine);
        map
    }

    // --- evaluate_expression ---

    #[tokio::test]
    async fn evaluate_expression_ok() {
        let map = vars_engine();
        let engines = Mutex::new(map);
        // The expression engine returns Ok for constant arithmetic
        let result = DebugReadService::evaluate_expression("s1", 0, "1 + 2", &engines)
            .await
            .unwrap();
        assert!(matches!(result, EvalResult::Value(_)));
    }

    #[tokio::test]
    async fn evaluate_expression_session_not_found() {
        let map = vars_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::evaluate_expression("missing", 0, "1", &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(ref s)) if s == "missing"));
    }

    // --- get_variables ---

    #[tokio::test]
    async fn get_variables_ok() {
        let map = vars_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_variables("s1", 0, &engines)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn get_variables_session_not_found() {
        let map = vars_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_variables("missing", 0, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    /// An id that was never captured is not an event without variables.
    ///
    /// This is the case that made the two halves of the contract look alike: an
    /// empty list is a real answer for an event with no frame data, and a
    /// misleading one for a bad id.
    #[tokio::test]
    async fn get_variables_event_not_found() {
        let map = vars_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_variables("s1", 999_999, &engines).await;
        assert!(
            matches!(
                result,
                Err(ServiceError::EventNotFound { event_id: 999_999 })
            ),
            "an event id that was never captured must be reported as not found, got {result:?}"
        );
    }

    /// The neighbouring case must keep its answer: the event exists, it simply
    /// carries no frame data. Without this pair, the fix above could be "fixed"
    /// by always erroring, which would pass the test above and break the real one.
    #[tokio::test]
    async fn get_variables_event_without_frame_data_is_empty_not_an_error() {
        let engine = QueryEngine::new(vec![trace_event(
            7,
            700,
            1,
            EventType::SyscallEnter,
            EventData::Empty,
        )]);
        let engines = Mutex::new(HashMap::from([("s1".to_string(), engine)]));

        let result = DebugReadService::get_variables("s1", 7, &engines).await;
        assert!(
            matches!(result, Ok(ref vars) if vars.is_empty()),
            "an existing event with no frame data must return an empty list, got {result:?}"
        );
    }

    // --- get_memory ---

    fn memory_engine() -> HashMap<String, QueryEngine> {
        let events = vec![trace_event(
            5,
            100,
            1,
            EventType::MemoryWrite,
            EventData::Memory {
                address: 0x1000,
                size: 4,
                data: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            },
        )];
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert("s2".to_string(), engine);
        map
    }

    #[tokio::test]
    async fn get_memory_ok() {
        let map = memory_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_memory("s2", 0x1000, 200, &engines)
            .await
            .unwrap();
        assert_eq!(result.address, 0x1000);
        assert_eq!(result.hex, "deadbeef");
    }

    #[tokio::test]
    async fn get_memory_not_found() {
        let map = memory_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_memory("s2", 0x9999, 200, &engines).await;
        assert!(matches!(
            result,
            Err(ServiceError::MemoryNotFound {
                address: 0x9999,
                timestamp_ns: 200
            })
        ));
    }

    #[tokio::test]
    async fn get_memory_session_not_found() {
        let map = memory_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_memory("missing", 0x1000, 200, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- get_registers ---

    fn register_engine() -> HashMap<String, QueryEngine> {
        let events = vec![trace_event(
            1,
            50,
            1,
            EventType::FunctionEntry,
            EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )];
        let engine = make_engine(events);
        let mut map = HashMap::new();
        map.insert("s3".to_string(), engine);
        map
    }

    #[tokio::test]
    async fn get_registers_event_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_registers("s3", 9999, &engines).await;
        assert!(matches!(
            result,
            Err(ServiceError::EventNotFound { event_id: 9999 })
        ));
    }

    #[tokio::test]
    async fn get_registers_session_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::get_registers("missing", 1, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- diff ---

    /// Two ids that were never captured cannot be compared, and must not be
    /// reported as "nothing changed between them".
    #[tokio::test]
    async fn diff_both_events_missing_returns_event_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::diff("s3", 9999, 8888, &engines).await;
        // `event_id_a` is validated first, so it is the id the error names.
        assert!(
            matches!(result, Err(ServiceError::EventNotFound { event_id: 9999 })),
            "two ids that were never captured must be reported as not found, got {result:?}"
        );
    }

    /// The caller asked two ids and only one is wrong. The error has to name
    /// that one, otherwise the caller cannot tell which side to fix.
    #[tokio::test]
    async fn diff_second_event_missing_names_that_id() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::diff("s3", 1, 9999, &engines).await;
        assert!(
            matches!(result, Err(ServiceError::EventNotFound { event_id: 9999 })),
            "the missing side must be the id named in the error, got {result:?}"
        );
    }

    #[tokio::test]
    async fn diff_first_event_missing_names_that_id() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::diff("s3", 9999, 1, &engines).await;
        assert!(
            matches!(result, Err(ServiceError::EventNotFound { event_id: 9999 })),
            "the missing side must be the id named in the error, got {result:?}"
        );
    }

    /// The case that must survive the fix: both events exist and neither
    /// carries frame data. "Nothing changed" is then a real answer, and erroring
    /// on it would leave a comparison of two real events unrepresentable.
    #[tokio::test]
    async fn diff_both_events_without_variables_is_a_zero_delta() {
        let engine = QueryEngine::new(vec![
            trace_event(
                1,
                50,
                1,
                EventType::FunctionEntry,
                EventData::Function {
                    name: "main".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
            trace_event(7, 700, 1, EventType::SyscallEnter, EventData::Empty),
        ]);
        let engines = Mutex::new(HashMap::from([("s3".to_string(), engine)]));

        let result = DebugReadService::diff("s3", 1, 7, &engines)
            .await
            .expect("two existing events without frame data must compare, not error");
        assert!(
            result.variables_added.is_empty()
                && result.variables_removed.is_empty()
                && result.variables_changed.is_empty()
                && result.registers_changed.is_empty(),
            "no frame data on either side means nothing changed, got {result:?}"
        );
    }

    /// Two existing events that do differ must still report the difference.
    /// Guards the fix from over-correcting into a blanket rejection.
    #[tokio::test]
    async fn diff_two_existing_events_reports_the_change() {
        let engine = QueryEngine::new(vec![
            trace_event(
                2,
                150,
                1,
                EventType::VariableWrite,
                EventData::Variable(VariableInfo::new(
                    "x",
                    "10",
                    "i32",
                    0x2000,
                    chronos_domain::value::VariableScope::Local,
                )),
            ),
            trace_event(
                3,
                250,
                1,
                EventType::VariableWrite,
                EventData::Variable(VariableInfo::new(
                    "y",
                    "20",
                    "i32",
                    0x2008,
                    chronos_domain::value::VariableScope::Local,
                )),
            ),
        ]);
        let engines = Mutex::new(HashMap::from([("s3".to_string(), engine)]));

        let result = DebugReadService::diff("s3", 2, 3, &engines).await.unwrap();
        assert_eq!(result.variables_removed, vec!["x".to_string()]);
        assert_eq!(result.variables_added, vec!["y".to_string()]);
        assert_eq!(result.timestamp_delta_ns, 100);
    }

    #[tokio::test]
    async fn diff_session_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::diff("missing", 1, 2, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- analyze_memory ---

    #[tokio::test]
    async fn analyze_memory_session_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result =
            DebugReadService::analyze_memory("missing", 0, u64::MAX, 0, u64::MAX, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- forensic_audit ---

    #[tokio::test]
    async fn forensic_audit_session_not_found() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::forensic_audit("missing", 0x1000, 10, &engines).await;
        assert!(matches!(result, Err(ServiceError::SessionNotFound(_))));
    }

    // --- forensic_audit: ordering contract ---

    /// Three writes to the same address, inserted out of timestamp order so
    /// that "newest first" cannot be satisfied by insertion order either.
    /// (event_id, timestamp_ns) pairs: (1, 100), (2, 300), (3, 200).
    fn audit_engine() -> HashMap<String, QueryEngine> {
        let events = vec![
            trace_event(
                1,
                100,
                1,
                EventType::MemoryWrite,
                EventData::Memory {
                    address: 0x1000,
                    size: 1,
                    data: Some(vec![0x11]),
                },
            ),
            trace_event(
                2,
                300,
                1,
                EventType::MemoryWrite,
                EventData::Memory {
                    address: 0x1000,
                    size: 1,
                    data: Some(vec![0x33]),
                },
            ),
            trace_event(
                3,
                200,
                1,
                EventType::MemoryWrite,
                EventData::Memory {
                    address: 0x1000,
                    size: 1,
                    data: Some(vec![0x22]),
                },
            ),
        ];
        HashMap::from([("s4".to_string(), QueryEngine::new(events))])
    }

    fn audit_pairs(audit: &crate::output::MemoryAudit) -> Vec<(u64, u64)> {
        audit
            .writes
            .iter()
            .map(|w| (w.event_id, w.timestamp_ns))
            .collect()
    }

    /// `MemoryAudit::writes` is documented as "sorted by timestamp, newest
    /// first". A client that reads `writes[0]` must get the most recent write
    /// of the session, never the oldest one.
    #[tokio::test]
    async fn forensic_audit_writes_are_newest_first() {
        let engines = Mutex::new(audit_engine());

        let audit = DebugReadService::forensic_audit("s4", 0x1000, 10, &engines)
            .await
            .unwrap();

        assert_eq!(
            audit_pairs(&audit),
            vec![(2, 300), (3, 200), (1, 100)],
            "writes must be newest first, so writes[0] is the most recent write"
        );
        assert!(
            audit
                .writes
                .windows(2)
                .all(|pair| pair[0].timestamp_ns > pair[1].timestamp_ns),
            "consecutive timestamps must be strictly decreasing, got {:?}",
            audit_pairs(&audit)
        );
    }

    /// The ordering above is what makes `truncate(limit)` mean "the most recent
    /// `limit` writes" instead of "the oldest `limit` writes".
    #[tokio::test]
    async fn forensic_audit_limit_keeps_the_most_recent_writes() {
        let engines = Mutex::new(audit_engine());

        let audit = DebugReadService::forensic_audit("s4", 0x1000, 2, &engines)
            .await
            .unwrap();

        assert_eq!(
            audit_pairs(&audit),
            vec![(2, 300), (3, 200)],
            "limit=2 must keep the two most recent writes, newest first"
        );
    }
}
