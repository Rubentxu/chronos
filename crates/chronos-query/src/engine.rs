//! Query engine — executes queries against trace data using indices.
//!
//! The `QueryEngine` is the central query processor. It takes a set of events,
//! optionally indexed with shadow and temporal indices, and executes `TraceQuery`
//! requests to find matching events, reconstruct call stacks, and compute
//! execution summaries.

use chronos_domain::{
    query::{
        CausalityQuery, CausalityResult, ExecutionSummary, FunctionStats, MutationRecord,
        PerfEntry, PerfQuery, PerfResult, PerfSortBy, PotentialIssue, RaceDetectionQuery,
        RaceDetectionResult, StackFrame, StateChange, StateDiff, SuspiciousConcurrentAccess,
    },
    CausalityIndex, EventData, EventType, MonotonicNs, PerformanceIndex, QueryResult, ShadowIndex,
    TemporalIndex, TimestampNs, TraceEvent, TraceQuery,
};
use std::collections::HashMap;

pub use crate::expr_eval::{EvalError, ExprEvaluator};

/// The query engine — holds trace data and indices for fast queries.
pub struct QueryEngine {
    /// All events in the trace (ordered by event_id).
    events: Vec<TraceEvent>,
    /// Shadow index (address → event IDs).
    shadow_index: Option<ShadowIndex>,
    /// Temporal index (timestamp → event IDs).
    temporal_index: Option<TemporalIndex>,
    /// Causality index (address/name → write mutations).
    causality_index: Option<CausalityIndex>,
    /// Performance index (function perf stats).
    performance_index: Option<PerformanceIndex>,
}

/// Result of a memory read.
#[derive(Debug, Clone)]
pub struct MemoryValue {
    /// Event ID of the memory write.
    pub event_id: u64,
    /// Timestamp of the memory write.
    pub timestamp_ns: u64,
    /// Memory address.
    pub address: u64,
    /// Size in bytes. This is the size the write declared, and it is known
    /// even when the bytes were not captured.
    pub size: usize,
    /// Raw data bytes, or `None` when the write did not carry them.
    ///
    /// `None` means "the contents of this write were not recorded", which is
    /// a different claim from "the contents are empty". A `Memory` event may
    /// legitimately report an address and a size without a payload, and
    /// substituting an empty `Vec` here made that read as a zeroed region
    /// paired with a real non-zero `size` -- the most misleading combination
    /// the output could carry, because the size invites the reader to believe
    /// the bytes were seen and found empty.
    pub data: Option<Vec<u8>>,
}

/// Saliency score for a function.
#[derive(Debug, Clone)]
pub struct FunctionSaliencyScore {
    /// Function name.
    pub function: String,
    /// Saliency score [0.0-1.0].
    pub saliency_score: f64,
    /// Call count.
    pub call_count: u64,
    /// Total CPU cycles (if available).
    pub total_cycles: u64,
}

impl QueryEngine {
    /// Create a new query engine from a vec of events (no indices).
    pub fn new(events: Vec<TraceEvent>) -> Self {
        Self {
            events,
            shadow_index: None,
            temporal_index: None,
            causality_index: None,
            performance_index: None,
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
            causality_index: None,
            performance_index: None,
        }
    }

    /// Create a query engine with all indices including causality.
    pub fn with_all_indices(
        events: Vec<TraceEvent>,
        shadow_index: ShadowIndex,
        temporal_index: TemporalIndex,
        causality_index: CausalityIndex,
    ) -> Self {
        Self {
            events,
            shadow_index: Some(shadow_index),
            temporal_index: Some(temporal_index),
            causality_index: Some(causality_index),
            performance_index: None,
        }
    }

    /// Set or replace the causality index.
    pub fn with_causality(mut self, causality: CausalityIndex) -> Self {
        self.causality_index = Some(causality);
        self
    }

    /// True iff a causality index has been configured.
    ///
    /// Per M10.3 follow-up #2 wiring: this accessor lets
    /// `chronos_services::live_streaming::causality_status_for_engine`
    /// decide whether the live stream is `Wired` (causality
    /// integration available) vs `Unsupported` (no index).
    ///
    /// ADR-0004 honest: this only inspects the field; it does NOT
    /// try to construct a CausalityIndex on demand. If the caller
    /// has not wired one, status is Unsupported — period.
    pub fn causality_index_is_configured(&self) -> bool {
        self.causality_index.is_some()
    }

    /// Set or replace the performance index.
    pub fn with_performance(mut self, performance: PerformanceIndex) -> Self {
        self.performance_index = Some(performance);
        self
    }

    /// Get the total number of events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Borrow all events in the engine (in insertion order).
    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    /// Merge new events into the engine and rebuild all indices.
    ///
    /// Events whose `event_id` already exists in the engine are
    /// deduplicated (the existing entry wins). New events may carry ids
    /// below the current maximum; the merged set is left sorted in
    /// ascending `event_id` order, which `get_event_by_id` requires. The
    /// shadow, temporal, causality, and performance indices are fully
    /// rebuilt so queries reflect the union of old + new events.
    pub fn merge(&mut self, new_events: Vec<TraceEvent>) {
        if new_events.is_empty() {
            return;
        }
        let mut existing_ids: std::collections::HashSet<u64> =
            self.events.iter().map(|e| e.event_id).collect();
        self.events.extend(
            new_events
                .into_iter()
                .filter(|e| existing_ids.insert(e.event_id)),
        );
        // The extend above is a pure append, so the order is broken
        // whenever an incoming id is not greater than the current
        // maximum. Unsorted, `get_event_by_id`'s binary search
        // returns None for events that exist — no panic, just a missing
        // answer. Re-sort before rebuilding the indices (which scan the
        // vec linearly and are therefore order-independent).
        //
        // An earlier version of this comment justified the sort with a
        // "second capture run reusing the same session id re-mints ids
        // from 1". That scenario is not reachable: every capture run
        // mints a fresh session UUID, so two runs never share a session
        // and their id spaces cannot overlap. The sort stays — it is
        // still the invariant `get_event_by_id` requires, and callers can
        // reach this method from outside — but the stated reason was
        // wrong, and a comment that names an impossible cause stops being
        // checked.
        self.events.sort_by_key(|e| e.event_id);
        self.rebuild_indices();
    }

    fn rebuild_indices(&mut self) {
        let mut builder = chronos_index::builder::IndexBuilder::new();
        builder.push_all(&self.events);
        let indices = builder.finalize();
        self.shadow_index = Some(indices.shadow);
        self.temporal_index = Some(indices.temporal);
        self.causality_index = Some(indices.causality);
        self.performance_index = Some(indices.performance);
    }

    /// Get an event by its ID using binary search.
    ///
    /// Requires events to be sorted by event_id. `merge` restores that
    /// order after every merge, so an engine grown through it is always
    /// sorted; an engine built by `new`/`with_indices` inherits the
    /// caller's order.
    pub fn get_event_by_id(&self, event_id: u64) -> Option<&TraceEvent> {
        self.events
            .binary_search_by_key(&event_id, |e| e.event_id)
            .ok()
            .map(|i| &self.events[i])
    }

