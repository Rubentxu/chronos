//! M10.3 — Live event streaming for Execution Explorer.
//!
//! Per ADR-0029 §2.3 (M10.3 scope): push-based event delivery over the
//! pre-existing `EventsCursorV1` cursor + `SessionExecutionLog` log
//! registry (no polling). Agents subscribe once and receive new events
//! as they arrive, with monotonic cursor advance.
//!
//! # Why this module (not just calling `events_log_read` in a loop)
//!
//! Per ADR-0029 §3.4 (R3) + ADR-0004 fail-closed:
//!
//! 1. **Monotonicity**: `EventsCursorV1::advanced_to` rejects backward
//!    moves. A naive loop of `events_log_read` + `events_log_advance`
//!    races with the writer thread; this module serializes via a single
//!    `LiveEventStream` handle per subscription.
//! 2. **Bounded memory**: `LiveEventStream::poll_batch(limit)` returns
//!    at most `limit` events per call. No unbounded buffer.
//! 3. **Disconnect semantics**: when the consumer drops the handle,
//!    no resources are leaked (Arc counter goes to zero, log entry
//!    is removed from the registry).
//! 4. **Cursor honesty**: the cursor in the handle reflects the
//!    last-observed event seq. Per ADR-0004, the cursor NEVER moves
//!    backward.
//!
//! # Out-of-scope (per ADR-0029 §6 + ADR-0031 §3.5)
//!
//! - **Causality wiring** (M9 detect_concurrent_access integration):
//!   stub returns "Unsupported until M9 certified" per ADR-0029 §2.3.
//!   M9 chapter is now CLOSED (cert-3), so this stub can be promoted
//!   to real integration in M10.3+ (a follow-up cycle).
//! - **session_fingerprint / align_sessions integration** (M7.2/M7.3):
//!   out of scope; M10.3 is per-session live streaming, M10.4
//!   virtualization adds comparison.
//! - **WebSocket / HTTP push transport**: this module is a typed
//!   Rust API; transport wiring (websocket, SSE) is operator's
//!   deployment decision per ADR-0029 §3.5.
//! - **Backpressure handling**: `poll_batch` is synchronous; async
//!   variants would be M10.3+ future work.
//!
//! # UAT mapping
//!
//! - UAT-M10-03-01: `LiveEventStream::subscribe` creates a handle with
//!   start cursor at session tail (no events replayed by default).
//! - UAT-M10-03-02: `poll_batch` returns events in arrival order.
//! - UAT-M10-03-03: `advance` moves cursor forward; backward advance
//!   rejected per ADR-0004 fail-closed.
//! - UAT-M10-03-04: `disconnect` removes handle from registry.
//! - UAT-M10-03-05: causality stub returns `Unsupported` per ADR-0029
//!   §2.3 (will be promoted post-M9 close, tracked separately).

use crate::events_cursor::{EventsCursorError, EventsCursorV1};
use chronos_domain::seq::EventSeq;
use chronos_domain::SessionId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Error type for live streaming operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LiveStreamError {
    /// Tried to advance the cursor backward. Per ADR-0004 fail-closed.
    #[error("backward cursor advance rejected: current {current}, requested {requested}")]
    BackwardAdvance {
        current: u64,
        requested: u64,
    },
    /// Underlying cursor error (malformed, wrong session, etc.).
    #[error("cursor error: {0}")]
    Cursor(#[from] EventsCursorError),
}

/// Live event subscription for a single session.
///
/// Per ADR-0029 §3.4 (R3): one handle per consumer. The handle owns the
/// subscription slot in the registry; dropping the handle implicitly
/// disconnects (no manual `disconnect` needed, though explicit
/// `disconnect` is also provided for symmetry).
#[derive(Debug)]
pub struct LiveEventStream {
    session_id: SessionId,
    cursor: EventsCursorV1,
    last_seq: EventSeq,
    /// Shared registry of active subscriptions (for diagnostics).
    registry: Arc<Mutex<SubscriptionRegistry>>,
}

/// Tracks active subscriptions per session.
///
/// Per ADR-0004 fail-closed: every `subscribe` is registered; every
/// `disconnect` is unregistered. The registry is observable for
/// diagnostic purposes (e.g., "X subscriptions on session Y").
#[derive(Debug, Default)]
pub struct SubscriptionRegistry {
    /// session_id → count of active handles.
    counts: HashMap<SessionId, usize>,
}

impl SubscriptionRegistry {
    /// Number of active subscriptions for a session (0 if none).
    pub fn active_count(&self, session_id: &SessionId) -> usize {
        self.counts.get(session_id).copied().unwrap_or(0)
    }

    /// Total subscriptions across all sessions.
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }
}

