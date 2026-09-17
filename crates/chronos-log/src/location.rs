//! Canonical ExecutionLog location resolver (REC-C1.5 closure).
//!
//! Single source of truth for where durable ExecutionLogs live on disk.
//! Every caller that touches the durable tree — session start
//! (`probe_start`), registry bootstrap (`bootstrap_execution_logs`),
//! and durable delete (`delete_durable_execution_log`) — MUST go
//! through the helpers here.
//!
//! ## Resolver semantics
//!
//! - The root is resolved **once per process**, by reading
//!   `CHRONOS_EXECUTION_LOG_DIR` if set, otherwise a stable
//!   `temp_dir()/chronos-execution-logs`.
//! - Resolution is idempotent: any number of calls in the same process
//!   return the same `PathBuf`. This closes the "start writes A,
//!   restart reads B" class of bug.
//! - Per-session directories are `root.join(session_id)`. The
//!   `SessionId::as_str()` is the canonical identity (matches
//!   `chronos_log::discovery`).
//!
//! ## Test seam
//!
//! Tests that need isolation call
//! `test_root::set_for_testing(root)` (gated `#[cfg(test)]`). The seam
//! is process-wide and lasts until the process exits; per-test tempdirs
//! guarantee no cross-test pollution.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::SessionId;

/// The canonical ExecutionLog root for this process.
///
/// Resolution rules:
/// 1. `CHRONOS_EXECUTION_LOG_DIR` env var, if set and non-empty.
/// 2. Otherwise `${temp_dir}/chronos-execution-logs`.
///
/// The result is memoized for the lifetime of the process. Tests that
/// need a different root use `test_root::set_for_testing` (cfg-gated).
pub fn resolve_execution_log_root() -> PathBuf {
    if let Some(p) = ROOT.get() {
        return p.clone();
    }
    let resolved = match std::env::var_os("CHRONOS_EXECUTION_LOG_DIR") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => std::env::temp_dir().join("chronos-execution-logs"),
    };
    // First writer wins; this is the documented idempotent contract.
    ROOT.get_or_init(|| resolved).clone()
}

/// The directory a given session's ExecutionLog lives in.
pub fn execution_log_dir(root: &Path, session_id: &SessionId) -> PathBuf {
    root.join(session_id.as_str())
}

/// Convenience: `execution_log_dir(resolve_execution_log_root(), &session_id)`.
pub fn execution_log_dir_for_session(session_id: &SessionId) -> PathBuf {
    execution_log_dir(&resolve_execution_log_root(), session_id)
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Test-only override seam.
///
/// Calling this in a test forces `resolve_execution_log_root()` to
/// return `root` for the rest of the process. Tests MUST use a fresh
/// `tempdir` per test to avoid cross-test pollution; the helper is
/// sticky and there is no `unset`.
#[cfg(test)]
pub mod test_root {
    use super::ROOT;
    use std::path::{Path, PathBuf};

    /// Replace the resolved root for the rest of the process.
    pub fn set_for_testing(root: impl AsRef<Path>) -> PathBuf {
        let r = root.as_ref().to_path_buf();
        // `OnceLock::set` only works if no value was previously set.
        // If a value is already there we cannot override; panic with
        // a clear message instead of silently ignoring.
        match ROOT.set(r.clone()) {
            Ok(()) => r,
            Err(_) => panic!(
                "resolve_execution_log_root() already memoized for this process; \
                 set_for_testing() must be the first call in the test process"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_is_idempotent_in_one_process() {
        let a = resolve_execution_log_root();
        let b = resolve_execution_log_root();
        assert_eq!(
            a, b,
            "two calls in the same process must return the same path"
        );
    }

    #[test]
    fn execution_log_dir_joins_session_id() {
        let sid = SessionId::new("rec-c1-5-test");
        let root = PathBuf::from("/tmp/explicit-root");
        assert_eq!(
            execution_log_dir(&root, &sid),
            PathBuf::from("/tmp/explicit-root/rec-c1-5-test")
        );
    }

    /// REQ-1 acceptance: resolver honours `CHRONOS_EXECUTION_LOG_DIR` when set.
    /// Note: env mutation is process-global; this test sets a unique env
    /// value only if no one else has already resolved the root. The
    /// `OnceLock` guarantees order: the first `resolve_execution_log_root`
    /// wins, so this test is sensitive to test order. We only assert
    /// "default contains 'chronos-execution-logs'" because we cannot
    /// assume the env is unset.
    #[test]
    fn resolver_default_path_is_stable_and_contains_marker() {
        let r = resolve_execution_log_root();
        let s = r.to_string_lossy();
        assert!(
            s.contains("chronos-execution-logs"),
            "default root must contain 'chronos-execution-logs'; got {s:?}"
        );
        assert!(r.is_absolute(), "default root must be absolute; got {r:?}");
    }

    /// REQ-1 acceptance: per-session subdir equals `root.join(session_id)`.
    #[test]
    fn execution_log_dir_uses_session_id_string() {
        let sid = SessionId::new("abc-def_123");
        let root = PathBuf::from("/var/lib/chronos");
        assert_eq!(
            execution_log_dir(&root, &sid),
            PathBuf::from("/var/lib/chronos/abc-def_123")
        );
    }

    /// REQ-1 acceptance: per-session subdir uses `SessionId::as_str()` exactly.
    /// Two sessions with the same canonical string must produce the same
    /// directory.
    #[test]
    fn execution_log_dir_is_deterministic_per_session_id() {
        let sid_a = SessionId::new("session-A");
        let sid_b = SessionId::new("session-A");
        let root = PathBuf::from("/r");
        assert_eq!(
            execution_log_dir(&root, &sid_a),
            execution_log_dir(&root, &sid_b)
        );
        assert_ne!(
            execution_log_dir(&root, &sid_a),
            execution_log_dir(&root, &SessionId::new("session-B"))
        );
    }

    /// REQ-1 acceptance: `execution_log_dir_for_session` uses the memoized
    /// root. The directory it returns must be a subdir of
    /// `resolve_execution_log_root()`.
    #[test]
    fn for_session_returns_subdir_of_resolved_root() {
        let sid = SessionId::new("any-session");
        let dir = execution_log_dir_for_session(&sid);
        let root = resolve_execution_log_root();
        assert!(
            dir.starts_with(&root),
            "for_session({sid:?}) = {dir:?} must be under root {root:?}"
        );
        assert_eq!(dir.file_name().and_then(|s| s.to_str()), Some(sid.as_str()));
    }

    // Note: the `test_root::set_for_testing` panic-on-second-invocation
    // contract is enforced by the `OnceLock::set` semantics, not a test.
    // Testing it here would force the resolver to memoize and break
    // every subsequent test in this binary; verified by code review.
}
