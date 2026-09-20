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

use chronos_domain::ports::browser_probe::BrowserProbeFactory;
use chronos_domain::ports::execution_log_factory::ExecutionLogFactory;
use chronos_domain::ports::uprobe::UprobeInjector;
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

/// REC-C3.3.2.3 — build the production `UprobeInjector`.
///
/// Returns an `Arc<dyn UprobeInjector>` that resolves to a real
/// `EbpfAdapter` on systems where eBPF uprobes are available, and to a
/// `CapabilityUnavailable::ebpf_uprobe` error otherwise. This is the
/// **only** function in the workspace that constructs the injector;
/// `server.rs` consumes the value and hands it to `chronos-services`
/// through `ProbeContext::uprobe_injector`.
///
/// Why a separate factory: `UprobeInjector::acquire` performs kernel
/// detection, which is bootstrap-time work that must NOT live in
/// `chronos_services` (the architectural law that excludes
/// `chronos-ebpf` from that crate). The injector is a `Copy` unit
/// struct (`EbpfUprobeInjector`); the heavy lifting happens inside
/// `acquire`, on the calling thread, with the resulting handle shared
/// with the live session via `Arc<dyn UprobeHandle>`.
pub fn default_uprobe_injector() -> Arc<dyn UprobeInjector> {
    Arc::new(chronos_ebpf::EbpfUprobeInjector::new())
}

/// REC-C3.3.2.4 — build the production `BrowserProbeFactory`.
///
/// Returns an `Arc<dyn BrowserProbeFactory>` that resolves to a fresh
/// `BrowserAdapter` (gated on Chrome availability) on every `create`
/// call. This is the **only** function in the workspace that
/// constructs the factory; `server.rs` consumes the value and hands it
/// to `chronos-services` through `BrowserProbeContext::factory`.
///
/// Why a factory instead of a singleton: browser probes are
/// session-scoped (every start spawns a fresh Chrome process and
/// obtains a dedicated backend). A shared singleton does not fit the
/// lifecycle; the factory pattern mirrors `UprobeInjector::acquire`
/// (per-session handle) but is async-friendly because the backend
/// creation has no async work — capability detection is sync.
pub fn default_browser_probe_factory() -> Arc<dyn BrowserProbeFactory> {
    Arc::new(chronos_browser::BrowserProbeFactoryImpl::new())
}

/// REC-C3.3.3 (Tren B slice C) — build the production `SessionArchive`.
///
/// Returns an `Arc<dyn SessionArchive>` driven by the existing
/// `SessionStore`. The factory is the only seam: services consume
/// the port, not the concrete store. Bootstrap-scoped: constructed
/// once at startup and held by `ChronosServer`.
pub fn default_session_archive(
    store: Arc<SessionStore>,
) -> Arc<dyn chronos_domain::ports::session::SessionArchive> {
    chronos_store::session_archive::SessionStoreBackedSessionArchive::new(store).into_arc()
}

/// REC-C3.3.3 (Tren B slice C) — build an in-memory `SessionArchive`
/// for tests and degraded mode.
///
/// Mirrors the `InMemorySessionRepository` pattern (REC-C3.3 territory):
/// composition-root fallback when no persistent store is configured,
/// or when tests need isolation between sessions.
pub fn in_memory_session_archive() -> Arc<dyn chronos_domain::ports::session::SessionArchive> {
    chronos_domain::ports::session::InMemorySessionArchive::new().into_arc()
}

/// REC-C3.3.3 (Tren B slice D) — build the production
/// `CounterexampleRepository`.
///
/// **EXPERIMENTAL (FIND-TB-AUDIT-2026-09-20)**: this port currently has
/// NO production consumer. `ChronosCounterexampleService` still uses
/// `&SessionStore` directly. The adapter is correct and the factory
/// works, but per audit §13 ("no abstraction without a real consumer"),
/// this is filed as experimental until a real service-side rewire lands.
/// See `apply-checkpoint.json` `carry_forward_debt` (C33.3-TB-DEBT-01).
pub fn default_counterexample_repository(
    store: Arc<SessionStore>,
) -> Arc<dyn chronos_domain::ports::counterexample::CounterexampleRepository> {
    chronos_store::counterexample_repository::SessionStoreBackedCounterexampleRepository::new(store)
        .into_arc()
}

