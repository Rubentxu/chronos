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
}
