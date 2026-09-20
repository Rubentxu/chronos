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

pub mod browser_probe;
pub mod counterexample;
pub mod diff;
pub mod execution_log;
pub mod execution_log_factory;
pub mod execution_log_maintenance;
pub mod execution_log_retention;
pub mod lifecycle_store;
pub mod notification;
mod probe;
pub mod session;
pub mod session_reader;
mod telemetry;
pub mod uprobe;

pub use browser_probe::{BrowserError, BrowserProbeBackend, BrowserProbeFactory};
pub use counterexample::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    CounterexampleRepository, CounterexampleRepositoryError, InMemoryCounterexampleRepository,
};
pub use diff::{DiffEngine, DiffReport, TimingDelta};
pub use execution_log::{
    ExecutionLogError, ExecutionLogKind, ExecutionLogPage, ExecutionLogProvider,
};
pub use execution_log_factory::ExecutionLogFactory;
pub use execution_log_maintenance::{
    CompactionMetrics, CompactionReport, ExecutionLogMaintenance, ExecutionLogMaintenanceError,
};
pub use execution_log_retention::{ExecutionLogRetention, RetentionError, RetentionOutcome};
pub use lifecycle_store::{InMemoryLifecycleStore, LifecycleStore, LifecycleStoreError};
pub use notification::{
    NotificationDeliveryError, NotificationRequest, NotificationSink, NotificationTarget,
    NullNotificationSink,
};
pub use probe::{
    AdvanceOutcome, NativeProbeBuildError, NativeProbeController, NativeProbeControllerFactory,
    NullProbeFactory, NullProbeRegistry, ProbeController, ProbeFactory, ProbeRegistry,
    RawAcceptedObserver, StepOutcome,
};
pub use session::{
    InMemorySessionArchive, InMemorySessionRepository, SessionArchive, SessionArchiveError,
    SessionHandle, SessionRepository, SessionState,
};
pub use session_reader::{InMemorySessionReader, SessionReader, SessionReaderError};
pub use telemetry::{
    Counters, InMemoryTelemetry, Metric, NoopTelemetry, TelemetryError, TelemetryReceiver,
};
pub use uprobe::{UprobeAttachError, UprobeHandle, UprobeInjector};
