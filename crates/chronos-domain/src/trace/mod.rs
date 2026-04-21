//! Trace event types and capture session management.

mod event;
mod location;
mod session;

pub use event::{
    EventData, EventId, EventType, GoEventKind, JavaEventKind, PythonEventKind, RegisterState,
    ThreadId, TimestampNs, TraceEvent,
};
pub use location::SourceLocation;
pub use session::{CaptureConfig, CaptureSession, Language, SessionState};
