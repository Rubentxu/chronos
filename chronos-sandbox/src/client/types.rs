//! Data types for MCP tool wrappers.
//!
//! These types represent the request/response structures for all MCP tools
//! exposed by the Chronos server.

use serde::{Deserialize, Serialize};

// ============================================================================
// Probe Tools
// ============================================================================

/// Parameters for probe_start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStartParams {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub trace_syscalls: bool,
    pub cwd: Option<String>,
    #[serde(default = "default_bus_capacity")]
    pub bus_capacity: usize,
}

fn default_true() -> bool {
    true
}

fn default_bus_capacity() -> usize {
    50000
}

/// Response from probe_start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStartResponse {
    pub session_id: String,
    pub status: String,
    pub target: String,
    pub language: String,
    pub bus_capacity: usize,
    pub hint: Option<String>,
}

/// Parameters for probe_stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStopParams {
    pub session_id: String,
}

/// Response from probe_stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStopResponse {
    pub session_id: String,
    pub status: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub total_events: usize,
    #[serde(default)]
    pub duration_ms: u64,
    pub hint: Option<String>,
}

/// Parameters for probe_drain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeDrainParams {
    pub session_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    1000
}

/// A semantic event from a live probe drain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEvent {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub language: String,
    pub kind: String,
    pub description: String,
}

/// Response from probe_drain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeDrainResponse {
    pub session_id: String,
    pub status: String,
    pub total_buffered: usize,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub events: Vec<SemanticEvent>,
    pub hint: Option<String>,
}

/// Parameters for probe_inject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeInjectParams {
    pub session_id: String,
    pub binary_path: String,
    pub symbol_name: String,
    pub pid: Option<u32>,
}

/// Response from probe_inject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeInjectResponse {
    pub session_id: String,
    pub binary_path: String,
    pub symbol_name: String,
    pub probes_attached: u32,
    pub message: String,
}

// ============================================================================
// Tripwire Tools
// ============================================================================

/// Condition types for tripwire creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TripwireConditionType {
    EventType {
        event_types: Vec<String>,
    },
    FunctionName {
        pattern: String,
    },
    ExceptionType {
        exc_type: String,
    },
    MemoryAddress {
        start: u64,
        end: u64,
    },
    SyscallNumber {
        numbers: Vec<u64>,
    },
    VariableName {
        name: String,
    },
    Signal {
        numbers: Vec<i32>,
    },
}

/// Parameters for tripwire_create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireCreateParams {
    pub condition: TripwireConditionType,
    pub label: Option<String>,
}

/// Response from tripwire_create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireCreateResponse {
    pub tripwire_id: String,
    pub status: String,
    pub active_count: usize,
    pub label: Option<String>,
}

/// Summary info for a tripwire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireInfo {
    pub id: String,
    pub label: Option<String>,
    pub condition: String,
    pub fire_count: usize,
}

/// A fired tripwire event notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireFiredEvent {
    pub tripwire_id: String,
    pub condition_description: String,
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
}

/// Response from tripwire_list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireListResponse {
    pub active_tripwires: Vec<TripwireInfo>,
    pub fired_events: Vec<TripwireFiredEvent>,
    pub total_active: usize,
    pub fired_count: usize,
}

/// Parameters for tripwire_delete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireDeleteParams {
    pub tripwire_id: String,
}

/// Response from tripwire_delete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireDeleteResponse {
    pub tripwire_id: String,
    pub status: String,
    pub remaining_active: usize,
}

/// Response from tripwire_query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireQueryResponse {
    pub active_tripwires: Vec<TripwireInfo>,
    pub total_active: usize,
}

// ============================================================================
// Query Tools
// ============================================================================

/// Filter parameters for query_events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub event_types: Option<Vec<String>>,
    pub thread_id: Option<u64>,
    pub timestamp_start: Option<u64>,
    pub timestamp_end: Option<u64>,
    pub function_pattern: Option<String>,
    #[serde(default = "default_query_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_query_limit() -> usize {
    100
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            function_pattern: None,
            limit: default_query_limit(),
            offset: 0,
        }
    }
}

/// A trace event returned from query operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub function: Option<String>,
    pub address: String,
}

/// Response from query_events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsResponse {
    #[serde(default)]
    pub total_matching: usize,
    #[serde(default)]
    pub returned_count: usize,
    #[serde(default)]
    pub next_offset: Option<usize>,
    #[serde(default)]
    pub events: Vec<TraceEvent>,
}

