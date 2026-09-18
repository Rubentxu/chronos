//! REC-C3.3.2 — composition root for the chronos-mcp binary.
//!
//! This module owns the wiring of concrete infrastructure adapters
//! (`SessionStore`, future `NativeProbeBackend`, `BrowserAdapter`,
//! `EbpfAdapter`, `ExecutionLogProvider`). It is the **only** place in
//! `chronos-mcp` where `::new()` / `try_open()` on infrastructure
//! types is called; every other module must consume the ports declared
//! in `chronos-domain::ports`.
//!
//! ## Scope discipline
//!
//! C3.3.2 deliberately keeps the surface narrow:
//!
//! - **bootstrap-scoped** factories (this module): long-lived
//!   repositories and singletons that exist for the entire server
//!   lifetime. Constructed once at startup.
//! - **session-scoped** factories (added in subsequent 3.3.2.x
//!   commits): per-session probes and execution logs. Constructed via
//!   `dyn Factory` ports that the composition root injects; services
//!   call the factory, never the concrete.
//!
//! See `cycle-artifacts/p-3416cfb8288f8964/rec-c3-3-2-composition-integration-inversion/exploration-report.md`
//! for the full inventory and classification.
//!
//! ## Why a module and not a crate
//!
//! One real bootstrap consumer today (the `chronos-mcp` binary). A new
//! crate would be speculative; if a second consumer (HTTP/gRPC/...)
//! arrives, the wiring is extracted then.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chronos_domain::ports::execution_log_factory::ExecutionLogFactory;
use chronos_log::factory::SegmentedExecutionLogFactory;
use chronos_store::{SessionStore, StoreError};

/// REC-C3.3.2 — build the production `ExecutionLogFactory`.
///
/// Today only the segmented backend exists; tomorrow this is where a
/// second backend (in-memory mock for an embedder, a remote store,
/// etc.) is selected based on configuration. The factory is the
/// single seam; services never name the concrete type.
///
/// Returned `Arc<dyn ExecutionLogFactory>` is what gets injected into
/// `SessionExecutionLogRegistry::with_factory` and into the bootstrap
/// path. `bootstrap_execution_logs` and `SessionExecutionLog::create`
/// both consume it.
pub fn default_execution_log_factory() -> Arc<dyn ExecutionLogFactory> {
    Arc::new(SegmentedExecutionLogFactory::new())
}

/// Default path for the session store, mirrored from `server.rs` so the
/// resolution stays testable without mutating the process environment.
///
/// `$CHRONOS_DB_PATH` wins; otherwise `$HOME/.local/share/chronos/sessions.redb`.
pub fn default_store_path(db_path: Option<&str>, home: Option<&str>) -> PathBuf {
    if let Some(explicit) = db_path.filter(|p| !p.is_empty()) {
        return PathBuf::from(explicit);
    }
    let mut path = PathBuf::from(home.filter(|h| !h.is_empty()).unwrap_or("."));
    path.push(".local");
    path.push("share");
    path.push("chronos");
    path.push("sessions.redb");
    path
}

/// Whether the caller explicitly opted into the in-memory fallback.
///
/// Strict on purpose: only `1`, `true` or `yes` (case-insensitive, trimmed)
/// enable it, because any looser rule reintroduces the silent degradation this
/// policy exists to prevent.
pub fn allow_in_memory_fallback(raw: Option<&str>) -> bool {
    matches!(
        raw.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

/// Compose the session store at `path`, falling back to in-memory only
/// when `allow_in_memory_fallback` is true.
///
/// This is the only function in `chronos-mcp` that may call
/// `SessionStore::try_open` or `SessionStore::in_memory`. `server.rs`
/// and any future bootstrap consumer delegate here.
pub fn open_session_store_at(
    path: &Path,
    allow_in_memory_fallback: bool,
) -> Result<SessionStore, StoreOpenError> {
    match SessionStore::try_open(path) {
        Ok(store) => {
            tracing::info!("Opened session store at {}", path.display());
            Ok(store)
        }
        Err(cause) if allow_in_memory_fallback => {
            tracing::error!(
                "Could not open session store at {}: {}. Starting with an in-memory store \
                 (degraded: nothing will be persisted).",
                path.display(),
                cause
            );
            SessionStore::in_memory().map_err(|e| StoreOpenError {
                path: path.to_path_buf(),
                cause: Box::new(e),
            })
        }
        Err(cause) => Err(StoreOpenError {
            path: path.to_path_buf(),
            cause: Box::new(cause),
        }),
    }
}

/// The session store this process was configured to open could not be opened.
///
/// m9-75 (closes `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`):
/// a store that exists but cannot be opened (locked by another process, corrupt,
/// permission denied) must not be silently replaced by an empty in-memory store.
#[derive(Debug)]
pub struct StoreOpenError {
    pub(crate) path: PathBuf,
    /// Boxed so the `Err` variant stays small (`clippy::result_large_err`).
    pub(crate) cause: Box<StoreError>,
}

impl StoreOpenError {
    /// The path that could not be opened.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The underlying store failure.
    pub fn cause(&self) -> &StoreError {
        &self.cause
    }
}

impl std::fmt::Display for StoreOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot open the session store at {}: {}; refusing to start with an empty \
             in-memory store (set CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1 to opt in to that \
             degraded mode explicitly)",
            self.path.display(),
            self.cause
        )
    }
}

impl std::error::Error for StoreOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.cause)
    }
}

// =====================================================================
// Tests for the composition module live in `composition_tests` (below).
// They exercise `default_store_path` and `allow_in_memory_fallback`
// with controlled inputs so the resolution is testable without
// mutating the process environment.
// =====================================================================

#[cfg(test)]
mod composition_tests {
    use super::*;

    #[test]
    fn default_store_path_explicit_wins() {
        let p = default_store_path(Some("/tmp/x.redb"), Some("/home/me"));
        assert_eq!(p, std::path::PathBuf::from("/tmp/x.redb"));
    }

    #[test]
    fn default_store_path_explicit_empty_falls_back_to_home() {
        let p = default_store_path(Some(""), Some("/home/me"));
        assert_eq!(
            p,
            std::path::PathBuf::from("/home/me/.local/share/chronos/sessions.redb")
        );
    }

    #[test]
    fn default_store_path_no_home_falls_back_to_dot() {
        let p = default_store_path(None, None);
        assert_eq!(
            p,
            std::path::PathBuf::from("./.local/share/chronos/sessions.redb")
        );
    }

    #[test]
    fn allow_in_memory_fallback_strict() {
        assert!(allow_in_memory_fallback(Some("1")));
        assert!(allow_in_memory_fallback(Some("true")));
        assert!(allow_in_memory_fallback(Some("TRUE")));
        assert!(allow_in_memory_fallback(Some(" yes ")));
        assert!(!allow_in_memory_fallback(Some("0")));
        assert!(!allow_in_memory_fallback(Some("false")));
        assert!(!allow_in_memory_fallback(None));
    }
}
