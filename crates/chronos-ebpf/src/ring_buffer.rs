//! BPF ring buffer reader.
//!
//! Provides an async-friendly polling interface over an eBPF ring buffer.
//! When compiled without the `ebpf` feature all operations return
//! [`EbpfError::Unavailable`].

use crate::{types::EbpfEvent, EbpfError};
use chronos_domain::TraceEvent;

/// State of a ring buffer poll operation.
#[derive(Debug)]
pub enum PollResult {
    /// An event was available and returned.
    Event(EbpfEvent),
    /// No events available right now (non-blocking poll returned empty).
    Empty,
    /// The ring buffer was closed / program detached.
    Closed,
}

/// Async-friendly ring buffer reader.
///
/// In a real `ebpf` build this wraps `aya::maps::RingBuf`. Without the
/// feature it is a stub that always returns `EbpfError::Unavailable`.
pub struct BpfRingBuffer {
    /// Next event ID to assign when converting to TraceEvent.
    next_event_id: u64,
    #[cfg(feature = "ebpf")]
    inner: BpfRingBufferInner,
    #[cfg(not(feature = "ebpf"))]
    _phantom: std::marker::PhantomData<()>,
}

/// Inner ring buffer that wraps aya's RingBuf without Debug derive.
#[cfg(feature = "ebpf")]
struct BpfRingBufferInner {
    ring: aya::maps::RingBuf<aya::maps::MapData>,
}

#[cfg(feature = "ebpf")]
impl std::fmt::Debug for BpfRingBufferInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BpfRingBufferInner").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for BpfRingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BpfRingBuffer")
            .field("next_event_id", &self.next_event_id)
            .finish()
    }
}

impl BpfRingBuffer {
    /// Create a new ring buffer reader wrapping an aya `RingBuf` map.
    ///
    /// Returns `Err(EbpfError::Unavailable)` without the `ebpf` feature.
    #[cfg(feature = "ebpf")]
    pub fn new(ring_buf: aya::maps::RingBuf<aya::maps::MapData>) -> Self {
        Self {
            next_event_id: 0,
            inner: BpfRingBufferInner { ring: ring_buf },
        }
    }

    /// Stub constructor — always returns `Unavailable`.
    #[cfg(not(feature = "ebpf"))]
    pub fn unavailable() -> Result<Self, EbpfError> {
        Err(EbpfError::Unavailable {
            reason: "ebpf feature not enabled".to_string(),
        })
    }

    /// Poll for the next event (non-blocking).
    ///
    /// Returns:
    /// - `Ok(PollResult::Event(e))` — an event was ready.
    /// - `Ok(PollResult::Empty)` — ring buffer is empty.
    /// - `Err(EbpfError::Unavailable)` — feature not compiled in.
    pub fn poll(&mut self) -> Result<PollResult, EbpfError> {
        #[cfg(not(feature = "ebpf"))]
        {
            Err(EbpfError::Unavailable {
                reason: "ebpf feature not enabled".to_string(),
            })
        }

        #[cfg(feature = "ebpf")]
        {
            use std::ops::Deref;
            // aya's RingBuf implements Iterator over &[u8] items
            match self.inner.ring.next() {
                Some(item) => {
                    let bytes = item.deref();
                    if bytes.len() < std::mem::size_of::<EbpfEvent>() {
                        return Err(EbpfError::RingBuffer(format!(
                            "short read: {} bytes",
                            bytes.len()
                        )));
                    }
                    // SAFETY: EbpfEvent is repr(C), bytes come from the kernel
                    // and are aligned to the ring buffer page boundary.
                    let event = unsafe {
                        std::ptr::read_unaligned(bytes.as_ptr() as *const EbpfEvent)
                    };
                    Ok(PollResult::Event(event))
                }
                None => Ok(PollResult::Empty),
            }
        }
    }

    /// Convert a polled `EbpfEvent` into a `chronos_domain::TraceEvent`,
    /// assigning a monotonically increasing event ID.
    pub fn to_trace_event(&mut self, ev: EbpfEvent) -> TraceEvent {
        let id = self.next_event_id;
        self.next_event_id += 1;
        ev.to_trace_event(id)
    }

    /// Drain all available events, converting each to a `TraceEvent`.
    ///
    /// Returns an empty vec (not an error) without the `ebpf` feature.
    pub fn drain_events(&mut self) -> Vec<TraceEvent> {
        let mut result = Vec::new();
        while let Ok(PollResult::Event(ev)) = self.poll() {
            let te = self.to_trace_event(ev);
            result.push(te);
        }
        result
    }
}

