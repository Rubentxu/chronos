//! Output data structures for debug-read service operations.
//!
//! These structs are the return types of [`DebugReadService`](super::debug_read::DebugReadService).
//! All types derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`
//! so they can cross RPC boundaries cleanly.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a trace event query.
///
/// A thin wrapper around [`chronos_domain::query::QueryResult`] that carries pagination
/// metadata (`total_matching`, `next_offset`) in addition to the event list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsResult {
    /// The raw query result from the engine.
    pub result: chronos_domain::query::QueryResult,
}

impl PartialEq for QueryEventsResult {
    fn eq(&self, other: &Self) -> bool {
        self.result.total_matching == other.result.total_matching
            && self.result.events.len() == other.result.events.len()
            && self.result.next_offset == other.result.next_offset
    }
}

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
// Session-lifecycle output types
// ---------------------------------------------------------------------------

/// Result of saving a session to persistent storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveResult {
    /// Number of events saved.
    pub event_count: usize,
    /// Number of unique content hashes stored (dedup).
    pub hash_count: usize,
    /// Language/runtime of the target.
    pub language: String,
    /// Target program path or name.
    pub target: String,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}

/// Result of loading a session from persistent storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadResult {
    /// Language/runtime of the target.
    pub language: String,
    /// Target program path or name.
    pub target: String,
    /// Number of events loaded.
    pub event_count: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Unix timestamp ms when the session was created.
    pub created_at: u64,
}

/// Summary metadata for one saved session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session identifier.
    pub session_id: String,
    /// Language/runtime of the target.
    pub language: String,
    /// Target program path or name.
    pub target: String,
    /// Number of events in the session.
    pub event_count: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Unix timestamp ms when the session was created.
    pub created_at: u64,
}

/// Result of listing all saved sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResult {
    /// All saved sessions.
    pub sessions: Vec<SessionSummary>,
}

/// Result of deleting a session from persistent storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteResult {
    /// Deleted session identifier.
    pub session_id: String,
}

/// Result of dropping a session from in-memory state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropResult {
    /// Dropped session identifier.
    pub session_id: String,
    /// Whether the session existed in memory before the drop.
    pub existed: bool,
}

// ---------------------------------------------------------------------------
// Tripwire output types
// ---------------------------------------------------------------------------

/// Result of creating a new tripwire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateResult {
    /// The assigned tripwire ID string (e.g. "tripwire-1").
    pub tripwire_id: String,
    /// Total number of active tripwires after registration.
    pub active_count: usize,
    /// The label supplied at creation time, if any.
    pub label: Option<String>,
}

/// Summary of one active tripwire, returned by list/query operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireSummary {
    /// Tripwire ID string (e.g. "tripwire-1").
    pub id: String,
    /// Human-readable label, if set.
    pub label: Option<String>,
    /// Human-readable condition description.
    pub condition: String,
    /// How many times this tripwire has fired.
    pub fire_count: u64,
}

/// A single tripwire-fire notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireFiredSummary {
    /// ID of the tripwire that fired.
    pub tripwire_id: String,
    /// Human-readable condition description at time of firing.
    pub condition_description: String,
    /// Source trace-event ID.
    pub event_id: u64,
    /// Nanosecond timestamp of the event.
    pub timestamp_ns: u64,
    /// Thread ID of the event.
    pub thread_id: u64,
}

/// Result of listing active tripwires and draining fired events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireListResult {
    /// All currently registered tripwires.
    pub tripwires: Vec<TripwireSummary>,
    /// Fired notifications drained from the buffer.
    pub fired_events: Vec<TripwireFiredSummary>,
    /// Total number of active tripwires.
    pub total_active: usize,
    /// Number of fired events returned.
    pub fired_count: usize,
}

/// Result of querying active tripwires without draining fired events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// All currently registered tripwires.
    pub tripwires: Vec<TripwireSummary>,
    /// Total number of active tripwires.
    pub total_active: usize,
}

/// Result of deleting a tripwire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripwireDeleteResult {
    /// ID of the deleted tripwire.
    pub tripwire_id: String,
    /// Number of active tripwires remaining after deletion.
    pub remaining_active: usize,
}

// ---------------------------------------------------------------------------
// Debug-trace specialized output types
// ---------------------------------------------------------------------------

