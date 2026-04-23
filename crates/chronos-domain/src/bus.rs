//! In-memory event bus with a ring buffer for real-time trace event delivery.
//!
//! The `EventBus` is the central hub for the live event bus model, replacing
//! the "record everything then analyze" approach. Events are pushed into a
//! ring buffer in real-time and can be drained via `snapshot()`.
//!
//! The bus maintains two buffers:
//! - `semantic_ring`: stores `SemanticEvent` for LLM-facing tools (probe_drain)
//! - `raw_ring`: stores `TraceEvent` for QueryEngine-facing operations (probe_stop, session_snapshot)

use crate::semantic::SemanticEvent;
use crate::TraceEvent;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Thread-safe event bus that collects semantic events in a ring buffer.
pub struct EventBus {
    /// Internal ring buffer of semantic events (for LLM-facing tools).
    semantic_ring: RwLock<VecDeque<SemanticEvent>>,
    /// Internal ring buffer of raw trace events (for QueryEngine).
    raw_ring: RwLock<VecDeque<TraceEvent>>,
    /// Maximum capacity of the ring buffer.
    capacity: usize,
    /// Metrics for monitoring.
    metrics: BusMetrics,
}

/// Metrics for the event bus.
#[derive(Debug, Default)]
pub struct BusMetrics {
    /// Total events pushed since creation.
    pub total_pushed: AtomicUsize,
    /// Current number of events in the buffer.
    pub current_len: AtomicUsize,
    /// Peak number of events ever in the buffer.
    pub high_water_mark: AtomicUsize,
    /// Number of events evicted due to overflow.
    pub evicted_count: AtomicUsize,
}