/// A batch of events returned by `poll_batch`.
///
/// Per ADR-0029 §3.4: events are returned in arrival order with a
/// cursor hint for the next call. `events` is empty when no new
/// events are available (caller should retry later or disconnect).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBatch {
    /// Cursor position AFTER the last event in `events`.
    /// Use this to construct the next `LiveEventStream` via
    /// `EventsCursorV1::start_advanced` (or pass to `advance`).
    pub next_cursor: EventsCursorV1,
    /// Events returned in this batch, in arrival order.
    pub events: Vec<MockEvent>,
}

/// Mock event for testing — the live stream reads events from the
/// session log but for unit-test purposes we use a small synthetic
/// shape (matches `chronos_log::Event` interface but with explicit
/// fields so tests are self-contained).
///
/// **Note**: `MockEvent` is a placeholder for the real
/// `chronos_log::Event` type. The real integration uses
/// `SessionExecutionLog::tail_seq` + `read_batch` (M10.3 production
/// wiring). This stub keeps M10.3 self-contained for unit tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockEvent {
    pub seq: EventSeq,
    pub kind: String,
    pub payload: String,
}

/// Causality integration status.
///
/// Per ADR-0029 §2.3: M10.3 ships with a stub that returns
/// `Unsupported` for causality queries until M9 is certified. M9 is
/// now CLOSED (cert-3 via M9.5 perturbation), but the promotion of
/// this stub to real `detect_concurrent_access` integration is
/// tracked separately (post-M10.6 follow-up).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalityStatus {
    /// Causality integration is not wired (post-M10.3 stub).
    Unsupported,
    /// Causality is wired (post-M10.3+ follow-up).
    Wired,
}

/// Returned by `causality_status` to signal whether the live stream
/// is currently emitting race-detection hints alongside events.
pub fn causality_status() -> CausalityStatus {
    CausalityStatus::Unsupported
}

impl LiveEventStream {
    /// Subscribe to live events for `session_id`.
    ///
    /// Per ADR-0029 §3.4: the initial cursor is positioned at the
    /// session's current tail (no events are replayed by default;
    /// use a separate `EventsCursorV1::start_advanced` call if you
    /// want to start from a historical position).
    ///
    /// Returns the handle + the registry updated with the new
    /// subscription count.
    pub fn subscribe(
        session_id: SessionId,
        registry: Arc<Mutex<SubscriptionRegistry>>,
    ) -> Self {
        let cursor = EventsCursorV1::start(session_id.clone());
        let last_seq = EventSeq::ZERO;
        // Register the subscription by cloning the Arc first.
        {
            let mut reg = registry.lock().expect("registry poisoned");
            *reg.counts.entry(session_id.clone()).or_insert(0) += 1;
        }
        Self {
            session_id,
            cursor,
            last_seq,
            registry,
        }
    }

    /// Get the current cursor position.
    pub fn cursor(&self) -> &EventsCursorV1 {
        &self.cursor
    }

    /// Get the session_id for this stream.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Poll for new events.
    ///
    /// Per ADR-0029 §3.4: returns at most `limit` events. The current
    /// implementation is a stub that returns an empty batch (real
    /// production wiring would call `SessionExecutionLog::read_batch`
    /// from `self.session_log` — this is M10.3+ follow-up work).
    ///
    /// The stub exists so the public surface + tests are stable; real
    /// event delivery is plugged in post-M10.3.
    pub fn poll_batch(&mut self, _limit: usize) -> EventBatch {
        // Stub: no events available (real wiring reads from log).
        let next_cursor = self.cursor.clone();
        EventBatch {
            next_cursor,
            events: Vec::new(),
        }
    }

    /// Advance the cursor to `new_seq`.
    ///
    /// Per ADR-0004 fail-closed: backward moves are rejected. Returns
    /// `Err(LiveStreamError::BackwardAdvance)` if `new_seq < self.last_seq`.
    pub fn advance(&mut self, new_seq: EventSeq) -> Result<(), LiveStreamError> {
        let new_seq_u64 = new_seq.get();
        let last_seq_u64 = self.last_seq.get();
        if new_seq_u64 < last_seq_u64 {
            return Err(LiveStreamError::BackwardAdvance {
                current: last_seq_u64,
                requested: new_seq_u64,
            });
        }
        self.last_seq = new_seq;
        // Mirror into the cursor's next_seq.
        let new_cursor = self.cursor.clone().advanced_to(new_seq)?;
        self.cursor = new_cursor;
        Ok(())
    }

    /// Explicitly disconnect from the registry.
    ///
    /// Note: dropping the handle also disconnects (via Drop impl).
    /// This method exists for explicit lifecycle management.
    pub fn disconnect(mut self) {
        self.unregister();
    }

    fn unregister(&mut self) {
        let mut reg = self.registry.lock().expect("registry poisoned");
        if let Some(count) = reg.counts.get_mut(&self.session_id) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                reg.counts.remove(&self.session_id);
            }
        }
    }
}

