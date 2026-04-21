//! Query types for trace event queries.

use crate::trace::{EventType, TimestampNs};
use crate::TraceEvent;
use serde::{Deserialize, Serialize};

/// A query against a trace file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceQuery {
    /// Session to query.
    pub session_id: String,
    /// Filter by event types (None = all types).
    pub event_types: Option<Vec<EventType>>,
    /// Filter by thread ID.
    pub thread_id: Option<u64>,
    /// Start of time range (inclusive, nanoseconds).
    pub timestamp_start: Option<TimestampNs>,
    /// End of time range (exclusive, nanoseconds).
    pub timestamp_end: Option<TimestampNs>,
    /// Start of address range.
    pub address_start: Option<u64>,
    /// End of address range.
    pub address_end: Option<u64>,
    /// Filter by function name pattern (glob).
    pub function_pattern: Option<String>,
    /// Filter by file name pattern (glob).
    pub file_pattern: Option<String>,
    /// Maximum events to return.
    pub limit: usize,
    /// Number of events to skip (for pagination).
    pub offset: usize,
}

impl TraceQuery {
    /// Create a new query for a session.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            event_types: None,
            thread_id: None,
            timestamp_start: None,
            timestamp_end: None,
            address_start: None,
            address_end: None,
            function_pattern: None,
            file_pattern: None,
            limit: 100,
            offset: 0,
        }
    }

    /// Set a time range filter.
    pub fn time_range(mut self, start: TimestampNs, end: TimestampNs) -> Self {
        self.timestamp_start = Some(start);
        self.timestamp_end = Some(end);
        self
    }

    /// Set an event type filter.
    pub fn event_types(mut self, types: Vec<EventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    /// Set a function pattern filter.
    pub fn function_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.function_pattern = Some(pattern.into());
        self
    }

    /// Set pagination.
    pub fn pagination(mut self, limit: usize, offset: usize) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }

    /// Check if an event matches this query's filters.
    pub fn matches(&self, event: &TraceEvent) -> bool {
        // Event type filter
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.event_type) {
                return false;
            }
        }

        // Thread filter
        if let Some(tid) = self.thread_id {
            if event.thread_id != tid {
                return false;
            }
        }

        // Time range filter
        if let Some(start) = self.timestamp_start {
            if event.timestamp_ns < start {
                return false;
            }
        }
        if let Some(end) = self.timestamp_end {
            if event.timestamp_ns >= end {
                return false;
            }
        }

        // Address range filter
        if let Some(start) = self.address_start {
            if event.location.address < start {
                return false;
            }
        }
        if let Some(end) = self.address_end {
            if event.location.address >= end {
                return false;
            }
        }

        // Function pattern filter (simple substring match for MVP)
        if let Some(ref pattern) = self.function_pattern {
            if let Some(ref func_name) = event.location.function {
                if !simple_glob_match(func_name, pattern) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // File pattern filter
        if let Some(ref pattern) = self.file_pattern {
            if let Some(ref file) = event.location.file {
                if !simple_glob_match(file, pattern) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// Simple glob matching: supports `*` (any chars) and `?` (single char).
fn simple_glob_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();
    glob_match_inner(&text_chars, &pattern_chars, 0, 0)
}

fn glob_match_inner(text: &[char], pattern: &[char], ti: usize, pi: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }

    if pattern[pi] == '*' {
        // Try matching zero or more characters
        for i in ti..=text.len() {
            if glob_match_inner(text, pattern, i, pi + 1) {
                return true;
            }
        }
        false
    } else if ti < text.len()
        && (pattern[pi] == '?' || pattern[pi] == text[ti])
    {
        glob_match_inner(text, pattern, ti + 1, pi + 1)
    } else {
        false
    }
}

/// Result of a trace query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Total number of matching events (before pagination).
    pub total_matching: u64,
    /// Matching events (after pagination).
    pub events: Vec<TraceEvent>,
    /// Offset for the next page (None if no more results).
    pub next_offset: Option<usize>,
}

/// An event filter that can be applied to a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub event_types: Option<Vec<EventType>>,
    pub thread_id: Option<u64>,
    pub timestamp_start: Option<TimestampNs>,
    pub timestamp_end: Option<TimestampNs>,
}

