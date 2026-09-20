//! Chronos Domain — Core types for time-travel debugging.
//!
//! This crate contains all domain types, traits, and errors used across
//! the Chronos MCP server. It has zero external I/O dependencies.

pub mod adapter;
pub mod capability;
pub mod causal_slice;
pub mod error;
pub mod evidence;
pub mod index;
pub mod ports;
pub mod property;
pub mod query;
pub mod semantic;
pub mod seq;
pub mod session;
pub mod session_id;
pub mod subscription_id;
pub mod trace;
pub mod tripwire;
pub mod value;

// Re-exports for convenience
pub use adapter::ProbeBackend;
pub use capability::{Capability, CapabilityUnavailable};
pub use causal_slice::{slice_from, CausalEdge, CausalSlice, EvidenceNode, EvidenceNodeId};
pub use error::TraceError;
pub use evidence::{
    ExecutionKind, ExecutionPayload, ExecutionRecord, Gap, GapReason, NewExecutionRecord,
    SealedTail, TailState, TripwireFiredEvidence, TRIPWIRE_FIRED_EVIDENCE_TAG,
};
pub use index::{
    CausalityEntry, CausalityIndex, CompressedTrace, CompressionLevel, DetailData,
    ExecutiveSummary, FunctionDetail, FunctionPerf, HotspotData, HotspotEntry, MicroscopyData,
    PerfCounters, PerformanceIndex, RawEventEntry, ShadowIndex, TemporalIndex,
};
pub use ports::{
    NotificationDeliveryError, NotificationRequest, NotificationSink, NotificationTarget,
    NullNotificationSink,
};
pub use property::{
    CallPathOutcome, ComparisonOp, ExistenceOutcome, InvariantCheck, InvariantOutcome,
    MutationActor, Property, PropertyExistencePredicate, PropertyHypothesisOutcome,
    PropertyHypothesisVerdict, PropertyId, PropertyObservationSource, PropertyOutcome,
    PropertySequenceOutcome, PropertyValue, PropertyViolation, StateTransition,
};
pub use query::{
    EventFilter, PerfEntry, PerfQuery, PerfResult, PerfSortBy, QueryResult, TraceQuery,
};
pub use semantic::{SemanticEvent, SemanticEventKind};
pub use seq::EventSeq;
pub use session::SessionMetadata;
pub use session_id::SessionId;
pub use subscription_id::SubscriptionId;
pub use trace::{
    CaptureConfig, CaptureSession, EventData, EventType, GoEventKind, InvocationId, JavaEventKind,
    JsEventKind, Language, MonotonicNs, PythonEventKind, RegisterState, RuntimeInfo, SessionState,
    SourceLocation, StackFrame, SymbolId, ThreadInfo, ThreadState, TimestampNs, TraceEvent,
    WallClockMs,
};
pub use tripwire::{Tripwire, TripwireCondition, TripwireId, TripwireManager, TripwireMatch};
pub use value::{DwarfValue, RegisterSnapshot, TypedValue, VariableInfo, VariableScope};
