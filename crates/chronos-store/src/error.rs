//! Error types for the chronos-store crate.

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] redb::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Hash not found: {0}")]
    HashNotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_error_display() {
        let err = StoreError::SessionNotFound("test-session".to_string());
        assert_eq!(err.to_string(), "Session not found: test-session");

        let err = StoreError::HashNotFound("abc123".to_string());
        assert_eq!(err.to_string(), "Hash not found: abc123");

        let err = StoreError::Serialization("bincode failed".to_string());
        assert_eq!(err.to_string(), "Serialization error: bincode failed");

        let err = StoreError::Compression("lz4 failed".to_string());
        assert_eq!(err.to_string(), "Compression error: lz4 failed");
    }
}
