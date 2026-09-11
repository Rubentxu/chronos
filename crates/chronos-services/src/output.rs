//! Output data structures for debug-read service operations.
//!
//! These structs are the return types of [`DebugReadService`](super::debug_read::DebugReadService).
//! All types derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`
//! so they can cross RPC boundaries cleanly.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::debug_trace::CallGraph;
use chronos_domain::query::{ExecutionSummary, StackFrame, StateDiff};
use chronos_store::SessionMetadata;

// Re-exports so MCP wrappers can refer to property types via
// `chronos_services::output::ComparisonOp` / `::PropertyValue`
// without importing from `chronos_domain` directly.
pub use chronos_domain::property::{ComparisonOp, PropertyValue};

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
// Observe (m7-02) v2 dispatcher output types
// ---------------------------------------------------------------------------

/// Verb discriminator for the unified v2 `observe` tool.
///
/// Folds 5 v1 tools (`tripwire_create`, `tripwire_list`,
/// `tripwire_delete`, `tripwire_query`, `probe_inject`) behind a single
/// endpoint. See `docs/milestones/m7-02-observability-merge.md` for
/// the full spec.
///
/// `Update` is reserved (rejected with `ServiceError::Unsupported` in
/// m7-02) because no v1 caller demands it today; deferring to m7+ keeps
/// the v2 surface minimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObserveVerb {
    /// Register a new subscription (tripwire condition or uprobe-injecting tripwire).
    Create,
    /// Enumerate subscriptions + drain fired events (destructive).
    List,
    /// Reserved; rejected with `Unsupported` in m7-02.
    Update,
    /// Unregister a subscription.
    Delete,
    /// Non-destructive snapshot of subscription state.
    Query,
}

/// Discriminator for the condition body of an `observe` request.
///
/// Tripwire conditions carry an already-parsed
/// [`chronos_domain::tripwire::TripwireCondition`]. The MCP layer is
/// responsible for parsing the v1-style JSON (`TripwireConditionType`)
/// into the domain type before handing the request to the dispatcher.
/// `Uprobe` carries `(binary_path, symbol_name, pid)` and routes through
/// `ProbeService::inject` at subscription creation time (matching
/// today's `probe_inject` semantics).
#[derive(Debug, Clone)]
pub enum ObserveCondition {
    /// Tripwire-style condition (event-type filter, function-name glob,
    /// memory range, syscall numbers, variable name, signal).
    Tripwire {
        /// Parsed domain condition.
        condition: chronos_domain::tripwire::TripwireCondition,
        /// Optional human-readable label.
        label: Option<String>,
    },
    /// Uprobe-injection condition (binary + symbol + pid).
    Uprobe {
        /// Path to the binary or shared library.
        binary_path: String,
        /// Symbol name to attach the uprobe to.
        symbol_name: String,
        /// Optional PID override (defaults to the probe session's traced PID).
        pid: Option<u32>,
        /// Optional human-readable label.
        label: Option<String>,
    },
}

/// What to do when a subscription fires.
///
/// `Record` captures the firing event in the tripwire manager's
/// internal buffer (the v1 default). `Notify` is identical to `Record`
/// today — the distinction exists so the v2 surface can grow streaming
/// notifications without an API break in m7+. `InjectUprobe` is the
/// only action that has *no* v1 equivalent semantics today (it is a
/// placeholder for true fire-on-condition uprobe injection, deferred to
/// a domain-layer change; m7-02 only honours it as a parsed-but-no-op
/// branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObserveAction {
    /// Capture the firing event into the tripwire buffer.
    Record,
    /// Same as `Record` today; reserved for streaming notifications in m7+.
    Notify,
    /// Reserved for fire-on-condition uprobe injection (deferred to m7+).
    /// Parsed but no-op in m7-02 — the uprobe is attached once at
    /// subscription creation when the `ObserveCondition::Uprobe` variant
    /// is used.
    InjectUprobe,
}

/// Retention policy for fired events.
///
/// `Drained` matches v1 `tripwire_list` behaviour (destructive read).
/// `RetainedUntilSessionEnd` keeps fired events in the buffer until
/// the session terminates. `Permanent` is reserved (rejected with
/// `Unsupported` in m7-02) — the tripwire manager does not currently
/// distinguish permanent retention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObserveRetention {
    /// Drain fired events on the next `verb=list` (default, matches v1).
    #[default]
    Drained,
    /// Keep fired events in the buffer until the session ends.
    RetainedUntilSessionEnd,
    /// Reserved; rejected with `Unsupported` in m7-02.
    Permanent,
}

/// Requested evidence for a subscription.
///
/// m7-02 only honours `EventTypes`; `Properties` is rejected with
/// `ServiceError::Unsupported` because the domain layer does not yet
/// expose property snapshots (same gap as m7-01 `gap_summary=None`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObserveRequestedEvidence {
    /// Capture events matching the given `event_types` filter.
    EventTypes {
        /// Event-type name list (e.g. `["function_entry", "exception"]`).
        event_types: Vec<String>,
    },
    /// Reserved; rejected with `Unsupported` in m7-02.
    Properties {
        /// Property names to project (deferred to m7+).
        names: Vec<String>,
    },
}

/// Scope discriminator — attaches a subscription to a session or
/// makes it global (across all live sessions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ObserveScope {
    /// Subscription attached to a specific session id.
    Session {
        /// Session id (required when `scope=session`).
        session_id: String,
    },
    /// Subscription applies to every live session.
    Global,
}

/// Input for [`crate::observe::ChronosObserveService::observe`].
///
/// Carries the v2 `verb` discriminator plus the verb-specific fields.
/// The dispatcher validates verb / field consistency (e.g. `update` is
/// rejected outright; `delete` + `query` require a `subscription_id`;
/// `create` requires a `condition`).
#[derive(Debug, Clone)]
pub struct ObserveInput {
    /// Which verb to dispatch (create | list | update | delete | query).
    pub verb: ObserveVerb,
    /// Subscription id (required for `update`, `delete`, `query`).
    pub subscription_id: Option<String>,
    /// Subscription body (required for `create`).
    pub condition: Option<ObserveCondition>,
    /// What to do when a subscription fires (optional; defaults to `Record`).
    pub action: Option<ObserveAction>,
    /// Retention policy (optional; defaults to `Drained`).
    pub retention: Option<ObserveRetention>,
    /// Requested evidence filter (optional; defaults to `EventTypes` with `[]`).
    pub requested_evidence: Option<ObserveRequestedEvidence>,
    /// Scope (session id or global). Required for `create`.
    pub scope: Option<ObserveScope>,
    /// Optional cursor for `verb=list` (matches the m7-01 cursor pattern).
    pub cursor: Option<crate::output::CursorDto>,
    /// Optional human-readable label (alternative to `condition.label`).
    pub label: Option<String>,
}

