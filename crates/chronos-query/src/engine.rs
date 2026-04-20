//! Query engine — executes queries against trace data using indices.
//!
//! The `QueryEngine` is the central query processor. It takes a set of events,
//! optionally indexed with shadow and temporal indices, and executes `TraceQuery`
//! requests to find matching events, reconstruct call stacks, and compute
//! execution summaries.

use chronos_domain::{
    EventData, EventType, QueryResult, ShadowIndex, TemporalIndex, TraceEvent,
    TraceQuery,
    query::{ExecutionSummary, FunctionStats, PotentialIssue, StackFrame, StateChange, StateDiff},
};
use std::collections::HashMap;

/// The query engine — holds trace data and indices for fast queries.
pub struct QueryEngine {
    /// All events in the trace (ordered by event_id).
    events: Vec<TraceEvent>,
    /// Shadow index (address → event IDs).
    shadow_index: Option<ShadowIndex>,
    /// Temporal index (timestamp → event IDs).
    temporal_index: Option<TemporalIndex>,
}

impl QueryEngine {
    /// Create a new query engine from a vec of events (no indices).
    pub fn new(events: Vec<TraceEvent>) -> Self {
        Self {
            events,
            shadow_index: None,
            temporal_index: None,
        }
    }

    /// Create a query engine with pre-built indices.
    pub fn with_indices(
        events: Vec<TraceEvent>,
        shadow_index: ShadowIndex,
        temporal_index: TemporalIndex,
    ) -> Self {
        Self {
            events,
            shadow_index: Some(shadow_index),
            temporal_index: Some(temporal_index),
        }
    }

    /// Get the total number of events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get an event by its ID (linear scan).
    pub fn get_event_by_id(&self, event_id: u64) -> Option<&TraceEvent> {
        self.events.iter().find(|e| e.event_id == event_id)
    }

    /// Execute a query and return matching events with pagination.
    pub fn execute(&self, query: &TraceQuery) -> QueryResult {
        // Use temporal index for time-range queries if available
        let candidate_ids: Option<Vec<u64>> = if let (Some(ref temporal), Some(ts_start), Some(ts_end)) =
            (&self.temporal_index, query.timestamp_start, query.timestamp_end)
        {
            Some(temporal.range(ts_start, ts_end))
        } else {
            None
        };

        // Use shadow index for address-range queries if available
        let address_candidate_ids: Option<Vec<u64>> =
            if let (Some(ref shadow), Some(addr_start), Some(addr_end)) =
                (&self.shadow_index, query.address_start, query.address_end)
            {
                Some(shadow.get_range(addr_start, addr_end))
            } else {
                None
            };

        // Determine which events to scan
        let matching_events: Vec<&TraceEvent> = if candidate_ids.is_some() || address_candidate_ids.is_some() {
            // If we have index results, intersect them
            let mut id_set: Option<std::collections::HashSet<u64>> = None;

            if let Some(ids) = candidate_ids {
                id_set = Some(ids.into_iter().collect());
            }
            if let Some(ids) = address_candidate_ids {
                match id_set {
                    None => id_set = Some(ids.into_iter().collect()),
                    Some(ref mut set) => {
                        let other: std::collections::HashSet<u64> = ids.into_iter().collect();
                        *set = set.intersection(&other).copied().collect();
                    }
                }
            }

            match id_set {
                Some(set) => self.events.iter().filter(|e| set.contains(&e.event_id)).collect(),
                None => self.events.iter().collect(),
            }
        } else {
            // No index hints — scan all events
            self.events.iter().collect()
        };

        // Apply remaining filters
        let filtered: Vec<&TraceEvent> = matching_events
            .into_iter()
            .filter(|e| query.matches(e))
            .collect();

        let total_matching = filtered.len() as u64;

        // Apply pagination
        let paginated: Vec<TraceEvent> = filtered
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .cloned()
            .collect();

        let next_offset = if (query.offset + query.limit) < total_matching as usize {
            Some(query.offset + query.limit)
        } else {
            None
        };

        QueryResult {
            total_matching,
            events: paginated,
            next_offset,
        }
    }

