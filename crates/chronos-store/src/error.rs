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

    /// m9-08 (closes FIND-M9-01-DV-COUP-02): the loader previously rejected
    /// future-versioned bundles as `StoreError::Serialization` with an
    /// upgrade message, which overloaded the Serialization variant: callers
    /// pattern-matching on it to catch corrupt blobs would also catch the
    /// forward-compatibility rejection. With three schema versions shipped
    /// (`KNOWN_BUNDLE_SCHEMA_VERSIONS = [1, 2, 3]`, `CURRENT = 3`), the
    /// forward-compat branch is real, not hypothetical, and worth
    /// distinguishing. Use `{ found, supported }` to branch without
    /// parsing the message.
    #[error("bundle schema_version {found} is newer than supported {supported}; upgrade chronos-store to read this bundle")]
    SchemaTooNew { found: u32, supported: u32 },
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

    // m9-08 (closes FIND-M9-01-DV-COUP-02): the new dedicated variant
    // produces a stable Display string and exposes `{ found, supported }`
    // fields for callers to pattern-match on without parsing the message.
    #[test]
    fn test_schema_too_new_variant_display_and_fields() {
        let err = StoreError::SchemaTooNew {
            found: 4,
            supported: 3,
        };
        assert_eq!(
            err.to_string(),
            "bundle schema_version 4 is newer than supported 3; \
             upgrade chronos-store to read this bundle"
        );
        match err {
            StoreError::SchemaTooNew { found, supported } => {
                assert_eq!(found, 4);
                assert_eq!(supported, 3);
            }
            other => panic!("expected SchemaTooNew, got: {other:?}"),
        }
    }
}