/// Payload returned by `verb=create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveCreateResult {
    /// Assigned subscription id (`tripwire-<n>` for tripwire conditions,
    /// `uprobe-<session>-<n>` for uprobe conditions).
    pub subscription_id: String,
    /// Subscription kind (`tripwire` or `uprobe`).
    pub kind: String,
    /// Status string (`"registered"` on success).
    pub status: String,
    /// Total number of active subscriptions of this kind after registration.
    pub active_count: usize,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// For uprobe subscriptions: the PID the uprobe was attached to (if any).
    /// `None` for tripwire-only subscriptions or when the probe is still starting.
    pub attached_pid: Option<u32>,
}

/// Payload returned by `verb=list` (destructive) and `verb=query`
/// (non-destructive). When `verb=list`, `fired_events` is drained.
/// When `verb=query`, `fired_events` is `[]` (the buffer is intact).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveListResult {
    /// All currently registered subscriptions.
    pub subscriptions: Vec<SubscriptionDto>,
    /// Fired events (drained on `verb=list`, empty on `verb=query`).
    pub fired_events: Vec<TripwireFiredSummary>,
    /// Total number of active subscriptions.
    pub total_active: usize,
    /// Number of fired events returned.
    pub fired_count: usize,
    /// Next cursor (only set when more pages exist and `cursor` was supplied).
    pub next_cursor: Option<crate::output::CursorDto>,
    /// Provenance / source info (engine version + retention used).
    pub provenance: ObserveProvenance,
}

/// Payload returned by `verb=delete`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveDeleteResult {
    /// ID of the deleted subscription.
    pub subscription_id: String,
    /// Number of active subscriptions remaining after deletion.
    pub remaining_active: usize,
}

/// Provenance info attached to list/query responses. Matches the
/// m7-01 `EventsReadProvenance` shape (engine_version + query_strategy).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObserveProvenance {
    /// Chronos engine version string (placeholder until the domain exposes one).
    pub engine_version: String,
    /// Strategy used for the read (`IndexLookup` for indexed, `FullScan` otherwise).
    pub query_strategy: String,
    /// Retention policy in effect (echo of the request default).
    pub retention_in_effect: String,
}

/// One subscription in the list/query response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionDto {
    /// Subscription id string.
    pub id: String,
    /// Kind string (`tripwire` or `uprobe`).
    pub kind: String,
    /// Optional label.
    pub label: Option<String>,
    /// Human-readable condition description.
    pub condition: String,
    /// How many times this subscription has fired.
    pub fire_count: u64,
}

/// Tagged output envelope returned by [`crate::observe::ChronosObserveService::observe`].
///
/// Each verb maps to exactly one variant. The MCP wrapper destructures
/// on `kind` to produce the JSON shape for each verb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObserveOutput {
    /// `verb=create` response.
    Create(ObserveCreateResult),
    /// `verb=list` response (destructive).
    List(ObserveListResult),
    /// `verb=delete` response.
    Delete(ObserveDeleteResult),
    /// `verb=query` response (non-destructive; `fired_events` is `[]`).
    Query(ObserveListResult),
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

/// Result of `ProbeService::stop` — the raw events + metadata needed by the
/// server-side wrapper to call `build_and_store_engine`. The wrapper then
/// turns this into the byte-identical MCP JSON shape.
#[derive(Debug)]
pub struct ProbeStopResult {
    /// Final raw events drained from the live probe bus.
    pub events: Vec<chronos_domain::TraceEvent>,
    /// Language recorded when the probe was started.
    pub language: chronos_domain::Language,
    /// Original target binary path.
    pub target: String,
    /// Total number of events drained.
    pub total_events: usize,
    /// Wall-clock duration of the probe (ns difference between first and last
    /// event, in milliseconds).
    pub duration_ms: u64,
    /// Whether the session had an eBPF attachment that was detached.
    pub ebpf_detached: bool,
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

/// Result of `ProbeService::drain` — semantic events + cursor metadata so the
/// server wrapper can serialize them into the byte-identical JSON shape.
#[derive(Debug)]
pub struct ProbeDrainResult {
    /// Semantic events returned by the backend's `read_since`.
    pub events: Vec<chronos_domain::SemanticEvent>,
    /// New cursor after this drain call.
    pub new_cursor: chronos_domain::EventCursor,
    /// Whether the cursor is stale (caller should re-anchor).
    pub cursor_stale: bool,
    /// Total events in the buffer (before offset/limit).
    pub total_buffered: usize,
    /// Number of tripwires fired during this drain.
    pub tripwires_fired: usize,
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
    pub events_indexed: usize,
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
    pub probes_attached: u32,
    pub message: String,
}

/// Output of `probe_status`. JSON shape matches the existing literal in
/// `chronos-mcp/src/server.rs::probe_status`. The shape is a snapshot of the
/// session — the wrapper turns the enum into the JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeStatusOutput {
    pub session_id: String,
    pub language: chronos_domain::Language,
    pub target: String,
    pub traced_pid: u32,
    /// `Some(json)` when an eBPF attachment is recorded, `None` otherwise.
    pub ebpf: Option<serde_json::Value>,
    pub state: String,
}

// ============================================================================
// Browser probe outputs (m5-07)
// ============================================================================

/// Output of `browser_probe_start`. Mirrors the JSON shape produced by the
/// legacy in-server implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserProbeStartOutput {
    pub session_id: String,
    pub status: String,
    pub url: String,
    pub hint: String,
}

/// Output of `browser_probe_stop`. Mirrors the JSON shape produced by the
/// legacy in-server implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserProbeStopOutput {
    pub session_id: String,
    pub status: String,
    pub url: String,
    pub total_events: usize,
    pub hint: String,
}

/// A single semantic event drained from a browser probe adapter. The shape
/// matches the JSON object emitted by the legacy `browser_probe_drain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainedBrowserEventDto {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub language: String,
    pub kind: String,
    pub description: String,
}

/// Output of `browser_probe_drain`. Mirrors the JSON shape produced by the
/// legacy in-server implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserProbeDrainOutput {
    pub session_id: String,
    pub status: String,
    pub total_buffered: usize,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub events: Vec<DrainedBrowserEventDto>,
    pub hint: String,
}