    /// Compute an execution summary for the entire trace.
    pub fn execution_summary(&self, session_id: &str) -> ExecutionSummary {
        let mut event_counts: HashMap<EventType, u64> = HashMap::new();
        let mut function_counts: HashMap<String, u64> = HashMap::new();
        let mut threads: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut min_ts: Option<u64> = None;
        let mut max_ts: Option<u64> = None;
        let mut issues: Vec<PotentialIssue> = Vec::new();

        for event in &self.events {
            // Count by type
            *event_counts.entry(event.event_type).or_insert(0) += 1;

            // Count function calls
            if event.event_type == EventType::FunctionEntry {
                if let Some(ref func) = event.location.function {
                    *function_counts.entry(func.clone()).or_insert(0) += 1;
                }
            }

            // Track threads
            threads.insert(event.thread_id);

            // Track time range
            min_ts = Some(min_ts.map_or(event.timestamp_ns, |m| m.min(event.timestamp_ns)));
            max_ts = Some(max_ts.map_or(event.timestamp_ns, |m| m.max(event.timestamp_ns)));

            // Detect signals as potential issues
            if event.event_type == EventType::SignalDelivered {
                if let EventData::Signal { signal_name, .. } = &event.data {
                    if signal_name != "SIGSTOP" && signal_name != "SIGCHLD" {
                        issues.push(PotentialIssue {
                            issue_type: "signal".into(),
                            confidence: if signal_name == "SIGSEGV" || signal_name == "SIGABRT" {
                                0.95
                            } else {
                                0.6
                            },
                            description: format!("Signal received: {}", signal_name),
                        });
                    }
                }
            }
        }

        let duration_ns = match (min_ts, max_ts) {
            (Some(min), Some(max)) => max - min,
            _ => 0,
        };

        // Sort functions by call count descending
        let mut top_functions: Vec<FunctionStats> = function_counts
            .into_iter()
            .map(|(name, call_count)| FunctionStats { name, call_count })
            .collect();
        top_functions.sort_by(|a, b| b.call_count.cmp(&a.call_count));
        top_functions.truncate(20); // Top 20

        // Sort event counts by count descending
        let mut event_counts_by_type: Vec<(String, u64)> = event_counts
            .into_iter()
            .map(|(et, count)| (et.to_string(), count))
            .collect();
        event_counts_by_type.sort_by(|a, b| b.1.cmp(&a.1));

        ExecutionSummary {
            session_id: session_id.into(),
            duration_ns,
            total_events: self.events.len() as u64,
            event_counts_by_type,
            top_functions,
            thread_count: threads.len() as u64,
            potential_issues: issues,
        }
    }

    /// Reconstruct the call stack at a given event ID.
    ///
    /// Uses FunctionEntry/FunctionExit events to build a virtual stack.
    /// Only considers events from the same thread as the target event.
    pub fn reconstruct_call_stack(&self, at_event_id: u64) -> Vec<StackFrame> {
        // Find the thread_id of the target event
        let target_thread = match self.get_event_by_id(at_event_id) {
            Some(e) => e.thread_id,
            None => {
                // If we can't find the event, use thread 1
                // (for events past the end of the trace)
                1
            }
        };

        let mut stack: Vec<StackFrame> = Vec::new();
        let mut depth: u32 = 0;

        for event in &self.events {
            if event.event_id > at_event_id {
                break;
            }

            // Only track events from the target thread
            if event.thread_id != target_thread {
                continue;
            }

            match event.event_type {
                EventType::FunctionEntry => {
                    let func_name = event.location.function.clone().unwrap_or_default();
                    stack.push(StackFrame {
                        depth,
                        function: func_name,
                        file: event.location.file.clone(),
                        line: event.location.line,
                        address: event.location.address,
                    });
                    depth += 1;
                }
                EventType::FunctionExit => {
                    if depth > 0 {
                        depth -= 1;
                        stack.pop();
                    }
                }
                _ => {}
            }
        }

        // Reverse so innermost frame is first
        stack.reverse();
        stack
    }

