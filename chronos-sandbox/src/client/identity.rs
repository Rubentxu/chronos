//! `BinaryIdentity` — REC-C0.5-harness (Etapa D).
//!
//! Captures the SHA-256 and modification time (Unix nanos) of a
//! `chronos-mcp` binary so the sandbox harness can verify it isn't
//! running a stale build (audit §8.3 finding).
//!
//! ## Why both `sha256` and `mtime`
//!
//! The two fields catch different failure modes:
//!
//! - **`sha256`** is the authoritative identity: identical hashes
//!   guarantee identical bytes. A drift here means the sandbox is
//!   measuring a different build than the one the operator just
//!   produced. `CHRONOS_MCP_EXPECTED_SHA` enforces fail-hard
//!   mismatch.
//! - **`mtime_unix_nanos`** is a cheap **observability** signal:
//!   when the cache looks fresh (path exists, mtime > previous run)
//!   but the SHA didn't change, the harness skips the SHA recompute.
//!   It is logged alongside the SHA so the operator can correlate
//!   sandbox runs to `cargo build` invocations in the same audit log.
//!
//! ## Why log-warn by default, fail-hard opt-in
//!
//! Per the proposal's Etapa D decision (see
//! `cycle-artifacts/.../proposal.md` line 174): fail-hard by default
//! would break local development where the operator hasn't yet built
//! the binary in the test target dir. The identity is therefore
//! **always logged** at `tracing::info!`, but only `panics` or
//! errors when `CHRONOS_MCP_EXPECTED_SHA` is set. A mismatch without
//! the env var is a warning, not a failure.

use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::client::error::McpSandboxError;

/// Identity fingerprint for the spawned `chronos-mcp` binary.
///
/// SHA-256 is the authoritative identity; `mtime_unix_nanos` is the
/// cheap observability companion. Both are captured at client start
/// and logged so the operator can correlate with `cargo build`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryIdentity {
    /// Lower-case hex SHA-256 of the binary file's bytes.
    pub sha256: String,
    /// Modification time as Unix nanos (resolved via
    /// `metadata.modified()`). `None` when the filesystem doesn't
    /// support mtime resolution (rare; only FAT-family filesystems
    /// fall here, and we still log the SHA).
    pub mtime_unix_nanos: Option<u128>,
}