// ============================================================================
// M3 — Mutation Lens & Causal Slice outputs (m5-08)
// ============================================================================

/// Output of `mutation_lens`. `transitions` carry the full
/// [`StateTransition`](chronos_domain::property::StateTransition) records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationLensOutput {
    pub session_id: String,
    pub count: usize,
    pub transitions: Vec<chronos_domain::property::StateTransition>,
}

/// Output of `causal_slice`. `included` lists backward-reachable event ids;
/// `missing` lists those whose evidence was unobserved (never silently dropped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausalSliceOutput {
    pub session_id: String,
    pub sink: u64,
    pub included: Vec<u64>,
    pub missing: Vec<u64>,
    pub depth: usize,
}

// ============================================================================
// M5 — Diff & Compare outputs (m5-09)
// ============================================================================

// ============================================================================
// M6 — Trace Slice outputs (m6-01)
// ============================================================================

/// Discriminator for [`TraceSliceOutput`]. Selects which v1 tool's payload
/// is produced by the v2 `trace_slice` dispatcher.
///
/// The four variants correspond to the four v1 tools that this v2 tool
/// supersedes (see `docs/milestones/m5-close-report.md` §4.1):
/// - `VariableOrigin` — formerly `debug_find_variable_origin`.
/// - `Crash` — formerly `debug_find_crash`.
/// - `Causality` — formerly `inspect_causality`.
/// - `MemoryAudit` — formerly `forensic_memory_audit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum TraceSliceKind {
    VariableOrigin,
    Crash,
    Causality,
    MemoryAudit,
}

impl TraceSliceKind {
    /// Snake-case name used as the JSON discriminator tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            TraceSliceKind::VariableOrigin => "variable_origin",
            TraceSliceKind::Crash => "crash",
            TraceSliceKind::Causality => "causality",
            TraceSliceKind::MemoryAudit => "memory_audit",
        }
    }
}

/// Output envelope of the v2 `trace_slice` tool.
///
/// The `slice_kind` tag tells the consumer which payload variant follows.
/// Each variant's inner DTO is flattened into the envelope so that the
/// resulting JSON preserves byte-for-byte the shape of the corresponding
/// v1 tool's output, plus a top-level `slice_kind` discriminator and an
/// explicit `session_id` echo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "slice_kind", rename_all = "snake_case")]
pub enum TraceSliceOutput {
    #[serde(rename = "variable_origin")]
    VariableOrigin {
        session_id: String,
        #[serde(flatten)]
        result: VariableOriginResult,
    },
    #[serde(rename = "crash")]
    Crash {
        session_id: String,
        #[serde(flatten)]
        result: CrashPoint,
    },
    #[serde(rename = "causality")]
    Causality {
        session_id: String,
        #[serde(flatten)]
        result: CausalityReport,
    },
    #[serde(rename = "memory_audit")]
    MemoryAudit {
        session_id: String,
        #[serde(flatten)]
        result: MemoryAudit,
    },
}

// ============================================================================
// M6 — State Query outputs (m6-02)
// ============================================================================

/// Discriminator for [`StateQueryOutput`]. Selects which v1 tool's payload
/// is produced by the v2 `state_query` dispatcher.
///
/// The five variants correspond to the five v1 tools that this v2 tool
/// supersedes (see `docs/milestones/m5-close-report.md` §4.1):
/// - `RegisterDiff` — formerly `state_diff`.
/// - `MemoryRead` — formerly `debug_get_memory`.
/// - `RegisterSnapshot` — formerly `debug_get_registers`.
/// - `MemoryAnalysis` — formerly `debug_analyze_memory`.
/// - `ExpressionEval` — formerly `evaluate_expression`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum StateQueryKind {
    RegisterDiff,
    MemoryRead,
    RegisterSnapshot,
    MemoryAnalysis,
    ExpressionEval,
}

/// Output envelope of the v2 `state_query` tool.
///
/// The `kind` tag tells the consumer which payload variant follows.
/// Each variant's inner DTO is flattened into the envelope so that the
/// resulting JSON preserves byte-for-byte the shape of the corresponding
/// v1 tool's output, plus a top-level `kind` discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StateQueryOutput {
    #[serde(rename = "register_diff")]
    RegisterDiff {
        #[serde(flatten)]
        result: StateDiff,
    },
    #[serde(rename = "memory_read")]
    MemoryRead {
        #[serde(flatten)]
        result: MemoryRead,
    },
    #[serde(rename = "register_snapshot")]
    RegisterSnapshot {
        #[serde(flatten)]
        result: RegisterRead,
    },
    #[serde(rename = "memory_analysis")]
    MemoryAnalysis {
        #[serde(flatten)]
        result: MemoryAnalysis,
    },
    #[serde(rename = "expression_eval")]
    ExpressionEval {
        #[serde(flatten)]
        result: EvalResult,
    },
}

// ============================================================================
// M6 — Execution Query outputs (m6-03)
// ============================================================================

/// Discriminator for [`ExecutionQueryOutput`]. Selects which v1 tool's
/// payload is produced by the v2 `execution_query` dispatcher.
///
/// The six variants correspond to the six v1 tools that this v2 tool
/// supersedes (see `docs/milestones/m5-close-report.md` §4.1):
/// - `CallStack` — formerly `get_call_stack`.
/// - `ExecutionSummary` — formerly `get_execution_summary`.
/// - `CallGraph` — formerly `debug_call_graph`.
/// - `RaceDetect` — formerly `debug_detect_races`.
/// - `Hotspot` — formerly `debug_expand_hotspot`.
/// - `Saliency` — formerly `debug_get_saliency_scores`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum ExecutionQueryKind {
    CallStack,
    ExecutionSummary,
    CallGraph,
    RaceDetect,
    Hotspot,
    Saliency,
}

/// Output envelope of the v2 `execution_query` tool.
///
/// The `kind` tag tells the consumer which payload variant follows.
/// Each variant's inner DTO is flattened into the envelope so that the
/// resulting JSON preserves byte-for-byte the shape of the corresponding
/// v1 tool's output, plus a top-level `kind` discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionQueryOutput {
    #[serde(rename = "call_stack")]
    CallStack {
        /// Stack frames at the requested event_id.
        frames: Vec<StackFrame>,
    },
    #[serde(rename = "execution_summary")]
    ExecutionSummary {
        #[serde(flatten)]
        summary: ExecutionSummary,
    },
    #[serde(rename = "call_graph")]
    CallGraph {
        #[serde(flatten)]
        graph: CallGraph,
    },
    #[serde(rename = "race_detect")]
    RaceDetect {
        #[serde(flatten)]
        report: RaceReport,
    },
    #[serde(rename = "hotspot")]
    Hotspot {
        #[serde(flatten)]
        report: HotspotReport,
    },
    #[serde(rename = "saliency")]
    Saliency {
        #[serde(flatten)]
        result: SaliencyScoreResult,
    },
}

