//! Output data structures for debug-read service operations.
//!
//! These structs are the return types of [`DebugReadService`](super::debug_read::DebugReadService).
//! All types derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`
//! so they can cross RPC boundaries cleanly.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of evaluating an arithmetic expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EvalResult {
    /// Expression evaluated successfully.
    Value(f64),
    /// Evaluation failed with a human-readable error message.
    Error(String),
}

/// A raw memory read at an address and timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRead {
    /// Memory address as a u64.
    pub address: u64,
    /// Nanosecond timestamp of the write event that produced this value.
    pub timestamp_ns: u64,
    /// Event ID of the write.
    pub event_id: u64,
    /// Size in bytes.
    pub size: usize,
    /// Raw bytes.
    pub data: Vec<u8>,
    /// Hex string of `data` (two lower-case hex chars per byte, no `0x` prefix).
    pub hex: String,
}

/// A flat set of all 17 x86-64 general-purpose + program-counter + flags registers.
/// Each field is a raw u64 — caller formats as `0x{:x}` if needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterSet {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

/// Result of `debug_get_registers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterRead {
    pub event_id: u64,
    pub registers: RegisterSet,
}

/// A single memory access (read or write) captured during `debug_analyze_memory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryAccess {
    /// Address as a hex string with `0x` prefix.
    pub address: String,
    /// Nanosecond timestamp of the access.
    pub timestamp_ns: u64,
    /// Hex string of the data (two lower-case hex chars per byte).
    pub data_hex: String,
    /// Event ID that produced this access.
    pub event_id: u64,
    /// Size in bytes.
    pub size: usize,
}

/// Analysis of memory accesses within a time/address window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    /// Start address as a hex string with `0x` prefix.
    pub start_address: String,
    /// End address as a hex string with `0x` prefix.
    pub end_address: String,
    /// Start of the time window (nanoseconds).
    pub start_ts: u64,
    /// End of the time window (nanoseconds).
    pub end_ts: u64,
    /// Total number of accesses in the window.
    pub total_writes: u64,
    /// Individual accesses, newest first.
    pub accesses: Vec<MemoryAccess>,
}

/// A call-stack frame embedded inside an audit entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditStackFrame {
    pub depth: u32,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// A single write entry in a forensic memory audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Nanosecond timestamp of the write.
    pub timestamp_ns: u64,
    /// Event ID that performed the write.
    pub event_id: u64,
    /// Hex string of the written data.
    pub data_hex: String,
    /// Reconstructed call stack at the write point.
    pub call_stack: Vec<AuditStackFrame>,
}

/// Forensic audit — all writes to a specific address across the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryAudit {
    /// Address as a hex string with `0x` prefix.
    pub address: String,
    /// Number of writes captured (capped by the `limit` parameter).
    pub write_count: usize,
    /// Write entries sorted by timestamp, newest first.
    pub writes: Vec<AuditEntry>,
}

/// A variable changed entry inside `StateDiffSnapshot`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableChange {
    pub name: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// A register changed entry inside `StateDiffSnapshot`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterChange {
    pub before: String,
    pub after: String,
}

/// Snapshot of the state diff between two events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateDiffSnapshot {
    /// Event ID of the "before" event.
    pub event_id_a: u64,
    /// Event ID of the "after" event.
    pub event_id_b: u64,
    /// Variables that existed at `event_id_b` but not at `event_id_a`.
    pub variables_added: Vec<String>,
    /// Variables that existed at `event_id_a` but not at `event_id_b`.
    pub variables_removed: Vec<String>,
    /// Variables that existed at both events but had different values.
    pub variables_changed: Vec<VariableChange>,
    /// Registers that had different values between the two events.
    pub registers_changed: HashMap<String, RegisterChange>,
    /// Time delta from `event_id_a` to `event_id_b` in nanoseconds.
    pub timestamp_delta_ns: u64,
}

// ---------------------------------------------------------------------------
// Serde round-trip tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_result_value_roundtrips() {
        use std::f64::consts::PI;
        let json = serde_json::to_value(EvalResult::Value(PI)).unwrap();
        // With #[serde(untagged)], Value(f64) serializes to a plain JSON number
        assert!(
            json.is_number(),
            "untagged Value should serialize to a number"
        );
        assert_eq!(json, serde_json::json!(PI));
    }

    #[test]
    fn eval_result_error_roundtrips() {
        let json = serde_json::to_value(EvalResult::Error("division by zero".into())).unwrap();
        // With #[serde(untagged)], Error(String) serializes to a plain JSON string
        assert!(
            json.is_string(),
            "untagged Error should serialize to a string"
        );
        assert_eq!(json.as_str().unwrap(), "division by zero");
    }

    #[test]
    fn memory_read_roundtrips() {
        let mr = MemoryRead {
            address: 0x1000,
            timestamp_ns: 42,
            event_id: 7,
            size: 4,
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            hex: "deadbeef".into(),
        };
        let json = serde_json::to_value(&mr).unwrap();
        assert_eq!(json["address"], 0x1000u64);
        assert_eq!(json["hex"], "deadbeef");
    }

    #[test]
    fn register_set_roundtrips() {
        let rs = RegisterSet {
            rax: 1,
            rbx: 2,
            rcx: 3,
            rdx: 4,
            rsi: 5,
            rdi: 6,
            rbp: 7,
            rsp: 8,
            r8: 9,
            r9: 10,
            r10: 11,
            r11: 12,
            r12: 13,
            r13: 14,
            r14: 15,
            r15: 16,
            rip: 17,
            rflags: 18,
        };
        let json = serde_json::to_value(&rs).unwrap();
        assert_eq!(json["rax"], 1u64);
    }

    #[test]
    fn memory_audit_roundtrips() {
        let ma = MemoryAudit {
            address: "0x1000".into(),
            write_count: 1,
            writes: vec![AuditEntry {
                timestamp_ns: 100,
                event_id: 5,
                data_hex: "aabb".into(),
                call_stack: vec![],
            }],
        };
        let json = serde_json::to_value(&ma).unwrap();
        assert_eq!(json["address"], "0x1000");
        assert_eq!(json["write_count"], 1);
    }

    #[test]
    fn state_diff_snapshot_roundtrips() {
        let sd = StateDiffSnapshot {
            event_id_a: 1,
            event_id_b: 2,
            variables_added: vec!["z".into()],
            variables_removed: vec![],
            variables_changed: vec![VariableChange {
                name: "x".into(),
                before: Some("0".into()),
                after: Some("1".into()),
            }],
            registers_changed: HashMap::new(),
            timestamp_delta_ns: 1000,
        };
        let json = serde_json::to_value(&sd).unwrap();
        assert_eq!(json["event_id_a"], 1u64);
        assert_eq!(json["timestamp_delta_ns"], 1000u64);
    }
}