    /// Execute a query and return matching events with pagination.
    pub fn execute(&self, query: &TraceQuery) -> QueryResult {
        // Use temporal index for time-range queries if available
        let candidate_ids: Option<Vec<u64>> =
            if let (Some(ref temporal), Some(ts_start), Some(ts_end)) = (
                &self.temporal_index,
                query.timestamp_start,
                query.timestamp_end,
            ) {
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
        let matching_events: Vec<&TraceEvent> =
            if candidate_ids.is_some() || address_candidate_ids.is_some() {
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
                    Some(set) => self
                        .events
                        .iter()
                        .filter(|e| set.contains(&e.event_id))
                        .collect(),
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

        // `offset` and `limit` are free `usize` values from tool input and are
        // clamped nowhere, so the sum can overflow. In release that wraps: with
        // `offset = 1` and `limit = usize::MAX` the result is `0`, the
        // comparison below passes, and `next_offset` becomes `Some(0)` — a
        // client that follows it receives the first page forever, silently.
        // Saturating puts the sum above any real page, so `next_offset` is
        // `None`, which is the truth: this page already covers everything.
        let next_offset = {
            let consumed = query.offset.saturating_add(query.limit);
            if consumed < total_matching as usize {
                Some(consumed)
            } else {
                None
            }
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
        let mut min_ts: Option<TimestampNs> = None;
        let mut max_ts: Option<TimestampNs> = None;
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
            (Some(min), Some(max)) => max.get() - min.get(),
            _ => 0,
        };

        // Census of the whole trace, taken BEFORE the ranking cutoff below:
        // after `truncate(20)` this number is gone for good, and it is the
        // only honest denominator for any share computed downstream.
        let total_function_calls: u64 = function_counts.values().sum();

        // Sort functions by call count descending
        let mut top_functions: Vec<FunctionStats> = function_counts
            .into_iter()
            .map(|(name, call_count)| FunctionStats { name, call_count })
            .collect();
        top_functions.sort_by_key(|a| std::cmp::Reverse(a.call_count));
        top_functions.truncate(20); // Top 20

        // REC-C1.7 drift #20: sort_by_key(Reverse(count)) alone leaves ties in
        // HashMap iteration order, which differs between process lifetimes. The
        // tie-break by event type name makes the projection a deterministic
        // function of the log (required for restart equivalence).
        let mut event_counts_by_type: Vec<(String, u64)> = event_counts
            .into_iter()
            .map(|(et, count)| (et.to_string(), count))
            .collect();
        event_counts_by_type.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        ExecutionSummary {
            session_id: session_id.into(),
            duration_ns,
            total_events: self.events.len() as u64,
            event_counts_by_type,
            total_function_calls,
            top_functions,
            thread_count: threads.len() as u64,
            potential_issues: issues,
        }
    }

    /// Reconstruct the call stack at a given event ID.
    ///
    /// Uses FunctionEntry/FunctionExit events to build a virtual stack.
    /// Only considers events from the same thread as the target event.
    ///
    /// # When the event cannot be resolved
    ///
    /// Returns no frames when `at_event_id` does not resolve to an event of
    /// this engine, and an empty result then means "the thread is unknown",
    /// not "the stack was empty at that point".
    ///
    /// Resolution goes through [`Self::get_event_by_id`], a binary search that
    /// requires the log to be sorted by `event_id`. An unsorted log can miss an
    /// event that is really present: frame events minted from a timestamp
    /// (`event_id == entry_monotonic_ns`) and flushed at the tail of the log
    /// on process kill arrive with ids that descend again, and a log built by
    /// `new`/`with_indices` keeps the caller's order. The thread to walk is
    /// unknown in that case, and this used to answer with thread 1 -- a
    /// well-formed stack belonging to a different thread, indistinguishable
    /// from a correct answer. Declining is the only answer that cannot be
    /// wrong; sorting the log is a separate decision owned by the services
    /// layer, which derives causal edges from the vector order.
    pub fn reconstruct_call_stack(&self, at_event_id: u64) -> Vec<StackFrame> {
        // Find the thread_id of the target event. There is no safe default
        // thread: guessing one reports another thread's frames as if they were
        // the answer to this query.
        let target_thread = match self.get_event_by_id(at_event_id) {
            Some(e) => e.thread_id,
            None => return Vec::new(),
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
                EventType::FunctionExit if depth > 0 => {
                    depth -= 1;
                    stack.pop();
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
    /// The returned [`StateDiff`] always reports whether any register evidence
    /// was found (`register_evidence`) and, when not, an `evidence_note` that
    /// explains why the diff is empty. This lets callers distinguish "the
    /// program did not change registers" from "no register snapshots were
    /// captured for this session".
    pub fn state_diff(&self, timestamp_a: TimestampNs, timestamp_b: TimestampNs) -> StateDiff {
        let mut changes: Vec<StateChange> = Vec::new();

        // Find register snapshots at or before each timestamp
        let regs_a = self.find_registers_at(timestamp_a);
        let regs_b = self.find_registers_at(timestamp_b);
        let register_evidence = regs_a.is_some() || regs_b.is_some();

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

        let evidence_note = if !register_evidence {
            Some(format!(
                "no register snapshots found in [{}, {}]. Register capture must be enabled when starting the probe.",
                timestamp_a.get(),
                timestamp_b.get()
            ))
        } else if regs_a.is_none() || regs_b.is_none() {
            Some(format!(
                "register snapshots missing on one side: a={}, b={}. Cannot compute a complete diff.",
                if regs_a.is_some() { "present" } else { "missing" },
                if regs_b.is_some() { "present" } else { "missing" },
            ))
        } else {
            None
        };

        StateDiff {
            timestamp_a,
            timestamp_b,
            changes,
            evidence_note,
            register_evidence,
        }
    }

    /// Find the register snapshot at or immediately before a timestamp.
    fn find_registers_at(&self, timestamp: TimestampNs) -> Option<chronos_domain::RegisterState> {
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

    /// Get all events as a owned vector.
    pub fn get_all_events(&self) -> Vec<TraceEvent> {
        self.events.clone()
    }

    // ─── Variable inspection queries ─────────────────────────────────────────────

    /// Get all variables in scope at the given event.
    ///
    /// Returns locals from PythonFrame/JavaFrame/GoFrame,
    /// or the single VariableInfo from VariableWrite events.
    /// Returns an empty vec if the event is not found or has no variables.
    pub fn get_variables_at_event(&self, event_id: u64) -> Vec<chronos_domain::VariableInfo> {
        let event = match self.get_event_by_id(event_id) {
            Some(e) => e,
            None => return vec![],
        };

        match &event.data {
            EventData::PythonFrame { locals, .. } => locals.clone().unwrap_or_default(),
            EventData::JavaFrame { locals, .. } => locals.clone().unwrap_or_default(),
            EventData::GoFrame { locals, .. } => locals.clone().unwrap_or_default(),
            EventData::JsFrame { locals, .. } => locals.clone().unwrap_or_default(),
            EventData::Variable(var_info) => vec![var_info.clone()],
            _ => vec![],
        }
    }

    /// Get the most recent memory write at `address` at or before `timestamp_ns`.
    ///
    /// Returns `None` if no memory event exists at that address at or before
    /// the requested timestamp.
    pub fn get_memory_at(&self, address: u64, timestamp_ns: TimestampNs) -> Option<MemoryValue> {
        // Try shadow index first for O(1) address lookup
        let candidate_ids: Vec<u64> = if let Some(ref shadow) = self.shadow_index {
            shadow.get(address).to_vec()
        } else {
            // Fallback: linear scan (no index)
            self.events
                .iter()
                .filter(|e| {
                    if let EventData::Memory { address: addr, .. } = &e.data {
                        *addr == address
                    } else {
                        false
                    }
                })
                .map(|e| e.event_id)
                .collect()
        };

        let mut best: Option<MemoryValue> = None;
        for id in candidate_ids {
            if let Some(event) = self.get_event_by_id(id) {
                if event.timestamp_ns <= timestamp_ns {
                    if let EventData::Memory {
                        address: addr,
                        size,
                        data,
                    } = &event.data
                    {
                        let is_newer = best
                            .as_ref()
                            .map(|b| event.timestamp_ns.get() > b.timestamp_ns)
                            .unwrap_or(true);
                        if is_newer {
                            best = Some(MemoryValue {
                                event_id: event.event_id,
                                timestamp_ns: event.timestamp_ns.get(),
                                address: *addr,
                                size: *size,
                                data: data.clone(),
                            });
                        }
                    }
                }
            }
        }
        best
    }

    /// Evaluate an arithmetic expression using local variables at the given event.
    ///
    /// Uses variables captured at the frame event to resolve variable names in the expression.
    /// Returns the result of evaluating the expression, or an error if evaluation fails.
    pub fn evaluate_expression(&self, event_id: u64, expression: &str) -> Result<f64, EvalError> {
        let vars = self.get_variables_at_event(event_id);
        let locals: HashMap<String, String> = vars.into_iter().map(|v| (v.name, v.value)).collect();
        let evaluator = ExprEvaluator::new(locals);
        evaluator.evaluate(expression)
    }

    // ─── Performance queries ──────────────────────────────────────────────────

    /// Query performance index for top functions.
    ///
    /// Returns `None` if no performance index is loaded.
    pub fn query_perf(&self, query: &PerfQuery) -> Option<PerfResult> {
        let perf = self.performance_index.as_ref()?;
        let counters = perf.read_counters();

        let functions: Vec<PerfEntry> = match query.sort_by {
            PerfSortBy::Cycles => perf.top_functions_by_cycles(query.limit),
            PerfSortBy::CallCount => perf.top_functions_by_calls(query.limit),
        }
        .into_iter()
        .filter(|f| {
            if let Some(ref filter) = query.function_filter {
                f.name.as_deref().unwrap_or("").contains(filter.as_str())
            } else {
                true
            }
        })
        .map(|f| PerfEntry {
            address: f.address,
            name: f.name.clone(),
            call_count: f.call_count,
            total_cycles: f.total_cycles,
            avg_cycles: f.avg_cycles(),
        })
        .collect();

        Some(PerfResult {
            functions,
            counters_available: counters.has_data(),
            total_session_cycles: counters.cycles,
        })
    }

    /// Convenience wrapper: return top N functions sorted by total_cycles.
    ///
    /// Returns an empty vec if no performance index is loaded.
    pub fn top_functions_by_cycles(&self, limit: usize) -> Vec<PerfEntry> {
        let query = PerfQuery::new("").top(limit);
        self.query_perf(&query)
            .map(|r| r.functions)
            .unwrap_or_default()
    }

    /// Find register state at or immediately before a given event.
    ///
    /// Uses the event_id to look up the event's timestamp, then finds
    /// the most recent register snapshot at or before that timestamp.
    pub fn find_registers_at_event(&self, event_id: u64) -> Option<chronos_domain::RegisterState> {
        let timestamp = self.get_event_by_id(event_id)?.timestamp_ns;
        self.find_registers_at(timestamp)
    }

    /// Get saliency scores for all functions.
    ///
    /// Returns a vector of function stats with computed saliency scores.
    /// Falls back to call-count based scoring if no performance index.
    pub fn get_saliency_scores(&self, limit: usize) -> Vec<FunctionSaliencyScore> {
        let summary = self.execution_summary("");

        // Try to get perf data
        let perf_result = self.query_perf(&PerfQuery {
            session_id: String::new(),
            function_filter: None,
            sort_by: PerfSortBy::Cycles,
            limit,
        });

        if let Some(perf) = perf_result {
            let total_cycles: u64 = perf.functions.iter().map(|e| e.total_cycles).sum();
            perf.functions
                .iter()
                .take(limit)
                .map(|entry| {
                    let score = if total_cycles > 0 {
                        entry.total_cycles as f64 / total_cycles as f64
                    } else {
                        0.0
                    };
                    FunctionSaliencyScore {
                        function: entry.name.clone().unwrap_or_default(),
                        saliency_score: (score * 10000.0).round() / 10000.0,
                        call_count: entry.call_count,
                        total_cycles: entry.total_cycles,
                    }
                })
                .collect()
        } else {
            // Fallback: call-count based scoring
            //
            // The denominator is the analysed set, not the trace total:
            // `execution_summary` keeps only the hottest 20 functions
            // (`truncate(20)` above), so the score is each function's share
            // *within that set*. That is the right question here -- the
            // rankings stay consistent because numerator and denominator
            // cover the same set. Using the true trace total would shrink
            // every score by the same factor while answering a different
            // one. (It is now available: `summary.total_function_calls`. This
            // fallback keeps the analysed-set semantics on purpose -- do not
            // "fix" it into a trace-total score without changing the
            // documented meaning of the field.) Mirrors the same fallback in
            // `chronos-services::debug_trace_specialized`.
            let analyzed_calls: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
            summary
                .top_functions
                .iter()
                .take(limit)
                .map(|f| {
                    let score = if analyzed_calls > 0 {
                        f.call_count as f64 / analyzed_calls as f64
                    } else {
                        0.0
                    };
                    FunctionSaliencyScore {
                        function: f.name.clone(),
                        saliency_score: (score * 10000.0).round() / 10000.0,
                        call_count: f.call_count,
                        total_cycles: 0,
                    }
                })
                .collect()
        }
    }

    // ─── Causality queries ────────────────────────────────────────────────────

    /// Query causality index for variable/memory write origin.
    ///
    /// Returns `None` if no causality index is loaded.
    pub fn query_causality(&self, query: &CausalityQuery) -> Option<CausalityResult> {
        let causality = self.causality_index.as_ref()?;

        // Resolve address from query
        let addr = if let Some(a) = query.address {
            a
        } else {
            let name = query.variable_name.as_ref()?;
            // Use trace_lineage to find any address associated with this name
            let lineage = causality.trace_lineage(name);
            if lineage.is_empty() {
                return Some(CausalityResult {
                    address: 0,
                    variable_name: Some(name.clone()),
                    mutations: vec![],
                });
            }
            lineage[0].event_id // use first entry's event_id as proxy; addr resolved below
        };

        if query.full_lineage {
            // Return full lineage by name or by address
            let entries: Vec<&chronos_domain::CausalityEntry> =
                if let Some(ref name) = query.variable_name {
                    causality.trace_lineage(name)
                } else {
                    // `trace_lineage` sorts by timestamp; `writes_at` returns
                    // insertion order, which is not chronological. Sorting
                    // here keeps one contract for `mutations`: without it the
                    // same field is ordered one way when filtered by name and
                    // another when filtered by address.
                    let mut by_addr: Vec<&chronos_domain::CausalityEntry> =
                        causality.writes_at(addr).iter().collect();
                    by_addr.sort_by_key(|e| e.timestamp);
                    by_addr
                };

            let mutations = entries
                .iter()
                .map(|e| mutation_record_from_entry(e))
                .collect();
            Some(CausalityResult {
                address: addr,
                variable_name: query.variable_name.clone(),
                mutations,
            })
        } else {
            // Return only last mutation before timestamp
            let before_ts = query
                .before_timestamp
                .unwrap_or(TimestampNs::from(u64::MAX));
            let entry = causality.find_last_mutation(addr, before_ts)?;
            Some(CausalityResult {
                address: addr,
                variable_name: query.variable_name.clone(),
                mutations: vec![mutation_record_from_entry(entry)],
            })
        }
    }

    /// Detect suspicious concurrent accesses: two writes to the same
    /// memory address from different threads within `threshold_ns`
    /// nanoseconds. m0-09: this used to be called `detect_races` — the
    /// new name makes the heuristic's epistemic status explicit (we do
    /// not run happens-before analysis, so this is a triage signal, not
    /// a verdict).
    pub fn detect_concurrent_access(&self, query: &RaceDetectionQuery) -> RaceDetectionResult {
        let causality = match &self.causality_index {
            Some(c) => c,
            None => {
                return RaceDetectionResult {
                    accesses: vec![],
                    addresses_checked: 0,
                }
            }
        };

        let mut accesses = Vec::new();
        let mut addresses_checked = 0;

        // Iterate all addresses in the causality index
        // We walk events to find VariableWrite/MemoryWrite events grouped by address
        let mut addr_writes: std::collections::HashMap<u64, Vec<&TraceEvent>> =
            std::collections::HashMap::new();

        for event in &self.events {
            if matches!(
                event.event_type,
                EventType::VariableWrite | EventType::MemoryWrite
            ) {
                // Apply time range filter if specified
                if let Some((start, end)) = query.time_range {
                    if event.timestamp_ns < start || event.timestamp_ns >= end {
                        continue;
                    }
                }
                addr_writes
                    .entry(event.location.address)
                    .or_default()
                    .push(event);
            }
        }

        for (addr, writes) in &addr_writes {
            addresses_checked += 1;
            // Check all pairs of writes at this address from different threads
            for i in 0..writes.len() {
                for j in (i + 1)..writes.len() {
                    let a = writes[i];
                    let b = writes[j];
                    if a.thread_id == b.thread_id {
                        continue; // same thread — not a suspicious concurrent access
                    }
                    let delta = a.timestamp_ns.get().abs_diff(b.timestamp_ns.get());
                    if delta <= query.threshold_ns {
                        // Build MutationRecords from causality index
                        let wa = causality
                            .find_last_mutation(*addr, MonotonicNs::from(a.timestamp_ns.get() + 1))
                            .map(mutation_record_from_entry)
                            .unwrap_or_else(|| event_to_mutation_record(a));
                        let wb = causality
                            .find_last_mutation(*addr, MonotonicNs::from(b.timestamp_ns.get() + 1))
                            .map(mutation_record_from_entry)
                            .unwrap_or_else(|| event_to_mutation_record(b));
                        accesses.push(SuspiciousConcurrentAccess {
                            address: *addr,
                            write_a: wa,
                            write_b: wb,
                            delta_ns: delta,
                        });
                    }
                }
            }
        }

        RaceDetectionResult {
            accesses,
            addresses_checked,
        }
    }
}

fn mutation_record_from_entry(e: &chronos_domain::CausalityEntry) -> MutationRecord {
    MutationRecord {
        event_id: e.event_id,
        timestamp: e.timestamp,
        thread_id: e.thread_id,
        value_before: e.value_before.clone(),
        value_after: e.value_after.clone(),
        function: e.function.clone(),
        file: e.file.clone(),
        line: e.line,
    }
}

fn event_to_mutation_record(e: &TraceEvent) -> MutationRecord {
    MutationRecord {
        event_id: e.event_id,
        timestamp: e.timestamp_ns,
        thread_id: e.thread_id,
        value_before: None,
        value_after: String::new(),
        function: e.location.function.clone().unwrap_or_default(),
        file: e.location.file.clone(),
        line: e.location.line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, RegisterState, SourceLocation, TraceEvent};

    fn make_event(
        id: u64,
        ts: u64,
        tid: u64,
        event_type: EventType,
        func: &str,
        addr: u64,
    ) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            event_type,
            SourceLocation::new("test.rs", 10, func, addr),
            EventData::Empty,
        )
    }

    fn make_signal_event(id: u64, ts: u64, tid: u64, sig_num: i32, sig_name: &str) -> TraceEvent {
        TraceEvent::signal(id, MonotonicNs::from(ts), tid, sig_num, sig_name, 0)
    }

    fn make_register_event(id: u64, ts: u64, regs: RegisterState) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            1,
            EventType::Custom,
            SourceLocation::from_address(regs.rip),
            EventData::Registers(regs),
        )
    }

    /// An overflowing `offset + limit` must not wrap into a `next_offset` that
    /// sends the client back to the start forever. `offset` and `limit` are
    /// free `usize` from tool input with no clamp anywhere, so this is
    /// reachable; in release the old sum wrapped to `0` and paginated the
    /// first page indefinitely, without ever erroring.
    #[test]
    fn overflowing_pagination_does_not_wrap_next_offset() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("s").pagination(usize::MAX, 1);
        let result = engine.execute(&query);
        assert_eq!(
            result.next_offset, None,
            "a saturated page covers everything, so there is no next page; got {:?}",
            result.next_offset
        );
    }

    /// The ordinary case must keep paginating. Without this the guard could
    /// silently disable pagination for every query.
    #[test]
    fn ordinary_pagination_still_reports_the_next_page() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("s").pagination(3, 0);
        let result = engine.execute(&query);
        assert_eq!(result.next_offset, Some(3));
        assert_eq!(result.events.len(), 3);
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
        let query =
            TraceQuery::new("session-1").time_range(MonotonicNs::from(300), MonotonicNs::from(700));
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
        let query = TraceQuery::new("session-1")
            .time_range(MonotonicNs::from(99999), MonotonicNs::from(100000));
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
        let signal_issue = summary
            .potential_issues
            .iter()
            .find(|i| i.issue_type == "signal");
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
        // Event 100 does not exist, so no thread can be resolved for it and the
        // engine reports no frames. Thread 1's own events here are balanced
        // (main -> helper -> helper_exit -> process -> process_exit ->
        // main_exit), so an empty result is also exactly what thread 1 would
        // have produced. That agreement is why this test cannot tell "thread
        // unknown" from "stack empty", and why the unsorted-log test below
        // carries the contract.
        let stack = engine.reconstruct_call_stack(100);
        assert!(stack.is_empty());
    }

