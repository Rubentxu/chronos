#[derive(Debug, thiserror::Error)]
pub enum PythonError {
    #[error("Python 3 not found in PATH")]
    PythonNotFound,

    #[error("Failed to spawn Python subprocess: {0}")]
    SpawnFailed(String),

    #[error("Failed to parse event JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Python process exited unexpectedly: code={0}")]
    ProcessExited(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors specific to the Python DAP adapter.
#[derive(Debug, thiserror::Error)]
pub enum PythonAdapterError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("DAP protocol error: {0}")]
    ProtocolError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_error_display() {
        let err = PythonError::PythonNotFound;
        assert_eq!(err.to_string(), "Python 3 not found in PATH");

        let err = PythonError::SpawnFailed("No such file".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to spawn Python subprocess: No such file"
        );

        let err = PythonError::ProcessExited("exit code: 1".to_string());
        assert_eq!(
            err.to_string(),
            "Python process exited unexpectedly: code=exit code: 1"
        );
    }
}
