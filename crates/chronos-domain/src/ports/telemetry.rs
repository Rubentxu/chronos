//! `TelemetryReceiver` — domain-side port for telemetry ingestion.
//!
//! Telemetry in Chronos means fine-grained counters/metrics from
//! running probes (e.g. snapshots/second, captured-bytes, dropped
//! events). Services today log to stdout; REC-C3.1 makes the contract
//! first-class so a future adapter (Prometheus, OTLP) can be wired
//! without touching the call sites.
//!
//! The port is **synchronous**, fire-and-forget: `TelemetryReceiver::record`
//! returns `Ok(())` if the counter was accepted, and `Err(_)` if
//! the receiver is at capacity or shutting down. Callers (services)
//! are expected to **swallow** the error — telemetry must never break
//! the hot path of a capture session.
//!
//! Tests for this module live under
//! `crates/chronos-domain/tests/ports/telemetry.rs`.

use std::sync::Mutex;

/// Atomic counter view, monotonic. The receiver does not interpret
/// the metric name; the recording side (`record`) just dispatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Counters {
    pub captured: u64,
    pub dropped: u64,
    pub snapshots_published: u64,
}

impl Counters {
    /// Per-metric increment helper. Returns the new value (mostly for
    /// tests that want to assert without re-reading).
    pub fn bump(&mut self, metric: Metric) -> u64 {
        match metric {
            Metric::Captured => {
                self.captured += 1;
                self.captured
            }
            Metric::Dropped => {
                self.dropped += 1;
                self.dropped
            }
            Metric::SnapshotsPublished => {
                self.snapshots_published += 1;
                self.snapshots_published
            }
        }
    }
}

/// Tag identifying which metric is being recorded. Each tag carries a
/// single increment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Metric {
    Captured,
    Dropped,
    SnapshotsPublished,
}

/// Port — receive telemetry updates during a session.
///
/// Implementations may buffer, batch, or push synchronously to an
/// external system. The contract guarantees **at-most-once** delivery
/// from the caller's perspective (record returns after the metric is
/// either accepted or rejected).
pub trait TelemetryReceiver: Send + Sync {
    /// Record one metric tick for `session_id`.
    fn record(&self, session_id: &str, metric: Metric) -> Result<(), TelemetryError>;

    /// Drain counters for `session_id`. After this returns, future
    /// `record` calls start from zero.
    fn drain(&self, session_id: &str) -> Result<Counters, TelemetryError>;

    /// Check whether the receiver is still accepting records (used
    /// during shutdown to skip work).
    fn is_open(&self) -> bool;
}

/// Errors emitted by `record`. Call sites should generally ignore
/// these (telemetry must not break the hot path) and aggregate via
/// tracing instead. The error is exposed so test code can assert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryError {
    /// Receiver has been closed (`is_open` returned false at the time
    /// of the call).
    Closed,
    /// Session id was empty. This is a programming error in the
    /// caller; receivers may reject it eagerly.
    EmptySessionId,
}

// =====================================================================
// No-op implementation (AD-5)
// =====================================================================

/// Receiver that always accepts the record and discards it. Useful for
/// tests and for the composition root's "telemetry disabled" fallback.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTelemetry;

impl NoopTelemetry {
    pub fn new() -> Self {
        Self
    }
}

impl TelemetryReceiver for NoopTelemetry {
    fn record(&self, _session_id: &str, _metric: Metric) -> Result<(), TelemetryError> {
        Ok(())
    }

    fn drain(&self, _session_id: &str) -> Result<Counters, TelemetryError> {
        Ok(Counters::default())
    }

    fn is_open(&self) -> bool {
        true
    }
}

// =====================================================================
// In-memory implementation for tests that need to assert counters
// =====================================================================

/// In-memory receiver that records per-session counts. Used by the
/// behavioral tests in `tests/ports/telemetry.rs`. Not exported for
/// production — production adapters (Prometheus, OTLP) belong in
/// their own crates.
#[derive(Debug, Default)]
pub struct InMemoryTelemetry {
    inner: Mutex<InMemoryTelemetryState>,
}

#[derive(Debug, Default)]
struct InMemoryTelemetryState {
    open: bool,
    by_session: std::collections::HashMap<String, Counters>,
}

impl InMemoryTelemetry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(InMemoryTelemetryState {
                open: true,
                by_session: std::collections::HashMap::new(),
            }),
        }
    }

    pub fn close(&self) {
        let mut g = self.inner.lock().expect("InMemoryTelemetry mutex poisoned");
        g.open = false;
    }
}

impl TelemetryReceiver for InMemoryTelemetry {
    fn record(&self, session_id: &str, metric: Metric) -> Result<(), TelemetryError> {
        if session_id.is_empty() {
            return Err(TelemetryError::EmptySessionId);
        }
        let mut g = self.inner.lock().expect("InMemoryTelemetry mutex poisoned");
        if !g.open {
            return Err(TelemetryError::Closed);
        }
        g.by_session
            .entry(session_id.to_string())
            .or_default()
            .bump(metric);
        Ok(())
    }

    fn drain(&self, session_id: &str) -> Result<Counters, TelemetryError> {
        let mut g = self.inner.lock().expect("InMemoryTelemetry mutex poisoned");
        Ok(g.by_session.remove(session_id).unwrap_or_default())
    }

    fn is_open(&self) -> bool {
        let g = self.inner.lock().expect("InMemoryTelemetry mutex poisoned");
        g.open
    }
}