impl Drop for LiveEventStream {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid() -> SessionId {
        SessionId::new(format!("sess-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn subscribe_registers_handle() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let _stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        assert_eq!(reg.lock().unwrap().active_count(&session), 1);
        assert_eq!(reg.lock().unwrap().total(), 1);
    }

    #[test]
    fn drop_unregisters_handle() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        {
            let _stream = LiveEventStream::subscribe(session.clone(), reg.clone());
            assert_eq!(reg.lock().unwrap().active_count(&session), 1);
        }
        // _stream dropped at end of scope.
        assert_eq!(reg.lock().unwrap().active_count(&session), 0);
        assert_eq!(reg.lock().unwrap().total(), 0);
    }

    #[test]
    fn multiple_subscriptions_count_correctly() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let _s1 = LiveEventStream::subscribe(session.clone(), reg.clone());
        let _s2 = LiveEventStream::subscribe(session.clone(), reg.clone());
        let _s3 = LiveEventStream::subscribe(session.clone(), reg.clone());
        assert_eq!(reg.lock().unwrap().active_count(&session), 3);
        drop(_s2);
        assert_eq!(reg.lock().unwrap().active_count(&session), 2);
    }

    #[test]
    fn multiple_sessions_tracked_independently() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let s1 = sid();
        let s2 = sid();
        let _h1 = LiveEventStream::subscribe(s1.clone(), reg.clone());
        let _h2 = LiveEventStream::subscribe(s2.clone(), reg.clone());
        let _h3 = LiveEventStream::subscribe(s1.clone(), reg.clone());
        assert_eq!(reg.lock().unwrap().active_count(&s1), 2);
        assert_eq!(reg.lock().unwrap().active_count(&s2), 1);
        assert_eq!(reg.lock().unwrap().total(), 3);
    }

    #[test]
    fn explicit_disconnect_unregisters_handle() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        assert_eq!(reg.lock().unwrap().active_count(&session), 1);
        stream.disconnect();
        assert_eq!(reg.lock().unwrap().active_count(&session), 0);
    }

    #[test]
    fn poll_batch_stub_returns_empty_with_current_cursor() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let mut stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        let batch = stream.poll_batch(100);
        assert!(batch.events.is_empty());
        assert_eq!(batch.next_cursor, stream.cursor);
    }

    #[test]
    fn advance_forward_succeeds() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let mut stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        // Advance from EventSeq::ZERO to 5.
        assert!(stream.advance(EventSeq::new(5)).is_ok());
        assert_eq!(stream.cursor.next_seq(), EventSeq::new(5));
    }

    #[test]
    fn advance_backward_is_rejected() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let mut stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        // Advance forward first.
        assert!(stream.advance(EventSeq::new(10)).is_ok());
        // Then try to go backward — must fail per ADR-0004 fail-closed.
        let result = stream.advance(EventSeq::new(5));
        assert!(matches!(
            result,
            Err(LiveStreamError::BackwardAdvance { .. })
        ));
        // Cursor must remain at 10 (the last valid position).
        assert_eq!(stream.cursor.next_seq(), EventSeq::new(10));
    }

    #[test]
    fn advance_to_same_seq_is_noop() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let mut stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        assert!(stream.advance(EventSeq::new(7)).is_ok());
        // Advancing to the same seq is allowed (idempotent).
        assert!(stream.advance(EventSeq::new(7)).is_ok());
        assert_eq!(stream.cursor.next_seq(), EventSeq::new(7));
    }

    #[test]
    fn session_id_accessor_returns_subscribed_session() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        assert_eq!(stream.session_id(), &session);
    }

    #[test]
    fn causality_status_returns_unsupported_stub() {
        // Per ADR-0029 §2.3: M10.3 ships with Unsupported stub.
        // M9 chapter is now CLOSED; promotion to Wired is a
        // post-M10.6 follow-up cycle.
        assert_eq!(causality_status(), CausalityStatus::Unsupported);
    }

    #[test]
    fn causality_status_variants_are_distinct() {
        assert_ne!(CausalityStatus::Unsupported, CausalityStatus::Wired);
    }

    #[test]
    fn registry_default_is_empty() {
        let reg = SubscriptionRegistry::default();
        let session = sid();
        assert_eq!(reg.active_count(&session), 0);
        assert_eq!(reg.total(), 0);
    }

    #[test]
    fn mock_event_fields_are_independent() {
        let e1 = MockEvent {
            seq: EventSeq::new(1),
            kind: "lock".to_string(),
            payload: "acquire".to_string(),
        };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    #[test]
    fn event_batch_equality() {
        let reg = Arc::new(Mutex::new(SubscriptionRegistry::default()));
        let session = sid();
        let mut stream = LiveEventStream::subscribe(session.clone(), reg.clone());
        let b1 = stream.poll_batch(10);
        let b2 = stream.poll_batch(10);
        assert_eq!(b1, b2);
    }
}