/// One row of the performance regression audit: a function with its
/// call counts in baseline and target and the percentage delta.
///
/// Positive `call_delta_pct` => more calls in target (potential regression).
/// Negative `call_delta_pct` => fewer calls in target (potential improvement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionRegressionEntry {
    pub function: String,
    pub baseline_calls: u64,
    pub target_calls: u64,
    pub call_delta_pct: f64,
}

/// Output of `performance_regression_audit`.
///
/// `regressions` contains functions whose call count grew by more than 50%
/// relative to baseline (sorted by `call_delta_pct` descending).
/// `improvements` contains functions whose call count shrank by more than
/// 50% (sorted by `call_delta_pct` ascending — most-improved first).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceRegressionAuditResult {
    pub baseline_session_id: String,
    pub target_session_id: String,
    pub regressions: Vec<FunctionRegressionEntry>,
    pub improvements: Vec<FunctionRegressionEntry>,
    pub functions_analyzed: usize,
    pub total_call_delta: i64,
    pub summary: String,
}

/// Output of `compare_sessions` (set diff via `chronos_store::TraceDiff`).
///
/// `summary` is an LLM-readable string with three tiers:
/// - `>= 90%` similarity => "Sessions are highly similar..."
/// - `>= 50%` similarity => "Sessions differ in N events..."
/// - `< 50%` similarity  => "Sessions are largely different..."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareSessionsResult {
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

// M6 — Hypothesis Test outputs (m6-04)
// ============================================================================

/// Discriminator for [`HypothesisOutput`]. Selects which typed hypothesis
/// shape is being evaluated by the v2 `hypothesis_test` dispatcher.
///
/// The three variants are net-new (no v1 tool backs this; see
/// `docs/milestones/m5-close-report.md` §4.2 and
/// `docs/.../specs/AGENT_API_V2.md` line 18):
/// - `Invariant` — reuses `chronos_domain::property::InvariantCheck` machinery:
///   compare a captured scalar value against a typed comparison + constant.
///   Defaults to "Pass" only when every observation satisfies the invariant;
///   never returns `Pass` on insufficient evidence (returns `Unsupported`).
/// - `Existence` — did any captured event satisfy the predicate?
///   Returns `Pass` if at least one event matched; `Violation` otherwise.
/// - `CallPath` — is `callee` reachable from `caller` in the call graph?
///   Returns `Pass` if reachable (or self-loop); `Violation` if not; lists
///   the longest un-reachable prefix path it did observe (raw support).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum HypothesisKind {
    Invariant,
    Existence,
    CallPath,
}

/// Verdict of a hypothesis evaluation. Tri-state: never false-PASS.
///
/// - `Pass` -- every required evidence observation satisfied the
///   hypothesis. This is the only state where the LLM / caller should
///   treat the hypothesis as true.
/// - `Violation` -- at least one observation broke the hypothesis, or
///   the absence requirement (Existence) was not met. `support_event_ids`
///   always contains the offending raw support so the caller can verify.
/// - `Unsupported` -- required evidence was not captured at all; we
///   cannot tell PASS from FAIL. The caller must NOT collapse this into
///   PASS (per `docs/.../specs/RUNTIME_PROPERTIES_AND_SLICING.md`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum HypothesisVerdict {
    Pass,
    Violation { reason: String },
    Unsupported { reason: String },
}

/// What scalar target an `Invariant` shape observes.
///
/// `EventCount` runs the invariant over the live event count (cheap fallback
/// when no specific property key exists). `LatencyMs` looks up the
/// `latency_us`/`latency_ms` value on each captured event's `EventData::Metric`.
/// `PropertyValue` looks up a recorded property by target key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum HypothesisScope {
    EventCount,
    LatencyMs,
    PropertyValue,
}

/// JSON-friendly wrapper around `chronos_domain::property::PropertyValue`
/// for the `hypothesis_test` MCP params. We can't re-export the domain
/// enum directly because it doesn't derive `JsonSchema`; the MCP wrapper
/// converts `HypothesisConstant` -> `PropertyValue` before invoking the
/// dispatcher. The schema covers only the three scalar variants.
///
/// Note: `Text` is preserved as-is so callers can constrain invariants
/// over captured string values (e.g. `repr()` of a `VariableInfo`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HypothesisConstant {
    Number { value: f64 },
    Text { value: String },
    Bool { value: bool },
}

impl From<HypothesisConstant> for PropertyValue {
    fn from(constant: HypothesisConstant) -> Self {
        match constant {
            HypothesisConstant::Number { value } => PropertyValue::Number(value),
            HypothesisConstant::Text { value } => PropertyValue::Text(value),
            HypothesisConstant::Bool { value } => PropertyValue::Bool(value),
        }
    }
}

/// Predicate shape for the `Existence` hypothesis.
///
/// These are deliberately small; richer filtering should be done by the LLM
/// composing `trace_slice` + `state_query` before calling `hypothesis_test`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExistencePredicate {
    /// Match `event_type == X` (e.g. `Syscall`, `FunctionCall`).
    EventTypeEquals { event_type: String },
    /// Match `thread_id == N`.
    ThreadEquals { thread_id: u64 },
    /// Match `property_target == X` (a recorded property key).
    PropertyKeyEquals { target: String },
}

/// Output envelope of the v2 `hypothesis_test` tool.
///
/// Always carries:
/// - `kind`: which hypothesis shape was evaluated.
/// - `verdict`: tri-state (Pass / Violation / Unsupported).
/// - `support_event_ids`: raw event IDs that DID satisfy (or for
///   `Violation`, the IDs whose evidence was the closest match — the raw
///   support the LLM/agent needs to verify).
/// - `counter_event_ids`: raw event IDs that DID NOT satisfy (for
///   Invariant violations, this is the offending observation list).
/// - `summary`: human-readable one-liner always populated, even on Unsupported.
#[derive(Debug, Clone, PartialEq, serde::Serialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HypothesisOutput {
    Invariant {
        verdict: HypothesisVerdict,
        support_event_ids: Vec<u64>,
        counter_event_ids: Vec<u64>,
        scope: HypothesisScope,
        summary: String,
    },
    Existence {
        verdict: HypothesisVerdict,
        support_event_ids: Vec<u64>,
        counter_event_ids: Vec<u64>,
        predicate: ExistencePredicate,
        summary: String,
    },
    CallPath {
        verdict: HypothesisVerdict,
        support_event_ids: Vec<u64>,
        counter_event_ids: Vec<u64>,
        caller: String,
        callee: String,
        reachable_path: Option<Vec<String>>,
        summary: String,
    },
}