impl BusMetrics {
    fn new() -> Self {
        Self {
            total_pushed: AtomicUsize::new(0),
            current_len: AtomicUsize::new(0),
            high_water_mark: AtomicUsize::new(0),
            evicted_count: AtomicUsize::new(0),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(10000) // Default 10k event capacity
    }
}

impl EventBus {
    /// Create a new empty event bus with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            semantic_ring: RwLock::new(VecDeque::with_capacity(capacity)),
            raw_ring: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
            metrics: BusMetrics::new(),
        }
    }

    /// Create a new shared event bus handle with the given capacity.
    pub fn new_shared(capacity: usize) -> Arc<Self> {
        Arc::new(Self::new(capacity))
    }

    /// Push a single semantic event into the bus.
    ///
    /// If the buffer is at capacity, the oldest event is evicted.
    pub fn push(&self, event: SemanticEvent) {
        let mut ring = match self.semantic_ring.write() {
            Ok(ring) => ring,
            Err(_) => return, // Poisoned lock - skip event
        };

        // Evict oldest if at capacity
        if ring.len() >= self.capacity {
            ring.pop_front();
            self.metrics.evicted_count.fetch_add(1, Ordering::Relaxed);
        }

        ring.push_back(event);
        drop(ring);

        // Update metrics
        self.metrics.total_pushed.fetch_add(1, Ordering::Relaxed);
        let len = self.metrics.current_len.fetch_add(1, Ordering::Relaxed) + 1;

        // Update high water mark if needed
        let mut hwm = self.metrics.high_water_mark.load(Ordering::Relaxed);
        while len > hwm {
            match self.metrics.high_water_mark.compare_exchange_weak(
                hwm,
                len,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(cur) => hwm = cur,
            }
        }
    }

    /// Push a single raw trace event into the bus (for QueryEngine operations).
    ///
    /// If the buffer is at capacity, the oldest event is evicted.
    pub fn push_raw(&self, event: TraceEvent) {
        let mut ring = match self.raw_ring.write() {
            Ok(ring) => ring,
            Err(_) => return, // Poisoned lock - skip event
        };

        // Evict oldest if at capacity
        if ring.len() >= self.capacity {
            ring.pop_front();
        }

        ring.push_back(event);
    }

    /// Push multiple semantic events into the bus.
    pub fn push_many(&self, batch: impl IntoIterator<Item = SemanticEvent>) {
        for event in batch {
            self.push(event);
        }
    }

    /// Take a snapshot of all semantic events and clear the buffer.
    ///
    /// This is the primary mechanism for draining semantic events from the bus.
    /// Returns all events currently in the buffer, leaving it empty.
    pub fn snapshot(&self) -> Vec<SemanticEvent> {
        let mut ring = match self.semantic_ring.write() {
            Ok(ring) => ring,
            Err(_) => return Vec::new(),
        };

        let count = ring.len();
        let result: Vec<SemanticEvent> = ring.drain(..).collect();

        // Update metrics
        if count > 0 {
            self.metrics.current_len.fetch_sub(count, Ordering::Relaxed);
        }

        result
    }

    /// Take a snapshot of all raw trace events and clear the buffer.
    ///
    /// Used for building QueryEngine after probe_stop or session_snapshot.
    pub fn snapshot_raw(&self) -> Vec<TraceEvent> {
        let mut ring = match self.raw_ring.write() {
            Ok(ring) => ring,
            Err(_) => return Vec::new(),
        };

        ring.drain(..).collect()
    }

    /// Drain up to `max` semantic events from the buffer.
    ///
    /// Unlike `snapshot()`, this only drains a portion of the buffer.
    pub fn drain(&self, max: usize) -> Vec<SemanticEvent> {
        let mut ring = match self.semantic_ring.write() {
            Ok(ring) => ring,
            Err(_) => return Vec::new(),
        };

        let drain_count = std::cmp::min(max, ring.len());
        let result: Vec<SemanticEvent> = ring.drain(..drain_count).collect();

        if !result.is_empty() {
            self.metrics.current_len.fetch_sub(result.len(), Ordering::Relaxed);
        }

        result
    }

    /// Clear all events from the buffer without returning them.
    pub fn clear(&self) {
        // Clear semantic ring
        let mut ring = match self.semantic_ring.write() {
            Ok(ring) => ring,
            Err(_) => return,
        };

        let cleared = ring.len();
        ring.clear();
        drop(ring);

        if cleared > 0 {
            self.metrics.current_len.fetch_sub(cleared, Ordering::Relaxed);
        }

        // Clear raw ring
        if let Ok(mut raw_ring) = self.raw_ring.write() {
            raw_ring.clear();
        }
    }

    /// Get the current number of semantic events in the buffer.
    pub fn len(&self) -> usize {
        self.semantic_ring
            .read()
            .map(|e| e.len())
            .unwrap_or(0)
    }

    /// Get the current number of raw trace events in the buffer.
    pub fn raw_len(&self) -> usize {
        self.raw_ring
            .read()
            .map(|e| e.len())
            .unwrap_or(0)
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the maximum capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get a reference to the bus metrics.
    pub fn metrics(&self) -> &BusMetrics {
        &self.metrics
    }
}

/// An `Arc<EventBus>` handle for sharing across threads.
pub type EventBusHandle = Arc<EventBus>;

