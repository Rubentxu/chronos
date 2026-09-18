//! Ports — abstract contracts that the domain exposes for driven adapters.
//!
//! A *port* is an inversion-of-control interface declared by the inner
//! layers (domain / application) and implemented by driven adapters
//! living in infrastructure crates. The domain declares the *intent*
//! (what must be delivered), never the transport (HTTP, gRPC, queue,
//! file, etc.).
//!
//! The trait methods here are intentionally **synchronous**. Driven
//! adapters may use async runtimes internally; the composition root
//! bridges sync → async via runtime handles or `spawn_blocking`.
//!
//! See `REC-C3.1` (exploration/spec/design/tasks) for the port list
//! and the rationale behind each one.

mod execution_log;
mod notification;
mod probe;
mod session;
mod telemetry;

pub use execution_log::{ExecutionLogError, ExecutionLogPage, ExecutionLogProvider};
pub use notification::{
    NotificationDeliveryError, NotificationRequest, NotificationSink, NotificationTarget,
    NullNotificationSink,
};
pub use probe::{
    NullProbeFactory, NullProbeRegistry, ProbeController, ProbeFactory, ProbeRegistry,
};
pub use session::{InMemorySessionRepository, SessionHandle, SessionRepository, SessionState};
pub use telemetry::{
    Counters, InMemoryTelemetry, Metric, NoopTelemetry, TelemetryError, TelemetryReceiver,
};