// ============================================================================
// M6 — Session Export outputs (m6-05)
// ----------------------------------------------------------------------------
// Net-new v2 tool: `session_export`. Materializes a full session bundle
// (metadata + trace events + properties snapshot) to a portable file on
// disk. See `docs/milestones/m6-05-session-export.md` for the full
// intent + scope + test matrix.
//
// No v1 analogue exists; this is the second net-new dispatcher identified
// in M5 close report §4.2. The DTOs are deliberately minimal: ExportFormat
// is the typed enum used by the service layer, while MCP callers pass
// strings and a small parser in the wrapper maps them in.
// ============================================================================

/// Output format for `session_export`.
///
/// - `Json` -- the canonical `ExportBundle` JSON (one document, pretty
///   printed). Round-trippable through `serde_json::from_slice`.
/// - `OtlpJson` -- the OpenTelemetry-compatible JSON wire format
///   (`{"resourceSpans":[{...}]}`). Compatible with Jaeger, Tempo, and
///   Honeycomb JSON receivers. Not full OTLP/gRPC proto.
/// - `ZipJson` -- reserved for m7+; would require bundling the bundle
///   plus a `manifest.json` into a single `.zip` archive. The variant is
///   declared now so the JSON schema stays forward-compatible, but the
///   service layer currently rejects it with `InvalidParameter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    OtlpJson,
    ZipJson,
}

/// One row of the properties snapshot embedded in an [`ExportBundle`].
///
/// A `Vec<ExportPropertiesEntry>` rather than a `HashMap<String, PropertyValue>`
/// so that JSON output preserves declaration order; humans diffing two
/// exports of the same session otherwise see shuffled rows.
///
/// `ExportPropertyValue` is a JSON-friendly mirror of
/// `chronos_domain::property::PropertyValue` (same pattern as
/// `HypothesisConstant` in m6-04). We can't re-export the domain enum
/// directly because it does not derive `JsonSchema`; the dispatcher
/// converts `ExportPropertyValue` -> `PropertyValue` before evaluating.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub struct ExportPropertiesEntry {
    pub name: String,
    pub value: ExportPropertyValue,
}

/// JSON-friendly mirror of `chronos_domain::property::PropertyValue` for
/// `session_export`. Only the three scalar variants are exposed because
/// the OTLP wire format and `ExportBundle` consumers only ever need to
/// serialize scalar measurements. Composite values (lists, maps) flatten
/// to their string repr.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportPropertyValue {
    Number { value: f64 },
    Text { value: String },
    Bool { value: bool },
}

impl From<PropertyValue> for ExportPropertyValue {
    fn from(v: PropertyValue) -> Self {
        match v {
            PropertyValue::Number(n) => ExportPropertyValue::Number { value: n },
            PropertyValue::Text(t) => ExportPropertyValue::Text { value: t },
            PropertyValue::Bool(b) => ExportPropertyValue::Bool { value: b },
        }
    }
}

impl From<ExportPropertyValue> for PropertyValue {
    fn from(v: ExportPropertyValue) -> Self {
        match v {
            ExportPropertyValue::Number { value } => PropertyValue::Number(value),
            ExportPropertyValue::Text { value } => PropertyValue::Text(value),
            ExportPropertyValue::Bool { value } => PropertyValue::Bool(value),
        }
    }
}

/// The payload of an exported session.
///
/// `schema_version` is a literal string so consumers can detect future
/// migrations. `metadata` reuses the persistent store's `SessionMetadata`
/// directly so a `session_load` -> `session_export` round trip produces
/// identical metadata. `events` is the full `Vec<TraceEvent>` as captured
/// by the live engine. `properties_snapshot` is best-effort: if the
/// engine has no `evaluate_all` view, it is empty.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportBundle {
    pub schema_version: String,
    pub metadata: SessionMetadata,
    pub events: Vec<chronos_domain::TraceEvent>,
    pub properties_snapshot: Vec<ExportPropertiesEntry>,
}

/// Result of a successful `session_export` call.
///
/// `path` is the final on-disk location (the tmp file is renamed onto it).
/// `bytes_written` is what serde emitted before fsync + rename, so a
/// consumer re-reading the file will see exactly this many bytes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub struct ExportResult {
    pub path: std::path::PathBuf,
    pub bytes_written: u64,
    pub format: ExportFormat,
}

// ============================================================================
// M7 — Events Read outputs (m7-01)
// ----------------------------------------------------------------------------
// V2 dispatcher that supersedes the two v1 event-read tools:
// - `query_events` → `EventsReadKind::Query`
// - `get_event`    → `EventsReadKind::ById`
//
// See `docs/milestones/m7-01-events-read-merge.md` for the full intent
// + scope + algorithm. The v2 surface adds three fields the v1 tools
// lacked (per spec line 60: "completeness, gap summary, provenance")
// plus a cursor-based pagination model that complements the existing
// offset/limit pagination. The cursor encoding reuses the existing
// `chronos_domain::EventCursor` shape (same as `probe_drain`) so the
// dispatcher does not introduce a parallel cursor type.
// ============================================================================

/// Discriminator for [`EventsReadOutput`]. Selects which v1 tool's payload
/// is produced by the v2 `events_read` dispatcher.
///
/// - `Query` — formerly `query_events`. Cursor-based, paginated read
///   with filters (event types, thread, time range, function pattern).
/// - `ById`  — formerly `get_event`. Single-event lookup by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum EventsReadKind {
    Query,
    ById,
}

impl EventsReadKind {
    /// Snake-case name used as the JSON discriminator tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            EventsReadKind::Query => "query",
            EventsReadKind::ById => "by_id",
        }
    }
}

