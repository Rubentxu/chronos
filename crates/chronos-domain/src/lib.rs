//! Chronos Domain — Core types for time-travel debugging.
//!
//! This crate contains all domain types, traits, and errors used across
//! the Chronos MCP server. It has zero external I/O dependencies.

pub mod error;
pub mod index;
pub mod query;
pub mod trace;
pub mod value;

// Re-exports for convenience
pub use error::TraceError;
pub use index::{ShadowIndex, TemporalIndex};
pub use query::{EventFilter, QueryResult, TraceQuery};
pub use trace::{
    CaptureConfig, CaptureSession, EventData, EventType, Language, RegisterState,
    SessionState, SourceLocation, TraceEvent,
};
pub use value::{TypedValue, VariableInfo, VariableScope};