    /// Compute a state diff between two timestamps.
    ///
    /// Compares register snapshots and variable values at two points in time.
    pub fn state_diff(&self, timestamp_a: u64, timestamp_b: u64) -> StateDiff {
        let mut changes: Vec<StateChange> = Vec::new();

        // Find register snapshots at or before each timestamp
        let regs_a = self.find_registers_at(timestamp_a);
        let regs_b = self.find_registers_at(timestamp_b);

        if let (Some(ra), Some(rb)) = (&regs_a, &regs_b) {
            // Compare each register
            let register_fields = [
                ("rax", ra.rax, rb.rax),
                ("rbx", ra.rbx, rb.rbx),
                ("rcx", ra.rcx, rb.rcx),
                ("rdx", ra.rdx, rb.rdx),
                ("rsi", ra.rsi, rb.rsi),
                ("rdi", ra.rdi, rb.rdi),
                ("rbp", ra.rbp, rb.rbp),
                ("rsp", ra.rsp, rb.rsp),
                ("rip", ra.rip, rb.rip),
                ("rflags", ra.rflags, rb.rflags),
            ];

            for (name, val_a, val_b) in &register_fields {
                if val_a != val_b {
                    changes.push(StateChange {
                        field: format!("registers.{}", name),
                        value_a: format!("0x{:x}", val_a),
                        value_b: format!("0x{:x}", val_b),
                    });
                }
            }
        }

        StateDiff {
            timestamp_a,
            timestamp_b,
            changes,
        }
    }

    /// Find the register snapshot at or immediately before a timestamp.
    fn find_registers_at(&self, timestamp: u64) -> Option<chronos_domain::RegisterState> {
        let mut latest: Option<chronos_domain::RegisterState> = None;

        for event in &self.events {
            if event.timestamp_ns > timestamp {
                break;
            }

            if let EventData::Registers(ref regs) = event.data {
                latest = Some(regs.clone());
            }
        }

        latest
    }

    /// Get all unique thread IDs in the trace.
    pub fn thread_ids(&self) -> Vec<u64> {
        let mut threads: std::collections::HashSet<u64> = std::collections::HashSet::new();
        for event in &self.events {
            threads.insert(event.thread_id);
        }
        let mut result: Vec<u64> = threads.into_iter().collect();
        result.sort();
        result
    }