/// Provenance descriptor attached to every `events_read` response.
///
/// Carries the *minimum* information an agent needs to know where the
/// evidence came from (spec line 66: "provenance, capability limitations
/// and stable IDs for follow-up calls"). The shape is deliberately
/// minimal in m7-01 — richer provenance (capture source, instrumentation
/// version, etc.) is m7+ territory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventsReadProvenance {
    /// Always `"query_engine"` in m7-01 — the v2 dispatcher only reads
    /// from the finalised-session `QueryEngine`. The live ring buffer
    /// (`probe_drain`) is a separate read path until m7-02 (`observe`
    /// merge) unifies them.
    pub source: String,
    /// The session this read is bound to. Mirrors the top-level
    /// `session_id` echo but is structurally nested for forward
    /// compatibility with multi-session responses.
    pub session_id: String,
}

/// Output envelope of the v2 `events_read` tool.
///
/// The `mode` tag tells the consumer which payload variant follows.
/// Each variant's inner DTO is flattened into the envelope so the
/// resulting JSON preserves the v1 tool's shape (with the addition of
/// the v2-spec fields `next_cursor`, `completeness`, `gap_summary`,
/// `provenance`).
///
/// Honest disclosure (m7-01): `gap_summary` is always `None` in m7-01
/// because `chronos_query::QueryEngine` does not currently expose gap
/// info. The field is final so m7+ can fill it without breaking
/// consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum EventsReadOutput {
    #[serde(rename = "query")]
    Query {
        session_id: String,
        #[serde(flatten)]
        result: QueryEventsResult,
        /// Opaque cursor for the next page. `None` when the result set
        /// was fully returned by this call. The encoding matches the
        /// existing `CursorDto` shape (`{total_pushed, snapshot_len}`).
        /// A fresh cursor is issued when the caller did not provide
        /// one (`cursor == None`); the dispatcher returns
        /// `ServiceError::CursorStale` if the caller's cursor is older
        /// than the session's current `total_pushed`.
        next_cursor: Option<CursorDto>,
        /// `"complete"` for the cursor path, `"best_effort"` for the
        /// offset/limit compatibility shim path. Forward-compatible:
        /// m7+ may add `"gap"` to signal a known gap window in the
        /// captured trace.
        completeness: String,
        /// Always `None` in m7-01 (see honest disclosure above).
        gap_summary: Option<Vec<serde_json::Value>>,
        provenance: EventsReadProvenance,
    },
    #[serde(rename = "by_id")]
    ById {
        session_id: String,
        /// `Some(event)` if found, `None` otherwise. Same semantics as
        /// v1 `get_event`.
        event: Option<chronos_domain::TraceEvent>,
        provenance: EventsReadProvenance,
    },
}

/// Wire format for [`chronos_domain::EventCursor`] in MCP JSON payloads.
///
/// Mirrors the existing `CursorDto` at the MCP boundary (see
/// `crates/chronos-mcp/src/server.rs:795`). We declare it here too so
/// the v2 DTO surface does not depend on the MCP boundary module's
/// internal types — the dispatcher accepts this DTO and converts to
/// `chronos_domain::EventCursor` internally.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CursorDto {
    #[serde(default)]
    pub total_pushed: Option<u64>,
    #[serde(default)]
    pub snapshot_len: Option<u64>,
}

impl CursorDto {
    /// Convert to the domain cursor, returning `None` if the payload is
    /// malformed (e.g., missing required values).
    pub fn to_domain(&self) -> Option<chronos_domain::EventCursor> {
        Some(chronos_domain::EventCursor {
            total_pushed: self.total_pushed?,
            snapshot_len: self.snapshot_len?,
        })
    }

    /// Convert from a domain cursor.
    pub fn from_domain(c: &chronos_domain::EventCursor) -> Self {
        Self {
            total_pushed: Some(c.total_pushed),
            snapshot_len: Some(c.snapshot_len),
        }
    }
}

// ============================================================================
// M7 — Session Compare + Session Explain outputs (m7-03)
// ----------------------------------------------------------------------------
// V2 dispatcher set that splits out the v1 diff surface into two
// semantically distinct v2 endpoints:
// - `session_compare` — two-session comparison (kind: divergence |
//   regression). Folds the v1 `compare_sessions` and
//   `performance_regression_audit` behind a single dispatcher with a
//   `kind` discriminator. The v1 names are preserved as deprecated
//   MCP shims (sunset 2027-09-11 per m6-close-report §4).
// - `session_explain` — one-session structured bundle (kind: facts |
//   derived | inferred | hypothesis). Net-new per spec line 21; no
//   v1 analogue exists, so this tool has no shim.
//
// Both tools return a `*Provenance` block on every variant per the
// v2 agent-ergonomics line. Both tools are bounded by either the
// two-session set (`session_compare`) or the per-session bundle
// (`session_explain`); no cursor pagination is needed in m7-03.
// ============================================================================

/// Kind discriminator for the unified v2 `session_compare` tool.
///
/// `Divergence` maps to v1 `compare_sessions` (BLAKE3 hash set-diff +
/// similarity_pct). `Regression` maps to v1
/// `performance_regression_audit` (per-function call-count regression
/// detection with a +50% / -50% threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionCompareKind {
    /// Set-diff + similarity comparison (v1 `compare_sessions`).
    Divergence,
    /// Per-function call-count regression detection
    /// (v1 `performance_regression_audit`).
    Regression,
}

/// Input for the unified v2 `session_compare` tool.
///
/// Carries the kind discriminator plus the per-kind arguments. The
/// dispatcher validates kind/argument consistency and rejects unknown
/// or mismatched fields with `ServiceError::InvalidInput`. Both v1
/// names map cleanly onto the kind variants; the dispatcher does
/// not require a separate `unknown` branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCompareInput {
    /// Which comparison kind to run.
    pub kind: SessionCompareKind,
    /// Required for `kind=divergence` (v1 `compare_sessions` shape).
    pub session_a: Option<String>,
    /// Required for `kind=divergence` (v1 `compare_sessions` shape).
    pub session_b: Option<String>,
    /// Required for `kind=regression` (v1 `performance_regression_audit` shape).
    pub baseline_session_id: Option<String>,
    /// Required for `kind=regression` (v1 `performance_regression_audit` shape).
    pub target_session_id: Option<String>,
    /// Optional, only honoured by `kind=regression`.
    pub top_n: Option<usize>,
}