/// A mock ring buffer for testing that holds pre-loaded events.
///
/// Available regardless of the `ebpf` feature for unit-testing consumers
/// without a real kernel.
pub struct MockRingBuffer {
    events: std::cell::RefCell<std::collections::VecDeque<EbpfEvent>>,
    next_event_id: std::cell::RefCell<u64>,
}

impl MockRingBuffer {
    /// Create a mock with a predefined list of events.
    pub fn new(events: Vec<EbpfEvent>) -> Self {
        Self {
            events: std::cell::RefCell::new(events.into()),
            next_event_id: std::cell::RefCell::new(0),
        }
    }

    /// Non-blocking poll — returns the next queued event or Empty.
    pub fn poll(&self) -> PollResult {
        match self.events.borrow_mut().pop_front() {
            Some(ev) => PollResult::Event(ev),
            None => PollResult::Empty,
        }
    }

    /// Pop and convert to TraceEvent.
    pub fn next_trace_event(&self) -> Option<TraceEvent> {
        let ev = self.events.borrow_mut().pop_front()?;
        let id = *self.next_event_id.borrow();
        *self.next_event_id.borrow_mut() += 1;
        Some(ev.to_trace_event(id))
    }

    /// Drain all queued events.
    pub fn drain_all(&self) -> Vec<TraceEvent> {
        let mut result = Vec::new();
        while let Some(te) = self.next_trace_event() {
            result.push(te);
        }
        result
    }

    /// Number of events still in the queue.
    pub fn pending(&self) -> usize {
        self.events.borrow().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EbpfEvent;
    use chronos_domain::EventType;

    fn make_entry_event(ts: u64, tid: u64, addr: u64, name: &str) -> EbpfEvent {
        EbpfEvent::function_entry(ts, tid, addr, name)
    }

    #[test]
    fn test_ring_buffer_poll_unavailable_without_feature() {
        #[cfg(not(feature = "ebpf"))]
        {
            // BpfRingBuffer::unavailable() returns an error
            let result = BpfRingBuffer::unavailable();
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                EbpfError::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn test_mock_ring_buffer_poll_returns_events() {
        let events = vec![
            make_entry_event(100, 1, 0x1000, "main"),
            make_entry_event(200, 1, 0x2000, "helper"),
        ];
        let mut buf = MockRingBuffer::new(events);

        assert_eq!(buf.pending(), 2);

        match buf.poll() {
            PollResult::Event(ev) => assert_eq!(ev.get_function_name(), "main"),
            _ => panic!("expected Event"),
        }
        assert_eq!(buf.pending(), 1);

        match buf.poll() {
            PollResult::Event(ev) => assert_eq!(ev.get_function_name(), "helper"),
            _ => panic!("expected Event"),
        }
        assert_eq!(buf.pending(), 0);

        // Now empty
        matches!(buf.poll(), PollResult::Empty);
    }

    #[test]
    fn test_mock_ring_buffer_drain_all() {
        let events = (0..5).map(|i| make_entry_event(i * 100, 1, 0x1000 + i, "fn")).collect();
        let mut buf = MockRingBuffer::new(events);

        let trace_events = buf.drain_all();
        assert_eq!(trace_events.len(), 5);
        assert_eq!(buf.pending(), 0);

        for (i, te) in trace_events.iter().enumerate() {
            assert_eq!(te.event_id, i as u64);
            assert_eq!(te.event_type, EventType::FunctionEntry);
        }
    }

    #[test]
    fn test_mock_ring_buffer_next_trace_event_converts_correctly() {
        let ev = make_entry_event(999, 7, 0xABCD, "my_func");
        let mut buf = MockRingBuffer::new(vec![ev]);

        let te = buf.next_trace_event().unwrap();
        assert_eq!(te.timestamp_ns, 999);
        assert_eq!(te.thread_id, 7);
        assert_eq!(te.location.address, 0xABCD);
        assert_eq!(te.event_type, EventType::FunctionEntry);

        // Queue is now empty
        assert!(buf.next_trace_event().is_none());
    }

    #[test]
    fn test_mock_ring_buffer_empty_from_start() {
        let mut buf = MockRingBuffer::new(vec![]);
        assert_eq!(buf.pending(), 0);
        assert!(buf.next_trace_event().is_none());
        assert_eq!(buf.drain_all().len(), 0);
    }
}