/// A single mutation / write event in a variable or address lineage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineageEntry {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub value_before: Option<String>,
    pub value_after: String,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl From<chronos_domain::query::MutationRecord> for LineageEntry {
    fn from(m: chronos_domain::query::MutationRecord) -> Self {
        LineageEntry {
            event_id: m.event_id,
            timestamp_ns: m.timestamp,
            thread_id: m.thread_id,
            value_before: m.value_before,
            value_after: m.value_after,
            function: m.function,
            file: m.file,
            line: m.line,
        }
    }
}

/// Result of the `debug_find_variable_origin` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableOriginResult {
    pub session_id: String,
    pub variable_name: String,
    pub mutation_count: usize,
    pub mutations: Vec<LineageEntry>,
    /// Set when the engine returned no causality result (no index or no writes).
    pub note: Option<String>,
}

/// A reconstructed stack frame at a crash event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashStackFrame {
    pub depth: u32,
    pub function: String,
    pub address: u64,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl From<chronos_domain::query::StackFrame> for CrashStackFrame {
    fn from(sf: chronos_domain::query::StackFrame) -> Self {
        CrashStackFrame {
            depth: sf.depth,
            function: sf.function,
            address: sf.address,
            file: sf.file,
            line: sf.line,
        }
    }
}

/// Result of the `debug_find_crash` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashPoint {
    pub session_id: String,
    pub crash_found: bool,
    pub signal: String,
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub call_stack_depth: usize,
    pub call_stack: Vec<CrashStackFrame>,
    /// Present only when crash_found is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Result of the `debug_detect_races` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceReport {
    pub session_id: String,
    pub threshold_ns: u64,
    /// Number of suspicious accesses found.
    pub access_count: usize,
    /// Raw accesses from the engine (contains write pairs with delta_ns).
    pub accesses: Vec<chronos_domain::query::SuspiciousConcurrentAccess>,
    /// Total addresses checked.
    pub total_writes: usize,
    /// Extracted (function_a, function_b) pairs for quick triage.
    pub suspicious_pairs: Vec<(String, String)>,
    /// Human-readable summary.
    pub summary: String,
}

/// Result of the `inspect_causality` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalityReport {
    pub session_id: String,
    pub address: u64,
    pub mutation_count: usize,
    pub mutations: Vec<LineageEntry>,
    /// Set when the engine returned no causality result.
    pub note: Option<String>,
}

/// A single hotspot function entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotEntry {
    pub function: String,
    pub call_count: u64,
    pub total_cycles: Option<u64>,
    pub avg_cycles_per_call: Option<f64>,
}

/// Result of the `debug_expand_hotspot` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotReport {
    pub session_id: String,
    pub compression_level: String,
    pub top_n: usize,
    pub total_calls_in_trace: u64,
    pub hotspot_functions: Vec<HotspotEntry>,
    pub hint: Option<String>,
}

/// A single function saliency score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaliencyScore {
    pub function: String,
    /// Score in [0.0, 1.0], four decimal places.
    pub saliency_score: f64,
    pub call_count: u64,
    /// Total cycles (only present when perf counters were available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cycles: Option<u64>,
    /// Cycles field emitted when perf counters were NOT available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<()>,
}

/// Result of the `debug_get_saliency_scores` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaliencyScoreResult {
    pub session_id: String,
    pub scored_functions: usize,
    pub scores: Vec<SaliencyScore>,
    pub hint: Option<String>,
}

// ---------------------------------------------------------------------------
// Serde round-trip tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Probe service output types
// ---------------------------------------------------------------------------

/// A cursor for non-destructive probe drainage (output side).
///
/// Mirrors the JSON shape produced by `probe_drain`'s existing `serde_json::json!({...})`
/// output (`cursor.total_pushed`, `cursor.snapshot_len`, `cursor.cursor_stale`).
///
/// Named `ProbeCursorDto` to avoid collision with the input-side `CursorDto` in
/// `chronos-mcp::server` (which carries `Option<u64>` fields for parsing
/// malformed payloads).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeCursorDto {
    pub total_pushed: u64,
    pub snapshot_len: usize,
}

/// A single event as returned by `probe_drain` (the JSON-shape consumed by
/// the LLM-facing probe tools).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainedEventDto {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub language: String,
    pub kind: String,
    pub description: String,
}

/// Output of `probe_start`. JSON shape matches the existing
/// `serde_json::json!({...})` literal in `chronos-mcp/src/server.rs::probe_start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeStartOutput {
    pub session_id: String,
    pub status: String,
    pub target: String,
    pub language: String,
    pub bus_capacity: usize,
    pub hint: String,
}