    /// Regression: an event id that IS in the log can still be unresolvable,
    /// because `get_event_by_id` binary-searches a vector that is not required
    /// to be sorted. The old code answered with thread 1, so the caller got a
    /// complete, plausible call stack belonging to a different thread -- the
    /// worst shape for a consumer, indistinguishable from a correct answer.
    ///
    /// Frame events minted from a timestamp (`event_id == entry_monotonic_ns`)
    /// and flushed at the tail of the log on process kill arrive with ids that
    /// descend again, which is where the unsorted log comes from.
    #[test]
    fn test_reconstruct_call_stack_unresolvable_id_does_not_borrow_thread_one() {
        // Ids 5, 20, 15, 12, 10: the target (id 20, thread 2) is followed by a
        // strictly descending tail, so the vector is unsorted and the binary
        // search never compares against index 1.
        let engine = QueryEngine::new(vec![
            make_event(5, 100, 1, EventType::FunctionEntry, "t1_outer", 0x1000),
            make_event(20, 200, 2, EventType::FunctionEntry, "t2_target", 0x4000),
            make_event(15, 300, 1, EventType::FunctionEntry, "t1_inner", 0x2000),
            make_event(12, 400, 1, EventType::SyscallEnter, "t1_inner", 0x2000),
            make_event(10, 500, 1, EventType::Custom, "t1_inner", 0x2000),
        ]);

        // Premise 1: the target event really is in this log.
        assert!(
            engine.events().iter().any(|e| e.event_id == 20),
            "the target event must exist, or this scenario proves nothing"
        );
        // Premise 2: the binary search cannot see it. Thread 1 is two frames
        // deep at id 20, so the old fallback answered with two wrong frames
        // rather than accidentally agreeing with the honest answer.
        assert!(
            engine.get_event_by_id(20).is_none(),
            "an unsorted log is the premise of this test"
        );

        // Same log, sorted: now the query is answerable and does answer, with
        // the open thread 2 frame. Without this, "returns no frames" could
        // pass by returning no frames for every input.
        let mut sorted_events = engine.events().to_vec();
        sorted_events.sort_by_key(|e| e.event_id);
        let sorted_stack = QueryEngine::new(sorted_events).reconstruct_call_stack(20);
        let sorted_frames: Vec<&str> = sorted_stack.iter().map(|f| f.function.as_str()).collect();
        assert_eq!(
            sorted_frames,
            vec!["t2_target"],
            "the sorted log must still answer, with the target thread's frame"
        );

        let stack = engine.reconstruct_call_stack(20);
        let frames: Vec<&str> = stack.iter().map(|f| f.function.as_str()).collect();
        assert!(
            frames.is_empty(),
            "an unresolvable event id must not borrow another thread's stack; \
             got {frames:?} (thread 1) for an event on thread 2: {stack:?}"
        );
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
        let diff = engine.state_diff(MonotonicNs::from(100), MonotonicNs::from(200));

        assert_eq!(diff.timestamp_a, MonotonicNs::from(100));
        assert_eq!(diff.timestamp_b, MonotonicNs::from(200));
        assert!(!diff.changes.is_empty());

        // rax changed from 42 to 99
        let rax_change = diff
            .changes
            .iter()
            .find(|c| c.field == "registers.rax")
            .unwrap();
        assert_eq!(rax_change.value_a, "0x2a");
        assert_eq!(rax_change.value_b, "0x63");

        // rip changed
        let rip_change = diff
            .changes
            .iter()
            .find(|c| c.field == "registers.rip")
            .unwrap();
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
        let diff = engine.state_diff(MonotonicNs::from(100), MonotonicNs::from(200));

        assert!(diff.changes.is_empty());
    }

