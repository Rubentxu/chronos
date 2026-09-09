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
    /// Returns the variable list directly; empty if the event has no frame data.
    pub async fn get_variables(
        session_id: &str,
        event_id: u64,
        engines: &Mutex<HashMap<String, QueryEngine>>,
    ) -> Result<Vec<chronos_domain::VariableInfo>, ServiceError> {
        let guard = engines.lock().await;
        let engine = guard
            .get(session_id)
            .ok_or_else(|| ServiceError::SessionNotFound(session_id.to_string()))?;

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

        let mem =
            engine
                .get_memory_at(address, timestamp_ns)
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
    /// When `event_id_a` or `event_id_b` is not found, the corresponding side is
    /// treated as absent (zero-delta, no variables/registers compared).
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
            (Some(ea), Some(eb)) => eb.timestamp_ns.saturating_sub(ea.timestamp_ns),
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
            if event.timestamp_ns < start_ts || event.timestamp_ns > end_ts {
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
                        timestamp_ns: event.timestamp_ns,
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
    /// Results are sorted by timestamp and truncated to `limit`.
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
                        timestamp_ns: event.timestamp_ns,
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

        writes.sort_by_key(|w| w.timestamp_ns);
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
            timestamp_ns,
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

    #[tokio::test]
    async fn diff_both_events_missing_returns_zero_delta() {
        let map = register_engine();
        let engines = Mutex::new(map);
        let result = DebugReadService::diff("s3", 9999, 8888, &engines)
            .await
            .unwrap();
        assert_eq!(result.timestamp_delta_ns, 0);
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
}
