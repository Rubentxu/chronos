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
}
