//! Index builder — consumes events and builds indices.

use chronos_domain::{
    CausalityEntry, CausalityIndex, EventData, EventType, PerformanceIndex, ShadowIndex,
    TemporalIndex, TraceEvent,
};

/// Built indices from a trace session.
#[derive(Debug, Clone)]
pub struct BuiltIndices {
    pub shadow: ShadowIndex,
    pub temporal: TemporalIndex,
    pub causality: CausalityIndex,
    pub performance: PerformanceIndex,
}

/// Builder that incrementally constructs indices from a stream of events.
pub struct IndexBuilder {
    shadow: ShadowIndex,
    temporal: TemporalIndex,
    causality: CausalityIndex,
    performance: PerformanceIndex,
    event_count: u64,
}

impl IndexBuilder {
    /// Create a new empty index builder.
    pub fn new() -> Self {
        Self {
            shadow: ShadowIndex::new(),
            temporal: TemporalIndex::new(),
            causality: CausalityIndex::new(),
            performance: PerformanceIndex::new(),
            event_count: 0,
        }
    }

    /// Push a single event into the index builder.
    pub fn push(&mut self, event: &TraceEvent) {
        self.event_count += 1;

        // Temporal index: always index by timestamp
        self.temporal.insert(event.timestamp_ns, event.event_id);

        // Shadow index: index memory-related events by address
        match event.event_type {
            EventType::MemoryWrite | EventType::VariableWrite => {
                self.shadow.insert(event.location.address, event.event_id);
            }
            EventType::FunctionEntry | EventType::FunctionExit => {
                self.shadow.insert(event.location.address, event.event_id);
            }
            _ => {}
        }

        // Performance index: record function calls on FunctionEntry
        if event.event_type == EventType::FunctionEntry {
            let func_name = match &event.data {
                EventData::Function { name, .. } => Some(name.clone()),
                _ => event.location.function.clone(),
            };
            // Cycle counts not available from trace events directly (requires perf_event_open).
            self.performance.record_call(event.location.address, func_name, None);
        }

        // Causality index: record write mutations for VariableWrite and MemoryWrite
        match event.event_type {
            EventType::VariableWrite => {
                if let EventData::Variable(ref var_info) = event.data {
                    let entry = CausalityEntry {
                        event_id: event.event_id,
                        timestamp: event.timestamp_ns,
                        thread_id: event.thread_id,
                        value_before: None,
                        value_after: var_info.value.clone(),
                        function: event.location.function.clone().unwrap_or_default(),
                        file: event.location.file.clone(),
                        line: event.location.line,
                    };
                    let addr = event.location.address;
                    let var_name = var_info.name.as_str();
                    self.causality.record_write(addr, entry, Some(var_name));
                }
            }
            EventType::MemoryWrite => {
                if let EventData::Memory { address, .. } = event.data {
                    let entry = CausalityEntry {
                        event_id: event.event_id,
                        timestamp: event.timestamp_ns,
                        thread_id: event.thread_id,
                        value_before: None,
                        value_after: format!("0x{address:x}"),
                        function: event.location.function.clone().unwrap_or_default(),
                        file: event.location.file.clone(),
                        line: event.location.line,
                    };
                    self.causality.record_write(address, entry, None);
                }
            }
            _ => {}
        }
    }

    /// Push multiple events.
    pub fn push_all(&mut self, events: &[TraceEvent]) {
        for event in events {
            self.push(event);
        }
    }

    /// Finalize the indices (builds temporal chunks for fast range queries).
    pub fn finalize(self) -> BuiltIndices {
        let mut temporal = self.temporal;
        temporal.build_chunks();

        BuiltIndices {
            shadow: self.shadow,
            temporal,
            causality: self.causality,
            performance: self.performance,
        }
    }

    /// Returns the number of events indexed so far.
    pub fn event_count(&self) -> u64 {
        self.event_count
    }
}