/// Output of `probe_stop`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_stop`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeStopOutput {
    pub session_id: String,
    pub status: String,
    pub target: String,
    pub total_events: usize,
    pub duration_ms: u64,
    pub ebpf_detached: bool,
    pub hint: String,
}

/// Output of `probe_drain`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_drain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeDrainOutput {
    pub session_id: String,
    pub status: String,
    pub total_buffered: usize,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub cursor: ProbeCursorDto,
    pub cursor_stale: bool,
    pub tripwires_fired: usize,
    pub events: Vec<DrainedEventDto>,
    pub hint: String,
}

/// Output of `probe_drain_log`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_drain_log`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeDrainLogOutput {
    pub session_id: String,
    pub returned: usize,
    pub tail_seq: Option<u64>,
    pub records: Vec<serde_json::Value>,
}

/// Output of `probe_compaction_metrics`. JSON shape matches the existing
/// literal in `chronos-mcp/src/server.rs::probe_compaction_metrics`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactionMetricsOutput {
    pub session_id: String,
    pub metrics: serde_json::Value,
}

/// Output of `session_snapshot`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::session_snapshot`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshotOutput {
    pub session_id: String,
    pub total_events: usize,
    pub hint: String,
}

/// Output of `probe_inject`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_inject`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeInjectOutput {
    pub session_id: String,
    pub binary_path: String,
    pub symbol_name: String,
    pub pid: u32,
    pub attached: bool,
    pub error: Option<String>,
    pub hint: String,
}