/// Response from get_event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEventResponse {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub location: SourceLocation,
    pub data: serde_json::Value,
}

/// Source location for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: u32,
    pub function: Option<String>,
    pub address: String,
}

/// A stack frame in a call stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    pub depth: u32,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub address: String,
}

/// Response from get_call_stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCallStackResponse {
    pub session_id: String,
    pub at_event_id: u64,
    pub depth: usize,
    pub frames: Vec<StackFrame>,
}

/// Thread information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub thread_id: u64,
    // Additional thread details may be present depending on the session
}

/// Response from list_threads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListThreadsResponse {
    pub session_id: String,
    pub thread_count: usize,
    pub thread_ids: Vec<u64>,
}

// ============================================================================
// Debug/Analysis Tools
// ============================================================================

/// Information about a detected crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashInfo {
    pub session_id: String,
    pub crash_found: bool,
    pub signal: Option<String>,
    pub event_id: Option<u64>,
    pub timestamp_ns: Option<u64>,
    pub thread_id: Option<u64>,
    pub call_stack_depth: Option<usize>,
    pub call_stack: Option<Vec<StackFrame>>,
    pub note: Option<String>,
}

/// Response from debug_find_crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugFindCrashResponse {
    pub session_id: String,
    pub crash_found: bool,
    pub signal: Option<String>,
    pub event_id: Option<u64>,
    pub timestamp_ns: Option<u64>,
    pub thread_id: Option<u64>,
    pub call_stack_depth: Option<usize>,
    pub call_stack: Option<Vec<StackFrame>>,
    pub note: Option<String>,
}

/// A write event in a race report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWriteInfo {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub value_before: Option<String>,
    pub value_after: Option<String>,
    pub function: Option<String>,
}

/// A detected data race.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceReport {
    pub address: String,
    pub delta_ns: u64,
    pub write_a: RaceWriteInfo,
    pub write_b: RaceWriteInfo,
}

/// Response from debug_detect_races.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugDetectRacesResponse {
    pub session_id: String,
    pub threshold_ns: u64,
    pub race_count: usize,
    pub races: Vec<RaceReport>,
}

/// A mutation entry in a causality report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalityMutation {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub value_before: Option<String>,
    pub value_after: Option<String>,
    pub function: Option<String>,
}

/// Response from inspect_causality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectCausalityResponse {
    pub session_id: String,
    pub address: String,
    pub mutation_count: usize,
    pub mutations: Vec<CausalityMutation>,
    pub note: Option<String>,
}

/// A hotspot function entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotDetail {
    pub function: String,
    pub call_count: u64,
    pub total_cycles: Option<u64>,
    pub avg_cycles_per_call: Option<f64>,
}

/// Response from debug_expand_hotspot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugExpandHotspotResponse {
    pub session_id: String,
    pub compression_level: String,
    pub top_n: usize,
    pub total_calls_in_trace: u64,
    pub hotspot_functions: Vec<HotspotDetail>,
    pub hint: Option<String>,
}

/// A function saliency score entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaliencyScore {
    pub function: String,
    pub saliency_score: f64,
    pub call_count: u64,
    pub total_cycles: Option<u64>,
    pub cycles: Option<u64>,
}

/// Response from debug_get_saliency_scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugGetSaliencyScoresResponse {
    pub session_id: String,
    pub scored_functions: usize,
    pub scores: Vec<SaliencyScore>,
    pub hint: Option<String>,
}

// ============================================================================
// Session/Persistence Tools
// ============================================================================

/// Metadata for a saved session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub language: String,
    pub target: String,
    pub event_count: usize,
    pub duration_ms: u64,
    pub created_at: u64,
    pub hint: Option<String>,
}

/// Parameters for save_session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionParams {
    pub session_id: String,
    pub language: String,
    pub target: String,
}

/// Response from save_session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSessionResponse {
    pub session_id: String,
    pub status: String,
    pub event_count: usize,
    pub hash_count: usize,
    pub language: String,
    pub target: String,
    pub duration_ms: u64,
    pub hint: Option<String>,
}

/// Response from load_session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSessionResponse {
    pub session_id: String,
    pub status: String,
    pub language: String,
    pub target: String,
    pub event_count: usize,
    pub duration_ms: u64,
    pub created_at: u64,
    pub hint: Option<String>,
}