/// Summary of an execution trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Trace/session ID.
    pub session_id: String,
    /// Total duration in nanoseconds.
    pub duration_ns: u64,
    /// Total number of events.
    pub total_events: u64,
    /// Event counts by type.
    pub event_counts_by_type: Vec<(String, u64)>,
    /// Top functions by call count.
    pub top_functions: Vec<FunctionStats>,
    /// Number of threads.
    pub thread_count: u64,
    /// Potential issues detected.
    pub potential_issues: Vec<PotentialIssue>,
}

/// Statistics for a single function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStats {
    pub name: String,
    pub call_count: u64,
}

/// A potential issue detected in the trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialIssue {
    pub issue_type: String,
    pub confidence: f32,
    pub description: String,
}

/// A single stack frame in a reconstructed call stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// Frame depth (0 = innermost).
    pub depth: u32,
    /// Function name.
    pub function: String,
    /// Source file.
    pub file: Option<String>,
    /// Line number.
    pub line: Option<u32>,
    /// Instruction address.
    pub address: u64,
}

/// A state diff between two timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    /// Timestamp A.
    pub timestamp_a: TimestampNs,
    /// Timestamp B.
    pub timestamp_b: TimestampNs,
    /// List of changes.
    pub changes: Vec<StateChange>,
}

/// A single state change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// What changed (e.g., "registers.rax", "variables.x").
    pub field: String,
    /// Value at timestamp A.
    pub value_a: String,
    /// Value at timestamp B.
    pub value_b: String,
}

// ─── Causality queries ────────────────────────────────────────────────────────

/// Query to find the origin of a memory/variable write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalityQuery {
    /// Session to query.
    pub session_id: String,
    /// Memory address to inspect (preferred over variable_name).
    pub address: Option<u64>,
    /// Variable name to inspect (exact match).
    pub variable_name: Option<String>,
    /// Return only mutations before this timestamp.
    pub before_timestamp: Option<TimestampNs>,
    /// If true, return the full lineage; if false, return only the last mutation.
    pub full_lineage: bool,
}

impl CausalityQuery {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            address: None,
            variable_name: None,
            before_timestamp: None,
            full_lineage: false,
        }
    }

    pub fn by_address(mut self, addr: u64) -> Self {
        self.address = Some(addr);
        self
    }

    pub fn by_name(mut self, name: impl Into<String>) -> Self {
        self.variable_name = Some(name.into());
        self
    }

    pub fn before(mut self, ts: TimestampNs) -> Self {
        self.before_timestamp = Some(ts);
        self
    }

    pub fn with_full_lineage(mut self) -> Self {
        self.full_lineage = true;
        self
    }
}

/// Result of a causality query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalityResult {
    /// The queried address (resolved from name if needed).
    pub address: u64,
    /// Variable name if known.
    pub variable_name: Option<String>,
    /// Mutations found (one if last-only, many if full lineage).
    pub mutations: Vec<MutationRecord>,
}

/// A single mutation record in a causality result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRecord {
    pub event_id: u64,
    pub timestamp: TimestampNs,
    pub thread_id: u64,
    pub value_before: Option<String>,
    pub value_after: String,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

// ─── Race detection query ─────────────────────────────────────────────────────

/// Query to detect potential data races in a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceDetectionQuery {
    pub session_id: String,
    /// Restrict to this time range (start, end in ns).
    pub time_range: Option<(TimestampNs, TimestampNs)>,
    /// Concurrent write threshold in nanoseconds (default: 100).
    pub threshold_ns: u64,
}

impl RaceDetectionQuery {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            time_range: None,
            threshold_ns: 100,
        }
    }
}

/// A single potential race condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentialRace {
    /// Memory address with concurrent writes.
    pub address: u64,
    /// First write event.
    pub write_a: MutationRecord,
    /// Second write event (within threshold).
    pub write_b: MutationRecord,
    /// Time difference in nanoseconds between the two writes.
    pub delta_ns: u64,
}

/// Result of a race detection query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceDetectionResult {
    pub races: Vec<PotentialRace>,
    pub addresses_checked: usize,
}

// ─── Performance queries ──────────────────────────────────────────────────────

/// Query to retrieve performance counter data from a trace session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfQuery {
    /// Session to query.
    pub session_id: String,
    /// If Some, filter results to functions matching this name substring.
    pub function_filter: Option<String>,
    /// Maximum number of results to return (default: 20).
    pub limit: usize,
    /// Sort order for results.
    pub sort_by: PerfSortBy,
}

/// Sort order for performance query results.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum PerfSortBy {
    /// Sort by total cycles descending.
    #[default]
    Cycles,
    /// Sort by call count descending.
    CallCount,
}