impl Default for IndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    fn make_event(id: u64, ts: u64, event_type: EventType, addr: u64) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            1,
            event_type,
            SourceLocation::from_address(addr),
            EventData::Empty,
        )
    }

    fn make_variable_write(id: u64, ts: u64, addr: u64, name: &str, value: &str) -> TraceEvent {
        let var_info = chronos_domain::VariableInfo {
            name: name.to_string(),
            value: value.to_string(),
            type_name: "i32".to_string(),
            address: addr,
            scope: chronos_domain::VariableScope::Local,
        };
        TraceEvent::new(
            id,
            ts,
            1,
            EventType::VariableWrite,
            chronos_domain::SourceLocation::from_address(addr),
            EventData::Variable(var_info),
        )
    }

    fn make_function_entry(id: u64, ts: u64, addr: u64, name: &str) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            1,
            EventType::FunctionEntry,
            chronos_domain::SourceLocation::from_address(addr),
            EventData::Function { name: name.to_string(), signature: None },
        )
    }

    #[test]
    fn test_index_builder_basic() {
        let mut builder = IndexBuilder::new();

        builder.push(&make_event(0, 100, EventType::FunctionEntry, 0x1000));
        builder.push(&make_event(1, 200, EventType::FunctionExit, 0x1000));
        builder.push(&make_event(2, 300, EventType::MemoryWrite, 0x2000));
        builder.push(&make_event(3, 400, EventType::SyscallEnter, 0x0000));

        assert_eq!(builder.event_count(), 4);

        let indices = builder.finalize();

        // Shadow should have entries for function and memory events
        assert!(!indices.shadow.get(0x1000).is_empty());
        assert!(!indices.shadow.get(0x2000).is_empty());

        // Syscall at address 0 should not be in shadow
        assert!(indices.shadow.get(0x0000).is_empty());

        // Temporal should have all 4 events
        assert_eq!(indices.temporal.len(), 4);
        assert_eq!(indices.temporal.min_timestamp(), Some(100));
        assert_eq!(indices.temporal.max_timestamp(), Some(400));
    }

    #[test]
    fn test_index_builder_temporal_chunks() {
        let mut builder = IndexBuilder::new();

        // Insert events across multiple 10ms chunks
        for i in 0..50 {
            builder.push(&make_event(i, (i as u64) * 1_000_000, EventType::FunctionEntry, 0x1000));
        }

        let indices = builder.finalize();
        assert!(indices.temporal.chunk_count() >= 2);
    }

    #[test]
    fn test_index_builder_push_all() {
        let mut builder = IndexBuilder::new();
        let events: Vec<TraceEvent> = (0..10)
            .map(|i| make_event(i, i * 100, EventType::FunctionEntry, 0x1000 + i))
            .collect();

        builder.push_all(&events);
        assert_eq!(builder.event_count(), 10);

        let indices = builder.finalize();
        assert_eq!(indices.temporal.len(), 10);
    }

    #[test]
    fn test_index_builder_default() {
        let builder = IndexBuilder::default();
        assert_eq!(builder.event_count(), 0);
    }

    #[test]
    fn test_index_builder_range_query() {
        let mut builder = IndexBuilder::new();
        for i in 0..100 {
            builder.push(&make_event(i, i * 1000, EventType::FunctionEntry, 0x1000));
        }
        let indices = builder.finalize();

        // Query events between 20ms and 50ms
        let events = indices.temporal.range(20_000, 50_000);
        assert_eq!(events.len(), 30); // events 20..49
    }

    #[test]
    fn test_build_performance_index() {
        let mut builder = IndexBuilder::new();

        // Three calls to "compute" and one to "helper"
        builder.push(&make_function_entry(0, 100, 0x3000, "compute"));
        builder.push(&make_function_entry(1, 200, 0x3000, "compute"));
        builder.push(&make_function_entry(2, 300, 0x3000, "compute"));
        builder.push(&make_function_entry(3, 400, 0x4000, "helper"));

        let indices = builder.finalize();

        // "compute" called 3 times
        let perf = indices.performance.function_perf(0x3000).unwrap();
        assert_eq!(perf.call_count, 3);
        assert_eq!(perf.name.as_deref(), Some("compute"));

        // "helper" called 1 time
        let helper = indices.performance.function_perf(0x4000).unwrap();
        assert_eq!(helper.call_count, 1);

        // top_functions_by_calls returns "compute" first
        let top = indices.performance.top_functions_by_calls(2);
        assert_eq!(top[0].address, 0x3000);
        assert_eq!(top[1].address, 0x4000);
    }

    #[test]
    fn test_perf_counters_graceful_fallback() {
        // Without perf_event_open, global counters remain empty but call counts still work.
        let mut builder = IndexBuilder::new();
        builder.push(&make_function_entry(0, 100, 0x5000, "any_fn"));
        let indices = builder.finalize();

        // Global counters have no data (not set during index building)
        assert!(!indices.performance.read_counters().has_data());

        // But call count is tracked
        let perf = indices.performance.function_perf(0x5000).unwrap();
        assert_eq!(perf.call_count, 1);
    }

    #[test]
    fn test_build_causality_index() {
        let mut builder = IndexBuilder::new();
        let addr = 0x5000;

        // Push two VariableWrite events for "counter"
        builder.push(&make_variable_write(0, 100, addr, "counter", "10"));
        builder.push(&make_variable_write(1, 200, addr, "counter", "20"));
        // Unrelated function event — should not appear in causality
        builder.push(&make_event(2, 300, EventType::FunctionEntry, 0x1000));

        let indices = builder.finalize();

        // Should have 2 causality entries for addr
        let writes = indices.causality.writes_at(addr);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].timestamp, 100);
        assert_eq!(writes[1].timestamp, 200);

        // find_last_mutation before ts=250 returns ts=200 entry
        let last = indices.causality.find_last_mutation(addr, 250).unwrap();
        assert_eq!(last.event_id, 1);

        // trace_lineage by name
        let lineage = indices.causality.trace_lineage("counter");
        assert_eq!(lineage.len(), 2);

        // Function events don't create causality entries
        assert!(indices.causality.writes_at(0x1000).is_empty());
    }
}