    #[test]
    fn test_state_diff_no_registers() {
        let engine = QueryEngine::new(sample_events());
        let diff = engine.state_diff(MonotonicNs::from(100), MonotonicNs::from(500));
        assert!(diff.changes.is_empty());
        // m0-06: when no register snapshots are captured, the diff must
        // explicitly report zero register evidence and a note that explains
        // why, so callers can distinguish "no changes" from "no evidence".
        assert!(!diff.register_evidence);
        assert!(
            diff.evidence_note.is_some(),
            "m0-06: empty diff must carry an evidence_note"
        );
        let note = diff.evidence_note.as_deref().unwrap_or("");
        assert!(
            note.contains("register snapshots"),
            "m0-06: evidence_note must mention register snapshots, got {:?}",
            note
        );
    }

    #[test]
    fn test_state_diff_with_evidence_no_note() {
        // When register snapshots exist for both anchors and there are no
        // changes, the diff is a legitimate "no diff" result and should NOT
        // carry an evidence_note (otherwise we'd be misleadingly explaining
        // away a real empty diff).
        use chronos_domain::{EventData, EventType, RegisterState, SourceLocation};
        let mut events: Vec<TraceEvent> = Vec::new();
        for ts in [100u64, 200u64] {
            events.push(TraceEvent::new(
                1,
                MonotonicNs::from(ts),
                1,
                EventType::BreakpointHit,
                SourceLocation::default(),
                EventData::Registers(RegisterState {
                    rax: 42,
                    ..Default::default()
                }),
            ));
        }
        let engine = QueryEngine::new(events);
        let diff = engine.state_diff(MonotonicNs::from(100), MonotonicNs::from(200));
        assert!(diff.register_evidence);
        assert!(diff.changes.is_empty());
        assert!(
            diff.evidence_note.is_none(),
            "m0-06: diff with evidence and no changes must NOT carry a note, got {:?}",
            diff.evidence_note
        );
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
            if matches!(
                event.event_type,
                EventType::FunctionEntry | EventType::FunctionExit
            ) {
                shadow.insert(event.location.address, event.event_id);
            }
        }
        temporal.build_chunks();