/// Output of `probe_status`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeStatusOutput {
    pub session_id: String,
    pub target: String,
    pub language: String,
    pub status: String,
    pub ebpf_attached: bool,
    pub ebpf_binary: Option<String>,
    pub ebpf_symbol: Option<String>,
    pub ebpf_pid: Option<u32>,
    pub hint: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

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
    fn query_events_result_roundtrips() {
        let events_result = QueryEventsResult {
            result: chronos_domain::query::QueryResult {
                total_matching: 42,
                events: vec![chronos_domain::TraceEvent {
                    event_id: 1,
                    timestamp_ns: 1000,
                    thread_id: 1,
                    event_type: EventType::FunctionEntry,
                    location: SourceLocation::default(),
                    data: EventData::Empty,
                }],
                next_offset: Some(100),
            },
        };
        let json = serde_json::to_value(&events_result).unwrap();
        assert_eq!(json["result"]["total_matching"], 42);
        assert_eq!(json["result"]["next_offset"], 100);
        assert_eq!(json["result"]["events"].as_array().unwrap().len(), 1);
        let round: QueryEventsResult = serde_json::from_value(json).unwrap();
        assert_eq!(round.result.total_matching, 42);
        assert_eq!(round.result.events.len(), 1);
        assert_eq!(round.result.next_offset, Some(100));
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

    #[test]
    fn save_result_roundtrips() {
        let sr = SaveResult {
            event_count: 42,
            hash_count: 38,
            language: "python".into(),
            target: "/usr/bin/python3".into(),
            duration_ms: 1234,
        };
        let json = serde_json::to_value(&sr).unwrap();
        assert_eq!(json["event_count"], 42u64);
        assert_eq!(json["hash_count"], 38u64);
        assert_eq!(json["language"], "python");
    }

    #[test]
    fn load_result_roundtrips() {
        let lr = LoadResult {
            language: "go".into(),
            target: "./server".into(),
            event_count: 100,
            duration_ms: 5000,
            created_at: 1_700_000_000_000,
        };
        let json = serde_json::to_value(&lr).unwrap();
        assert_eq!(json["language"], "go");
        assert_eq!(json["event_count"], 100u64);
    }

    #[test]
    fn list_result_roundtrips() {
        let lr = ListResult {
            sessions: vec![SessionSummary {
                session_id: "s1".into(),
                language: "c".into(),
                target: "main".into(),
                event_count: 10,
                duration_ms: 100,
                created_at: 1_700_000_000_000,
            }],
        };
        let json = serde_json::to_value(&lr).unwrap();
        assert_eq!(json["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(json["sessions"][0]["session_id"], "s1");
    }

    // ---- Tripwire output type round-trip tests ----

    #[test]
    fn tripwire_create_result_roundtrips() {
        let cr = CreateResult {
            tripwire_id: "tripwire-5".into(),
            active_count: 3,
            label: Some("my-watch".into()),
        };
        let json = serde_json::to_value(&cr).unwrap();
        assert_eq!(json["tripwire_id"], "tripwire-5");
        assert_eq!(json["active_count"], 3);
        assert_eq!(json["label"], "my-watch");
        let round = serde_json::from_value::<CreateResult>(json).unwrap();
        assert_eq!(round.tripwire_id, "tripwire-5");
        assert_eq!(round.active_count, 3);
        assert_eq!(round.label.as_deref(), Some("my-watch"));
    }

    #[test]
    fn tripwire_summary_roundtrips() {
        let ts = TripwireSummary {
            id: "tripwire-1".into(),
            label: None,
            condition: "EventType([FunctionEntry])".into(),
            fire_count: 7,
        };
        let json = serde_json::to_value(&ts).unwrap();
        assert_eq!(json["id"], "tripwire-1");
        assert_eq!(json["fire_count"], 7u64);
        assert!(json["label"].is_null());
        let round = serde_json::from_value::<TripwireSummary>(json).unwrap();
        assert_eq!(round.id, "tripwire-1");
        assert_eq!(round.fire_count, 7);
    }

    #[test]
    fn tripwire_fired_summary_roundtrips() {
        let tf = TripwireFiredSummary {
            tripwire_id: "tripwire-2".into(),
            condition_description: "FunctionName { pattern: \"main\" }".into(),
            event_id: 99,
            timestamp_ns: 1_000_000_000,
            thread_id: 42,
        };
        let json = serde_json::to_value(&tf).unwrap();
        assert_eq!(json["tripwire_id"], "tripwire-2");
        assert_eq!(json["event_id"], 99u64);
        let round = serde_json::from_value::<TripwireFiredSummary>(json).unwrap();
        assert_eq!(round.event_id, 99);
        assert_eq!(round.thread_id, 42);
    }

    #[test]
    fn tripwire_list_result_roundtrips() {
        use super::{TripwireFiredSummary, TripwireListResult, TripwireSummary};
        let lr = TripwireListResult {
            tripwires: vec![TripwireSummary {
                id: "tripwire-1".into(),
                label: Some("main-watch".into()),
                condition: "EventType([FunctionEntry])".into(),
                fire_count: 3,
            }],
            fired_events: vec![TripwireFiredSummary {
                tripwire_id: "tripwire-1".into(),
                condition_description: "EventType([FunctionEntry])".into(),
                event_id: 50,
                timestamp_ns: 500_000_000,
                thread_id: 1,
            }],
            total_active: 1,
            fired_count: 1,
        };
        let json = serde_json::to_value(&lr).unwrap();
        assert_eq!(json["total_active"], 1u64);
        assert_eq!(json["fired_count"], 1u64);
        assert_eq!(json["tripwires"][0]["id"], "tripwire-1");
        assert_eq!(json["fired_events"][0]["event_id"], 50u64);
        let round = serde_json::from_value::<TripwireListResult>(json).unwrap();
        assert_eq!(round.total_active, 1);
        assert_eq!(round.fired_count, 1);
    }

    #[test]
    fn tripwire_query_result_roundtrips() {
        use super::QueryResult as TwQueryResult;
        let qr = TwQueryResult {
            tripwires: vec![
                super::TripwireSummary {
                    id: "tripwire-1".into(),
                    label: None,
                    condition: "FunctionName { pattern: \"*\" }".into(),
                    fire_count: 0,
                },
                super::TripwireSummary {
                    id: "tripwire-2".into(),
                    label: Some("sigsegv".into()),
                    condition: "Signal { numbers: [11] }".into(),
                    fire_count: 5,
                },
            ],
            total_active: 2,
        };
        let json = serde_json::to_value(&qr).unwrap();
        assert_eq!(json["total_active"], 2u64);
        assert_eq!(json["tripwires"].as_array().unwrap().len(), 2);
        let round = serde_json::from_value::<TwQueryResult>(json).unwrap();
        assert_eq!(round.total_active, 2);
        assert_eq!(round.tripwires[1].label.as_deref(), Some("sigsegv"));
    }

    #[test]
    fn tripwire_delete_result_roundtrips() {
        let dr = super::TripwireDeleteResult {
            tripwire_id: "tripwire-3".into(),
            remaining_active: 4,
        };
        let json = serde_json::to_value(&dr).unwrap();
        assert_eq!(json["tripwire_id"], "tripwire-3");
        assert_eq!(json["remaining_active"], 4u64);
        let round = serde_json::from_value::<super::TripwireDeleteResult>(json).unwrap();
        assert_eq!(round.tripwire_id, "tripwire-3");
        assert_eq!(round.remaining_active, 4);
    }
}