impl PerfQuery {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            function_filter: None,
            limit: 20,
            sort_by: PerfSortBy::Cycles,
        }
    }

    pub fn filter_function(mut self, name: impl Into<String>) -> Self {
        self.function_filter = Some(name.into());
        self
    }

    pub fn top(mut self, n: usize) -> Self {
        self.limit = n;
        self
    }

    pub fn sort_by_calls(mut self) -> Self {
        self.sort_by = PerfSortBy::CallCount;
        self
    }
}

/// A single entry in a performance query result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfEntry {
    /// Function address.
    pub address: u64,
    /// Function name (if known).
    pub name: Option<String>,
    /// Number of times this function was called.
    pub call_count: u64,
    /// Total estimated CPU cycles.
    pub total_cycles: u64,
    /// Average cycles per call.
    pub avg_cycles: f64,
}

/// Result of a performance query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfResult {
    /// Top functions according to sort order.
    pub functions: Vec<PerfEntry>,
    /// True if hardware counters (perf_event_open) were available.
    pub counters_available: bool,
    /// Global cycle count for the session (None if counters unavailable).
    pub total_session_cycles: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{EventData, SourceLocation};

    fn make_event(id: u64, ts: u64, tid: u64, event_type: EventType, func: &str) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            tid,
            event_type,
            SourceLocation::new("test.rs", 1, func, 0x1000),
            EventData::Empty,
        )
    }

    #[test]
    fn test_query_new() {
        let q = TraceQuery::new("session-1");
        assert_eq!(q.session_id, "session-1");
        assert_eq!(q.limit, 100);
        assert_eq!(q.offset, 0);
    }

    #[test]
    fn test_query_builder() {
        let q = TraceQuery::new("s1")
            .time_range(100, 1000)
            .event_types(vec![EventType::FunctionEntry])
            .function_pattern("main")
            .pagination(50, 10);

        assert_eq!(q.timestamp_start, Some(100));
        assert_eq!(q.timestamp_end, Some(1000));
        assert_eq!(q.event_types.as_ref().unwrap().len(), 1);
        assert_eq!(q.function_pattern.as_deref(), Some("main"));
        assert_eq!(q.limit, 50);
        assert_eq!(q.offset, 10);
    }

    #[test]
    fn test_query_matches_event_type() {
        let q = TraceQuery::new("s1").event_types(vec![EventType::FunctionEntry]);
        let e = make_event(1, 100, 1, EventType::FunctionEntry, "main");
        assert!(q.matches(&e));

        let e2 = make_event(2, 200, 1, EventType::FunctionExit, "main");
        assert!(!q.matches(&e2));
    }

    #[test]
    fn test_query_matches_time_range() {
        let q = TraceQuery::new("s1").time_range(100, 500);
        let e_ok = make_event(1, 200, 1, EventType::FunctionEntry, "main");
        let e_before = make_event(2, 50, 1, EventType::FunctionEntry, "main");
        let e_after = make_event(3, 500, 1, EventType::FunctionEntry, "main");

        assert!(q.matches(&e_ok));
        assert!(!q.matches(&e_before));
        assert!(!q.matches(&e_after));
    }

    #[test]
    fn test_query_matches_thread() {
        let q = TraceQuery::new("s1");
        // No thread filter
        let e = make_event(1, 100, 42, EventType::FunctionEntry, "main");
        assert!(q.matches(&e));
    }

    #[test]
    fn test_query_matches_function_pattern() {
        let q = TraceQuery::new("s1").function_pattern("main");
        let e = make_event(1, 100, 1, EventType::FunctionEntry, "main");
        assert!(q.matches(&e));

        let e2 = make_event(2, 200, 1, EventType::FunctionEntry, "other");
        assert!(!q.matches(&e2));
    }

    #[test]
    fn test_glob_match() {
        assert!(simple_glob_match("main", "main"));
        assert!(simple_glob_match("main", "mai*"));
        assert!(simple_glob_match("main.rs", "*.rs"));
        assert!(simple_glob_match("test_file.rs", "test_*.rs"));
        assert!(simple_glob_match("a", "?"));
        assert!(!simple_glob_match("ab", "?"));
        assert!(simple_glob_match("anything", "*"));
    }

    #[test]
    fn test_query_result() {
        let result = QueryResult {
            total_matching: 1000,
            events: vec![make_event(1, 100, 1, EventType::FunctionEntry, "main")],
            next_offset: Some(100),
        };
        assert_eq!(result.total_matching, 1000);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.next_offset, Some(100));
    }
}
