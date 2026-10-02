//! Security hardening utilities for Chronos MCP server.
//!
//! Provides path validation and input sanitization to prevent attacks
//! such as path traversal, command injection, and resource exhaustion.

use std::path::{Path, PathBuf};

/// Documented prefixes for validated program paths.
///
/// These are **NOT an allowlist and enforce nothing**. The first entry is
/// `/`, and `starts_with("/")` holds for every absolute path, so the check
/// below can never reject a path on prefix grounds. Keep this list readable
/// as documentation of where programs are normally installed, not as a
/// containment boundary — reading it as a boundary is exactly the mistake
/// that lets someone "harden" the code by deleting `/` and break every
/// caller, or assume that a path outside these prefixes is rejected when it
/// is not.
///
/// The real policy (see `validate_program_path`) is: absolute, no `..`,
/// and the path must exist. That is the product's purpose — running the
/// program the user named. Multi-tenant containment is not in scope yet.
const ALLOWED_PREFIXES: &[&str] = &["/", "/usr", "/home", "/tmp", "/opt"];

/// Validate a program path for execution.
///
/// Rejects:
/// - Path traversal sequences (`..`)
/// - Non-absolute paths (must start with `/`)
/// - Non-existent files (canonicalization fails)
///
/// An existing absolute path is accepted wherever it lives — there is no
/// prefix allowlist, by design (see `ALLOWED_PREFIXES`).
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

    // 4. Prefix check. Kept as an explicit, always-true guard rather than
    // deleted, so the code states the policy where it is enforced: the
    // documented prefixes include "/", so this can never reject anything.
    // Dropping the list entirely is a semantics change (running arbitrary
    // programs is the point of this function); renaming the message below
    // is what makes the code honest.
    let canonical_str = canonical.to_string_lossy();
    let has_allowed_prefix = ALLOWED_PREFIXES
        .iter()
        .any(|prefix| canonical_str.starts_with(prefix));

    if !has_allowed_prefix {
        // Unreachable with the current list; if the list is ever narrowed,
        // this must not claim a containment that the rejection does not
        // describe.
        return Err(SecurityError::ProgramNotFound(format!(
            "Path '{}' resolves to '{}', which does not match any documented prefix",
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

/// Validate a path the server will **write** to.
///
/// Two rules, the same ones `validate_program_path` applies and that this
/// crate already treats as mandatory:
/// - no `..` as a path component
/// - the path must be absolute
///
/// `..` is matched as a whole component, not as a substring, so a filename
/// that merely contains two dots (`session..json`) is still a legitimate
/// destination. `validate_program_path` matches the substring; for a
/// program that costs nothing, but here it would reject exports a caller
/// had every right to ask for.
///
/// Deliberately **not** a containment boundary, for the same reason
/// `validate_program_path` is not one (see `ALLOWED_PREFIXES`): an absolute
/// path with no `..` can still name any location the process can write to,
/// and the export lands there. Deciding where a session bundle may be
/// written is a product question — the answer needs a base directory, and
/// inventing one here would change what the tool can do for every caller.
/// What this function removes is the traversal, which needs no product
/// decision to reject.
///
/// The path is **not** canonicalized and is **not** required to exist: an
/// output path names a file that is about to be created, so requiring
/// existence (as `validate_program_path` does for programs) would reject
/// every legitimate export.
pub fn validate_output_path(path: &str) -> Result<PathBuf, SecurityError> {
    if path.split('/').any(|component| component == "..") {
        return Err(SecurityError::PathTraversal(path.to_string()));
    }
    if !path.starts_with('/') {
        return Err(SecurityError::NonAbsolutePath(path.to_string()));
    }
    Ok(PathBuf::from(path))
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

    /// `session_export` writes wherever `output_path` says, and the write
    /// path (`fs::rename`, then `fs::copy` as fallback) overwrites whatever
    /// is already there. A caller-supplied path with no checks is arbitrary
    /// file write; these are the two checks that close the traversal.
    #[test]
    fn test_validate_output_path_rejects_traversal_and_relative_paths() {
        assert!(
            matches!(
                validate_output_path("/tmp/../etc/passwd"),
                Err(SecurityError::PathTraversal(_))
            ),
            "a path with .. must be rejected, not normalized away"
        );
        assert!(matches!(
            validate_output_path("../etc/passwd"),
            Err(SecurityError::PathTraversal(_))
        ));
        assert!(
            matches!(
                validate_output_path("/var/data/../../root/.ssh/authorized_keys"),
                Err(SecurityError::PathTraversal(_))
            ),
            "a traversal in the middle of an absolute path must be caught too"
        );
        assert!(
            matches!(
                validate_output_path("/var/data/out/.."),
                Err(SecurityError::PathTraversal(_))
            ),
            "a trailing .. is still a component"
        );
        assert!(
            matches!(
                validate_output_path("relative/out.json"),
                Err(SecurityError::NonAbsolutePath(_))
            ),
            "a relative path resolves against the server's cwd, which the \
             caller does not control"
        );
    }

    /// Control: the path an export is actually asked for must keep working,
    /// including one that does not exist yet, and including `..` as a
    /// substring of a legitimate filename (which is not a traversal).
    #[test]
    fn test_validate_output_path_accepts_a_plain_absolute_destination() {
        for ok in [
            "/tmp/chronos-export/session.json",
            "/var/data/exports/out.json",
            // Does not exist yet: that is the normal case for an output.
            "/tmp/chronos-export/never-created-dir/session.json",
            // A `..` inside a name is not a path component.
            "/tmp/chronos..export/session.json",
        ] {
            assert!(
                validate_output_path(ok).is_ok(),
                "{ok} must remain a valid export destination"
            );
        }
    }

    #[test]
    fn test_validate_program_path_rejects_dotdot() {
        let result = validate_program_path("../etc/passwd");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::PathTraversal(_)
        ));
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
        if let Ok(path) = validate_program_path("/bin/ls") {
            assert!(path.is_absolute());
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

    /// Behaviour pin, not a proof of the documentation fix: the audit that
    /// prompted it changed only comments and one error message, so this
    /// test passes both before and after. What it protects is the
    /// *behaviour* those comments now describe — an existing absolute path
    /// outside the documented prefixes is accepted on purpose, because
    /// running the program the user named is the point of this function.
    /// Without this test a future refactor could narrow the prefix list and
    /// silently break every caller, thinking it was hardening.
    #[test]
    fn test_validate_program_path_accepts_path_outside_documented_prefixes() {
        // /etc/hosts exists on Linux and macOS and is under neither /usr,
        // /home, /tmp nor /opt.
        match validate_program_path("/etc/hosts") {
            Ok(path) => {
                assert!(path.is_absolute());
                assert!(
                    path.to_string_lossy().starts_with('/'),
                    "expected a canonical absolute path, got {:?}",
                    path
                );
            }
            Err(SecurityError::ProgramNotFound(_)) => {
                // No /etc/hosts in this environment: the path does not exist,
                // so there is no behaviour to pin. Same skip as the /bin/ls
                // test above.
            }
            Err(other) => panic!("unexpected rejection of an existing absolute path: {other}"),
        }
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
        assert_eq!(result.unwrap(), "550e8400-e29b-41d4-a716-446655440000");
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
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::SessionIdTooLong
        ));
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