/// A summary of a saved session (from list_sessions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub language: String,
    pub target: String,
    pub event_count: usize,
    pub duration_ms: u64,
    pub created_at: u64,
}

/// Response from list_sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsResponse {
    pub session_count: usize,
    pub sessions: Vec<SessionSummary>,
}

/// Response from delete_session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteSessionResponse {
    pub session_id: String,
    pub status: String,
    pub message: String,
}

/// Response from compare_sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareSessionsResponse {
    pub session_a_id: String,
    pub session_b_id: String,
    pub only_in_a_count: usize,
    pub only_in_b_count: usize,
    pub total_a: usize,
    pub total_b: usize,
    pub common_count: usize,
    pub similarity_pct: f64,
    pub timing_delta_ms: Option<i64>,
    pub summary: String,
}

/// A function regression entry in performance audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionRegressionEntry {
    pub function: String,
    pub baseline_calls: u64,
    pub target_calls: u64,
    pub call_delta_pct: f64,
}

/// Response from performance_regression_audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRegressionAuditResponse {
    pub baseline_session_id: String,
    pub target_session_id: String,
    pub regressions: Vec<FunctionRegressionEntry>,
    pub improvements: Vec<FunctionRegressionEntry>,
    pub functions_analyzed: usize,
    pub total_call_delta: i64,
    pub summary: String,
}

// ============================================================================
// Execution Summary
// ============================================================================

/// Top function info in execution summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopFunction {
    pub name: String,
    pub call_count: u64,
}

/// Event count by type entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCountByType {
    pub event_type: String,
    pub count: u64,
}

/// A potential issue detected in the trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialIssue {
    pub issue_type: String,
    pub confidence: f32,
    pub description: String,
}

/// Response from get_execution_summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummaryResponse {
    pub session_id: String,
    pub duration_ns: u64,
    pub total_events: u64,
    pub event_counts_by_type: Vec<EventCountByType>,
    pub top_functions: Vec<TopFunction>,
    pub thread_count: u64,
    pub potential_issues: Vec<PotentialIssue>,
}

/// A node in the call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphNode {
    pub function: String,
    pub call_count: u64,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
}

/// Response from debug_call_graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphResponse {
    pub session_id: String,
    pub max_depth: usize,
    pub unique_functions: usize,
    pub nodes: Vec<CallGraphNode>,
}

/// A state change between two timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub field: String,
    pub value_a: String,
    pub value_b: String,
}

/// Response from state_diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiffResponse {
    pub timestamp_a: u64,
    pub timestamp_b: u64,
    pub changes: Vec<StateChange>,
}

// ============================================================================
// Extended Debug/Analysis Tools (Phase 6.1)
// ============================================================================

/// CPU register state at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registers {
    pub session_id: String,
    pub thread_id: u64,
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub values: std::collections::HashMap<String, u64>,
}

/// Response from debug_get_registers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugGetRegistersResponse {
    pub session_id: String,
    pub thread_id: u64,
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub registers: std::collections::HashMap<String, u64>,
}

/// Variable information at a frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub name: String,
    pub value: String,
    pub var_type: Option<String>,
    pub address: Option<String>,
}

/// Response from debug_get_variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugGetVariablesResponse {
    pub event_id: u64,
    pub variables: Vec<VariableInfo>,
}

/// Response from debug_get_memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugGetMemoryResponse {
    pub session_id: String,
    pub address: u64,
    pub size: usize,
    pub data: Vec<u8>,
}

/// Diff result between two events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub session_id: String,
    pub event_a_id: u64,
    pub event_b_id: u64,
    pub registers_diff: Option<std::collections::HashMap<String, (u64, u64)>>,
    pub memory_diff: Option<Vec<MemoryDiffEntry>>,
    pub summary: String,
}

/// A memory region difference entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDiffEntry {
    pub address: u64,
    pub value_before: Option<u8>,
    pub value_after: Option<u8>,
}

/// Response from debug_diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugDiffResponse {
    pub session_id: String,
    pub event_a_id: u64,
    pub event_b_id: u64,
    pub registers_diff: Option<std::collections::HashMap<String, (u64, u64)>>,
    pub memory_diff: Option<Vec<MemoryDiffEntry>>,
    pub summary: String,
}

/// Response from evaluate_expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateExpressionResponse {
    pub session_id: String,
    pub expression: String,
    pub result: serde_json::Value,
    pub result_type: Option<String>,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: Option<String>,
    pub uptime_ms: Option<u64>,
}
