//! Trace event types and capture session management.

mod event;
mod inspect;
mod location;
mod session;

pub use event::{
    EventData, EventId, EventType, GoEventKind, InvocationId, JavaEventKind, JsEventKind,
    PythonEventKind, RegisterState, SymbolId, ThreadId, TimestampNs, TraceEvent, WasmEventKind,
    WasmFunctionInfo, WasmModuleInfo,
};
pub use inspect::{RuntimeInfo, StackFrame, ThreadInfo, ThreadState};
pub use location::SourceLocation;
pub use session::{CaptureConfig, CaptureSession, Language, SessionState};