        let engine = QueryEngine::with_indices(events, shadow, temporal);
        assert_eq!(engine.event_count(), 10);

        // Query with time range should use temporal index
        let query =
            TraceQuery::new("session-1").time_range(MonotonicNs::from(300), MonotonicNs::from(700));
        let result = engine.execute(&query);
        // Same as test_execute_query_filter_by_time: 4 events
        assert_eq!(result.total_matching, 4);
    }

    #[test]
    fn test_execute_query_combined_filters() {
        let engine = QueryEngine::new(sample_events());
        let query = TraceQuery::new("session-1")
            .event_types(vec![EventType::FunctionEntry])
            .time_range(MonotonicNs::from(200), MonotonicNs::from(600));

        let result = engine.execute(&query);
        // FunctionEntry events in [200, 600): IDs 1 (helper, ts 200), 5 (process, ts 600 is excluded)
        assert_eq!(result.total_matching, 1);
        assert_eq!(
            result.events[0].location.function.as_deref(),
            Some("helper")
        );
    }

    // ─── Causality tests ──────────────────────────────────────────────────────

    fn make_causality_engine() -> QueryEngine {
        use chronos_domain::{CausalityEntry, CausalityIndex};

        let addr = 0xA000u64;
        let mut causality = CausalityIndex::new();

        causality.record_write(
            addr,
            CausalityEntry {
                event_id: 10,
                timestamp: MonotonicNs::from(100),
                thread_id: 1,
                value_before: None,
                value_after: "0".to_string(),
                function: "init".to_string(),
                file: None,
                line: None,
            },
            Some("counter"),
        );

        causality.record_write(
            addr,
            CausalityEntry {
                event_id: 11,
                timestamp: MonotonicNs::from(200),
                thread_id: 1,
                value_before: Some("0".to_string()),
                value_after: "1".to_string(),
                function: "increment".to_string(),
                file: None,
                line: None,
            },
            Some("counter"),
        );

        causality.record_write(
            addr,
            CausalityEntry {
                event_id: 12,
                timestamp: MonotonicNs::from(300),
                thread_id: 2,
                value_before: Some("1".to_string()),
                value_after: "2".to_string(),
                function: "increment".to_string(),
                file: None,
                line: None,
            },
            Some("counter"),
        );

        QueryEngine::new(vec![]).with_causality(causality)
    }

    #[test]
    fn test_query_causality_find_last_mutation() {
        use chronos_domain::query::CausalityQuery;

        let engine = make_causality_engine();
        let query = CausalityQuery::new("s1")
            .by_address(0xA000)
            .before(MonotonicNs::from(250));

        let result = engine.query_causality(&query).unwrap();
        assert_eq!(result.mutations.len(), 1);
        assert_eq!(result.mutations[0].timestamp, MonotonicNs::from(200));
        assert_eq!(result.mutations[0].value_after, "1");
    }

    #[test]
    fn test_query_causality_trace_lineage() {
        use chronos_domain::query::CausalityQuery;

        let engine = make_causality_engine();
        let query = CausalityQuery::new("s1")
            .by_address(0xA000)
            .with_full_lineage();

        let result = engine.query_causality(&query).unwrap();
        assert_eq!(result.mutations.len(), 3);
        // Ordered by timestamp
        assert_eq!(result.mutations[0].timestamp, MonotonicNs::from(100));
        assert_eq!(result.mutations[2].value_after, "2");
    }

    /// The discriminating case. `test_query_causality_trace_lineage` above
    /// asserts chronological order too, but its fixture records the writes in
    /// chronological order, so it passes with or without the sort. Here the
    /// writes arrive out of order — which the builder allows, because the
    /// events reaching `push_all` are not guaranteed sorted — and the by-name
    /// and by-address paths must still agree.
    #[test]
    fn lineage_by_address_is_chronological_even_when_writes_arrive_unsorted() {
        use chronos_domain::query::CausalityQuery;
        use chronos_domain::{CausalityEntry, CausalityIndex};

        let addr = 0xC000u64;
        let mut causality = CausalityIndex::new();
        // Deliberately out of order: 300, then 100, then 200.
        for (event_id, ts, value) in [(30u64, 300u64, "c"), (10, 100, "a"), (20, 200, "b")] {
            causality.record_write(
                addr,
                CausalityEntry {
                    event_id,
                    timestamp: MonotonicNs::from(ts),
                    thread_id: 1,
                    value_before: None,
                    value_after: value.to_string(),
                    function: "f".to_string(),
                    file: None,
                    line: None,
                },
                Some("unsorted"),
            );
        }
        let engine = QueryEngine::new(vec![]).with_causality(causality);

        let by_addr = engine
            .query_causality(
                &CausalityQuery::new("s1")
                    .by_address(addr)
                    .with_full_lineage(),
            )
            .unwrap();
        let by_name = engine
            .query_causality(
                &CausalityQuery::new("s1")
                    .by_name("unsorted")
                    .with_full_lineage(),
            )
            .unwrap();

        let stamps = |m: &[chronos_domain::query::MutationRecord]| {
            m.iter().map(|x| x.timestamp.get()).collect::<Vec<_>>()
        };
        assert_eq!(
            stamps(&by_addr.mutations),
            vec![100, 200, 300],
            "the by-address path must be chronological, not insertion-ordered"
        );
        assert_eq!(
            stamps(&by_addr.mutations),
            stamps(&by_name.mutations),
            "both filter paths must yield one order for the same mutations"
        );
    }

    #[test]
    fn test_detect_concurrent_access_100ns_threshold() {
        use chronos_domain::query::RaceDetectionQuery;
        use chronos_domain::{CausalityEntry, CausalityIndex, EventType, SourceLocation};

        let addr = 0xB000u64;
        let mut causality = CausalityIndex::new();

        // Two writes to same address from different threads within 50ns
        // (suspicious concurrent access).
        causality.record_write(
            addr,
            CausalityEntry {
                event_id: 1,
                timestamp: MonotonicNs::from(1000),
                thread_id: 1,
                value_before: None,
                value_after: "x".to_string(),
                function: "f1".to_string(),
                file: None,
                line: None,
            },
            None,
        );
        causality.record_write(
            addr,
            CausalityEntry {
                event_id: 2,
                timestamp: MonotonicNs::from(1050),
                thread_id: 2,
                value_before: None,
                value_after: "y".to_string(),
                function: "f2".to_string(),
                file: None,
                line: None,
            },
            None,
        );

        // Events to drive address_writes detection
        let events = vec![
            TraceEvent::new(
                1,
                MonotonicNs::from(1000),
                1,
                EventType::VariableWrite,
                SourceLocation::from_address(addr),
                chronos_domain::EventData::Empty,
            ),
            TraceEvent::new(
                2,
                MonotonicNs::from(1050),
                2,
                EventType::VariableWrite,
                SourceLocation::from_address(addr),
                chronos_domain::EventData::Empty,
            ),
        ];

        let engine = QueryEngine::new(events).with_causality(causality);
        let query = RaceDetectionQuery::new("s1"); // threshold = 100ns

        let result = engine.detect_concurrent_access(&query);
        assert_eq!(result.accesses.len(), 1);
        assert_eq!(result.accesses[0].address, addr);
        assert_eq!(result.accesses[0].delta_ns, 50);
    }

    // ─── Performance tests ────────────────────────────────────────────────────

    fn make_perf_engine() -> QueryEngine {
        let mut perf = PerformanceIndex::new();
        perf.record_call(0x1000, Some("hot_fn".to_string()), Some(9000));
        perf.record_call(0x1000, Some("hot_fn".to_string()), Some(1000));
        perf.record_call(0x2000, Some("cold_fn".to_string()), Some(200));
        perf.record_call(0x3000, Some("medium_fn".to_string()), Some(5000));
        QueryEngine::new(vec![]).with_performance(perf)
    }

    #[test]
    fn test_query_perf_top_by_cycles() {
        use chronos_domain::query::PerfQuery;

        let engine = make_perf_engine();
        let query = PerfQuery::new("s1").top(2);
        let result = engine.query_perf(&query).unwrap();

        assert_eq!(result.functions.len(), 2);
        // hot_fn has 10000 total cycles
        assert_eq!(result.functions[0].address, 0x1000);
        assert_eq!(result.functions[0].total_cycles, 10000);
        assert_eq!(result.functions[0].call_count, 2);
        assert!((result.functions[0].avg_cycles - 5000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_query_perf_sort_by_calls() {
        use chronos_domain::query::PerfQuery;

        let engine = make_perf_engine();
        let query = PerfQuery::new("s1").sort_by_calls().top(3);
        let result = engine.query_perf(&query).unwrap();

        // hot_fn called 2 times, others 1 time
        assert_eq!(result.functions[0].address, 0x1000);
        assert_eq!(result.functions[0].call_count, 2);
    }

    #[test]
    fn test_query_perf_function_filter() {
        use chronos_domain::query::PerfQuery;

        let engine = make_perf_engine();
        let query = PerfQuery::new("s1").filter_function("hot");
        let result = engine.query_perf(&query).unwrap();

        assert_eq!(result.functions.len(), 1);
        assert_eq!(result.functions[0].name.as_deref(), Some("hot_fn"));
    }

    #[test]
    fn test_query_perf_no_index_returns_none() {
        use chronos_domain::query::PerfQuery;

        let engine = QueryEngine::new(vec![]);
        let result = engine.query_perf(&PerfQuery::new("s1"));
        assert!(result.is_none());
    }

    #[test]
    fn test_top_functions_by_cycles_convenience() {
        let engine = make_perf_engine();
        let top = engine.top_functions_by_cycles(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].total_cycles, 10000); // hot_fn
    }

    #[test]
    fn test_perf_counters_unavailable() {
        use chronos_domain::query::PerfQuery;

        // No global counters set → counters_available = false
        let engine = make_perf_engine();
        let result = engine.query_perf(&PerfQuery::new("s1")).unwrap();
        assert!(!result.counters_available);
        assert!(result.total_session_cycles.is_none());
    }

    #[test]
    fn test_get_all_events_returns_all() {
        let engine = QueryEngine::new(sample_events());
        let all = engine.get_all_events();
        assert_eq!(all.len(), 10);
        assert_eq!(all[0].event_id, 0);
        assert_eq!(all[9].event_id, 9);
        // Verify it's a clone, not a reference
        assert_eq!(all[0].event_id, engine.events[0].event_id);
    }

    #[test]
    fn test_get_event_by_id_binary_search_correctness() {
        // Create 1000 events with sequential IDs
        let events: Vec<TraceEvent> = (0..1000u64)
            .map(|i| {
                TraceEvent::new(
                    i,
                    MonotonicNs::from(i * 100),
                    1,
                    EventType::FunctionEntry,
                    SourceLocation::new("test.rs", 10, format!("fn_{}", i), 0x1000 + i),
                    EventData::Empty,
                )
            })
            .collect();
        let engine = QueryEngine::new(events);

        // Test retrieving various IDs
        assert!(engine.get_event_by_id(0).is_some());
        assert_eq!(engine.get_event_by_id(0).unwrap().event_id, 0);

        assert!(engine.get_event_by_id(500).is_some());
        assert_eq!(engine.get_event_by_id(500).unwrap().event_id, 500);

        assert!(engine.get_event_by_id(999).is_some());
        assert_eq!(engine.get_event_by_id(999).unwrap().event_id, 999);

        // Test non-existent ID
        assert!(engine.get_event_by_id(1000).is_none());
        assert!(engine.get_event_by_id(123456).is_none());

        // Test boundary cases
        assert!(engine.get_event_by_id(1).is_some());
        assert_eq!(engine.get_event_by_id(1).unwrap().event_id, 1);
    }

    // ─── Variable inspection tests ─────────────────────────────────────────────

    fn make_python_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        TraceEvent::python_call_with_locals(
            id,
            MonotonicNs::from(ts),
            tid,
            "my_module.my_func",
            "/path/to/script.py",
            10,
            locals,
        )
    }

    fn make_java_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        use chronos_domain::JavaEventKind;
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::FunctionEntry,
            SourceLocation::new("Test.java", 10, "com.example.Foo.bar", 0x1000 + id),
            EventData::JavaFrame {
                class_name: "com.example.Foo".to_string(),
                method_name: "bar".to_string(),
                signature: None,
                file: Some("Test.java".to_string()),
                line: Some(10),
                locals: Some(locals),
                event_kind: JavaEventKind::MethodEntry,
            },
        )
    }

    fn make_go_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        use chronos_domain::GoEventKind;
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::BreakpointHit,
            SourceLocation::new("main.go", 10, "main.foo", 0x1000 + id),
            EventData::GoFrame {
                goroutine_id: tid,
                function_name: "main.foo".to_string(),
                file: Some("main.go".to_string()),
                line: Some(10),
                locals: Some(locals),
                event_kind: GoEventKind::Breakpoint,
            },
        )
    }

    fn make_js_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        use chronos_domain::JsEventKind;
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::BreakpointHit,
            SourceLocation::new("app.js", 10, "myFunction", 0x1000 + id),
            EventData::JsFrame {
                function_name: "myFunction".to_string(),
                script_url: "http://localhost:3000/app.js".to_string(),
                line_number: 10,
                column_number: 2,
                locals: Some(locals),
                scope_chain: vec!["Local".to_string()],
                event_kind: JsEventKind::Breakpoint,
            },
        )
    }

    fn make_variable_write_event(
        id: u64,
        ts: u64,
        tid: u64,
        var_info: chronos_domain::VariableInfo,
    ) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::VariableWrite,
            SourceLocation::new("test.rs", 10, "main", 0x1000 + id),
            EventData::Variable(var_info),
        )
    }

    fn var_x() -> chronos_domain::VariableInfo {
        chronos_domain::VariableInfo::new(
            "x",
            "42",
            "i32",
            0x7FFE1000,
            chronos_domain::VariableScope::Local,
        )
    }

    fn var_count() -> chronos_domain::VariableInfo {
        chronos_domain::VariableInfo::new(
            "count",
            "100",
            "i64",
            0x7FFE2000,
            chronos_domain::VariableScope::Local,
        )
    }

    #[test]
    fn test_get_variables_python_frame() {
        let locals = vec![var_x(), var_count()];
        let events = vec![make_python_frame_with_locals(0, 100, 1, locals)];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "x");
        assert_eq!(vars[0].value, "42");
        assert_eq!(vars[1].name, "count");
        assert_eq!(vars[1].value, "100");
    }

    #[test]
    fn test_get_variables_python_frame_empty_locals() {
        // PythonFrame with None locals
        let event = TraceEvent::python_call(
            0,
            MonotonicNs::from(100),
            1,
            "my_module.my_func",
            "/path/to/script.py",
            10,
        );
        let engine = QueryEngine::new(vec![event]);

        let vars = engine.get_variables_at_event(0);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_get_variables_java_frame() {
        let locals = vec![var_x()];
        let events = vec![make_java_frame_with_locals(0, 100, 1, locals)];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "x");
        assert_eq!(vars[0].value, "42");
    }

    #[test]
    fn test_get_variables_go_frame() {
        let locals = vec![var_count()];
        let events = vec![make_go_frame_with_locals(0, 100, 1, locals)];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "count");
        assert_eq!(vars[0].value, "100");
    }

    #[test]
    fn test_get_variables_js_frame() {
        let locals = vec![var_x()];
        let events = vec![make_js_frame_with_locals(0, 100, 1, locals)];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "x");
        assert_eq!(vars[0].value, "42");
    }

    #[test]
    fn test_get_variables_js_frame_empty_locals() {
        // JsFrame with None locals
        let event = TraceEvent::js_frame(
            0,
            MonotonicNs::from(100),
            1,
            "myFunction",
            "http://localhost:3000/app.js".to_string(),
            10,
            2,
            chronos_domain::JsEventKind::Breakpoint,
        );
        let engine = QueryEngine::new(vec![event]);

        let vars = engine.get_variables_at_event(0);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_get_variables_variable_write() {
        let var_info = chronos_domain::VariableInfo::new(
            "result",
            "99",
            "i32",
            0x7FFE3000,
            chronos_domain::VariableScope::Local,
        );
        let events = vec![make_variable_write_event(0, 100, 1, var_info.clone())];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "result");
        assert_eq!(vars[0].value, "99");
    }

    #[test]
    fn test_get_variables_non_frame_empty() {
        // Regular Function event (not a frame with locals)
        let events = vec![make_event(
            0,
            100,
            1,
            EventType::FunctionEntry,
            "main",
            0x1000,
        )];
        let engine = QueryEngine::new(events);

        let vars = engine.get_variables_at_event(0);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_get_variables_event_not_found() {
        let engine = QueryEngine::new(vec![]);
        let vars = engine.get_variables_at_event(999);
        // Returns None from get_event_by_id, so get_variables_at_event returns empty vec
        assert!(vars.is_empty());
    }

    // ─── Memory inspection tests ─────────────────────────────────────────────

    fn make_memory_event(
        id: u64,
        ts: u64,
        tid: u64,
        address: u64,
        size: usize,
        data: Vec<u8>,
    ) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::MemoryWrite,
            SourceLocation::from_address(address),
            EventData::Memory {
                address,
                size,
                data: Some(data),
            },
        )
    }

    /// A `Memory` write that declared a size but carried no bytes.
    ///
    /// `make_memory_event` above always wraps its payload in `Some`, which is
    /// why no test in this crate could ever reach the `None` arm.
    fn make_uncaptured_memory_event(
        id: u64,
        ts: u64,
        tid: u64,
        address: u64,
        size: usize,
    ) -> TraceEvent {
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::MemoryWrite,
            SourceLocation::from_address(address),
            EventData::Memory {
                address,
                size,
                data: None,
            },
        )
    }

    /// Discriminante for the read path, at the layer that produced the defect.
    ///
    /// `get_memory_at` used to carry the payload through `unwrap_or_default()`,
    /// collapsing "the write carried no bytes" into "the region is empty" while
    /// keeping the write's real non-zero `size`. Restoring that exact
    /// expression (`Some(data.clone().unwrap_or_default())`) makes this fail
    /// with `data: Some([])` instead of `data: None`.
    ///
    /// The positive control is `test_get_memory_found` above, which still pins
    /// the captured-bytes payload, so this cannot be satisfied by returning
    /// `None` for every read.
    #[test]
    fn test_get_memory_preserves_an_uncaptured_payload_as_absent() {
        let addr = 0x7FFF0000u64;
        let events = vec![make_uncaptured_memory_event(1, 1000, 1, addr, 64)];
        let engine = QueryEngine::new(events);

        let mem = engine
            .get_memory_at(addr, MonotonicNs::from(1500))
            .expect("an uncaptured write is still a write, so the read succeeds");

        assert_eq!(mem.address, addr);
        assert_eq!(mem.event_id, 1);
        // The declared size is known even though the bytes are not.
        assert_eq!(
            mem.size, 64,
            "the declared write size must survive the read"
        );
        assert!(
            mem.data.is_none(),
            "a write that carried no bytes must not read back as an empty \
             buffer, got {:?}",
            mem.data
        );
    }

    /// The neighbouring case: a later captured write supersedes an earlier
    /// uncaptured one at the same address. Without this, the guard above could
    /// be satisfied by treating the newest write as uncaptured.
    #[test]
    fn test_get_memory_prefers_the_latest_write_with_payload() {
        let addr = 0x7FFF0000u64;
        let events = vec![
            make_uncaptured_memory_event(1, 1000, 1, addr, 64),
            make_memory_event(2, 2000, 1, addr, 2, vec![0xAB, 0xCD]),
        ];
        let engine = QueryEngine::new(events);

        let mem = engine
            .get_memory_at(addr, MonotonicNs::from(2500))
            .expect("the captured write must be found");
        assert_eq!(mem.event_id, 2);
        assert_eq!(mem.size, 2);
        assert_eq!(mem.data.as_deref(), Some(vec![0xAB, 0xCD].as_slice()));
    }

    #[test]
    fn test_get_memory_found() {
        let addr = 0x7FFF0000u64;
        let events = vec![
            make_memory_event(1, 1000, 1, addr, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_memory_event(2, 2000, 1, addr, 4, vec![0xFF, 0xFE, 0xFD, 0xFC]),
        ];
        let engine = QueryEngine::new(events);

        // Get memory at timestamp 1500 - should return first event
        let result = engine.get_memory_at(addr, MonotonicNs::from(1500));
        assert!(result.is_some());
        let mem = result.unwrap();
        assert_eq!(mem.event_id, 1);
        assert_eq!(mem.timestamp_ns, 1000);
        assert_eq!(
            mem.data.as_deref(),
            Some(vec![0x01, 0x02, 0x03, 0x04].as_slice())
        );
    }

    #[test]
    fn test_get_memory_before_timestamp() {
        let addr = 0x7FFF0000u64;
        let events = vec![
            make_memory_event(1, 1000, 1, addr, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_memory_event(2, 2000, 1, addr, 4, vec![0xFF, 0xFE, 0xFD, 0xFC]),
            make_memory_event(3, 3000, 1, addr, 4, vec![0xAA, 0xBB, 0xCC, 0xDD]),
        ];
        let engine = QueryEngine::new(events);

        // Get memory at timestamp 2500 - should return second event
        let result = engine.get_memory_at(addr, MonotonicNs::from(2500));
        assert!(result.is_some());
        let mem = result.unwrap();
        assert_eq!(mem.event_id, 2);
        assert_eq!(mem.timestamp_ns, 2000);
        assert_eq!(
            mem.data.as_deref(),
            Some(vec![0xFF, 0xFE, 0xFD, 0xFC].as_slice())
        );
    }

    #[test]
    fn test_get_memory_not_found() {
        let events = vec![make_memory_event(
            1,
            1000,
            1,
            0x7FFF0000,
            4,
            vec![0x01, 0x02, 0x03, 0x04],
        )];
        let engine = QueryEngine::new(events);

        // Address doesn't exist
        let result = engine.get_memory_at(0x12345678, MonotonicNs::from(2000));
        assert!(result.is_none());
    }

    #[test]
    fn test_get_memory_no_index_fallback() {
        let addr = 0x7FFF0000u64;
        // Create engine without indices
        let events = vec![
            make_memory_event(1, 1000, 1, addr, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_memory_event(2, 2000, 1, addr, 4, vec![0xFF, 0xFE, 0xFD, 0xFC]),
        ];
        let engine = QueryEngine::new(events); // No indices

        let result = engine.get_memory_at(addr, MonotonicNs::from(1500));
        assert!(result.is_some());
        let mem = result.unwrap();
        assert_eq!(mem.event_id, 1);
        assert_eq!(mem.timestamp_ns, 1000);
    }

    #[test]
    fn test_get_memory_exact_timestamp() {
        let addr = 0x7FFF0000u64;
        let events = vec![
            make_memory_event(1, 1000, 1, addr, 4, vec![0x01, 0x02, 0x03, 0x04]),
            make_memory_event(2, 2000, 1, addr, 4, vec![0xFF, 0xFE, 0xFD, 0xFC]),
        ];
        let engine = QueryEngine::new(events);

        // Get at exactly timestamp 1000 - should return first event
        let result = engine.get_memory_at(addr, MonotonicNs::from(1000));
        assert!(result.is_some());
        assert_eq!(result.unwrap().event_id, 1);
    }

    #[test]
    fn test_get_memory_before_first_write() {
        let addr = 0x7FFF0000u64;
        let events = vec![make_memory_event(
            1,
            1000,
            1,
            addr,
            4,
            vec![0x01, 0x02, 0x03, 0x04],
        )];
        let engine = QueryEngine::new(events);

        // Get before first write - should return None
        let result = engine.get_memory_at(addr, MonotonicNs::from(500));
        assert!(result.is_none());
    }

    // m0-02-make-snapshots-cumulative (UAT-M0-02)
    #[test]
    fn test_engine_merge_appends_new_events() {
        let mut engine = QueryEngine::new(sample_events());
        assert_eq!(engine.event_count(), 10);

        let new_events = vec![
            make_event(10, 1100, 1, EventType::FunctionEntry, "second_main", 0x5000),
            make_event(11, 1200, 1, EventType::FunctionExit, "second_main", 0x5000),
        ];
        engine.merge(new_events);

        assert_eq!(engine.event_count(), 12);
        assert_eq!(engine.first_event().unwrap().event_id, 0);
        assert_eq!(engine.last_event().unwrap().event_id, 11);
    }

    #[test]
    fn test_engine_merge_dedupes_overlapping_ids() {
        let mut engine = QueryEngine::new(sample_events());

        // Re-add event_id 5 (already present) plus a new event_id 10.
        let new_events = vec![
            make_event(5, 999, 99, EventType::FunctionEntry, "duplicate", 0x9999),
            make_event(10, 1100, 1, EventType::FunctionEntry, "fresh", 0x5000),
        ];
        engine.merge(new_events);

        // Total goes from 10 to 11 (1 dup dropped, 1 new appended).
        assert_eq!(engine.event_count(), 11);
        // The duplicate event_id 5 keeps its original metadata
        // (timestamp_ns=600 in sample_events), not the new one (ts=999).
        let ev5 = engine.get_event_by_id(5).unwrap();
        assert_eq!(ev5.timestamp_ns, MonotonicNs::from(600));
        assert_eq!(ev5.thread_id, 1);
    }

    #[test]
    fn test_engine_merge_empty_is_noop() {
        let mut engine = QueryEngine::new(sample_events());
        let before = engine.events().to_vec();
        engine.merge(vec![]);
        let after = engine.events().to_vec();
        assert_eq!(before, after);
    }

    // Regression: a merge that backfills a gap BELOW the current maximum
    // must leave `events` in ascending event_id order. A pure append makes
    // `get_event_by_id`'s binary search miss an event that exists, and it
    // does so silently (None, no panic). The gapped base set is the shape
    // the MCP path stores: noise filtering drops some ids, and a second
    // capture run on the same session re-mints ids from 1, so a dropped id
    // arrives later and lands under the current maximum.
    #[test]
    fn test_engine_merge_backfills_lower_gap_keeps_order() {
        let mut engine = QueryEngine::new(vec![
            make_event(0, 100, 1, EventType::FunctionEntry, "gap_main", 0x1000),
            make_event(2, 200, 1, EventType::FunctionEntry, "gap_helper", 0x2000),
            make_event(4, 300, 1, EventType::FunctionExit, "gap_helper", 0x2000),
            make_event(6, 400, 2, EventType::FunctionEntry, "gap_worker", 0x4000), // thread 2
            make_event(8, 500, 1, EventType::FunctionExit, "gap_main", 0x1000),
        ]);
        assert_eq!(engine.event_count(), 5);

        // id 3 is absent from the base set and lower than the maximum (8).
        engine.merge(vec![make_event(
            3,
            250,
            1,
            EventType::SyscallEnter,
            "backfilled",
            0x3000,
        )]);
        assert_eq!(engine.event_count(), 6);

        // The backfilled event is the discriminating lookup: with a pure
        // append the vec is [0, 2, 4, 6, 8, 3] and id 3 is unreachable.
        // Probe the whole id space at once so a regression reports every
        // id the binary search lost, not just the first one asserted.
        let missing: Vec<u64> = [0u64, 1, 2, 3, 4, 6, 8]
            .iter()
            .copied()
            .filter(|id| engine.get_event_by_id(*id).is_none())
            .collect();
        assert_eq!(
            missing,
            vec![1],
            "id 1 was never stored; every other probed id must be findable"
        );

        let backfilled = engine.get_event_by_id(3).unwrap();
        assert_eq!(backfilled.timestamp_ns, MonotonicNs::from(250));
        assert_eq!(backfilled.thread_id, 1);
        assert_eq!(backfilled.location.function.as_deref(), Some("backfilled"));

        // Pre-existing events stay reachable and unchanged, so the fix did
        // not trade one missing lookup for another.
        assert_eq!(
            engine.get_event_by_id(2).map(|e| e.timestamp_ns),
            Some(MonotonicNs::from(200))
        );
        assert_eq!(engine.get_event_by_id(6).map(|e| e.thread_id), Some(2));
        assert_eq!(
            engine.get_event_by_id(8).map(|e| e.timestamp_ns),
            Some(MonotonicNs::from(500))
        );

        // The documented invariant: events are ordered by event_id.
        let ids: Vec<u64> = engine.events().iter().map(|e| e.event_id).collect();
        assert_eq!(ids, vec![0, 2, 3, 4, 6, 8]);
    }
}