    /// Get events for a specific thread.
    pub fn events_for_thread(&self, thread_id: u64) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.thread_id == thread_id)
            .collect()
    }

    /// Get the first event (by event_id).
    pub fn first_event(&self) -> Option<&TraceEvent> {
        self.events.first()
    }

    /// Get the last event (by event_id).
    pub fn last_event(&self) -> Option<&TraceEvent> {
        self.events.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{
        EventData, EventType, RegisterState, SourceLocation, TraceEvent,
    };

    fn make_event(id: u64, ts: u64, tid: u64, event_type: EventType, func: &str, addr: u64) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            tid,
            event_type,
            SourceLocation::new("test.rs", 10, func, addr),
            EventData::Empty,
        )
    }

    fn make_signal_event(id: u64, ts: u64, tid: u64, sig_num: i32, sig_name: &str) -> TraceEvent {
        TraceEvent::signal(id, ts, tid, sig_num, sig_name, 0)
    }

    fn make_register_event(id: u64, ts: u64, regs: RegisterState) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            1,
            EventType::Custom,
            SourceLocation::from_address(regs.rip),
            EventData::Registers(regs),
        )
    }

    fn sample_events() -> Vec<TraceEvent> {
        vec![
            make_event(0, 100, 1, EventType::FunctionEntry, "main", 0x1000),
            make_event(1, 200, 1, EventType::FunctionEntry, "helper", 0x2000),
            make_event(2, 300, 1, EventType::SyscallEnter, "helper", 0x2000),
            make_event(3, 400, 1, EventType::SyscallExit, "helper", 0x2000),
            make_event(4, 500, 1, EventType::FunctionExit, "helper", 0x2000),
            make_event(5, 600, 1, EventType::FunctionEntry, "process", 0x3000),
            make_event(6, 700, 2, EventType::FunctionEntry, "worker", 0x4000), // thread 2
            make_signal_event(7, 800, 1, 11, "SIGSEGV"),
            make_event(8, 900, 1, EventType::FunctionExit, "process", 0x3000),
            make_event(9, 1000, 1, EventType::FunctionExit, "main", 0x1000),
        ]
    }

    #[test]
    fn test_engine_new() {
        let engine = QueryEngine::new(vec![]);
        assert_eq!(engine.event_count(), 0);
        assert!(engine.first_event().is_none());
        assert!(engine.last_event().is_none());
    }

    #[test]
    fn test_engine_with_events() {
        let engine = QueryEngine::new(sample_events());
        assert_eq!(engine.event_count(), 10);
        assert_eq!(engine.first_event().unwrap().event_id, 0);
        assert_eq!(engine.last_event().unwrap().event_id, 9);
    }

    #[test]
    fn test_get_event_by_id() {
        let engine = QueryEngine::new(sample_events());
        let event = engine.get_event_by_id(5).unwrap();
        assert_eq!(event.location.function.as_deref(), Some("process"));

        assert!(engine.get_event_by_id(999).is_none());
    }

    #[test]
    fn test_execute_query_all() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1");
        let result = engine.execute(&query);
        assert_eq!(result.total_matching, 10);
        assert_eq!(result.events.len(), 10);
        assert!(result.next_offset.is_none());
    }

    #[test]
    fn test_execute_query_with_pagination() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1").pagination(3, 0);
        let result = engine.execute(&query);
        assert_eq!(result.total_matching, 10);
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.next_offset, Some(3));
    }

    #[test]
    fn test_execute_query_second_page() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1").pagination(3, 3);
        let result = engine.execute(&query);
        assert_eq!(result.total_matching, 10);
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.events[0].event_id, 3);
    }

    #[test]
    fn test_execute_query_filter_by_type() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1")
            .event_types(vec![EventType::FunctionEntry, EventType::FunctionExit]);
        let result = engine.execute(&query);
        // FunctionEntry: main, helper, process, worker (4)
        // FunctionExit: helper, process, main (3)
        assert_eq!(result.total_matching, 7);
    }

    #[test]
    fn test_execute_query_filter_by_time() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1").time_range(300, 700);
        let result = engine.execute(&query);
        // Events with ts 300-699: IDs 2(300),3(400),4(500),5(600) = 4 events
        // ID 6 has ts 700 which is excluded (end is exclusive)
        assert_eq!(result.total_matching, 4);
    }

    #[test]
    fn test_execute_query_filter_by_function() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1").function_pattern("helper");
        let result = engine.execute(&query);
        // Events where function is "helper": IDs 1,2,3,4
        assert_eq!(result.total_matching, 4);
    }

    #[test]
    fn test_execute_query_empty_result() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1").time_range(99999, 100000);
        let result = engine.execute(&query);
        assert_eq!(result.total_matching, 0);
        assert!(result.events.is_empty());
        assert!(result.next_offset.is_none());
    }

    #[test]
    fn test_execution_summary() {
        let engine = QueryEngine::new(sample_events());
        let summary = engine.execution_summary("session-1");

        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.total_events, 10);
        assert_eq!(summary.duration_ns, 900); // 1000 - 100
        assert_eq!(summary.thread_count, 2); // threads 1 and 2
        assert!(!summary.top_functions.is_empty());
        assert!(!summary.event_counts_by_type.is_empty());
    }

    #[test]
    fn test_execution_summary_top_functions() {
        let engine = QueryEngine::new(sample_events());
        let summary = engine.execution_summary("session-1");

        // main: 1 entry, helper: 1 entry, process: 1 entry, worker: 1 entry
        assert!(summary.top_functions.len() <= 4);
        // All functions should have call_count 1
        for f in &summary.top_functions {
            assert_eq!(f.call_count, 1);
        }
    }

    #[test]
    fn test_execution_summary_detects_signals() {
        let engine = QueryEngine::new(sample_events());
        let summary = engine.execution_summary("session-1");

        // Event 7 is a signal (SIGSEGV would be nice but our test has generic signal)
        assert!(!summary.potential_issues.is_empty());
        let signal_issue = summary.potential_issues.iter().find(|i| i.issue_type == "signal");
        assert!(signal_issue.is_some());
    }

    #[test]
    fn test_execution_summary_empty_trace() {
        let engine = QueryEngine::new(vec![]);
        let summary = engine.execution_summary("empty");
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.duration_ns, 0);
        assert_eq!(summary.thread_count, 0);
    }

    #[test]
    fn test_reconstruct_call_stack() {
        let engine = QueryEngine::new(sample_events());
        let stack = engine.reconstruct_call_stack(3); // During syscall in helper

        // Stack after reverse: innermost first
        assert_eq!(stack.len(), 2);
        assert_eq!(stack[0].function, "helper"); // innermost (most recently called)
        assert_eq!(stack[1].function, "main"); // outermost
    }

    #[test]
    fn test_reconstruct_call_stack_after_exit() {
        let engine = QueryEngine::new(sample_events());
        // Event 10 doesn't exist — engine falls back to thread 1
        // Thread 1 events: main→helper→helper_exit→process→process_exit→main_exit
        // All balanced, stack should be empty
        let stack = engine.reconstruct_call_stack(100);
        assert!(stack.is_empty());
    }

    #[test]
    fn test_reconstruct_call_stack_at_main() {
        let engine = QueryEngine::new(sample_events());
        let stack = engine.reconstruct_call_stack(0); // Just entered main

        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].function, "main");
    }

    #[test]
    fn test_state_diff() {
        let regs_a = RegisterState {
            rax: 42,
            rip: 0x1000,
            ..Default::default()
        };
        let regs_b = RegisterState {
            rax: 99,
            rip: 0x2000,
            ..Default::default()
        };

        let events = vec![
            make_register_event(1, 100, regs_a),
            make_register_event(2, 200, regs_b),
        ];

        let engine = QueryEngine::new(events);
        let diff = engine.state_diff(100, 200);

        assert_eq!(diff.timestamp_a, 100);
        assert_eq!(diff.timestamp_b, 200);
        assert!(!diff.changes.is_empty());

        // rax changed from 42 to 99
        let rax_change = diff.changes.iter().find(|c| c.field == "registers.rax").unwrap();
        assert_eq!(rax_change.value_a, "0x2a");
        assert_eq!(rax_change.value_b, "0x63");

        // rip changed
        let rip_change = diff.changes.iter().find(|c| c.field == "registers.rip").unwrap();
        assert_eq!(rip_change.value_a, "0x1000");
        assert_eq!(rip_change.value_b, "0x2000");
    }

    #[test]
    fn test_state_diff_no_change() {
        let regs = RegisterState {
            rax: 42,
            ..Default::default()
        };

        let events = vec![
            make_register_event(1, 100, regs.clone()),
            make_register_event(2, 200, regs),
        ];

        let engine = QueryEngine::new(events);
        let diff = engine.state_diff(100, 200);

        assert!(diff.changes.is_empty());
    }

    #[test]
    fn test_state_diff_no_registers() {
        let engine = QueryEngine::new(sample_events());
        let diff = engine.state_diff(100, 500);
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn test_thread_ids() {
        let engine = QueryEngine::new(sample_events());
        let threads = engine.thread_ids();
        assert_eq!(threads, vec![1, 2]);
    }

    #[test]
    fn test_events_for_thread() {
        let engine = QueryEngine::new(sample_events());
        let t1_events = engine.events_for_thread(1);
        let t2_events = engine.events_for_thread(2);

        assert_eq!(t1_events.len(), 9); // All except event 6
        assert_eq!(t2_events.len(), 1); // Only event 6
    }

    #[test]
    fn test_execute_with_indices() {
        use chronos_domain::{ShadowIndex, TemporalIndex};

        let events = sample_events();
        let mut shadow = ShadowIndex::new();
        let mut temporal = TemporalIndex::new();

        for event in &events {
            temporal.insert(event.timestamp_ns, event.event_id);
            if matches!(event.event_type, EventType::FunctionEntry | EventType::FunctionExit) {
                shadow.insert(event.location.address, event.event_id);
            }
        }
        temporal.build_chunks();

        let engine = QueryEngine::with_indices(events, shadow, temporal);
        assert_eq!(engine.event_count(), 10);

        // Query with time range should use temporal index
        let query = TraceQuery::new("session-1").time_range(300, 700);
        let result = engine.execute(&query);
        // Same as test_execute_query_filter_by_time: 4 events
        assert_eq!(result.total_matching, 4);
    }

    #[test]
    fn test_execute_query_combined_filters() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1")
            .event_types(vec![EventType::FunctionEntry])
            .time_range(200, 600);

        let result = engine.execute(&query);
        // FunctionEntry events in [200, 600): IDs 1 (helper, ts 200), 5 (process, ts 600 is excluded)
        assert_eq!(result.total_matching, 1);
        assert_eq!(result.events[0].location.function.as_deref(), Some("helper"));
    }
}
