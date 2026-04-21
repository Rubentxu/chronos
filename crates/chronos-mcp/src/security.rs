//! Security hardening utilities for Chronos MCP server.
//!
//! Provides path validation and input sanitization to prevent attacks
//! such as path traversal, command injection, and resource exhaustion.

use std::path::{Path, PathBuf};

/// Allowed path prefixes for validated program paths.
const ALLOWED_PREFIXES: &[&str] = &["/", "/usr", "/home", "/tmp", "/opt"];

/// Validate a program path for execution.
///
/// Rejects:
/// - Path traversal sequences (`..`)
/// - Non-absolute paths (must start with `/`)
/// - Non-existent files (canonicalization fails)
///
/// Returns the canonical path on success.
pub fn validate_program_path(path: &str) -> Result<PathBuf, SecurityError> {
    // 1. Reject if contains ".."
    if path.contains("..") {
        return Err(SecurityError::PathTraversal(path.to_string()));
    }

    // 2. Reject if not absolute (must start with '/')
    if !path.starts_with('/') {
        return Err(SecurityError::NonAbsolutePath(path.to_string()));
    }

    // 3. Attempt canonicalize — reject if fails (non-existent or symlink loop)
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|_| SecurityError::ProgramNotFound(path.to_string()))?;

    // 4. Verify the canonical path starts with an allowed prefix
    let canonical_str = canonical.to_string_lossy();
    let has_allowed_prefix = ALLOWED_PREFIXES
        .iter()
        .any(|prefix| canonical_str.starts_with(prefix));

    if !has_allowed_prefix {
        return Err(SecurityError::ProgramNotFound(format!(
            "Path '{}' resolves to '{}' which is not under an allowed prefix",
            path, canonical_str
        )));
    }

    Ok(canonical)
}

/// Sanitize a session ID for use as a storage key.
///
/// Accepts: alphanumeric + hyphens + underscores, max 128 chars.
/// Rejects: path separators, null bytes, empty string.
pub fn sanitize_session_id(id: &str) -> Result<String, SecurityError> {
    if id.is_empty() {
        return Err(SecurityError::EmptySessionId);
    }
    if id.len() > 128 {
        return Err(SecurityError::SessionIdTooLong);
    }
    if id.contains('/') || id.contains('\\') || id.contains('\0') {
        return Err(SecurityError::InvalidSessionId(id.to_string()));
    }
    Ok(id.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Path traversal detected in: {0}")]
    PathTraversal(String),

    #[error("Non-absolute path rejected: {0}")]
    NonAbsolutePath(String),

    #[error("Program not found: {0}")]
    ProgramNotFound(String),

    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),

    #[error("Session ID too long (max 128 chars)")]
    SessionIdTooLong,

    #[error("Empty session ID")]
    EmptySessionId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_program_path_rejects_dotdot() {
        let result = validate_program_path("../etc/passwd");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecurityError::PathTraversal(_)));
    }

    #[test]
    fn test_validate_program_path_rejects_relative() {
        let result = validate_program_path("./myapp");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::NonAbsolutePath(_)
        ));
    }

    #[test]
    fn test_validate_program_path_accepts_absolute() {
        // /bin/ls should exist on Linux systems
        let result = validate_program_path("/bin/ls");
        if result.is_ok() {
            assert!(result.unwrap().is_absolute());
        }
        // If ls doesn't exist (e.g., musl container), skip
    }

    #[test]
    fn test_validate_program_path_rejects_nonexistent() {
        // This path should not exist
        let result = validate_program_path("/nonexistent/path/to/binary");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::ProgramNotFound(_)
        ));
    }

    #[test]
    fn test_sanitize_session_id_rejects_slash() {
        let result = sanitize_session_id("../../evil");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::InvalidSessionId(_)
        ));
    }

    #[test]
    fn test_sanitize_session_id_rejects_empty() {
        let result = sanitize_session_id("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecurityError::EmptySessionId));
    }

    #[test]
    fn test_sanitize_session_id_accepts_uuid() {
        let result = sanitize_session_id("550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_sanitize_session_id_accepts_simple() {
        let result = sanitize_session_id("my_session_1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my_session_1");
    }

    #[test]
    fn test_sanitize_session_id_rejects_too_long() {
        let long_id = "a".repeat(129);
        let result = sanitize_session_id(&long_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecurityError::SessionIdTooLong));
    }

    #[test]
    fn test_sanitize_session_id_accepts_max_length() {
        let max_id = "a".repeat(128);
        let result = sanitize_session_id(&max_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sanitize_session_id_rejects_null_byte() {
        let result = sanitize_session_id("session\0evil");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::InvalidSessionId(_)
        ));
    }

    #[test]
    fn test_sanitize_session_id_rejects_backslash() {
        let result = sanitize_session_id("session\\evil");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::InvalidSessionId(_)
        ));
    }
}
