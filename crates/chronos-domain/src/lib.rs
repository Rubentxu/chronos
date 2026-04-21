//! Chronos Domain — Core types for time-travel debugging.
//!
//! This crate contains all domain types, traits, and errors used across
//! the Chronos MCP server. It has zero external I/O dependencies.

pub mod adapter;
pub mod error;
pub mod index;
pub mod query;
pub mod trace;
pub mod value;

// Re-exports for convenience
pub use adapter::TraceAdapter;
pub use error::TraceError;
pub use index::{
    CausalityEntry, CausalityIndex,
    CompressionLevel, CompressedTrace, DetailData, ExecutiveSummary, FunctionDetail,
    HotspotData, HotspotEntry, MicroscopyData, RawEventEntry,
    FunctionPerf, PerfCounters, PerformanceIndex,
    ShadowIndex, TemporalIndex,
};
pub use query::{EventFilter, PerfEntry, PerfQuery, PerfResult, PerfSortBy, QueryResult, TraceQuery};
pub use trace::{
    CaptureConfig, CaptureSession, EventData, EventType, GoEventKind, JavaEventKind, Language,
    PythonEventKind, RegisterState, SessionState, SourceLocation, TraceEvent,
};
pub use value::{TypedValue, VariableInfo, VariableScope};