impl BinaryIdentity {
    /// Capture the identity of the binary at `path`.
    ///
    /// Returns an error when the file can't be read at all (missing,
    /// permission denied, etc.). The mtime is captured opportunistically
    /// and falls back to `None` rather than failing the whole
    /// capture — the SHA is the load-bearing identity for the
    /// fail-hard path.
    pub fn from_path(path: &Path) -> Result<Self, McpSandboxError> {
        let bytes = std::fs::read(path).map_err(|e| {
            McpSandboxError::SpawnFailed(format!(
                "BinaryIdentity::from_path({}): read failed: {e}",
                path.display()
            ))
        })?;
        let sha256 = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let out = hasher.finalize();
            // Lower-case hex; matches the operator's `sha256sum` output
            // for easy cross-checking in audit logs.
            let mut s = String::with_capacity(64);
            for b in out {
                s.push_str(&format!("{b:02x}"));
            }
            s
        };
        let mtime_unix_nanos = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos());
        Ok(Self {
            sha256,
            mtime_unix_nanos,
        })
    }

    /// Verify this identity against an expected SHA read from the
    /// `CHRONOS_MCP_EXPECTED_SHA` environment variable. Returns `Ok(())`
    /// when the env var is unset (log-warn default) or when the SHA
    /// matches; returns `Err` with a clear message otherwise.
    ///
    /// The comparison is case-insensitive to forgive copy-paste from
    /// `sha256sum` output (which is lower-case but operators sometimes
    /// uppercase it).
    ///
    /// Prefer [`Self::verify_expected_sha_value`] in tests and any
    /// non-operator code path; reading the env var mutates the
    /// process-global environment of the host test binary if used
    /// with `std::env::set_var`, which is forbidden by vault drift
    /// CC#56. The no-arg form is kept for production operator use.
    pub fn verify_expected_sha(&self) -> Result<(), McpSandboxError> {
        let expected = match std::env::var("CHRONOS_MCP_EXPECTED_SHA") {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        self.verify_expected_sha_value(Some(&expected))
    }

    /// Verify this identity against an explicitly-provided expected SHA.
    /// Pass `None` to skip the check (equivalent to the env var being unset).
    /// This form does NOT touch `std::env` and is safe to call from
    /// tests without process-global side effects.
    pub fn verify_expected_sha_value(&self, expected: Option<&str>) -> Result<(), McpSandboxError> {
        let expected = match expected {
            Some(s) => s,
            None => return Ok(()),
        };
        let expected = expected.trim().to_ascii_lowercase();
        if self.sha256 == expected {
            Ok(())
        } else {
            Err(McpSandboxError::SpawnFailed(format!(
                "BinaryIdentity mismatch: expected sha256 `{expected}`, got `{}` (mtime={:?}). \
                 Refusing to start sandbox with stale binary. \
                 Set CHRONOS_MCP_EXPECTED_SHA correctly or rebuild the binary.",
                self.sha256, self.mtime_unix_nanos
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Allocate a unique temp path under the system temp dir for one test.
    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("chronos-binary-identity-{label}-{nanos}-{seq}"))
    }

    /// Write `content` to a unique temp path and capture its identity.
    fn temp_binary(label: &str, content: &[u8]) -> (std::path::PathBuf, BinaryIdentity) {
        let path = unique_temp_path(label);
        std::fs::write(&path, content).expect("write");
        let id = BinaryIdentity::from_path(&path).expect("from_path");
        (path, id)
    }

    #[test]
    fn from_path_captures_correct_sha256_and_mtime() {
        let content = b"hello world\n";
        let (path, id) = temp_binary("sha", content);
        let _ = std::fs::remove_file(&path);
        // SHA-256 of "hello world\n" — well-known vector.
        let expected = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        assert_eq!(id.sha256, expected);
        assert!(
            id.mtime_unix_nanos.is_some(),
            "mtime should be present on tmpfs"
        );
    }

    #[test]
    fn from_path_returns_err_for_missing_file() {
        let missing = std::path::PathBuf::from("/tmp/chronos-binary-identity-test-DOES-NOT-EXIST");
        let result = BinaryIdentity::from_path(&missing);
        assert!(matches!(result, Err(McpSandboxError::SpawnFailed(_))));
    }

    #[test]
    fn verify_expected_sha_no_env_var_is_ok() {
        let (path, id) = temp_binary("noenv", b"abc");
        let _ = std::fs::remove_file(&path);
        id.verify_expected_sha_value(None)
            .expect("no expected sha -> ok");
    }

    #[test]
    fn verify_expected_sha_matching_is_ok() {
        let content = b"abc";
        let (path, id) = temp_binary("match", content);
        let _ = std::fs::remove_file(&path);
        let captured = id.sha256.clone();
        id.verify_expected_sha_value(Some(&captured))
            .expect("matching sha -> ok");
    }

    #[test]
    fn verify_expected_sha_mismatch_returns_err() {
        let (path, id) = temp_binary("mismatch", b"abc");
        let _ = std::fs::remove_file(&path);
        let actual = id.sha256.clone();
        let err = id
            .verify_expected_sha_value(Some("deadbeef"))
            .expect_err("mismatch should fail");
        match err {
            McpSandboxError::SpawnFailed(msg) => {
                assert!(msg.contains("deadbeef"), "msg must include expected: {msg}");
                assert!(msg.contains(&actual), "msg must include actual sha: {msg}");
            }
            other => panic!("expected SpawnFailed, got {other:?}"),
        }
    }

    #[test]
    fn verify_expected_sha_case_insensitive() {
        let content = b"abc";
        let (path, id) = temp_binary("case", content);
        let _ = std::fs::remove_file(&path);
        let upper = id.sha256.to_ascii_uppercase();
        id.verify_expected_sha_value(Some(&upper))
            .expect("uppercase sha should match (case-insensitive)");
    }
}