/// Create a new shared event bus with default capacity.
pub fn shared_bus(capacity: usize) -> EventBusHandle {
    Arc::new(EventBus::new(capacity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{SemanticEvent, SemanticEventKind};
    use crate::Language;

    fn make_test_semantic_event(id: u64) -> SemanticEvent {
        SemanticEvent {
            source_event_id: id,
            timestamp_ns: id * 1000,
            thread_id: 1,
            language: Language::C,
            kind: SemanticEventKind::Unresolved,
            description: format!("event_{}", id),
        }
    }

    #[test]
    fn test_event_bus_push_and_snapshot() {
        let bus = EventBus::new(10);
        assert!(bus.is_empty());

        bus.push(make_test_semantic_event(1));
        bus.push(make_test_semantic_event(2));
        assert_eq!(bus.len(), 2);

        let events = bus.snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].source_event_id, 1);
        assert_eq!(events[1].source_event_id, 2);

        assert!(bus.is_empty());
    }

    #[test]
    fn test_event_bus_clear() {
        let bus = EventBus::new(10);
        bus.push(make_test_semantic_event(1));
        bus.push(make_test_semantic_event(2));
        assert_eq!(bus.len(), 2);

        bus.clear();
        assert!(bus.is_empty());
    }

    #[test]
    fn test_event_bus_drain() {
        let bus = EventBus::new(10);
        for i in 1..=5 {
            bus.push(make_test_semantic_event(i));
        }

        let events = bus.drain(2);
        assert_eq!(events.len(), 2);
        assert_eq!(bus.len(), 3);
    }

    #[test]
    fn test_event_bus_overflow() {
        let bus = EventBus::new(3);
        bus.push(make_test_semantic_event(1));
        bus.push(make_test_semantic_event(2));
        bus.push(make_test_semantic_event(3));
        assert_eq!(bus.len(), 3);

        // Adding one more should evict the oldest
        bus.push(make_test_semantic_event(4));
        assert_eq!(bus.len(), 3);

        let events = bus.snapshot();
        assert_eq!(events.len(), 3);
        // Oldest (event 1) should be evicted
        assert_eq!(events[0].source_event_id, 2);
        assert_eq!(events[1].source_event_id, 3);
        assert_eq!(events[2].source_event_id, 4);
    }

    #[test]
    fn test_event_bus_metrics() {
        let bus = EventBus::new(10);
        bus.push(make_test_semantic_event(1));
        bus.push(make_test_semantic_event(2));

        let metrics = bus.metrics();
        assert_eq!(metrics.total_pushed.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.current_len.load(Ordering::Relaxed), 2);

        bus.snapshot();
        let metrics = bus.metrics();
        assert_eq!(metrics.current_len.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_event_bus_shared() {
        let bus = Arc::new(EventBus::new(10));
        let bus_clone = bus.clone();

        bus.push(make_test_semantic_event(1));
        assert_eq!(bus_clone.len(), 1);

        let events = bus_clone.snapshot();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_event_bus_many() {
        let bus = EventBus::new(100);
        let events: Vec<_> = (1..=50).map(make_test_semantic_event).collect();
        bus.push_many(events);

        assert_eq!(bus.len(), 50);
    }

    #[test]
    fn test_event_bus_is_empty() {
        let bus = EventBus::new(10);
        assert!(bus.is_empty());

        bus.push(make_test_semantic_event(1));
        assert!(!bus.is_empty());

        bus.snapshot();
        assert!(bus.is_empty());
    }

    #[test]
    fn test_event_bus_high_water_mark() {
        let bus = EventBus::new(5);
        for i in 1..=3 {
            bus.push(make_test_semantic_event(i));
        }
        assert_eq!(bus.metrics().high_water_mark.load(Ordering::Relaxed), 3);

        // Drain and add fewer
        bus.drain(2);
        bus.push(make_test_semantic_event(10));
        assert_eq!(bus.metrics().high_water_mark.load(Ordering::Relaxed), 3); // Should still be 3
    }

    #[test]
    fn test_event_bus_raw_buffer() {
        let bus = EventBus::new(10);
        // Push raw events directly (simulating before semantic resolution)
        bus.push_raw(crate::TraceEvent::new(
            1,
            1000,
            1,
            crate::EventType::FunctionEntry,
            crate::SourceLocation::default(),
            crate::EventData::Empty,
        ));
        bus.push_raw(crate::TraceEvent::new(
            2,
            2000,
            1,
            crate::EventType::FunctionExit,
            crate::SourceLocation::default(),
            crate::EventData::Empty,
        ));

        assert_eq!(bus.raw_len(), 2);
        assert_eq!(bus.len(), 0); // Semantic buffer is empty

        // Snapshot raw should return the raw events
        let raw_events = bus.snapshot_raw();
        assert_eq!(raw_events.len(), 2);
        assert_eq!(bus.raw_len(), 0);
    }
}