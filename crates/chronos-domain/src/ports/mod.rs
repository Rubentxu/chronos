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

pub mod execution_log;
pub mod execution_log_factory;
pub mod notification;
mod probe;
mod session;
mod telemetry;
pub mod browser_probe;
pub mod uprobe;

pub use execution_log::{ExecutionLogError, ExecutionLogPage, ExecutionLogProvider};
pub use execution_log_factory::ExecutionLogFactory;
pub use browser_probe::{BrowserError, BrowserProbeBackend, BrowserProbeFactory};
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
pub use uprobe::{UprobeAttachError, UprobeHandle, UprobeInjector};
