//! Error types for Chronos.

use std::path::PathBuf;

/// Errors that can occur across the Chronos system.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    // --- Process errors ---
    #[error("Process not found: PID {0}")]
    ProcessNotFound(u32),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    // --- Session errors ---
    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: String },

    #[error("Invalid session state: expected {expected}, got {actual}")]
    InvalidSessionState {
        session_id: String,
        expected: String,
        actual: String,
    },

    // --- Capture errors ---
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Capture failed: {0}")]
    CaptureFailed(String),

    #[error("Capture timed out after {0}ms")]
    CaptureTimeout(u64),

    #[error("Target process crashed: {0}")]
    TargetCrashed(String),

    // --- Trace file errors ---
    #[error("Trace file not found: {0}")]
    TraceFileNotFound(PathBuf),

    #[error("Trace file corrupted: {0}")]
    TraceFileCorrupted(String),

    #[error("Trace file I/O error: {0}")]
    TraceFileIO(#[from] std::io::Error),

    // --- Query errors ---
    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("No events found matching query")]
    NoEventsFound,

    #[error("Index error: {0}")]
    IndexError(String),

    // --- Expression errors ---
    #[error("Invalid expression: {0}")]
    InvalidExpression(String),

    // --- Internal errors ---
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl TraceError {
    /// Create a session not found error.
    pub fn session_not_found(session_id: impl Into<String>) -> Self {
        Self::SessionNotFound {
            session_id: session_id.into(),
        }
    }

    /// Create a capture failed error from any error.
    pub fn capture_failed(err: impl std::fmt::Display) -> Self {
        Self::CaptureFailed(err.to_string())
    }
}

/// Result type alias for Chronos operations.
pub type Result<T> = std::result::Result<T, TraceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TraceError::ProcessNotFound(12345);
        assert_eq!(err.to_string(), "Process not found: PID 12345");

        let err = TraceError::SessionNotFound {
            session_id: "abc".to_string(),
        };
        assert!(err.to_string().contains("abc"));
    }

    #[test]
    fn test_error_convenience() {
        let err = TraceError::session_not_found("my-session");
        assert!(matches!(err, TraceError::SessionNotFound { .. }));
    }

    #[test]
    fn test_result_type() {
        fn returns_ok() -> Result<i32> {
            Ok(42)
        }
        fn returns_err() -> Result<i32> {
            Err(TraceError::NoEventsFound)
        }
        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }
}