/// REC-C3.3.3 (Tren B slice D) — build an in-memory
/// `CounterexampleRepository` for tests and degraded mode.
/// See `default_counterexample_repository` for the experimental-status note.
pub fn in_memory_counterexample_repository(
) -> Arc<dyn chronos_domain::ports::counterexample::CounterexampleRepository> {
    chronos_domain::ports::counterexample::InMemoryCounterexampleRepository::new().into_arc()
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

    // =============================================================
    // REC-C3.3.3 (Tren B slice C) — SessionArchive factory tests
    // =============================================================

    fn temp_store() -> (std::path::PathBuf, Arc<SessionStore>) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("chronos-comp-tests-{}-{}", std::process::id(), seq));
        std::fs::create_dir_all(&dir).expect("create temp store dir");
        let path = dir.join("sessions.redb");
        let store = SessionStore::try_open(&path).expect("open temp store");
        (path, Arc::new(store))
    }

    fn sample_metadata(id: &str) -> chronos_domain::SessionMetadata {
        chronos_domain::SessionMetadata {
            session_id: id.to_string(),
            created_at: 1,
            language: "native".to_string(),
            target: "/bin/true".to_string(),
            event_count: 0,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        }
    }

    #[test]
    fn default_session_archive_roundtrips_via_real_store() {
        let (_path, store) = temp_store();
        let archive = default_session_archive(store.clone());

        let meta = sample_metadata("session-roundtrip");
        archive.save(meta.clone(), &[]).expect("save");
        let (loaded, _events) = archive.load("session-roundtrip").expect("load");
        assert_eq!(loaded.session_id, meta.session_id);
    }

    #[test]
    fn in_memory_session_archive_is_fake() {
        let archive = in_memory_session_archive();
        assert!(!archive.is_persistent());
        let meta = sample_metadata("mem-1");
        archive.save(meta, &[]).expect("save");
        let (loaded, _) = archive.load("mem-1").expect("load");
        assert_eq!(loaded.session_id, "mem-1");
    }

    #[test]
    fn session_archive_is_persistent_true_for_persistent_store() {
        let (_path, store) = temp_store();
        assert!(store.is_persistent(), "temp redb store must be persistent");
        let archive = default_session_archive(store);
        assert!(archive.is_persistent());
    }

    #[test]
    fn session_archive_is_persistent_false_for_in_memory_store() {
        let store = SessionStore::in_memory().expect("in-memory store");
        assert!(!store.is_persistent());
        let archive = default_session_archive(Arc::new(store));
        assert!(!archive.is_persistent());
    }

    // =============================================================
    // REC-C3.3.3 (Tren B slice D) — CounterexampleRepository factory tests
    // =============================================================

    use chronos_domain::ports::counterexample::{
        CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    };

    fn ce_record(id: &str, kind: &str, ws: &str) -> CounterexampleBundleRecord {
        CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: id.to_string(),
                property_kind: kind.to_string(),
                workspace_id: ws.to_string(),
                created_at_ms: 1000,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
                events_count: 0,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        }
    }

    #[test]
    fn default_counterexample_repository_roundtrips_bundle() {
        let (_path, store) = temp_store();
        let repo = default_counterexample_repository(store);

        let id = repo
            .save_bundle(ce_record("b-1", "invariant", "ws-1"))
            .expect("save");
        assert_eq!(id, "b-1");
        let events = repo.load_bundle_events("b-1").expect("load events");
        assert!(events.is_empty());
        let count = repo.count_bundle_events("b-1").expect("count");
        assert_eq!(count, 0);
    }

    #[test]
    fn in_memory_counterexample_repository_filters_by_kind() {
        let repo = in_memory_counterexample_repository();
        repo.save_bundle(ce_record("b-a", "invariant", "ws-1"))
            .expect("save");
        repo.save_bundle(ce_record("b-b", "existence", "ws-1"))
            .expect("save");
        repo.save_bundle(ce_record("b-c", "invariant", "ws-2"))
            .expect("save");

        let filter = CounterexampleBundleFilter {
            property_kind: Some("invariant".to_string()),
            ..Default::default()
        };
        let summaries = repo.list_bundles(&filter).expect("list");
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().all(|s| s.property_kind == "invariant"));
    }
}
