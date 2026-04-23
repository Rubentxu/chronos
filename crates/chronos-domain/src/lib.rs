//! Chronos Domain — Core types for time-travel debugging.
//!
//! This crate contains all domain types, traits, and errors used across
//! the Chronos MCP server. It has zero external I/O dependencies.

pub mod adapter;
pub mod bus;
pub mod error;
pub mod index;
pub mod query;
pub mod semantic;
pub mod trace;
pub mod tripwire;
pub mod value;

// Re-exports for convenience
pub use adapter::ProbeBackend;
pub use bus::{BusMetrics, EventBus, EventBusHandle};
pub use error::TraceError;
pub use index::{
    CausalityEntry, CausalityIndex, CompressedTrace, CompressionLevel, DetailData,
    ExecutiveSummary, FunctionDetail, FunctionPerf, HotspotData, HotspotEntry, MicroscopyData,
    PerfCounters, PerformanceIndex, RawEventEntry, ShadowIndex, TemporalIndex,
};
pub use query::{
    EventFilter, PerfEntry, PerfQuery, PerfResult, PerfSortBy, QueryResult, TraceQuery,
};
pub use trace::{
    CaptureConfig, CaptureSession, EventData, EventType, GoEventKind, JavaEventKind, JsEventKind,
    Language, PythonEventKind, RegisterState, RuntimeInfo, SessionState, SourceLocation,
    StackFrame, ThreadInfo, ThreadState, TraceEvent,
};
pub use value::{TypedValue, VariableInfo, VariableScope};
