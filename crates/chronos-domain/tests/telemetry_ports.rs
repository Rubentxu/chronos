//! Behavioral tests for the telemetry port (REC-C3.1, B7).
//!
//! Coverage:
//! - `NoopTelemetry` accepts every record and reports open.
//! - `InMemoryTelemetry` accumulates per-session counters across
//!   `record` calls and returns them via `drain`.
//! - Closing the receiver reports `TelemetryError::Closed` and
//!   `is_open = false`.
//! - Empty session id is rejected eagerly with `EmptySessionId`.

use chronos_domain::ports::{
    Counters, InMemoryTelemetry, Metric, NoopTelemetry, TelemetryError, TelemetryReceiver,
};

mod common;
use common::session_id;

#[test]
fn noop_accepts_all_records() {
    let t = NoopTelemetry::new();
    assert!(t.is_open());
    t.record("any", Metric::Captured).unwrap();
    t.record("any", Metric::Dropped).unwrap();
    let out = t.drain("any").unwrap();
    assert_eq!(out, Counters::default(), "noop must not retain state");
}

#[test]
fn in_memory_accumulates_per_session_and_drain() {
    let t = InMemoryTelemetry::new();
    let sid_a = session_id("alpha");
    let sid_b = session_id("bravo");

    t.record(sid_a.as_str(), Metric::Captured).unwrap();
    t.record(sid_a.as_str(), Metric::Captured).unwrap();
    t.record(sid_a.as_str(), Metric::Dropped).unwrap();
    t.record(sid_b.as_str(), Metric::SnapshotsPublished)
        .unwrap();

    assert_eq!(
        t.drain(sid_a.as_str()).unwrap(),
        Counters {
            captured: 2,
            dropped: 1,
            snapshots_published: 0,
        },
    );
    assert_eq!(
        t.drain(sid_b.as_str()).unwrap(),
        Counters {
            captured: 0,
            dropped: 0,
            snapshots_published: 1,
        },
    );
    // Second drain is zero.
    assert_eq!(t.drain(sid_a.as_str()).unwrap(), Counters::default());
}

#[test]
fn closing_rejects_subsequent_records() {
    let t = InMemoryTelemetry::new();
    let sid = session_id("charlie");
    t.close();
    assert!(!t.is_open());
    let err = t.record(sid.as_str(), Metric::Captured).unwrap_err();
    assert_eq!(err, TelemetryError::Closed);
}

#[test]
fn empty_session_id_is_rejected() {
    let t = InMemoryTelemetry::new();
    let err = t.record("", Metric::Captured).unwrap_err();
    assert_eq!(err, TelemetryError::EmptySessionId);
}

#[test]
fn noop_is_open_after_construction() {
    let t = NoopTelemetry::new();
    assert!(t.is_open());
}