/// Tagged output envelope returned by `session_compare`.
///
/// Each variant preserves the v1 result shape 1:1 plus a
/// `provenance` field. MCP shims drop the `provenance` field so
/// existing v1 callers see the same JSON they did before m7-03.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionCompareOutput {
    /// Set-diff result for `kind=divergence`.
    Divergence {
        /// Set-diff result (same shape as v1 `compare_sessions`).
        result: CompareSessionsResult,
        /// Provenance metadata (engine version, source tag).
        provenance: SessionCompareProvenance,
    },
    /// Per-function regression result for `kind=regression`.
    Regression {
        /// Regression-audit result (same shape as v1
        /// `performance_regression_audit`).
        result: PerformanceRegressionAuditResult,
        /// Provenance metadata (engine version, source tag).
        provenance: SessionCompareProvenance,
    },
}

/// Provenance metadata for `session_compare` responses.
///
/// Carries the engine version (matching `chronos_query::QueryEngine::engine_version()`)
/// and a short `source` tag identifying which dispatcher produced
/// the response (`"session_compare:divergence"` or
/// `"session_compare:regression"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCompareProvenance {
    /// Engine version string (matches
    /// `chronos_query::QueryEngine::engine_version()`).
    pub engine_version: String,
    /// Source tag (e.g., `"session_compare:divergence"`).
    pub source: String,
}

/// Kind discriminator for the net-new v2 `session_explain` tool.
///
/// `Facts` returns direct observations from the session. `Derived`
/// returns projections computed from facts. `Inferred` returns
/// best-effort characterisations (with a typed `Unknown` fallback).
/// `Hypothesis` returns a typed plan that the agent can execute via
/// the m6-04 `hypothesis_test` tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionExplainKind {
    /// Direct observations from the session.
    Facts,
    /// Projections computed from facts (hotspots, call graph summary).
    Derived,
    /// Best-effort characterisations (crash / I/O-heavy / CPU-bound /
    /// single-threaded, with a typed `Unknown` fallback).
    Inferred,
    /// Typed `hypothesis_test` plan the agent can execute separately.
    Hypothesis,
}

/// Hypothesis-test kind selector for `session_explain{kind=hypothesis}`.
///
/// Mirrors the m6-04 surface but exposes only the kinds the explain
/// bundle can plan without additional agent input. `Existence` is
/// not plannable from `session_explain` because the predicate
/// requires an agent-supplied event-type filter at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisTestKind {
    /// Invariant: did a captured scalar violate the typed comparison?
    /// Used by `session_explain` to plan a "this session crashed"
    /// hypothesis by checking `exit_status` against `Equal(0)`.
    CrashInvariant,
    /// CallPath: is `callee` reachable from `caller`? Used by
    /// `session_explain` to plan a "the dominant function was
    /// called N times" hypothesis.
    DominantFunctionCallPath,
}

/// Input for the net-new v2 `session_explain` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionExplainInput {
    /// Which bundle kind to produce.
    pub kind: SessionExplainKind,
    /// Target session id (required for every kind).
    pub session_id: String,
    /// Required only for `kind=hypothesis` — selects the
    /// hypothesis-test variant the plan targets.
    pub hypothesis_kind: Option<HypothesisTestKind>,
}

/// Tagged output envelope returned by `session_explain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionExplainOutput {
    Facts {
        bundle: FactsBundle,
        provenance: SessionExplainProvenance,
    },
    Derived {
        bundle: DerivedBundle,
        provenance: SessionExplainProvenance,
    },
    Inferred {
        bundle: InferredBundle,
        provenance: SessionExplainProvenance,
    },
    Hypothesis {
        bundle: HypothesisBundle,
        provenance: SessionExplainProvenance,
    },
}

/// Direct observations from a single session (m7-03 starter set).
///
/// m7-03 ships 7 fields. The bundle is deliberately minimal; richer
/// facts (`signal_received`, `exit_status`, `peak_thread_count`) are
/// deferred to m7+ unless execute-cycle evidence shows they are
/// load-bearing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactsBundle {
    pub session_id: String,
    pub total_events: usize,
    pub distinct_event_types: Vec<String>,
    pub distinct_functions: Vec<String>,
    pub duration_ms: Option<u64>,
    pub thread_count: usize,
    pub crash_detected: bool,
    pub signal_delivered: Option<String>,
}

/// Projections computed from facts (m7-03 starter set).
///
/// `regressions_vs_none` is a typed `None` in m7-03 because
/// regression detection requires a baseline session; that flow lives
/// in `session_compare{kind=regression}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedBundle {
    pub session_id: String,
    pub hotspots: Vec<FunctionHotspot>,
    pub call_graph_summary: CallGraphSummary,
    pub regressions_vs_none: Option<Vec<FunctionRegressionEntry>>,
}

/// Hotspot projection (one entry per dominant function).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionHotspot {
    pub function: String,
    pub call_count: u64,
    pub share_pct: f64,
}

/// Call-graph summary (counts only — full graph is `execution_query`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallGraphSummary {
    pub total_calls: u64,
    pub distinct_callees: usize,
    pub max_depth: u32,
}

/// Best-effort characterisation bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferredBundle {
    pub session_id: String,
    pub inferences: Vec<InferredTag>,
}

/// Typed characterisation tag applied by the inferred bundle.
///
/// Multiple tags can co-exist on the same session (e.g., a session
/// can be both `IoHeavy` and `SingleThreaded`). `Unknown` is the
/// typed fallback when no characterisation applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InferredTag {
    /// The session crashed (one or more crash-class events observed).
    CrashDetected,
    /// >50% of events are syscall_enter/exit for IO syscalls.
    IoHeavy,
    /// One function dominates >70% of total call counts.
    CpuBound,
    /// `facts.thread_count == 1`.
    SingleThreaded,
    /// No characterisation applied (typed fallback).
    Unknown,
}

/// Typed hypothesis plan returned by `session_explain{kind=hypothesis}`.
///
/// The agent executes the plan by calling the v2 `hypothesis_test`
/// tool (m6-04) with the plan's kind + session_id. `session_explain`
/// does **not** execute the plan itself — execution lives in
/// `hypothesis_test`. The plan + hint bundle keeps the
/// responsibility split clean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisBundle {
    pub session_id: String,
    pub plan: HypothesisTestPlan,
    pub hint: String,
}

/// Typed hypothesis-test plan.
///
/// Mirrors the m6-04 surface but only carries the kinds `session_explain`
/// can plan without additional agent input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HypothesisTestPlan {
    /// Plan: did the session's exit_status violate `Equal(0)`?
    CrashInvariant {
        session_id: String,
        comparison: crate::output::ComparisonOp,
    },
    /// Plan: was the dominant function called `at_least_n` times?
    DominantFunctionCallPath {
        session_id: String,
        function: String,
        at_least_n: u64,
    },
}

