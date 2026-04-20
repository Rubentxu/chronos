//! Index builder — consumes events and builds indices.

use chronos_domain::{
    EventType, ShadowIndex, TemporalIndex, TraceEvent,
};

/// Built indices from a trace session.
#[derive(Debug, Clone)]
pub struct BuiltIndices {
    pub shadow: ShadowIndex,
    pub temporal: TemporalIndex,
}

/// Builder that incrementally constructs indices from a stream of events.
pub struct IndexBuilder {
    shadow: ShadowIndex,
    temporal: TemporalIndex,
    event_count: u64,
}

impl IndexBuilder {
    /// Create a new empty index builder.
    pub fn new() -> Self {
        Self {
            shadow: ShadowIndex::new(),
            temporal: TemporalIndex::new(),
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
}
