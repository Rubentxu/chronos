//! Service-level error types for chronos-services.

use thiserror::Error;

/// Errors that can occur when calling a service operation.
#[derive(Debug, Error)]
pub enum ServiceError {
    /// The requested session does not exist.
    #[error("session '{0}' not found")]
    SessionNotFound(String),

    /// A shared mutex was poisoned by a panicking thread.
    #[error("lock poisoned")]
    LockPoisoned,

    /// No memory write event was found at the given address before the timestamp.
    #[error("no memory at address 0x{address:x} before timestamp {timestamp_ns}")]
    MemoryNotFound { address: u64, timestamp_ns: u64 },

    /// The requested trace event does not exist.
    #[error("event {event_id} not found")]
    EventNotFound { event_id: u64 },

    /// No register state is available at the given event.
    #[error("no register state at event {event_id}")]
    NoRegisterState { event_id: u64 },

    /// Arithmetic expression evaluation failed.
    #[error("evaluation error: {0}")]
    EvalError(String),

    /// Session not found in memory (engines map).
    #[error("session '{0}' not found in memory")]
    SessionNotInMemory(String),

    /// Session has no events to save.
    #[error("session '{0}' has no events to save")]
    EmptySession(String),

    /// SessionStore::save_session failed.
    #[error("save failed: {0}")]
    SaveFailed(String),

    /// SessionStore::load_session failed.
    #[error("load failed: {0}")]
    LoadFailed(String),

    /// SessionStore::list_sessions failed.
    #[error("list failed: {0}")]
    ListFailed(String),

    /// SessionStore::delete_session failed.
    #[error("delete failed: {0}")]
    DeleteFailed(String),

    /// The event_type string could not be parsed into a known [`TripwireCondition`].
    #[error("unknown event_type '{0}'")]
    InvalidCondition(String),

    /// The tripwire ID string did not match the expected "tripwire-<number>" format.
    #[error("invalid tripwire ID format '{0}'")]
    InvalidTripwireIdFormat(String),

    /// The named tripwire does not exist (remove returned false).
    #[error("tripwire '{0}' not found")]
    TripwireNotFound(String),

    /// A trace query could not be executed.
    #[error("query execution error: {0}")]
    QueryExecutionError(String),

    // --- Probe service variants -----------------------------------------------
    /// Invalid program path supplied to a probe tool (empty, not a file, etc.).
    #[error("invalid program path: {0}")]
    InvalidProgramPath(String),

    /// The live probe session id is not registered in the live-probe map.
    #[error("probe not found: {0}")]
    ProbeNotFound(String),

    /// A `probe_start` call failed to start the backend.
    #[error("probe start failed: {0}")]
    ProbeStartFailed(String),

    /// A `probe_stop` call failed to drain / detach.
    #[error("probe stop error: {0}")]
    ProbeStopError(String),

    /// A cursor payload could not be base64-decoded / parsed.
    #[error("invalid cursor payload")]
    InvalidCursorPayload,

    /// The cursor was decoded but its total_pushed is older than the live bus.
    #[error("cursor stale")]
    CursorStale,

    /// A non-destructive drain encountered a backend error.
    #[error("drain failed: {0}")]
    DrainFailed(String),

    /// The probe is registered but the underlying backend has not yet emitted
    /// any events (start-up race).
    #[error("probe still starting up")]
    ProbeStarting,

    /// eBPF is not supported on this host (kernel / permissions / missing probes).
    #[error("eBPF unsupported: {0}")]
    EbpfUnsupported(String),

    /// eBPF uprobe injection failed.
    #[error("injection failed: {0}")]
    InjectionFailed(String),
}