/// Provenance metadata for `session_explain` responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionExplainProvenance {
    /// Engine version string (matches
    /// `chronos_query::QueryEngine::engine_version()`).
    pub engine_version: String,
    /// Source tag (e.g., `"session_explain:facts"`).
    pub source: String,
}

// ============================================================================
// Session lifecycle (m7-04)
//
// Three net-new v2 tools: `session_start`, `session_stop`, `capabilities`.
// The dispatcher `ChronosSessionLifecycleService` lives in
// `crates/chronos-services/src/session_lifecycle.rs`.
//
// `session_start` unifies `probe_start` (action=spawn), `session_load`
// (action=load), and a stubbed action=attach (m7+) behind a single
// endpoint with a capability snapshot. `session_stop` wraps `probe_stop`
// plus two new flags (seal_tail + drain_subscriptions). `capabilities`
// is read-only introspection over target + session_id.
// ============================================================================

use chronos_domain::trace::{EventType, Language};

/// Discriminator for the v2 `session_start` tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartAction {
    /// Spawn a new process under a probe (mirrors v1 `probe_start`).
    Spawn,
    /// Load a previously persisted session from the store (mirrors
    /// v1 `session_load`).
    Load,
    /// Attach to an already-running PID (m7+ stub; rejected with
    /// `ServiceError::Unsupported` in m7-04 until the domain-layer
    /// attach API lands).
    Attach,
}

/// Input for the v2 `session_start` tool.
///
/// The per-action fields are gated: `spawn` requires `spawn_fields`,
/// `load` requires `session_id`, `attach` requires `pid`. The
/// dispatcher validates kind/argument consistency and rejects unknown
/// or mismatched fields with `ServiceError::InvalidInput`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStartInput {
    pub action: SessionStartAction,
    /// Required for `action=spawn` — mirrors v1 `ProbeStartInput`.
    #[serde(default)]
    pub spawn_fields: Option<SessionStartSpawnFields>,
    /// Required for `action=load`.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Required for `action=attach` (m7+).
    #[serde(default)]
    pub pid: Option<u32>,
    /// Optional override for `action=load` (deferred to m7+).
    #[serde(default)]
    pub path: Option<String>,
}

/// Per-action payload for `session_start{action=spawn}`.
///
/// Mirrors v1 `ProbeStartInput` 1:1. Field names + types are
/// preserved so existing v1 callers can compose a v2 input by
/// adding `action="spawn"` without renaming any other field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStartSpawnFields {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub trace_syscalls: bool,
    #[serde(default)]
    pub bus_capacity: Option<usize>,
    #[serde(default)]
    pub track_function_frames: bool,
}

/// Output for the v2 `session_start` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStartOutput {
    pub session_id: String,
    pub action: SessionStartAction,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub event_count: Option<usize>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub bus_capacity: Option<usize>,
    pub capability_snapshot: CapabilitySnapshot,
    pub provenance: SessionLifecycleProvenance,
}

/// Input for the v2 `session_stop` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStopInput {
    pub session_id: String,
    /// Mark `SessionMetadata.tail_sealed=true` and record `sealed_at`.
    /// Default `true` (m7-04 v2 semantic: "seal tail").
    #[serde(default = "default_true")]
    pub seal_tail: bool,
    /// Drain `observe{verb=list}` before stop. Default `true`.
    /// Destructive (matches m7-02 `retention=Drained` default).
    #[serde(default = "default_true")]
    pub drain_subscriptions: bool,
}

fn default_true() -> bool {
    true
}

/// Output for the v2 `session_stop` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStopOutput {
    pub session_id: String,
    pub status: String,
    pub target: String,
    pub total_events: u64,
    pub duration_ms: u64,
    pub ebpf_detached: bool,
    #[serde(default)]
    pub sealed_at: Option<u64>,
    pub drained_subscriptions: bool,
    pub capability_snapshot: CapabilitySnapshot,
    pub provenance: SessionLifecycleProvenance,
}

/// Input for the v2 `capabilities` tool.
///
/// At least one of `target` or `session_id` is required. Both are
/// allowed for a combined static + dynamic view. The dispatcher
/// rejects calls with neither set (ServiceError::InvalidInput).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesInput {
    #[serde(default)]
    pub target: Option<TargetSpec>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Output for the v2 `capabilities` tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesOutput {
    #[serde(default)]
    pub static_capabilities: Option<StaticCapabilities>,
    #[serde(default)]
    pub dynamic_capabilities: Option<DynamicCapabilities>,
    pub provenance: SessionLifecycleProvenance,
}

/// Static target description — what evidence mechanisms a target
/// supports before any session is started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub language: Option<Language>,
}

/// Snapshot of what evidence mechanisms a session currently exposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    #[serde(default)]
    pub probe_type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub bus_capacity: Option<usize>,
    #[serde(default)]
    pub bus_fill: Option<usize>,
    #[serde(default)]
    pub query_engine_ready: bool,
    #[serde(default)]
    pub active_subscriptions: Vec<String>,
    #[serde(default)]
    pub tail_sealed: bool,
    #[serde(default)]
    pub sealed_at: Option<u64>,
}

/// Static capability surface (per-target, pre-session).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticCapabilities {
    pub probe_type: String,
    pub language: Language,
    pub target_event_types: Vec<EventType>,
    pub language_adapters: Vec<LanguageAdapterStatus>,
    pub projections: Vec<ProjectionKind>,
}

/// Per-language adapter availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageAdapterStatus {
    pub language: Language,
    pub available: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Projection kind that the v2 tool surface offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    EventsRead,
    ExecutionQuery,
    StateQuery,
    TraceSlice,
    SessionCompare,
    SessionExplain,
}

/// Dynamic capability surface (per-session, live).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicCapabilities {
    pub bus_capacity: usize,
    pub bus_fill: usize,
    pub event_types_emitted: Vec<EventType>,
    pub event_type_counts: HashMap<EventType, u64>,
    pub query_engine_ready: bool,
    pub active_subscriptions: Vec<String>,
    pub tail_sealed: bool,
    #[serde(default)]
    pub sealed_at: Option<u64>,
}

/// Provenance metadata for `session_start` / `session_stop` /
/// `capabilities` responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLifecycleProvenance {
    /// Engine version string. Hardcoded to `"chronos-0.1.0"` until
    /// `chronos_query::QueryEngine::engine_version()` exists.
    pub engine_version: String,
    /// Source tag (e.g., `"session_start:spawn"`,
    /// `"session_stop"`, `"capabilities:target"`).
    pub source: String,
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
