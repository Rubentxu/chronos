//! Chronos MCP server — exposes debugging tools via MCP.
//!
//! Implements 10 tools for AI-assisted debugging.
//!
//! # Concurrency Model
//!
//! This server is designed to handle multiple concurrent client connections safely:
//!
//! - **Sessions are isolated**: Each debug session has a unique ID. Events collected
//!   for one session cannot leak into another, even under concurrent access.
//!
//! - **Shared state is protected**: All shared mutable state uses `Arc<Mutex<...>>` or
//!   `Arc<tokio::sync::Mutex<...>>`. The mutex granularity is at the session map level,
//!   not individual sessions, which is sufficient since operations are batched per session.
//!
//! - **Engine immutability**: `QueryEngine` is immutable after construction (indices are
//!   built once, then read-only). This makes sharing across threads safe.
//!
//! - **Atomic CAS operations**: The content-addressable store (`ContentStore::put`) uses
//!   a single write transaction with internal deduplication, ensuring atomicity under
//!   concurrent writes of identical content.
//!
//! - **Background sessions**: none. The placeholder map once reserved for
//!   pending background sessions was removed because no code path ever read or
//!   mutated it.

// BrowserAdapter / CaptureConfig / CaptureSession are used by the in-file
// `#[cfg(test)] mod tests` block; the lib code itself delegates everything
// to BrowserProbeService. The clippy::unused_imports lint complains even
// though the imports are real (just only used in tests). Allow explicitly.
#[allow(unused_imports)]
use chronos_browser::BrowserAdapter;
use chronos_domain::ports::browser_probe::BrowserProbeFactory;
use chronos_domain::ports::uprobe::UprobeInjector;
use chronos_domain::tripwire::TripwireManager;
use chronos_domain::MonotonicNs;
#[allow(unused_imports)]
use chronos_domain::{
    CaptureConfig, CaptureSession, EventData, EventType, Language, TraceEvent, VariableInfo,
};
use chronos_index::builder::IndexBuilder;
use chronos_query::QueryEngine;
use chronos_services::browser_probe::BrowserProbeSession;
use chronos_services::browser_probe::{
    BrowserProbeContext, BrowserProbeService, BrowserProbeStartInput,
};
use chronos_services::debug_read::DebugReadService;
use chronos_services::error::ServiceError;
use chronos_services::events_read::{ChronosEventsReadService, EventsReadContext, EventsReadInput};
use chronos_services::execution_query::{
    ChronosExecutionQueryService, ExecutionQueryContext, ExecutionQueryInput,
};
use chronos_services::observe::{ChronosObserveService, ObserveContext, ObserveInput};
use chronos_services::output::{CapabilitySnapshot, SessionLifecycleProvenance, SessionStopOutput};
use chronos_services::probe::LiveProbeSession;
use chronos_services::projection::{self, ProjectionMeta};
use chronos_services::query_service::QueryService;
use chronos_services::session_lifecycle::{
    ChronosSessionLifecycleService, SessionLifecycleContext, SessionStopPersistence,
};
use chronos_services::sessions::{SessionsContext, SessionsService};
use chronos_services::state_query::{ChronosStateQueryService, StateQueryContext, StateQueryInput};
use chronos_services::trace_slice::{ChronosTraceSliceService, TraceSliceContext, TraceSliceInput};
#[allow(unused_imports)]
use chronos_store::{SessionMetadata, SessionStore};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content};
use rmcp::tool;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// Tool parameter types, toolset constants and pure helpers were extracted to
// `tools_params`. They are re-exported here so `chronos_mcp::server::*` keeps
// resolving to the same items, and so the integration tests under
// `crates/chronos-mcp/tests/` that reference them through the server module
// continue to compile unchanged.
pub use crate::tools_params::*;

// Helper to create JSON text content
fn json_content(value: &serde_json::Value) -> Vec<Content> {
    vec![Content::text(
        serde_json::to_string_pretty(value).unwrap_or_default(),
    )]
}

// m9-82: wrap a session-persistence tool envelope with a top-level
// `degraded: <bool>` so MCP callers can tell whether the underlying
// store is in-memory (degraded) or file-backed (persistent). Closes
// FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE. The existing
// envelope is preserved unchanged; only one field is added.
//
// Used by `save_session`, `list_sessions`, `load_session`,
// `delete_session`, `drop_session`.
fn session_envelope(degraded: bool, value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            map.insert("degraded".to_string(), serde_json::Value::Bool(degraded));
            serde_json::Value::Object(map)
        }
        // Defensive: if a future caller hands us a non-object (e.g. an
        // accidental bare array), wrap it so the top-level shape stays a
        // JSON object — same wire contract.
        other => serde_json::json!({
            "result": other,
            "degraded": degraded,
        }),
    }
}

fn text_content(text: impl Into<String>) -> Vec<Content> {
    vec![Content::text(text.into())]
}

// Resource limits for read-path operations.
//
// SCALE_BUDGETS §5 D2 decided that `max_events` / `timeout_secs` are a hard
// ceiling on any read-path operation, not only on capture. The struct now
// lives in `chronos_services::read_budget` because the loops it governs are
// in `chronos-services`, and a ceiling enforced from the composition root
// into a lower layer cannot depend on a higher one. It is re-exported here so
// `chronos_mcp::server::ResourceLimits` keeps resolving unchanged.
//
// Before this move the struct was defined in this file and its only readers
// were the two `#[test]` functions further down: a decision with no
// mechanism. `ReadPathService` now holds the value and charges it at page
// boundaries; `read_budget`'s module docs carry the acceptance criterion and
// the reason the ceiling has to be cooperative.
pub use chronos_services::read_budget::ResourceLimits;

/// Empty parameter type for tools that take no arguments.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoParams {}

/// The Chronos MCP server state.
pub struct ChronosServer {
    /// Loaded query engines (session_id → engine).
    engines: Arc<Mutex<HashMap<String, QueryEngine>>>,
    /// Session languages (session_id → language) for routing evaluations.
    session_languages: Arc<Mutex<HashMap<String, chronos_domain::Language>>>,
    /// Persistent session store.
    store: Arc<SessionStore>,
    /// `SessionReader` port (REC-C3.5-B.1). Built from `store` via
    /// `SessionStoreBackedSessionReader` at composition time. Services
    /// consume the port; `store` stays for non-port consumers (probe
    /// persistence, etc.).
    reader: Arc<dyn chronos_domain::ports::session_reader::SessionReader>,
    /// `DiffEngine` port (REC-C3.5-residual-inversion R.2). Built once
    /// at composition time via `default_diff_engine()`. Services
    /// (`ChronosDiffService::compare_sessions`,
    /// `ChronosSessionCompareService::compare`) consume the port; the
    /// store stays out of the production code path.
    diff_engine: Arc<dyn chronos_domain::ports::diff::DiffEngine>,
    /// `LifecycleStore` port (REC-C3.5-B.4). Extends `SessionReader` with
    /// write-side methods (`save_session_meta`, `delete_session`) needed by
    /// `SessionLifecycleService`. Built from `store` via
    /// `SessionStoreBackedLifecycleStore` at composition time.
    lifecycle_store: Arc<dyn chronos_domain::ports::lifecycle_store::LifecycleStore>,
    /// `CounterexampleRepository` port (REC-C3.5-B'). Built from `store` via
    /// `SessionStoreBackedCounterexampleRepository` at composition time.
    /// `CounterexampleService` consumes the port; the store stays for
    /// non-port consumers (probe persistence, etc.).
    counterexample_repository:
        Arc<dyn chronos_domain::ports::counterexample::CounterexampleRepository>,
    /// `SessionArchive` port (REC-C3.3.3 Tren B). Built from `store` via
    /// `SessionStoreBackedSessionArchive` at composition time. Services
    /// consume the port; `store` stays for non-port consumers (probe
    /// persistence, etc.).
    archive: Arc<dyn chronos_domain::ports::session::SessionArchive>,
    /// Sessions with connected debug clients (Python debugpy or JS Node.js inspector).
    /// Used to track which sessions have active DAP/CDP connections.
    connected_sessions: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Currently active session for phased workflows.
    /// Automatically set after probe_start or capture completes.
    active_session: Arc<Mutex<Option<String>>>,
    /// Tripwire manager for condition-based event notification.
    tripwire_manager: Arc<TripwireManager>,
    /// Uprobe counter map (session_id → next uprobe subscription id
    /// index). Guarded by a std Mutex; used by the observe dispatcher to
    /// generate stable `uprobe-<session>-<n>` ids.
    uprobe_counter: Arc<std::sync::Mutex<HashMap<String, usize>>>,
    /// REC-C1.3: session-scoped ExecutionLog registry. Outlives `live_probes`
    /// so a stopped session's log stays readable without a second source.
    execution_logs: Arc<chronos_services::session_log::SessionExecutionLogRegistry>,
    /// M10 read-path entry point (`read_path`): resolves a `session_id`
    /// to its authoritative `SessionExecutionLog` and drives the live
    /// stream / virtualization / causality consumers. Built from the
    /// SAME `execution_logs` registry, so there is exactly one
    /// authority for reads (REC-C1 single-truth). Cheap to clone
    /// (Arc inside), so it can be handed to tool handlers by value.
    read_path: chronos_services::read_path::ReadPathService,
    execution_log_root: std::path::PathBuf,
    /// REC-C1.7: projection metadata for the canonical MCP operations.
    /// A session is in this map iff a projection has been built from its
    /// SessionExecutionLog via `chronos_services::projection::build_engine`.
    /// Used by the MCP-wrapper gate (`meta_is_full`) on execution_query,
    /// state_query, trace_slice to reject queries whose projection is
    /// Truncated or Empty. Mirrored with `engines` in two directions:
    /// every entry here has a corresponding entry in `engines`, because
    /// `ensure_projection` inserts both under the same lock pair and
    /// `cleanup_session_memory` evicts both; and the other way round, an
    /// entry here is invalidated by `build_and_store_engine` whenever a
    /// live-capture drain arrives for that session, so a surviving entry
    /// always describes an `engines` slot that `build_engine` produced.
    /// The converse does not hold — `build_and_store_engine`, used by the
    /// live capture drains, registers an engine without a projection, so
    /// `engines` can be a strict superset.
    /// The map is a cache, never an authority: a missing entry means "not
    /// projected yet", and the next query rebuilds it from the log.
    projection_meta: Arc<Mutex<HashMap<String, chronos_services::projection::ProjectionMeta>>>,
    /// Live probe sessions: session_id → LiveProbeSession.
    /// These are real-time probe sessions using `NativeProbeBackend` where events
    /// stream into the session's durable `ExecutionLog` via the accepted-Raw
    /// seam (REC-C2.3 retired the parallel in-memory bus). Use `probe_drain`
    /// to read current events and `probe_stop` to finalize.
    live_probes: Arc<std::sync::Mutex<HashMap<String, LiveProbeSession>>>,
    /// Live browser probe sessions: session_id → BrowserProbeSession.
    /// These are real-time WASM debugging sessions via Chrome CDP.
    /// Use `browser_probe_drain` to read events and `browser_probe_stop` to finalize.
    live_browser_probes: Arc<std::sync::Mutex<HashMap<String, BrowserProbeSession>>>,
    /// Whether the underlying store is in-memory (degraded) instead of
    /// file-backed (persistent). m9-82 closes FIND-M9-75 by exposing this
    /// to tool callers via `is_degraded()` and through a top-level
    /// `degraded` field in the session-persistence tool envelopes
    /// (`save_session`, `list_sessions`, `load_session`, `delete_session`,
    /// `drop_session`). Set once at construction; immutable thereafter.
    degraded: bool,
    /// Active toolset profile. Controls which tools are listed and which
    /// language-specific tools are available. Set from `CHRONOS_ACTIVE_TOOLSET`
    /// env var at construction. Valid values: `auto`, `native`, `ebpf`,
    /// `python`, `java`, `go`, `js`, `minimal`.
    ///
    /// **Behavior on unset / unknown values (post-H1.4-B fix of the
    /// doc-vs-code drift previously tracked as
    /// `CAP-GAP-CHRONOS-MCP-TOOLSET-VALIDATION`):**
    ///
    /// - Unset → the field defaults to `"auto"` (`std::env::var::unwrap_or_else`).
    /// - Set to a valid value → the field stores that value verbatim.
    /// - Set to an unknown value (e.g. `"foo"`) → **the field stores
    ///   the unknown value verbatim** (NOT normalized to `"auto"` at
    ///   construction). At dispatch time, `is_tool_listed` normalizes
    ///   unknown values to the `"auto"` behavior (all tools listed) per
    ///   REQ-CAP-004. This is a two-layer design: the field preserves
    ///   operator intent for diagnostics (so a wrong value is visible in
    ///   `active_toolset()`), while the dispatch layer never silently
    ///   rejects the request — it falls back to the safest profile.
    ///
    /// **Why this design?** `auto` is the most permissive profile, so
    /// failing-open (treating unknown as auto) is the least surprising
    /// choice. Failing-closed (treating unknown as minimal) would
    /// silently disable tools that the operator may have been relying on.
    /// The trade-off is documented: a typo in `CHRONOS_ACTIVE_TOOLSET`
    /// gives the operator access to MORE tools than intended, not fewer.
    ///
    /// Pinned by `inv_5b_unknown_toolset_value_documented_as_passthrough`
    /// (field-level, §3) and the implicit `is_tool_listed` invariant
    /// (§3 fallback arm). See `docs/architecture/H1.4-chronos-server-
    /// cohesion-map.md` §3 INV-5.
    active_toolset: String,
    /// REC-C3.3.2.3 — composition-root uprobe capability injector.
    ///
    /// `chronos_services` cannot name the concrete `EbpfAdapter`; it
    /// receives the capability through this `Arc<dyn UprobeInjector>`
    /// and threads it into every `ProbeContext`. The injector must
    /// outlive every probe session, which is guaranteed because the
    /// server holds it as an `Arc` for its entire lifetime.
    uprobe_injector: Arc<dyn UprobeInjector>,
    /// REC-C3.5-residual-inversion R.3 — composition-root
    /// `NativeProbeControllerFactory` (audit §4.5 S4).
    ///
    /// `chronos_services::probe` cannot name the concrete
    /// `NativeProbeBackend`; it receives the construction capability
    /// through this `Arc<dyn NativeProbeControllerFactory>` and threads
    /// it into every `ProbeContext`. The factory must outlive every
    /// probe session, which is guaranteed because the server holds it
    /// as an `Arc` for its entire lifetime. Before R.3 the production
    /// edge `chronos-services -> chronos-native` was the composition
    /// leak; after R.3 it disappears.
    native_probe_factory: Arc<dyn chronos_domain::ports::NativeProbeControllerFactory>,
    /// REC-C3.3.2.4 — composition-root browser probe factory.
    ///
    /// `chronos_services::browser_probe` cannot name the concrete
    /// `BrowserAdapter`; it receives the capability through this
    /// `Arc<dyn BrowserProbeFactory>` and threads it into every
    /// `BrowserProbeContext`. The factory is stateless and cheap to
    /// share — every `create` call spawns a fresh backend.
    browser_probe_factory: Arc<dyn BrowserProbeFactory>,
}

impl ChronosServer {
    /// Build a server around the configured store, opening it first.
    ///
    /// This is the entrypoint every production caller should use: it reports a
    /// store that cannot be opened instead of degrading. See
    /// [`StoreOpenError`] for the policy and
    /// `CHRONOS_ALLOW_IN_MEMORY_FALLBACK` for the explicit opt-in.
    pub fn try_new() -> Result<Self, crate::init_error::ChronosServerInitError> {
        let store = Self::try_open_default_store()?;
        let server = Self::from_store(store);
        let root = chronos_log::resolve_execution_log_root();
        // C3.3.2 — composition root builds the factory once and wires
        // it into both the registry (already done in `from_store`)
        // and the bootstrap call. Services never name the concrete
        // factory type.
        let factory = crate::composition::default_execution_log_factory();
        chronos_services::execution_log_bootstrap::bootstrap_execution_logs(
            &root,
            &server.execution_logs,
            &factory,
        )
        .map_err(|cause| {
            crate::init_error::ChronosServerInitError::ExecutionLogBootstrap { root, cause }
        })?;
        Ok(server)
    }

    /// Infallible convenience wrapper around [`ChronosServer::try_new`].
    ///
    /// # Panics
    ///
    /// Panics with the [`StoreOpenError`] message when the configured store
    /// cannot be opened and the in-memory fallback is not enabled. Prefer
    /// [`ChronosServer::try_new`].
    pub fn new() -> Self {
        match Self::try_new() {
            Ok(server) => server,
            Err(e) => panic!("{e}"),
        }
    }

    /// Build a server around an explicitly provided store.
    fn from_store(store: SessionStore) -> Self {
        let degraded = !store.is_persistent();
        let active_toolset =
            std::env::var("CHRONOS_ACTIVE_TOOLSET").unwrap_or_else(|_| "auto".to_string());
        // C3.3.2 — wire the same factory the bootstrap call uses,
        // so every session-scoped log construction goes through the
        // composition root.
        let execution_log_factory = crate::composition::default_execution_log_factory();
        // The events reader (`events_read`) and the read path
        // (`read_path`) MUST share ONE registry: they are two views of
        // the same authoritative per-session ExecutionLog, not two
        // sources. Bind the Arc once, clone it into both fields.
        let execution_logs = Arc::new(
            chronos_services::session_log::SessionExecutionLogRegistry::with_factory(
                execution_log_factory,
            ),
        );
        // REC-C3.3.2.3 — uprobe capability injector is built once and
        // threaded into every probe call. `chronos_services` only sees
        // the trait object.
        let uprobe_injector = crate::composition::default_uprobe_injector();
        // REC-C3.3.2.4 — browser probe factory. Stateless and shared
        // across all probe sessions; each `create` produces a fresh
        // backend.
        let browser_probe_factory = crate::composition::default_browser_probe_factory();
        let store_arc = Arc::new(store);
        let archive = crate::composition::default_session_archive(store_arc.clone());
        let reader: Arc<dyn chronos_domain::ports::session_reader::SessionReader> = Arc::new(
            chronos_store::session_reader_adapter::SessionStoreBackedSessionReader::new(
                store_arc.clone(),
            ),
        );
        let diff_engine = crate::composition::default_diff_engine();
        let lifecycle_store: Arc<dyn chronos_domain::ports::lifecycle_store::LifecycleStore> =
            Arc::new(
                chronos_store::lifecycle_store_adapter::SessionStoreBackedLifecycleStore::new(
                    store_arc.clone(),
                ),
            );
        let counterexample_repository: Arc<
            dyn chronos_domain::ports::counterexample::CounterexampleRepository,
        > = crate::composition::default_counterexample_repository(store_arc.clone());
        Self {
            engines: Arc::new(Mutex::new(HashMap::new())),
            session_languages: Arc::new(Mutex::new(HashMap::new())),
            store: store_arc,
            reader,
            diff_engine,
            lifecycle_store,
            counterexample_repository,
            archive,
            connected_sessions: Arc::new(std::sync::Mutex::new(HashSet::new())),
            active_session: Arc::new(Mutex::new(None)),
            tripwire_manager: Arc::new(TripwireManager::new()),
            uprobe_counter: Arc::new(std::sync::Mutex::new(HashMap::new())),
            execution_logs: execution_logs.clone(),
            read_path: chronos_services::read_path::ReadPathService::new(execution_logs),
            execution_log_root: chronos_log::resolve_execution_log_root(),
            projection_meta: Arc::new(Mutex::new(HashMap::new())),
            live_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
            live_browser_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
            degraded,
            active_toolset,
            uprobe_injector,
            native_probe_factory: crate::composition::default_native_probe_controller_factory(),
            browser_probe_factory,
        }
    }

    /// Build a server around the default store with a forced `active_toolset`.
    ///
    /// Exists only to make tests deterministic — production code must not
    /// override the toolset via code; use `CHRONOS_ACTIVE_TOOLSET` instead.
    #[cfg(test)]
    pub fn with_toolset(toolset: &str) -> Self {
        match Self::try_open_default_store() {
            Ok(store) => {
                let store_arc = Arc::new(store);
                let archive = crate::composition::default_session_archive(store_arc.clone());
                let reader: Arc<dyn chronos_domain::ports::session_reader::SessionReader> =
                    Arc::new(
                        chronos_store::session_reader_adapter::SessionStoreBackedSessionReader::new(
                            store_arc.clone(),
                        ),
                    );
                let diff_engine = crate::composition::default_diff_engine();
                let lifecycle_store: Arc<
                    dyn chronos_domain::ports::lifecycle_store::LifecycleStore,
                > = Arc::new(
                    chronos_store::lifecycle_store_adapter::SessionStoreBackedLifecycleStore::new(
                        store_arc.clone(),
                    ),
                );
                let counterexample_repository: Arc<
                    dyn chronos_domain::ports::counterexample::CounterexampleRepository,
                > = crate::composition::default_counterexample_repository(store_arc.clone());
                let execution_logs =
                    Arc::new(chronos_services::session_log::SessionExecutionLogRegistry::new());
                Self {
                    engines: Arc::new(Mutex::new(HashMap::new())),
                    session_languages: Arc::new(Mutex::new(HashMap::new())),
                    store: store_arc,
                    reader,
                    diff_engine,
                    lifecycle_store,
                    counterexample_repository,
                    archive,
                    connected_sessions: Arc::new(std::sync::Mutex::new(HashSet::new())),
                    active_session: Arc::new(Mutex::new(None)),
                    tripwire_manager: Arc::new(TripwireManager::new()),
                    uprobe_counter: Arc::new(std::sync::Mutex::new(HashMap::new())),
                    execution_logs: execution_logs.clone(),
                    read_path: chronos_services::read_path::ReadPathService::new(execution_logs),
                    execution_log_root: chronos_log::resolve_execution_log_root(),
                    projection_meta: Arc::new(Mutex::new(HashMap::new())),
                    live_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
                    live_browser_probes: Arc::new(std::sync::Mutex::new(HashMap::new())),
                    degraded: false,
                    active_toolset: toolset.to_string(),
                    uprobe_injector: crate::composition::default_uprobe_injector(),
                    native_probe_factory:
                        crate::composition::default_native_probe_controller_factory(),
                    browser_probe_factory: crate::composition::default_browser_probe_factory(),
                }
            }
            Err(e) => panic!("{e}"),
        }
    }

    /// Handle to the session-scoped `ExecutionLog` registry built during
    /// `try_new` (`bootstrap_execution_logs`).
    ///
    /// Used by tooling that needs to confirm the registry is populated
    /// (RECs C1.5.5 — readiness invariant tests). This is the same data
    /// `events_read` consults at read time; making it readable to test
    /// helpers does not expose anything `events_read` does not already
    /// surface to MCP callers.
    pub fn execution_log_registry(
        &self,
    ) -> &std::sync::Arc<chronos_services::session_log::SessionExecutionLogRegistry> {
        &self.execution_logs
    }

    /// Whether the server is operating in degraded (in-memory, ephemeral)
    /// mode because the on-disk store could not be opened and the
    /// `CHRONOS_ALLOW_IN_MEMORY_FALLBACK` opt-in fired.
    ///
    /// m9-82: when this is `true`, the session-persistence tool envelopes
    /// (`save_session`, `list_sessions`, `load_session`, `delete_session`,
    /// `drop_session`) include `"degraded": true` at the top level so MCP
    /// callers can confirm the runtime is not persisting to disk.
    /// Closes FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE.
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Returns the active toolset profile (e.g. "auto", "native", "python").
    pub fn active_toolset(&self) -> &str {
        &self.active_toolset
    }

    /// Returns true if the given tool name is listed in the `tools/list`
    /// response for the current `active_toolset` profile.
    ///
    /// Fallback path for REQ-CAP-005: since rmcp does not expose a hook into
    /// `tools/list` dispatch, this method is called by tool handlers that need
    /// filtering (currently `evaluate_expression`). The `capabilities` tool
    /// response encodes the full availability map so callers can check before
    /// attempting a call.
    pub fn is_tool_listed(&self, tool_name: &str) -> bool {
        let listed = match self.active_toolset.as_str() {
            "auto" => true,
            "native" => NATIVE_TOOL_NAMES.contains(&tool_name),
            "ebpf" => EBPF_TOOL_NAMES.contains(&tool_name),
            "python" => PYTHON_TOOL_NAMES.contains(&tool_name),
            "java" => JAVA_TOOL_NAMES.contains(&tool_name),
            "go" => GO_TOOL_NAMES.contains(&tool_name),
            "js" => JS_TOOL_NAMES.contains(&tool_name),
            "minimal" => MINIMAL_TOOL_NAMES.contains(&tool_name),
            // Unrecognised → treat as auto (per REQ-CAP-004)
            _ => true,
        };
        listed
    }

    /// Single source of truth for the toolset-guard error message.
    ///
    /// Toolset filtering is delegated to `is_tool_listed`; the formatting of the
    /// rejection error lives here. Closes FIND-DEBT-001 (overeng) by replacing 10
    /// byte-identical inline blocks at dispatch handlers.
    ///
    /// Spec: REQ-CAP-008 `ToolsetGuardSingleSource`.
    fn toolset_guard(&self, tool_name: &str) -> Option<CallToolResult> {
        if self.is_tool_listed(tool_name) {
            None
        } else {
            Some(CallToolResult::error(text_content(format!(
                "{tool_name} is not available in the current active toolset (see CHRONOS_ACTIVE_TOOLSET)"
            ))))
        }
    }

    /// Build the per-tool availability map for the `capabilities` response.
    ///
    /// `target_language` is the optional target language from the caller.
    /// If `None`, all tools that are listed by the active toolset are marked
    /// available; language-specific tools are marked available with an
    /// `available: true` but `required_capabilities` filled in.
    pub fn build_tool_availability(
        &self,
        all_tool_names: &[&str],
        target_language: Option<&str>,
    ) -> std::collections::HashMap<String, chronos_services::output::ToolAvailability> {
        let mut map = std::collections::HashMap::new();
        for name in all_tool_names {
            let required = tool_required_capabilities(name);
            let listed = self.is_tool_listed(name);
            // Determine availability:
            // - If the tool is NOT listed in the current toolset → unavailable.
            // - If listed BUT requires a language runtime and target != that runtime
            //   → unavailable (only for non-auto toolsets; for auto, mark available
            //   with required_capabilities set so caller can see the requirement).
            let available = if listed {
                // For auto toolset: all listed tools are available (but requirements are noted).
                // For specific toolsets: also check language compatibility.
                if self.active_toolset == "auto" {
                    true
                } else {
                    // Specific toolset: language-specific tools are only available
                    // if the target language matches the required language.
                    if required.is_empty() {
                        true
                    } else {
                        // Tool requires a language runtime
                        if let Some(lang) = target_language {
                            required.iter().any(|r| r.contains(lang))
                        } else {
                            // No target specified: assume compatible
                            true
                        }
                    }
                }
            } else {
                false
            };

            let reason = if !listed {
                Some(format!(
                    "tool '{}' is not in the active toolset '{}'",
                    name, self.active_toolset
                ))
            } else if !available {
                let langs: Vec<_> = required
                    .iter()
                    .filter(|r| r.starts_with("language/"))
                    .map(|r| &r[9..])
                    .collect();
                if !langs.is_empty() {
                    let target = target_language.unwrap_or("(unspecified)");
                    Some(format!(
                        "'{}' requires a {} runtime; active toolset is '{}' but target is '{}'",
                        name,
                        langs.join(" or "),
                        self.active_toolset,
                        target
                    ))
                } else {
                    Some(format!(
                        "'{}' is not available in toolset '{}'",
                        name, self.active_toolset
                    ))
                }
            } else {
                None
            };

            map.insert(
                name.to_string(),
                chronos_services::output::ToolAvailability {
                    available,
                    required_capabilities: required,
                    reason_if_unavailable: reason,
                },
            );
        }
        map
    }

    /// Open the session store configured for this process.
    ///
    /// Path: `$CHRONOS_DB_PATH`, else `$HOME/.local/share/chronos/sessions.redb`.
    /// A failure to open is reported instead of being turned into an empty
    /// in-memory store; see [`StoreOpenError`].
    #[cfg(not(test))]
    fn try_open_default_store() -> Result<SessionStore, StoreOpenError> {
        let path = crate::composition::default_store_path(
            std::env::var("CHRONOS_DB_PATH").ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        );
        let allow_fallback = crate::composition::allow_in_memory_fallback(
            std::env::var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK")
                .ok()
                .as_deref(),
        );
        crate::composition::open_session_store_at(&path, allow_fallback)
    }

    /// Open the session store for unit tests.
    ///
    /// Tests are **hermetic**: they get a fresh in-memory store so the suite
    /// never reads from or writes to the developer's real `$HOME` store (that
    /// coupling previously made `test_list_sessions_after_save` depend on
    /// whatever happened to be in the local database — see
    /// FIND-M9-69-MCP-STORE-ISOLATION). A test that specifically needs a real
    /// file can still opt in by setting `CHRONOS_DB_PATH`, and a test that cannot
    /// open the store it asked for now fails instead of silently degrading
    /// (m9-75), unless it opts in through `CHRONOS_ALLOW_IN_MEMORY_FALLBACK`.
    #[cfg(test)]
    fn try_open_default_store() -> Result<SessionStore, StoreOpenError> {
        match std::env::var("CHRONOS_DB_PATH") {
            Ok(p) => {
                let allow_fallback = crate::composition::allow_in_memory_fallback(
                    std::env::var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK")
                        .ok()
                        .as_deref(),
                );
                crate::composition::open_session_store_at(std::path::Path::new(&p), allow_fallback)
            }
            Err(_) => SessionStore::in_memory().map_err(|e| StoreOpenError {
                path: std::path::PathBuf::from(":memory:"),
                cause: Box::new(e),
            }),
        }
    }

    /// Inject a `QueryEngine` into the server's engine map for testing.
    ///
    /// **`#[cfg(test)]` limits this to this crate's own unit tests.** An
    /// earlier version of this doc claimed the method existed "only to
    /// support integration tests in `tests/debug_read_tools.rs`", which is
    /// impossible: `cfg(test)` is not set when the crate is compiled as a
    /// dependency of an integration test, so the method does not exist from
    /// `tests/`. That file never called it, and neither does anything else —
    /// this currently has no callers.
    ///
    /// It stays because the seam is legitimate and the map it writes is
    /// otherwise unreachable from a test. If an integration test ever needs
    /// it, the repo already has two working precedents for widening a
    /// production symbol for tests, and either is the shape to follow:
    ///
    /// - `chronos-services`'s `test_support` module, gated on
    ///   `#[cfg(any(test, feature = "test-utils"))]` with the feature
    ///   taken as a `[dev-dependencies]` edge by the consumer crate,
    ///   which is how the unit tests in this file and
    ///   `chronos-mcp/tests/bootstrap_readiness.rs` reach it. It is a
    ///   dev-dependency, not a normal one, so the production binary
    ///   does not compile that module.
    /// - `chronos-store`'s `ce_test_hooks`, a `#[doc(hidden)] pub` surface
    ///   documented as a narrow, versioned test chokepoint.
    ///
    /// Do not remove the `#[cfg(test)]` to make an integration test compile;
    /// that widens the production API to accommodate the test.
    #[cfg(test)]
    pub async fn inject_engine_for_testing(
        &self,
        session_id: &str,
        engine: chronos_query::QueryEngine,
    ) {
        self.engines
            .lock()
            .await
            .insert(session_id.to_string(), engine);
    }

    /// Remove all in-memory state for a session: query engine, language tag,
    /// projection metadata, and connected-session marker.
    async fn cleanup_session_memory(&self, session_id: &str) {
        // REC-C1.3: drop/delete ends the log's in-memory life. Segment files are
        // NOT removed here; retention policy belongs to REC-C1.5.
        self.execution_logs.remove(session_id);
        self.engines.lock().await.remove(session_id);
        self.session_languages.lock().await.remove(session_id);
        // REC-C1.7: `projection_meta` is populated by `ensure_projection` and
        // by nothing else, so omitting it here made the map insert-only: every
        // session recovered through `load_session`, the `list_sessions`
        // bootstrap or any stored-session recovery left an entry that outlived
        // the session itself, and the map grew monotonically for the life of
        // the process. Evicting is safe because `ensure_projection` treats a
        // cache miss as the normal path and rebuilds from the canonical
        // `SessionExecutionLog`.
        self.projection_meta.lock().await.remove(session_id);
        if let Ok(mut sessions) = self.connected_sessions.lock() {
            sessions.remove(session_id);
        }
    }

    /// REC-C1.7: ensure a QueryEngine projection exists for the session,
    /// built from the canonical SessionExecutionLog.
    ///
    /// This is the **only** code path that may insert into `engines` or
    /// `projection_meta` for a session that was opened via `load_session`,
    /// `list_sessions` bootstrap, or any other stored-session recovery.
    /// Live capture paths (`probe_stop`, `session_snapshot`) still drain
    /// via `build_and_store_engine` because they receive events from the
    /// ring buffer rather than from a SessionExecutionLog; they emit a
    /// projection at the end of the drain so subsequent queries hit the
    /// gate. REC-C1.7 closes TRUTH-001 by making QueryEngine a
    /// reconstructible projection of the log, not a second authority.
    ///
    /// Returns the projection meta. If the session has a registered
    /// SessionExecutionLog, builds from it. If the session is already
    /// projected, returns the cached meta. If the session has neither,
    /// returns `SessionNotFound` (callers should treat this the same as
    /// a missing session).
    async fn ensure_projection(&self, session_id: &str) -> Result<ProjectionMeta, ServiceError> {
        // Fast path: already projected.
        if let Some(meta) = self.projection_meta.lock().await.get(session_id).cloned() {
            return Ok(meta);
        }

        // Look up the canonical log.
        let log = self.execution_logs.get(session_id)?;

        // Build the projection (filters registers/unknowns, decodes via
        // shared helper, walks segments via events_log_read::decode).
        let result = projection::build_engine(&log)?;
        let engine = result.engine;
        let meta = result.meta;

        // Insert atomically. Both maps are tokio mutexes, and both are taken
        // here in the same order every time, so this cannot deadlock against
        // itself.
        //
        // What this does NOT establish is that the two maps are always read
        // together. They are not. Only four MCP handlers consult
        // `projection_meta` before touching `engines`: `execution_query`,
        // `state_query`, `trace_slice`, and the Causality arm of
        // `execution_log_read`. Roughly thirty other handlers, and every
        // `chronos-services` reader that takes an engines map
        // (SessionsService, QueryService, AnalysisService, the debug and
        // export paths), read `engines` without ever consulting the meta.
        // Verify the current count with
        // `grep -n "gate_projection" crates/chronos-mcp/src/server.rs`
        // rather than trusting a number frozen into a comment.
        //
        // An earlier version of this comment claimed the opposite — "no other
        // code path reads one without the other (gate + service both look up
        // projection_meta first, then engines)" — and that was false, so it
        // was removed rather than softened. Recording the asymmetry here is
        // the point: an uncertified engine is reachable, and `save_session`
        // in particular persists one into durable storage without gating it.
        // Tracked as open debt under REC-C1.9, not fixed.
        let mut engines = self.engines.lock().await;
        let mut metas = self.projection_meta.lock().await;
        // Double-check after acquiring the locks: another caller may have
        // raced and built the projection in the meantime.
        if let Some(existing) = metas.get(session_id).cloned() {
            return Ok(existing);
        }
        engines.insert(session_id.to_string(), engine);
        metas.insert(session_id.to_string(), meta.clone());

        info!(
            "Built projection for session {} (completeness: {:?})",
            session_id, meta.completeness
        );

        Ok(meta)
    }

    /// REC-C1.7: gate a query by projection meta. Returns the meta iff the
    /// projection is readable; otherwise returns the appropriate error.
    /// This is the wrapper-side gate that keeps the dual-truth divergence
    /// closed. The services themselves do not gate — they accept any
    /// session_id so existing tests can call them directly. Only the MCP
    /// wire enforces the projection invariant.
    ///
    /// Two error cases, both fail-closed:
    /// - no `ExecutionLog` for the session -> `ExecutionLogUnavailable`, which
    ///   names the session and says why. Not `SessionNotFound`: a session
    ///   loaded from the session store exists but has no log yet, so calling
    ///   it "not found" would be false.
    /// - a `Truncated` projection -> `EvidenceUnavailableDueToRetention`,
    ///   because the engine would otherwise answer as if retired history had
    ///   never existed. An `Empty` projection is accepted: a session with no
    ///   records has no history to be missing.
    ///
    /// `gate_projection_for_wire` wraps either into a single
    /// `internal_error` whose message carries the variant, so the distinction
    /// reaches the caller as text rather than as a separate JSON-RPC code.
    async fn gate_projection(&self, session_id: &str) -> Result<ProjectionMeta, ServiceError> {
        // If the session is not in projection_meta yet, run the
        // projection build (covers load_session and bootstrap). If the
        // session has no log, the registry answers ExecutionLogUnavailable,
        // which the wire surfaces as isError:true.
        let meta = self.ensure_projection(session_id).await?;
        projection::meta_is_full(&meta).map(|()| meta)
    }

    /// REC-C1.7: gate a query by projection meta, returning an
    /// `ErrorData` (the rmcp wire error type) on failure so callers can
    /// use `?` directly. Three canonical operations (execution_query,
    /// state_query, trace_slice) call this at the very top of the
    /// handler so the wire never sees a query dispatched against an
    /// unprojected or truncated session.
    async fn gate_projection_for_wire(
        &self,
        session_id: &str,
    ) -> Result<ProjectionMeta, rmcp::ErrorData> {
        match self.gate_projection(session_id).await {
            Ok(meta) => Ok(meta),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("projection gate failed: {}", e),
                None,
            )),
        }
    }

    async fn build_and_store_engine(
        &self,
        session_id: &str,
        events: Vec<TraceEvent>,
        language: Language,
    ) {
        // Filter out internal/noisy events before indexing:
        // - EventType::Custom with EventData::Registers → ptrace register snapshots (infrastructure noise)
        // - EventType::Unknown → unclassified ptrace stops
        // These are implementation details of the tracer, not meaningful for AI
        // analysis. The rule itself lives in `chronos_services::projection`, which
        // `build_engine` uses, so a server-built engine and a log-derived
        // projection index the same events. This used to be a byte-identical copy
        // of that predicate, which meant the two engines could silently diverge
        // whenever the noise set grew on one side only.
        let events: Vec<TraceEvent> = events
            .into_iter()
            .filter(|e| !projection::is_noisy(e))
            .collect();

        let mut engines = self.engines.lock().await;
        let mut session_languages = self.session_languages.lock().await;
        // REC-C1.7 mirror invariant, third direction. `projection_meta` is
        // the gate's cache and its entry *is* the claim "this session's
        // `engines` slot is `projection::build_engine(&log)`" — `Full`
        // asserts verbatim equality and `projected_through` asserts the last
        // seq the projection actually read (projection.rs:48-49, 64-65).
        // This function never consulted it, so a drain over a session a
        // query had already published mutated the certified engine in place
        // while the surviving meta kept certifying it.
        //
        // The evidence says invalidate, do not mutate, and it says so for a
        // reason that is not "merging is expensive": every native drain
        // reads its events back out of the very log the projection came
        // from (`canonical_drain::read_all_raw_events`, reached via
        // `ProbeService::stop`/`session_snapshot`), so a drain cannot
        // introduce evidence the log lacks and there is nothing to rescue by
        // merging. What it can do is make the engine something
        // `build_engine(&log)` never produced: `merge` is the *id-union* of
        // engine and drain (engine.rs:207-213), while `build_engine` is the
        // log's record list (projection.rs:138-144, no dedup), and `merge`
        // also appends, so it can reorder and can silently drop a repeated
        // `event_id`. `ensure_projection` treats a missing meta as its
        // normal path (see the fast path above) and rebuilds from the
        // canonical log, so dropping the entry makes the next gated query
        // honest instead of freezing a meta whose `projected_through` is
        // already behind the log's tail.
        //
        // Locked in the same order as `ensure_projection` (`engines`, then
        // `projection_meta`) so the check and the decision cannot race a
        // projection that is being published concurrently.
        //
        // The engine itself is deliberately left in the map. It is the
        // capture path's own state — `SessionsService::save_session` reads
        // this slot directly with no gate (sessions.rs:77-82) — and a
        // reader that cannot use it must not destroy it. The next
        // `ensure_projection` overwrites it.
        let published_projection = self
            .projection_meta
            .lock()
            .await
            .remove(session_id)
            .is_some();

        if let Some(existing) = engines.get_mut(session_id) {
            if published_projection {
                info!(
                    "Drain for session {} invalidated its published projection \
                     instead of merging into the certified engine (engine still \
                     holds {} events); the log is authoritative and already \
                     holds everything the drain delivered, so the next gated \
                     query rebuilds from it",
                    session_id,
                    existing.event_count()
                );
            } else {
                // Cumulative refresh: merge new events into the existing
                // engine and rebuild indices.
                // m0-02-make-snapshots-cumulative (UAT-M0-02). This is the
                // pure live-capture shape — no log backs the engine, so
                // there is no projection to invalidate and the cumulative
                // merge is the only way the session becomes queryable.
                existing.merge(events);
                info!(
                    "Refreshed query engine for session {} (cumulative, now {} events)",
                    session_id,
                    existing.event_count()
                );
            }
        } else {
            // First snapshot: build from scratch.
            let mut builder = IndexBuilder::new();
            builder.push_all(&events);
            let indices = builder.finalize();

            let engine = QueryEngine::with_indices(events, indices.shadow, indices.temporal)
                .with_causality(indices.causality)
                .with_performance(indices.performance);

            info!(
                "Built query engine for session {} (language: {:?})",
                session_id, language
            );
            engines.insert(session_id.to_string(), engine);
        }
        session_languages.insert(session_id.to_string(), language);

        // Set this session as the active session
        drop(engines);
        drop(session_languages);
        self.set_active_session(session_id).await;
    }

    /// Set the active session after capture or load.
    async fn set_active_session(&self, session_id: &str) {
        let mut active = self.active_session.lock().await;
        *active = Some(session_id.to_string());
    }

    /// Run the server on stdio.
    pub async fn run_stdio(self) -> Result<(), Box<dyn std::error::Error>> {
        use rmcp::ServiceExt;
        let server = Arc::new(self);
        // m1-08: spawn the auto-compaction daemon alongside the
        // MCP service. The daemon periodically walks `live_probes`
        // and calls `maybe_compact()` on each attached ExecutionLog,
        // so operators don't need to wire a separate scheduler.
        // `waiting()` is what blocks until the service shuts down;
        // we tie the daemon to that with a shutdown signal so they
        // die together.
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let daemon_handle = tokio::spawn(Self::auto_compaction_daemon(server.clone(), shutdown_rx));
        let transport = (tokio::io::stdin(), tokio::io::stdout());
        let service = server.serve(transport).await?;
        info!("Chronos MCP server started on stdio");
        let result = service.waiting().await;
        // Signal the daemon to stop and let it drain.
        let _ = shutdown_tx.send(true);
        // daemon_handle.await returns Result<(), JoinError>;
        // we don't care about the daemon's exit status here
        // (it returns Ok on graceful shutdown, Err only if it
        // panicked — which we want to propagate).
        let daemon_result = daemon_handle.await;
        result?;
        daemon_result
            .map_err(|e| Box::<dyn std::error::Error>::from(format!("daemon panicked: {}", e)))?;
        Ok(())
    }

    /// Background task that periodically walks every live probe
    /// session and calls `maybe_compact()` on its attached
    /// ExecutionLog (m1-08).
    ///
    /// Interval comes from `CHRONOS_AUTO_COMPACT_INTERVAL_SECS`
    /// (default 30 s). Setting the env var to `0` disables the
    /// daemon entirely — useful for tests.
    ///
    /// Errors from `maybe_compact()` are logged at `warn` level and
    /// swallowed; compaction is best-effort and the next tick will
    /// retry.
    async fn auto_compaction_daemon(
        server: Arc<Self>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let interval_secs: u64 = std::env::var("CHRONOS_AUTO_COMPACT_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);
        if interval_secs == 0 {
            info!("auto-compaction daemon disabled (CHRONOS_AUTO_COMPACT_INTERVAL_SECS=0)");
            return;
        }
        info!(
            "auto-compaction daemon started (interval = {}s)",
            interval_secs
        );
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        // Skip the first immediate tick — the daemon shouldn't fire
        // before the server has even accepted a single probe.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // consume the initial 0-time tick
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("auto-compaction daemon shutting down");
                        return;
                    }
                }
                _ = ticker.tick() => {
                    Self::run_one_compaction_round(&server).await;
                }
            }
        }
    }

    /// Single pass over live_probes; one `maybe_compact()` call per
    /// backend that has an attached log. Logs cumulative metrics
    /// before and after so the operator can see reclaimed space.
    ///
    /// Holds `live_probes` for the entire pass — `maybe_compact()`
    /// is a short call (it operates on the backend's own log, not
    /// the live_probes map), and we want a consistent snapshot.
    /// The mutex is std::sync, never held across .await, so this
    /// does not block the async runtime.
    async fn run_one_compaction_round(server: &Arc<Self>) {
        let probes = server.live_probes.lock().unwrap();
        for (session_id, live_probe) in probes.iter() {
            // REC-C3.3.2.5 — compaction runs through the maintenance
            // port's `compact_retired`. Counter snapshots come from
            // the same port. The port only exposes the conservative
            // counter subset (segments_reclaimed, compaction_passes,
            // no_op_passes, highest_seq_observed), so the per-run
            // delta math drops bytes_reclaimed (the underlying
            // segmented backend's filesystem metric is not part of
            // the application-shape contract).
            let before = match live_probe.execution_log.compaction_metrics() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let report = match live_probe.execution_log.compact_retired() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let removed: Vec<std::path::PathBuf> = report
                .reclaimed_paths
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect();
            if !removed.is_empty() {
                let after = report.metrics;
                info!(
                    "auto-compact: session={} removed {} segment(s); \
                     cumulative segments_reclaimed={} compaction_passes={} \
                     no_op_passes={} highest_seq={:?}",
                    session_id,
                    removed.len(),
                    after.segments_reclaimed,
                    after.compaction_passes,
                    after.no_op_passes,
                    after.highest_seq_observed,
                );
                let _ = before;
            }
        }
    }
}

impl Default for ChronosServer {
    fn default() -> Self {
        Self::new()
    }
}
// ============================================================================
// Tool handlers using rmcp macros
// ============================================================================

#[rmcp::tool_router(vis = "pub")]
impl ChronosServer {
    #[tool(
        name = "execution_query",
        description = "v2 dispatcher for execution/call/performance projection queries. Select the query kind via `kind` (call_stack | execution_summary | call_graph | race_detect | hotspot | saliency). Each kind has its own required/optional fields; see docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md."
    )]
    async fn execution_query(
        &self,
        params: Parameters<ExecutionQueryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // REC-C1.7: gate by projection meta before constructing the
        // service context. This closes TRUTH-001: the QueryEngine
        // serving this query is a reconstructible projection of the
        // SessionExecutionLog, not a second authority.
        let _meta = self.gate_projection_for_wire(&params.session_id).await?;

        let ctx = ExecutionQueryContext {
            engines: &self.engines,
            projection_meta: &self.projection_meta,
        };
        let input = ExecutionQueryInput {
            session_id: params.session_id,
            kind: params.kind,
            event_id: params.event_id,
            max_depth: params.max_depth,
            threshold_ns: params.threshold_ns,
            top_n: params.top_n,
            saliency_limit: params.saliency_limit,
        };

        match ChronosExecutionQueryService::query(&ctx, input).await {
            Ok(out) => {
                // A serialization failure is a fault, not an empty result.
                // `unwrap_or(json!({}))` answered `success({})`, which reads
                // downstream as "the query found nothing" when in fact the
                // answer could not be rendered. Both output types carry `f64`
                // fields (output.rs), and a non-finite one is enough to make
                // serde refuse. The other handlers in this file already use
                // the `map_err(..)?` form.
                let value = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("query serialize: {e}"), None)
                })?;
                Ok(CallToolResult::success(json_content(&value)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "state_query",
        description = "v2 dispatcher for state transition/value evidence queries. Select the query kind via `kind` (register_diff | memory_read | register_snapshot | memory_analysis | expression_eval). Each kind has its own required fields; see `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md`. DATA HANDLING: `kind=memory_read` returns `data` (raw bytes) and `hex` for the address and timestamp requested, and `kind=memory_analysis` returns per-access `data_hex`; both carry captured process memory verbatim and unredacted, limited to bytes the capture configuration already recorded. The other kinds do not return memory contents. Treat those two responses as sensitive: they can leave the machine through the model context."
    )]
    async fn state_query(
        &self,
        params: Parameters<StateQueryParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // REC-C1.7: gate by projection meta (see execution_query above).
        let _meta = self.gate_projection_for_wire(&params.session_id).await?;

        let ctx = StateQueryContext {
            engines: &self.engines,
            projection_meta: &self.projection_meta,
        };
        let input = StateQueryInput {
            session_id: params.session_id,
            kind: params.kind,
            timestamp_a: params.timestamp_a,
            timestamp_b: params.timestamp_b,
            event_id: params.event_id,
            address: params.address,
            timestamp_ns: params.timestamp_ns,
            start_address: params.start_address,
            end_address: params.end_address,
            start_ts: params.start_ts,
            end_ts: params.end_ts,
            expression: params.expression,
        };

        match ChronosStateQueryService::query(&ctx, input).await {
            Ok(out) => {
                // A serialization failure is a fault, not an empty result.
                // `unwrap_or(json!({}))` answered `success({})`, which reads
                // downstream as "the query found nothing" when in fact the
                // answer could not be rendered. Both output types carry `f64`
                // fields (output.rs), and a non-finite one is enough to make
                // serde refuse. The other handlers in this file already use
                // the `map_err(..)?` form.
                let value = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("query serialize: {e}"), None)
                })?;
                Ok(CallToolResult::success(json_content(&value)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected error: {}",
                e
            )))),
        }
    }

    #[tool(
        name = "list_threads",
        description = "List all thread IDs in the trace."
    )]
    async fn list_threads(
        &self,
        params: Parameters<ListThreadsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let threads = match QueryService::list_threads(&params.session_id, &self.engines).await {
            Ok(t) => t,
            Err(ServiceError::SessionNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found",
                    params.session_id
                ))));
            }
            Err(ServiceError::LockPoisoned) => {
                return Ok(CallToolResult::error(text_content("lock poisoned")));
            }
            // REC-C2.1.4a: cannot occur from list_threads, but the enum gained
            // a variant and the match is exhaustive.
            Err(ServiceError::NoActiveSession) => {
                return Ok(CallToolResult::error(text_content(
                    "no active session: supply scope=session{session_id} or start a session",
                )));
            }
            // The new ServiceError variants cannot occur from list_threads,
            // but must be listed for exhaustiveness.
            Err(ServiceError::MemoryNotFound { .. })
            | Err(ServiceError::UnsatisfiableCondition(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected memory error",
                )));
            }
            // Session integrity / comparison cannot occur from list_threads,
            // but the enum gained two variants and the match must remain
            // exhaustive. They are answered on their own tool instead.
            Err(ServiceError::SessionIncomplete { .. })
            | Err(ServiceError::NothingToCompare { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected session comparison error",
                )));
            }
            // REC-C3.3.3 (Tren B slice G): SessionRunning/SessionStopped
            // cannot occur from list_threads, but the enum gained two
            // variants and the match must remain exhaustive.
            Err(ServiceError::SessionRunning(_)) | Err(ServiceError::SessionStopped(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe-state error",
                )));
            }
            // REC-C3.3.2.5: retention errors cannot occur from
            // list_threads either; listed for exhaustiveness.
            Err(ServiceError::RetentionBackwardsMove { .. })
            | Err(ServiceError::RetentionPastAllocated { .. })
            | Err(ServiceError::RetentionSealed { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected retention error",
                )));
            }
            Err(ServiceError::EventNotFound { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected event error",
                )));
            }
            Err(ServiceError::NoRegisterState { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected register error",
                )));
            }
            Err(ServiceError::EvalError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eval error",
                )));
            }
            Err(ServiceError::QueryExecutionError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected query error",
                )));
            }
            // New session-lifecycle error variants (cannot occur from list_threads
            // but must be listed for exhaustiveness).
            Err(ServiceError::SessionNotInMemory(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected session error",
                )));
            }
            Err(ServiceError::EmptySession(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected empty session",
                )));
            }
            Err(ServiceError::SaveFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected save error",
                )));
            }
            Err(ServiceError::LoadFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected load error",
                )));
            }
            Err(ServiceError::ListFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected list error",
                )));
            }
            Err(ServiceError::DeleteFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected delete error",
                )));
            }
            // New tripwire error variants (cannot occur from list_threads).
            Err(ServiceError::InvalidCondition(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected condition error",
                )));
            }
            Err(ServiceError::InvalidTripwireIdFormat(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire ID format error",
                )));
            }
            Err(ServiceError::TripwireNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected tripwire not found",
                )));
            }
            // REC-C1.2/C1.2a/C1.3 variants (cannot occur from list_threads).
            Err(ServiceError::NoExecutionLog(_))
            | Err(ServiceError::ExecutionLogIdentityMismatch { .. })
            | Err(ServiceError::EvidenceDecodeFailed { .. })
            | Err(ServiceError::EvidenceReadStalled { .. })
            | Err(ServiceError::ExecutionLogUnavailable { .. })
            | Err(ServiceError::EvidenceUnavailableDueToRetention { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected ExecutionLog error",
                )));
            }
            Err(ServiceError::Unsupported(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected unsupported error",
                )));
            }
            // Probe service variants cannot be produced by query_events but are
            // listed for exhaustiveness against the ServiceError enum.
            Err(ServiceError::InvalidProgramPath(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe program path error",
                )));
            }
            Err(ServiceError::ProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe not found",
                )));
            }
            Err(ServiceError::ProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe start failure",
                )));
            }
            Err(ServiceError::ProbeStopError(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe stop error",
                )));
            }
            Err(ServiceError::InvalidCursorPayload) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid cursor",
                )));
            }
            Err(ServiceError::CursorStale { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected cursor stale",
                )));
            }
            Err(ServiceError::SessionStillActive { .. }) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected still-active session",
                )));
            }
            Err(ServiceError::DrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected drain failure",
                )));
            }
            Err(ServiceError::ProbeStarting) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected probe starting",
                )));
            }
            Err(ServiceError::EbpfUnsupported(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected eBPF unsupported",
                )));
            }
            Err(ServiceError::InjectionFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected injection failure",
                )));
            }
            // Browser probe service variants cannot be produced by
            // this call site but are listed for exhaustiveness against
            // the ServiceError enum.
            Err(ServiceError::ChromeUnavailable) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected Chrome unavailable",
                )));
            }
            Err(ServiceError::BrowserProbeNotFound(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe not found",
                )));
            }
            Err(ServiceError::BrowserProbeStartFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe start failure",
                )));
            }
            Err(ServiceError::BrowserProbeDrainFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected browser probe drain failure",
                )));
            }
            Err(ServiceError::InvalidInput(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid input",
                )));
            }
            // m6-05: session_export variants cannot be produced by this
            // call site but are listed for exhaustiveness against the
            // ServiceError enum.
            Err(ServiceError::ExportFailed(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected export failure",
                )));
            }
            Err(ServiceError::InvalidExportParameter(_)) => {
                return Ok(CallToolResult::error(text_content(
                    "internal error: unexpected invalid export parameter",
                )));
            }
            // m9-77: AttachFailed cannot be produced by this call site
            // (list_threads does not invoke the attach path) but is
            // listed for exhaustiveness against the ServiceError enum.
            Err(ServiceError::AttachFailed(msg)) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "internal error: unexpected attach failure: {}",
                    msg
                ))));
            }
        };

        let output = serde_json::json!({
            "session_id": params.session_id,
            "thread_count": threads.len(),
            "thread_ids": threads,
        });
        Ok(CallToolResult::success(json_content(&output)))
    }

    // ========================================================================
    // SF4 — Semantic Compression + Advanced Tools (T17–T23)
    // ========================================================================

    // ========================================================================
    // SF5 — Persistence Tools (T10–T14)
    // ========================================================================

    #[tool(
        name = "save_session",
        description = "Save an in-memory session to persistent storage. Saves the session's events to the CAS store and records metadata. Returns hash count and dedup statistics."
    )]
    async fn save_session(
        &self,
        params: Parameters<SaveSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };

        match SessionsService::save_session(
            &params.session_id,
            Language::from_string(&params.language),
            params.target.clone(),
            &ctx,
        )
        .await
        {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "saved",
                    "event_count": result.event_count,
                    "hash_count": result.hash_count,
                    "language": result.language,
                    "target": result.target,
                    "duration_ms": result.duration_ms,
                    "hint": "Use load_session to reload this session, or list_sessions to see all saved sessions.",
                });
                Ok(CallToolResult::success(json_content(&session_envelope(
                    self.degraded,
                    output,
                ))))
            }
            Err(ServiceError::SessionNotInMemory(s)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Session '{}' not found in memory. Run probe_start first.",
                    s
                ))))
            }
            Err(ServiceError::EmptySession(_)) => Ok(CallToolResult::error(text_content(
                "Session has no events to save.".to_string(),
            ))),
            Err(ServiceError::SaveFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to save session: {}",
                e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "load_session",
        description = "Load a session from persistent storage into a new in-memory query engine. Returns metadata and event count."
    )]
    async fn load_session(
        &self,
        params: Parameters<LoadSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };

        match SessionsService::load_session(&params.session_id, &ctx).await {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "loaded",
                    "language": result.language,
                    "target": result.target,
                    "event_count": result.event_count,
                    "duration_ms": result.duration_ms,
                    "created_at": result.created_at,
                    "hint": "Session is now queryable. Use query_events, get_execution_summary, etc.",
                });
                Ok(CallToolResult::success(json_content(&session_envelope(
                    self.degraded,
                    output,
                ))))
            }
            Err(ServiceError::LoadFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to load session '{}': {}",
                params.session_id, e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "list_sessions",
        description = "List all saved sessions from persistent storage. Returns metadata for each session (no event data)."
    )]
    async fn list_sessions(
        &self,
        _params: Parameters<NoParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };

        match SessionsService::list_sessions(&ctx).await {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_count": result.sessions.len(),
                    "sessions": result.sessions.iter().map(|s| serde_json::json!({
                        "session_id": s.session_id,
                        "language": s.language,
                        "target": s.target,
                        "event_count": s.event_count,
                        "duration_ms": s.duration_ms,
                        "created_at": s.created_at,
                    })).collect::<Vec<_>>(),
                });
                Ok(CallToolResult::success(json_content(&session_envelope(
                    self.degraded,
                    output,
                ))))
            }
            Err(ServiceError::ListFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to list sessions: {}",
                e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "delete_session",
        description = "Delete a session from persistent storage. Does not affect in-memory sessions."
    )]
    async fn delete_session(
        &self,
        params: Parameters<DeleteSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };

        match SessionsService::delete_session(&params.session_id, &ctx).await {
            Ok(result) => {
                // Also purge all in-memory state for this session.
                self.cleanup_session_memory(&params.session_id).await;

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "deleted",
                    "paths_removed": result.paths_removed,
                    "message": format!("Session '{}' deleted from persistent storage and memory.", params.session_id),
                });
                Ok(CallToolResult::success(json_content(&session_envelope(
                    self.degraded,
                    output,
                ))))
            }
            Err(e @ ServiceError::SessionStillActive { .. }) => {
                // REC-C1.6: refuse to delete a still-live session. The service
                // raised this BEFORE any side effect, so the durable dir and
                // the store row are intact; we do not run cleanup_session_memory
                // here because the session is still alive. The Display impl
                // already names the recovery action (session_stop).
                Ok(CallToolResult::error(text_content(e.to_string())))
            }
            Err(ServiceError::DeleteFailed(e)) => Ok(CallToolResult::error(text_content(format!(
                "Failed to delete session '{}': {}",
                params.session_id, e
            )))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "drop_session",
        description = "Remove a session from in-memory state WITHOUT touching persistent storage. Complement to delete_session which removes from store. Returns success even if session was not found in memory (idempotent)."
    )]
    async fn drop_session(
        &self,
        params: Parameters<DropSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };

        match SessionsService::drop_session(&params.session_id, &ctx).await {
            Ok(result) => {
                // Clean up all in-memory state for this session.
                self.cleanup_session_memory(&params.session_id).await;

                if result.existed {
                    let output = serde_json::json!({
                        "session_id": params.session_id,
                        "status": "dropped",
                        "message": "Session removed from memory. Persistent storage not affected.",
                    });
                    Ok(CallToolResult::success(json_content(&session_envelope(
                        self.degraded,
                        output,
                    ))))
                } else {
                    let output = serde_json::json!({
                        "session_id": params.session_id,
                        "status": "not_found",
                        "message": "Session not found in memory. No action taken.",
                    });
                    Ok(CallToolResult::success(json_content(&session_envelope(
                        self.degraded,
                        output,
                    ))))
                }
            }
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // SF6 — Inspection Tools (T5–T7)
    // ========================================================================

    #[tool(
        name = "debug_get_variables",
        description = "Get all variables in scope at a specific event. Returns locals from Python/Java/Go frame events or VariableWrite events."
    )]
    async fn debug_get_variables(
        &self,
        params: Parameters<DebugGetVariablesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(err) = self.toolset_guard("debug_get_variables") {
            return Ok(err);
        }
        let params = params.0;

        let result =
            DebugReadService::get_variables(&params.session_id, params.event_id, &self.engines)
                .await;

        match result {
            Ok(vars) => {
                let output = serde_json::json!({
                    "event_id": params.event_id,
                    "variables": vars,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // SF7 — Phase 11 Missing Tools (T20–T24)
    // ========================================================================

    #[tool(
        name = "debug_diff",
        description = "Compare process state between two event_ids — variables, registers, memory."
    )]
    async fn debug_diff(
        &self,
        params: Parameters<DebugDiffParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(err) = self.toolset_guard("debug_diff") {
            return Ok(err);
        }
        let params = params.0;

        let result = DebugReadService::diff(
            &params.session_id,
            params.event_id_a,
            params.event_id_b,
            &self.engines,
        )
        .await;

        match result {
            Ok(snap) => {
                // Convert RegisterChange back to the existing JSON Map shape
                let mut registers_changed = serde_json::Map::new();
                for (name, change) in snap.registers_changed {
                    registers_changed.insert(
                        name,
                        serde_json::json!({
                            "before": change.before,
                            "after": change.after,
                        }),
                    );
                }

                let output = serde_json::json!({
                    "event_id_a": snap.event_id_a,
                    "event_id_b": snap.event_id_b,
                    "variables_added": snap.variables_added,
                    "variables_removed": snap.variables_removed,
                    "variables_changed": snap.variables_changed.iter().map(|vc| {
                        serde_json::json!({
                            "name": vc.name,
                            "before": vc.before,
                            "after": vc.after,
                        })
                    }).collect::<Vec<_>>(),
                    "registers_changed": registers_changed,
                    "timestamp_delta_ns": snap.timestamp_delta_ns,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "lock poisoned".to_string(),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // SF8 — Tripwire Tools (T21–T23)
    // ========================================================================

    // ========================================================================
    // SF9 — Live Probe Tools
    // ========================================================================

    #[tool(
        name = "capture_session",
        description = "REC-C3.3.3 (Tren B slice F): single-shot capture. Spawns the target under a live probe, waits for the target to exit on its own, stops the probe, and persists the session (metadata + events) via the SessionArchive port. Returns the session id and a `saved: true` acknowledgement. Non-zero target exit is NOT a capture failure. Use load_session to re-query the captured session."
    )]
    async fn capture_session(
        &self,
        params: Parameters<CaptureSessionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Security gate, identical to probe_start.
        if let Err(e) = crate::security::validate_program_path(&params.program) {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid program path: {}",
                e
            ))));
        }

        // ---- Step 1: start the live probe (mirrors probe_start's spawn path).
        let v2_input = chronos_services::output::SessionStartInput {
            action: chronos_services::output::SessionStartAction::Spawn,
            spawn_fields: Some(chronos_services::output::SessionStartSpawnFields {
                program: params.program.clone(),
                args: params.args.clone(),
                trace_syscalls: params.trace_syscalls,
                cwd: params.cwd.clone(),
                track_function_frames: false,
            }),
            session_id: None,
            pid: None,
            path: None,
        };
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        let observe_ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };
        let lifecycle_ctx = SessionLifecycleContext {
            store: std::sync::Arc::clone(&self.lifecycle_store),
            probe: &probe_ctx,
            observe: &observe_ctx,
        };

        let session_id = match ChronosSessionLifecycleService::start(&lifecycle_ctx, v2_input).await
        {
            Ok(out) => out.session_id,
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "capture failed at probe_start: {e}"
                ))));
            }
        };

        // ---- Step 2: wait for the target to exit on its own.
        // The probe registry entry is removed by ProbeService::stop, so its
        // disappearance means either the target exited and someone stopped the
        // probe, or we stopped it below. We poll `live_probes` while the target
        // is still traced; when the traced pid is gone from /proc the target
        // has exited and we finalise. Poll every 20 ms, bounded by a generous
        // wall-clock cap so a runaway target cannot hang the server forever.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            // Scope the std MutexGuard so it never crosses the `.await`.
            let target_gone = {
                let probes = self.live_probes.try_lock().ok();
                match probes
                    .as_ref()
                    .and_then(|p| p.get(&session_id))
                    .map(|lp| lp.session.pid)
                {
                    Some(pid) => pid != 0 && std::path::Path::new(&format!("/proc/{pid}")).exists(),
                    // Contended lock or probe already removed (stopped elsewhere)
                    // → do not block; retry next tick / go to persistence.
                    None => false,
                }
            };
            if !target_gone || std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        // ---- Step 3: stop the probe (drains durable evidence, builds nothing yet).
        let stop_result = match chronos_services::probe::ProbeService::stop(&probe_ctx, &session_id)
        {
            Ok(r) => r,
            Err(ServiceError::ProbeNotFound(_)) => {
                // Probe already removed (e.g. concurrently stopped): finalise
                // from the durable log instead of failing the capture.
                return Ok(CallToolResult::error(text_content(format!(
                    "capture_session: live probe '{}' vanished before stop; run load_session after save_session to query it",
                    session_id
                ))));
            }
            Err(e) => {
                return Ok(CallToolResult::error(text_content(format!(
                    "capture failed at probe_stop: {e}"
                ))));
            }
        };

        // Build the in-memory query engine (same side effect as probe_stop).
        self.build_and_store_engine(
            &session_id,
            stop_result.events.clone(),
            stop_result.language,
        )
        .await;

        // ---- Step 4: persist via the SessionArchive port (SessionsService).
        let sessions_ctx = SessionsContext {
            engines: &self.engines,
            session_languages: &self.session_languages,
            connected_sessions: &self.connected_sessions,
            archive: &*self.archive,
            execution_log_registry: &self.execution_logs,
            execution_log_root: &self.execution_log_root,
        };
        let language = match params.language.as_deref() {
            Some(tag) => {
                let parsed = Language::from_string(tag);
                if matches!(parsed, Language::Unknown) && !tag.eq_ignore_ascii_case("unknown") {
                    stop_result.language
                } else {
                    parsed
                }
            }
            None => stop_result.language,
        };

        match SessionsService::save_session(
            &session_id,
            language,
            params.program.clone(),
            &sessions_ctx,
        )
        .await
        {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": session_id,
                    "saved": true,
                    "status": "captured",
                    "event_count": result.event_count,
                    "duration_ms": result.duration_ms,
                    "target": params.program,
                    "hint": "Session persisted and queryable. Use load_session / query_events against this session_id.",
                });
                Ok(CallToolResult::success(json_content(&session_envelope(
                    self.degraded,
                    output,
                ))))
            }
            Err(ServiceError::EmptySession(_)) => {
                // REQ-TB-01 EC-TB-01: an empty capture still persisted zero-event
                // sessions is NOT supported by save_session (it refuses). Surface
                // this honestly instead of fabricating a save.
                Ok(CallToolResult::error(text_content(format!(
                    "capture_session: target produced zero events; session '{}' was not saved",
                    session_id
                ))))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "capture failed at save: {e}"
            )))),
        }
    }

    #[tool(
        name = "probe_advance",
        description = "REC-C3.3.3 (Tren B slice G): advance a paused live probe session. The native backend delegates to PtraceTracer::continue_execution. Returns the AdvanceOutput (advanced, paused_reason, running) on success. Maps ServiceError::ProbeNotFound -> error.code = session_not_found; ServiceError::SessionStopped -> error.code = session_stopped."
    )]
    async fn probe_advance(
        &self,
        params: Parameters<ProbeAdvanceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        match chronos_services::probe::ProbeService::advance(&probe_ctx, &params.session_id) {
            Ok(out) => Ok(CallToolResult::success(text_content(
                serde_json::to_string(&out).unwrap_or_else(|e| format!("serialise error: {e}")),
            ))),
            Err(ServiceError::ProbeNotFound(_)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "session_not_found: session '{}' not found",
                    params.session_id
                ))))
            }
            Err(ServiceError::SessionStopped(_)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "session_stopped: session '{}' has stopped; cannot advance",
                    params.session_id
                ))))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "advance failed: {e}"
            )))),
        }
    }

    #[tool(
        name = "probe_step",
        description = "REC-C3.3.3 (Tren B slice G): single-step a paused live probe session by one instruction. The native backend delegates to PtraceTracer::step. Returns StepOutput { stepped: true } on success. Maps ServiceError::ProbeNotFound -> error.code = session_not_found; ServiceError::SessionRunning -> error.code = session_running (the target must be paused to step)."
    )]
    async fn probe_step(
        &self,
        params: Parameters<ProbeStepParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        match chronos_services::probe::ProbeService::step(&probe_ctx, &params.session_id) {
            Ok(out) => Ok(CallToolResult::success(text_content(
                serde_json::to_string(&out).unwrap_or_else(|e| format!("serialise error: {e}")),
            ))),
            Err(ServiceError::ProbeNotFound(_)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "session_not_found: session '{}' not found",
                    params.session_id
                ))))
            }
            Err(ServiceError::SessionRunning(_)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "session_running: session '{}' is running; cannot step",
                    params.session_id
                ))))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "step failed: {e}"
            )))),
        }
    }

    #[tool(
        name = "probe_start",
        description = "Start a live probe on a target program. Unlike debug_run (which blocks until the program exits), probe_start returns immediately and streams events to a ring buffer. Use probe_drain to read events in real-time and probe_stop to finalize the session."
    )]
    async fn probe_start(
        &self,
        params: Parameters<ProbeStartParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Validate the program path (security gate, kept in the server).
        if let Err(e) = crate::security::validate_program_path(&params.program) {
            return Ok(CallToolResult::error(text_content(format!(
                "Invalid program path: {}",
                e
            ))));
        }

        // v1 shim: route through `session_start{action=spawn}` (m7-05).
        // The v2 dispatcher returns `SessionStartOutput` which carries
        // the v1-compatible fields (`session_id`, `language`) and a
        // `capability_snapshot`. We preserve the original v1 JSON shape
        // 1:1 minus `bus_capacity` (REC-C2.3: the bus is gone).
        let v2_input = chronos_services::output::SessionStartInput {
            action: chronos_services::output::SessionStartAction::Spawn,
            spawn_fields: Some(chronos_services::output::SessionStartSpawnFields {
                program: params.program,
                args: params.args,
                trace_syscalls: params.trace_syscalls,
                cwd: params.cwd,
                track_function_frames: params.track_function_frames.unwrap_or(false),
            }),
            session_id: None,
            pid: None,
            path: None,
        };
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        let observe_ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };
        let lifecycle_ctx = SessionLifecycleContext {
            store: std::sync::Arc::clone(&self.lifecycle_store),
            probe: &probe_ctx,
            observe: &observe_ctx,
        };

        match ChronosSessionLifecycleService::start(&lifecycle_ctx, v2_input).await {
            Ok(out) => {
                let output = serde_json::json!({
                    "session_id": out.session_id,
                    "status": "started",
                    "target": out.target,
                    "language": out.capability_snapshot.language,
                    "hint": "Session is live. Use probe_drain to read events, session_stop / probe_stop to finalise."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::InvalidProgramPath(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid program path: {}", msg),
            ))),
            Err(ServiceError::InvalidInput(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid session_start input: {}", msg),
            ))),
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected session_start(spawn) error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_stop",
        description = "Stop a live probe session. Drains remaining events from the ring buffer, builds a QueryEngine, and makes the session fully queryable (events_read, execution_query, trace_slice, etc.)."
    )]
    async fn probe_stop(
        &self,
        params: Parameters<ProbeStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // v1 tool — preserved as-is for backward compatibility.
        // The v2 `session_stop` tool is the new canonical entrypoint
        // and supports `seal_tail` + `drain_subscriptions`; this v1
        // entrypoint keeps the original `ProbeService::stop` path +
        // engine-build side effect. The m7-05 dispatcher does NOT
        // route through this shim; instead callers are expected to
        // migrate to `session_stop` (with `seal_tail=true,
        // drain_subscriptions=true` defaults matching v1 behavior).
        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::stop(&ctx, &params.session_id) {
            Ok(result) => {
                let stop_completeness = result.completeness;
                self.build_and_store_engine(&params.session_id, result.events, result.language)
                    .await;

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "stopped",
                    "target": result.target,
                    "total_events": result.total_events,
                    "duration_ms": result.duration_ms,
                    "ebpf_detached": result.ebpf_detached,
                    // REC-C2.2.3: `total_events` is now a statement about the
                    // session's durable execution log, not about how much of a
                    // bounded EventBus ring survived. Completeness travels with
                    // it so a gap-bearing capture is never presented as a clean
                    // total.
                    "completeness": stop_completeness,
                    "examined_records": result.examined_records,
                    "hint": "Session is now queryable. Use query_events, get_call_stack, etc. Prefer session_stop for the v2 contract (adds seal_tail + drain_subscriptions)."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Live probe session '{}' not found. It may have already been stopped.",
                    s
                ))))
            }
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe stop error: {}",
                other
            )))),
        }
    }

    // ---- v2 session_start / session_stop / capabilities (m7-05) ----

    #[tool(
        name = "session_start",
        description = "Start, load, or attach a session. action='spawn' starts a new probe (returns session_id + capability_snapshot); action='load' reads an existing session from the store; action='attach' ptrace-attaches to a running process by pid and returns its capability_snapshot (Linux only; m9-77)."
    )]
    async fn session_start(
        &self,
        params: Parameters<SessionStartParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Parse action discriminator.
        let action = match params.action.as_str() {
            "spawn" => chronos_services::output::SessionStartAction::Spawn,
            "load" => chronos_services::output::SessionStartAction::Load,
            "attach" => chronos_services::output::SessionStartAction::Attach,
            other => {
                return Ok(CallToolResult::error(text_content(format!(
                    "Invalid action '{}': expected 'spawn' | 'load' | 'attach'",
                    other
                ))));
            }
        };

        // Server-side program-path validation (security gate; mirrors probe_start).
        if action == chronos_services::output::SessionStartAction::Spawn {
            if let Some(sf) = &params.spawn_fields {
                if let Err(e) = crate::security::validate_program_path(&sf.program) {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Invalid program path: {}",
                        e
                    ))));
                }
            }
        }

        let v2_input = chronos_services::output::SessionStartInput {
            action,
            spawn_fields: params.spawn_fields.as_ref().map(|sf| {
                chronos_services::output::SessionStartSpawnFields {
                    program: sf.program.clone(),
                    args: sf.args.clone(),
                    trace_syscalls: sf.trace_syscalls,
                    cwd: sf.cwd.clone(),
                    track_function_frames: sf.track_function_frames.unwrap_or(false),
                }
            }),
            session_id: params.session_id.clone(),
            pid: params.pid,
            path: params.path.clone(),
        };
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        let observe_ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };
        let lifecycle_ctx = SessionLifecycleContext {
            store: std::sync::Arc::clone(&self.lifecycle_store),
            probe: &probe_ctx,
            observe: &observe_ctx,
        };

        match ChronosSessionLifecycleService::start(&lifecycle_ctx, v2_input).await {
            Ok(out) => {
                // REC-C1.6: mark the session as "live" ONLY when a probe is
                // actually attached. Spawn/Attach attach a live probe; Load
                // is a pure read of an already-stopped session and must not
                // be refused on delete_session. `cleanup_session_memory`
                // removes this marker on the delete path; `session_stop`
                // also removes it so a stop-then-delete succeeds.
                if let Ok(mut live) = self.connected_sessions.lock() {
                    match out.action {
                        chronos_services::output::SessionStartAction::Spawn
                        | chronos_services::output::SessionStartAction::Attach => {
                            live.insert(out.session_id.clone());
                        }
                        chronos_services::output::SessionStartAction::Load => {
                            // Defensive: ensure no stale marker leaks across
                            // a load. The marker should never be set for Load
                            // (no probe), but a prior session_start{action=spawn}
                            // for the same id would have set it before
                            // session_stop cleared it.
                            live.remove(&out.session_id);
                        }
                    }
                }
                let json = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("session_start serialize: {}", e), None)
                })?;
                Ok(CallToolResult::success(json_content(&json)))
            }
            Err(ServiceError::InvalidProgramPath(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid program path: {}", msg),
            ))),
            Err(ServiceError::InvalidInput(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid session_start input: {}", msg),
            ))),
            Err(ServiceError::Unsupported(msg)) => Ok(CallToolResult::error(text_content(msg))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::LoadFailed(msg)) => Ok(CallToolResult::error(text_content(msg))),
            Err(ServiceError::AttachFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("session_start{{action=attach}} failed: {}", msg),
            ))),
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected session_start error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "session_stop",
        description = "Stop a live probe session. Defaults: seal_tail=true (mark metadata sealed), drain_subscriptions=true (destructive drain of observe subscriptions before stopping). Pass seal_tail=false / drain_subscriptions=false to opt out."
    )]
    async fn session_stop(
        &self,
        params: Parameters<SessionStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let v2_input = chronos_services::output::SessionStopInput {
            session_id: params.session_id,
            seal_tail: params.seal_tail,
            drain_subscriptions: params.drain_subscriptions,
        };
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        let observe_ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };
        let lifecycle_ctx = SessionLifecycleContext {
            store: std::sync::Arc::clone(&self.lifecycle_store),
            probe: &probe_ctx,
            observe: &observe_ctx,
        };

        // Architecture (m7-07 single-call path): we used to do a 3-step
        // dance (pre-stop probe → save_session → build_and_store_engine
        // → call dispatcher.stop which double-called ProbeService::stop
        // internally). The double-call forced a load_session fallback in
        // the dispatcher which was structurally a workaround. m7-07
        // replaces all of that with a single
        // `stop_with_persistence` call that returns the events for us
        // to persist + build_engine on exactly one probe-stop.
        //
        // REC-C1.6: the liveness marker is cleared INSIDE each match arm
        // below (where the session_id is in scope). We do not remove the
        // marker before the persistence dispatch because we cannot yet
        // know the session_id in the AlreadyStopped branch shape; the
        // persisted arm has it directly.
        let (drained_subscriptions, persistence) =
            match ChronosSessionLifecycleService::stop_with_persistence(&lifecycle_ctx, v2_input)
                .await
            {
                Ok(parts) => parts,
                Err(ServiceError::InvalidInput(msg)) => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "Invalid session_stop input: {}",
                        msg
                    ))));
                }
                Err(other) => {
                    return Ok(CallToolResult::error(text_content(format!(
                        "internal error: unexpected session_stop error: {}",
                        other
                    ))));
                }
            };

        match persistence {
            SessionStopPersistence::Stopped {
                session_id,
                events,
                language,
                target,
                total_events,
                duration_ms,
                ebpf_detached,
                sealed_at,
            } => {
                // REC-C1.6: probe is no longer live after a successful stop;
                // clear the liveness marker so a subsequent delete_session
                // for this id is allowed (not refused as SessionStillActive).
                if let Ok(mut live) = self.connected_sessions.lock() {
                    live.remove(&session_id);
                }
                // 1. Persist metadata + events to the redb store so
                //    `session_start{action=load}` can find the session.
                let meta = chronos_store::SessionMetadata {
                    session_id: session_id.clone(),
                    created_at: 0,
                    language: language.to_string(),
                    target: target.clone(),
                    event_count: events.len(),
                    duration_ms,
                    tail_sealed: sealed_at.is_some(),
                    sealed_at,
                };
                if let Err(e) = self.store.save_session(meta, &events) {
                    // The probe is already stopped here and cannot be resumed,
                    // so swallowing this left the events in memory only: the
                    // tool answered `status: "stopped"` with
                    // `query_engine_ready: true`, and a later
                    // `session_start{action=load}` found nothing. The comment
                    // above explained what gets written, never why the error
                    // could be dropped, which is the part that mattered.
                    return Ok(CallToolResult::error(text_content(format!(
                        "session '{session_id}' was stopped but could not be persisted: {e}. \
                         The captured events exist only in this process, and a later \
                         session_start{{action=load}} will not find this session."
                    ))));
                }
                // 2. Build the in-memory QueryEngine for query_* tools.
                self.build_and_store_engine(&session_id, events, language)
                    .await;

                let snapshot = CapabilitySnapshot {
                    probe_type: Some("ebpf_user".to_string()),
                    language: Some(language.to_string()),
                    query_engine_ready: true,
                    active_subscriptions: vec![],
                    tail_sealed: sealed_at.is_some(),
                    sealed_at,
                    ..Default::default()
                };
                let out = SessionStopOutput {
                    session_id,
                    status: "stopped".to_string(),
                    target,
                    total_events,
                    duration_ms,
                    ebpf_detached,
                    sealed_at,
                    drained_subscriptions,
                    capability_snapshot: snapshot,
                    provenance: SessionLifecycleProvenance {
                        engine_version: "chronos-0.1.0".to_string(),
                        source: "session_stop".to_string(),
                    },
                };
                let json = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("session_stop serialize: {}", e), None)
                })?;
                Ok(CallToolResult::success(json_content(&json)))
            }
            SessionStopPersistence::AlreadyStopped { session_id } => {
                // Idempotent path: load_session and synthesise the
                // output from the already-persisted metadata + events.
                // REC-C1.6: the probe was already stopped (likely by a
                // previous v1 probe_stop) so the liveness marker must
                // not exist here; defensively clear it to keep the
                // invariant that any session_id in `connected_sessions`
                // has a live probe attached.
                if let Ok(mut live) = self.connected_sessions.lock() {
                    live.remove(&session_id);
                }
                let (meta, events) = match self.store.load_session(&session_id) {
                    Ok(pair) => pair,
                    Err(e) => {
                        return Ok(CallToolResult::error(text_content(format!(
                            "Live probe session '{}' not found ({}). It may have been stopped and the metadata is missing.",
                            session_id, e
                        ))));
                    }
                };
                let snapshot = CapabilitySnapshot {
                    probe_type: Some("ebpf_user".to_string()),
                    language: Some(meta.language.clone()),
                    query_engine_ready: true,
                    active_subscriptions: vec![],
                    tail_sealed: meta.tail_sealed,
                    sealed_at: meta.sealed_at,
                    ..Default::default()
                };
                let out = SessionStopOutput {
                    session_id: session_id.clone(),
                    status: "already_stopped".to_string(),
                    target: meta.target,
                    total_events: events.len() as u64,
                    duration_ms: meta.duration_ms,
                    ebpf_detached: true,
                    sealed_at: meta.sealed_at,
                    drained_subscriptions,
                    capability_snapshot: snapshot,
                    provenance: SessionLifecycleProvenance {
                        engine_version: "chronos-0.1.0".to_string(),
                        source: "session_stop".to_string(),
                    },
                };
                let json = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("session_stop serialize: {}", e), None)
                })?;
                Ok(CallToolResult::success(json_content(&json)))
            }
        }
    }

    #[tool(
        name = "capabilities",
        description = "Enumerate available evidence mechanisms. Pass `target` for static (pre-session) capabilities; pass `session_id` for dynamic (post-session) capabilities. Both allowed for a combined view."
    )]
    async fn capabilities(
        &self,
        params: Parameters<CapabilitiesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let v2_input = chronos_services::output::CapabilitiesInput {
            target: params
                .target
                .as_ref()
                .map(|t| chronos_services::output::TargetSpec {
                    program: t.program.clone(),
                    args: t.args.clone(),
                    language: t.language.as_ref().and_then(|s| parse_language(s)),
                }),
            session_id: params.session_id.clone(),
        };
        // Extract target language for tool availability computation.
        let target_language = params.target.as_ref().and_then(|t| t.language.as_deref());
        // Build tool availability map (MS-CAP-DISCOVERY REQ-CAP-001/002/003).
        let tool_availability = self.build_tool_availability(ALL_TOOL_NAMES, target_language);
        // Use the full-context entrypoint so `capabilities{session_id}`
        // can resolve live-only sessions (those still in
        // `live_probes` but not yet persisted to `SessionStore`)
        // by synthesising a stub metadata from the LiveProbeSession.
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };
        let observe_ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };
        let lifecycle_ctx = SessionLifecycleContext {
            store: std::sync::Arc::clone(&self.lifecycle_store),
            probe: &probe_ctx,
            observe: &observe_ctx,
        };
        match ChronosSessionLifecycleService::capabilities_with_context(
            &lifecycle_ctx,
            v2_input,
            Some(tool_availability),
        ) {
            Ok(out) => {
                let json = serde_json::to_value(&out).map_err(|e| {
                    rmcp::ErrorData::internal_error(format!("capabilities serialize: {}", e), None)
                })?;
                Ok(CallToolResult::success(json_content(&json)))
            }
            Err(ServiceError::InvalidInput(msg)) => Ok(CallToolResult::error(text_content(
                format!("Invalid capabilities input: {}", msg),
            ))),
            Err(ServiceError::LoadFailed(msg)) => Ok(CallToolResult::error(text_content(msg))),
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected capabilities error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_drain",
        description = "Drain canonical evidence from a live probe session without stopping it. Events are projected from the session's durable ExecutionLog. Pass the 'evidence_cursor' (ecv1:...) returned by a previous call to continue. The probe keeps running; use probe_stop to finalize."
    )]
    async fn probe_drain(
        &self,
        params: Parameters<ProbeDrainParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let input = chronos_services::probe::ProbeDrainInput {
            session_id: params.session_id.clone(),
            evidence_cursor: params.evidence_cursor.clone(),
            offset: params.offset,
            limit: params.limit,
        };

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::drain(&ctx, input) {
            Ok(result) => {
                // Apply offset/limit (matches existing contract: slicing happens
                // after the drain, with the wrapper doing the slicing).
                let sliced: Vec<_> = result
                    .events
                    .into_iter()
                    .skip(params.offset)
                    .take(params.limit)
                    .map(|e| {
                        serde_json::json!({
                            "event_id": e.source_event_id,
                            "timestamp_ns": e.timestamp_ns,
                            "thread_id": e.thread_id,
                            "language": format!("{:?}", e.language),
                            "kind": format!("{:?}", e.kind),
                            "description": e.description,
                        })
                    })
                    .collect();

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "running",
                    "total_buffered": result.total_buffered,
                    "returned": sliced.len(),
                    "offset": params.offset,
                    "limit": params.limit,
                    // Canonical coordinate: reuse it verbatim to continue.
                    "evidence_cursor": result.evidence_cursor,
                    // Evidence facts about the EXAMINED range, same model as
                    // probe_events. Pagination is reported by `exhausted`.
                    "completeness": result.completeness,
                    "exhausted": result.exhausted,
                    // COMPATIBILITY ONLY, carries no evidence: the legacy ring
                    // cursor is a different coordinate space and is never
                    // converted into an EventSeq.
                    "legacy_cursor": serde_json::Value::Null,
                    "tripwires_fired": result.tripwires_fired,
                    "events": sliced,
                    "hint": "Probe is still running. Call probe_drain again with 'evidence_cursor' for more events, or probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::CursorStale {
                requested_next_seq,
                retained_from_seq,
            }) => Ok(CallToolResult::error(text_content(format!(
                "Cursor at seq {requested_next_seq} is stale; the earliest available position is \
{retained_from_seq}. This is retention, not evidence loss: re-anchor deliberately."
            )))),
            Err(ServiceError::EvidenceDecodeFailed {
                session_id,
                seq,
                payload_tag,
            }) => Ok(CallToolResult::error(text_content(format!(
                "Undecodable evidence at seq {seq} (payload tag '{payload_tag}') in session \
'{session_id}'. No cursor was advanced over it and no derived facts were reported: reading \
further would be a Silent Lie."
            )))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to drain events: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe drain error: {}",
                other
            )))),
        }
    }

    /// m1-03: MCP query path that reads `TraceEvent`s from the
    /// `ExecutionLog` (segmented, persistent) of a live probe
    /// session. Available only when the native backend was
    /// configured with `with_execution_log_dir`.
    ///
    /// Cursor is a non-destructive seq-based filter: passing `None`
    /// returns every record appended so far; passing a value returns
    /// records with seq strictly greater. The returned `tail_seq`
    /// can be passed back as `since` for the next call.
    #[tool(
        name = "probe_drain_log",
        description = "m1-03 ExecutionLog-backed query path. Reads TraceEvents from the durable ExecutionLog attached to a live probe session, instead of the legacy in-memory EventBus. Cursor is a seq number (None for fresh). Returns 0 records when the probe is not configured with an ExecutionLog directory. DATA HANDLING: each record's `data` field is the captured `EventData` serialized verbatim and unredacted. `Memory` records carry the raw bytes read from the target process as a JSON array of numbers, with no byte cap; `Variable` records carry the variable value verbatim. No redaction layer sits on this path, so a response can carry whatever the target process held in memory, including credentials. Treat the response as sensitive: it can leave the machine through the model context."
    )]
    async fn probe_drain_log(
        &self,
        params: Parameters<ProbeDrainLogParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::drain_log(
            &ctx,
            &params.session_id,
            params.since,
            params.limit,
        ) {
            Ok((records, tail_seq, unparseable_payload_count, total_records_seen)) => {
                let events: Vec<serde_json::Value> = records
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "event_id": r.event_id,
                            "timestamp_ns": r.timestamp_ns,
                            "thread_id": r.thread_id,
                            "kind": format!("{:?}", r.event_type),
                            "location": {
                                "file": r.location.file,
                                "line": r.location.line,
                                "function": r.location.function,
                            },
                            "data": serde_json::to_value(&r.data).unwrap_or(serde_json::Value::Null),
                        })
                    })
                    .collect();
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "source": "chronos-log::SegmentedExecutionLog",
                    "returned": events.len(),
                    "since": params.since,
                    "tail_seq": tail_seq,
                    "total_records_seen": total_records_seen,
                    "unparseable_payload_count": unparseable_payload_count,
                    "events": events,
                    "hint": "m1-04: ExecutionLog query path with decoder counters. \
                             `unparseable_payload_count` is the number of records whose JSON \
                             payload did not decode as a TraceEvent — these are still durable on \
                             disk; the counter is the signal that another producer (or schema \
                             drift) wrote to the same log. `total_records_seen` includes them.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to read ExecutionLog: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected drain_log error: {}",
                other
            )))),
        }
    }

    /// m1-07: snapshot the maintenance port's counter surface of the
    /// live probe session's `ExecutionLog`. Returns the conservative
    /// counter subset (segments_reclaimed, compaction_passes,
    /// no_op_passes, highest_seq_observed) plus a `log_attached: bool`
    /// flag (false when the probe was not configured with an
    /// ExecutionLog directory).
    #[tool(
        name = "probe_compaction_metrics",
        description = "m1-07 ExecutionLog compaction metrics. Returns a snapshot of the live probe's execution-log maintenance counters (segments_reclaimed, compaction_passes, no_op_passes, highest_seq_observed) plus a log_attached flag. Counter values are zero until at least one compaction pass has occurred. Available only when the native backend was configured with with_execution_log_dir."
    )]
    async fn probe_compaction_metrics(
        &self,
        params: Parameters<ProbeCompactionMetricsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::compaction_metrics(&ctx, &params.session_id) {
            Ok(metrics_out) => {
                let output = match metrics_out {
                    None => serde_json::json!({
                        "session_id": params.session_id,
                        "log_attached": false,
                        "hint": "Backend was not configured with with_execution_log_dir. \
                                 The probe is still functional; it just does not have an \
                                 on-disk log to surface counters for.",
                    }),
                    Some(m) => serde_json::json!({
                        "session_id": params.session_id,
                        "log_attached": true,
                        "source": "chronos_domain::ports::ExecutionLogMaintenance",
                        "segments_reclaimed": m.segments_reclaimed,
                        "compaction_passes": m.compaction_passes,
                        "no_op_passes": m.no_op_passes,
                        "highest_seq_observed": m.highest_seq_observed.map(|s| s.get()),
                    }),
                };
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::DrainFailed(msg)) => Ok(CallToolResult::error(text_content(
                format!("Failed to read compaction metrics: {}", msg),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected compaction metrics error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "session_snapshot",
        description = "Freeze a live probe session and build query indices without stopping the probe. This makes the session queryable (events_read, execution_query, trace_slice, etc.) while the probe continues collecting events. Call again to refresh the indices with newer events."
    )]
    async fn session_snapshot(
        &self,
        params: Parameters<SessionSnapshotParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::session_snapshot(&ctx, &params.session_id) {
            Ok(snapshot) => {
                let total_events = snapshot.events.len();
                let completeness = snapshot.completeness;

                // Build and store the query engine with proper noise filtering.
                self.build_and_store_engine(&params.session_id, snapshot.events, snapshot.language)
                    .await;

                // Set as active session
                {
                    let mut active = self.active_session.lock().await;
                    *active = Some(params.session_id.clone());
                }

                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "status": "running",
                    "events_indexed": total_events,
                    // REC-C2.2.3: completeness of the evidence this snapshot
                    // read. A snapshot is a read, not a drain, so re-running it
                    // sees the same evidence plus anything newer.
                    "completeness": completeness,
                    "hint": "Session is now queryable. Probe is still running. Call session_snapshot again to refresh indices with newer events."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected session snapshot error: {}",
                other
            )))),
        }
    }

    #[tool(
        name = "probe_status",
        description = "Inspect a live probe session: ptrace target, eBPF attachment state, uptime hints."
    )]
    async fn probe_status(
        &self,
        params: Parameters<ProbeStatusParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let session_id = params.0.session_id;

        let ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        match chronos_services::probe::ProbeService::status(&ctx, &session_id) {
            Ok(snapshot) => {
                let output = serde_json::json!({
                    "session_id": snapshot.session_id,
                    "language": snapshot.language,
                    "target": snapshot.target,
                    "traced_pid": snapshot.traced_pid,
                    "ebpf": snapshot.ebpf,
                    "state": snapshot.state,
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::ProbeNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Live probe session '{}' not found.", s),
            ))),
            Err(ServiceError::LockPoisoned) => {
                Ok(CallToolResult::error(text_content("lock poisoned")))
            }
            Err(other) => Ok(CallToolResult::error(text_content(format!(
                "internal error: unexpected probe status error: {}",
                other
            )))),
        }
    }

    // ========================================================================
    // SF10 — Browser/WASM Probe Tools (T15–T16)
    // ========================================================================

    #[tool(
        name = "browser_probe_start",
        description = "Start a browser debugging session. Launches Chrome headless, connects via CDP, detects WASM modules, and sets breakpoints. Use browser_probe_drain to read events and browser_probe_stop to finalize."
    )]
    async fn browser_probe_start(
        &self,
        params: Parameters<BrowserProbeStartParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(err) = self.toolset_guard("browser_probe_start") {
            return Ok(err);
        }
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
            factory: &self.browser_probe_factory,
        };

        match BrowserProbeService::start(
            &ctx,
            BrowserProbeStartInput {
                url: params.url,
                headless: params.headless,
                chrome_path: params.chrome_path,
            },
        )
        .await
        {
            Ok(result) => {
                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "running",
                    "url": result.url,
                    "hint": "Use browser_probe_drain to read WASM events, browser_probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "browser_probe_stop",
        description = "Stop a browser probe session. Drains remaining events, disconnects CDP, and kills Chrome process."
    )]
    async fn browser_probe_stop(
        &self,
        params: Parameters<BrowserProbeStopParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(err) = self.toolset_guard("browser_probe_stop") {
            return Ok(err);
        }
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
            factory: &self.browser_probe_factory,
        };

        match BrowserProbeService::stop(
            &ctx,
            chronos_services::browser_probe::BrowserProbeStopInput {
                session_id: params.session_id,
            },
        )
        .await
        {
            Ok(result) => {
                if result.total_events > 0 {
                    self.build_and_store_engine(
                        &result.session_id,
                        result.raw_events,
                        result.language,
                    )
                    .await;
                }

                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "stopped",
                    "url": result.url,
                    "total_events": result.total_events,
                    "hint": "Session is now queryable. Use query_events, get_call_stack, etc."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "browser_probe_drain",
        description = "Drain current events from a browser probe session without stopping it. Returns a snapshot of events currently buffered. The probe continues running."
    )]
    async fn browser_probe_drain(
        &self,
        params: Parameters<BrowserProbeDrainParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(err) = self.toolset_guard("browser_probe_drain") {
            return Ok(err);
        }
        let params = params.0;

        let ctx = BrowserProbeContext {
            live_browser_probes: &self.live_browser_probes,
            active_session: &self.active_session,
            factory: &self.browser_probe_factory,
        };

        match BrowserProbeService::drain(
            &ctx,
            chronos_services::browser_probe::BrowserProbeDrainInput {
                session_id: params.session_id,
                offset: params.offset,
                limit: params.limit,
            },
        )
        .await
        {
            Ok(result) => {
                let events_json: Vec<serde_json::Value> = result
                    .events
                    .into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "event_id": e.event_id,
                            "timestamp_ns": e.timestamp_ns,
                            "thread_id": e.thread_id,
                            "language": e.language,
                            "kind": e.kind,
                            "description": e.description,
                        })
                    })
                    .collect();

                let output = serde_json::json!({
                    "session_id": result.session_id,
                    "status": "running",
                    "total_buffered": result.total_buffered,
                    "returned": result.returned,
                    "offset": result.offset,
                    "limit": result.limit,
                    "events": events_json,
                    "hint": "Browser probe is still running. Call browser_probe_drain again for more events, or browser_probe_stop to finalize."
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
        }
    }

    #[tool(
        name = "performance_regression_audit",
        description = "DEPRECATED v1 shim. Routes to session_compare{kind=regression} via ChronosSessionCompareService. The v1 parameter names baseline_session_id / target_session_id are preserved; top_n is forwarded unchanged. The v1 response shape is preserved too (flat PerformanceRegressionAuditResult — no session_compare envelope). Prefer session_compare (v2)."
    )]
    async fn performance_regression_audit(
        &self,
        params: Parameters<PerformanceRegressionAuditParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::session_compare::SessionCompareContext {
            reader: Arc::clone(&self.reader),
            engine: Arc::clone(&self.diff_engine),
        };
        let v2_params = SessionCompareParams {
            kind: "regression".to_string(),
            session_a: params.baseline_session_id,
            session_b: params.target_session_id,
            top_n: params.top_n,
        };
        Self::dispatch_session_compare(&ctx, v2_params, SessionCompareWire::V1Flat).await
    }

    #[tool(
        name = "compare_sessions",
        description = "DEPRECATED v1 shim. Routes to session_compare{kind=divergence} via ChronosSessionCompareService. The v1 parameter names session_a / session_b are preserved. The v1 response shape is preserved too (flat CompareSessionsResult — no session_compare envelope). Prefer session_compare (v2)."
    )]
    async fn compare_sessions(
        &self,
        params: Parameters<CompareSessionsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::session_compare::SessionCompareContext {
            reader: Arc::clone(&self.reader),
            engine: Arc::clone(&self.diff_engine),
        };
        let v2_params = SessionCompareParams {
            kind: "divergence".to_string(),
            session_a: params.session_a,
            session_b: params.session_b,
            top_n: None,
        };
        Self::dispatch_session_compare(&ctx, v2_params, SessionCompareWire::V1Flat).await
    }

    #[tool(
        name = "session_compare",
        description = "v2 dispatcher for session-vs-session comparison. Discriminated by `kind`: `divergence` reports pairwise divergence (functions only in A / only in B / common + similarity%); `regression` audits top-function performance regression (baseline vs target). For divergence, `session_a` and `session_b` are the two sessions; for regression, `session_a` is the baseline and `session_b` is the target. Supersedes the v1 `compare_sessions` (kind=divergence) and `performance_regression_audit` (kind=regression) tools. See docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md (line 14)."
    )]
    async fn session_compare(
        &self,
        params: Parameters<SessionCompareParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::session_compare::SessionCompareContext {
            reader: Arc::clone(&self.reader),
            engine: Arc::clone(&self.diff_engine),
        };
        Self::dispatch_session_compare(&ctx, params, SessionCompareWire::V2Envelope).await
    }

    #[tool(
        name = "session_explain",
        description = "v2 net-new dispatcher for session-level explanations (no v1 shim). Discriminated by `kind`: `facts` returns raw counts and metadata; `derived` returns function hotspots + call-graph summary + syscall breakdown; `inferred` returns heuristic characterisations (IoHeavy / CpuBound / SingleThreaded / CrashDetected / Unknown); `hypothesis` returns typed hypothesis test plans (CrashInvariant / DominantFunctionCallPath). See docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md (line 14)."
    )]
    async fn session_explain(
        &self,
        params: Parameters<SessionExplainParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let explain_ctx = chronos_services::session_explain::SessionExplainContext {
            reader: Arc::clone(&self.reader),
        };
        let kind = parse_session_explain_kind(&params.kind)?;
        let input = chronos_services::output::SessionExplainInput {
            kind,
            session_id: params.session_id,
            // The MCP `session_explain` surface does not expose a
            // per-hypothesis discriminator; the dispatcher picks the
            // right plan based on what the bundle carries. The
            // hypothesis-kind param can be added to the wire shape
            // later without a breaking change (Option is forward-compat).
            hypothesis_kind: None,
        };
        match chronos_services::session_explain::ChronosSessionExplainService::explain(
            &explain_ctx,
            input,
        ) {
            Ok(out) => match serde_json::to_value(out) {
                Ok(v) => Ok(CallToolResult::success(json_content(&v))),
                Err(e) => Ok(CallToolResult::error(text_content(format!(
                    "Serialization error: {}",
                    e
                )))),
            },
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // M3 — Mutation Lens Tools
    // ========================================================================

    #[tool(
        name = "mutation_lens",
        description = "Mutation Lens: list StateTransition records derived from VariableWrite events in a session. If `target` is given, filter to that variable name; otherwise return all observed transitions."
    )]
    async fn mutation_lens(
        &self,
        params: Parameters<MutationLensParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::analysis::AnalysisContext {
            engines: &self.engines,
        };

        match chronos_services::analysis::ChronosAnalysisService::mutation_lens(
            &ctx,
            chronos_services::analysis::MutationLensInput {
                session_id: params.session_id,
                target: params.target,
                limit: params.limit,
            },
        )
        .await
        {
            Ok(result) => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": result.session_id,
                "count": result.count,
                "transitions": result.transitions,
            })))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "causal_slice",
        description = "Compute the conservative backward causal slice from a sink event. Returns evidence nodes reachable backward (included) and any included nodes whose evidence is unobserved (missing, never silently dropped)."
    )]
    async fn causal_slice(
        &self,
        params: Parameters<CausalSliceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let ctx = chronos_services::analysis::AnalysisContext {
            engines: &self.engines,
        };

        match chronos_services::analysis::ChronosAnalysisService::causal_slice(
            &ctx,
            chronos_services::analysis::CausalSliceInput {
                session_id: params.session_id.clone(),
                sink_event_id: params.sink_event_id,
            },
        )
        .await
        {
            Ok(result) => Ok(CallToolResult::success(json_content(&serde_json::json!({
                "session_id": result.session_id,
                "sink": result.sink,
                "included": result.included,
                "missing": result.missing,
                "depth": result.depth,
            })))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::EventNotFound { event_id }) => {
                Ok(CallToolResult::error(text_content(format!(
                    "Sink event {} not found in session '{}'",
                    event_id, params.session_id
                ))))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "hypothesis_test",
        description = "Evaluate a typed hypothesis against captured session evidence and return Pass / Violation / Unsupported. Three supported shapes: 'invariant' (reuses chronos_domain::property::Property::evaluate on EventCount or PropertyValue), 'existence' (predicate scan over captured events; Pass if >=1 match, else Violation), 'call_path' (BFS reachability in the call graph). Returns raw support_event_ids + counter_event_ids per the v2 spec ('without hiding raw support'). v2 net-new tool (no v1 shim)."
    )]
    async fn hypothesis_test(
        &self,
        params: Parameters<HypothesisTestParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Parse the JSON-friendly strings into the typed enums the
        // dispatcher expects. Invalid values become ServiceError::InvalidInput
        // via the dispatcher's own error path; here we just convert.
        let scope = params.scope.as_deref().and_then(parse_hypothesis_scope);
        let comparison = params.comparison.as_deref().and_then(parse_comparison_op);
        let constant = params
            .constant
            .map(chronos_services::output::PropertyValue::from);

        let ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let input = chronos_services::hypothesis_test::HypothesisInput {
            session_id: params.session_id,
            kind: params.kind,
            scope,
            comparison,
            constant,
            property_target: params.property_target,
            predicate: params.predicate,
            caller: params.caller,
            callee: params.callee,
            max_depth: Some(params.max_depth),
        };

        match chronos_services::hypothesis_test::ChronosHypothesisTestService::test(&ctx, input)
            .await
        {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{s}' not found"),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "session_export",
        description = "Export a session bundle (metadata + trace events + properties snapshot) to a portable file on disk. Two supported formats: 'json' (pretty-printed canonical ExportBundle) and 'otlp_json' (OpenTelemetry-compatible JSON wire format, ready for Jaeger/Tempo/Honeycomb). The 'zip_json' format is reserved for m7+ and is rejected by the dispatcher. v2 net-new tool (no v1 shim). NOTE: in m6-05 the `properties_snapshot` field is always empty -- the QueryEngine has no property-table accessor yet. The DTO and JSON schema are final so m7+ can fill the field without a breaking change."
    )]
    async fn session_export(
        &self,
        params: Parameters<SessionExportParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        let format = match parse_export_format(&params.format) {
            Some(f) => f,
            None => {
                return Ok(CallToolResult::error(text_content(format!(
                    "unknown format '{}'; allowed values: 'json', 'otlp_json' (zip_json is reserved for a future cycle)",
                    params.format
                ))));
            }
        };

        let output_path = match crate::security::validate_output_path(&params.output_path) {
            Ok(p) => p,
            Err(e) => return Ok(CallToolResult::error(text_content(e.to_string()))),
        };
        let ctx = chronos_services::session_export::SessionExportContext {
            engines: &self.engines,
        };

        match chronos_services::session_export::ChronosSessionExportService::export(
            &params.session_id,
            Language::from_string(&params.language),
            params.target.clone(),
            format,
            &output_path,
            &ctx,
        )
        .await
        {
            Ok(out) => {
                let output = serde_json::json!({
                    "session_id": params.session_id,
                    "path": out.path.display().to_string(),
                    "bytes_written": out.bytes_written,
                    "format": match out.format {
                        chronos_services::output::ExportFormat::Json => "json",
                        chronos_services::output::ExportFormat::OtlpJson => "otlp_json",
                        chronos_services::output::ExportFormat::ZipJson => "zip_json",
                    },
                    "hint": "The file at `path` contains the full session bundle. Round-trip via `serde_json::from_slice` for the `json` format, or feed the `otlp_json` format directly to an OpenTelemetry JSON receiver.",
                });
                Ok(CallToolResult::success(json_content(&output)))
            }
            Err(ServiceError::SessionNotInMemory(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{s}' not found in memory. Run probe_start first."),
            ))),
            Err(ServiceError::EmptySession(s)) => Ok(CallToolResult::error(text_content(format!(
                "Session '{s}' has no events to export."
            )))),
            Err(ServiceError::InvalidExportParameter(s)) => {
                Ok(CallToolResult::error(text_content(s)))
            }
            Err(ServiceError::ExportFailed(s)) => Ok(CallToolResult::error(text_content(format!(
                "export failed: {s}"
            )))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "trace_slice",
        description = "Causal evidence around a target. Discriminated by slice_kind: variable_origin (mutations to a named variable), crash (call stack at last fatal signal), causality (full reads + writes at an address), memory_audit (writes at an address with call stacks). DATA HANDLING: for slice_kind causality and memory_audit, each returned entry carries data_hex, the hex-encoded bytes of the captured process memory at that address, verbatim and unredacted; only bytes the capture configuration already recorded are returned, and nothing is fetched from outside the session. Treat those responses as sensitive: they can leave the machine through the model context. Supersedes the v1 debug_find_variable_origin, debug_find_crash, inspect_causality, and forensic_memory_audit tools."
    )]
    async fn trace_slice(
        &self,
        params: Parameters<TraceSliceParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // REC-C1.7: gate by projection meta (see execution_query above).
        let _meta = self.gate_projection_for_wire(&params.session_id).await?;

        let ctx = TraceSliceContext {
            engines: &self.engines,
            projection_meta: &self.projection_meta,
        };
        let input = TraceSliceInput {
            session_id: params.session_id,
            slice_kind: params.slice_kind,
            variable_name: params.variable_name,
            address: params.address,
            limit: params.limit,
        };

        match ChronosTraceSliceService::slice(&ctx, input).await {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "events_read",
        description = "v2 dispatcher for event reads. Select the read mode via `mode` (query | by_id). `mode=query` is a cursor-based, non-destructive event-list read with filters (event_types, thread_id, timestamp range, function_pattern, limit, cursor); `mode=by_id` is a single-event lookup by event_id. DATA HANDLING: events are returned as recorded, so any event whose `data` is the `Memory` variant includes `data` with the captured process memory bytes verbatim and unredacted; other event kinds carry no memory contents. Treat such events as sensitive: they can leave the machine through the model context. Supersedes the v1 `query_events` and `get_event` tools. See docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md (line 15)."
    )]
    async fn events_read(
        &self,
        params: Parameters<EventsReadParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // MS-EVT-TYPED: `event_types` arrives as a typed `Vec<EventType>`;
        // rmcp rejects unknown names at JSON-RPC parse time, so no string
        // parsing remains on the v2 path.
        let event_types: Option<Vec<EventType>> =
            params.event_types.filter(|types| !types.is_empty());

        // REC-C1.3: the authoritative read needs the sessions (session-owned
        // logs), not the engine map.
        let ctx = EventsReadContext {
            execution_logs: &self.execution_logs,
        };
        let input = EventsReadInput {
            session_id: params.session_id,
            mode: params.mode,
            event_types,
            thread_id: params.thread_id,
            timestamp_start: params.timestamp_start,
            timestamp_end: params.timestamp_end,
            function_pattern: params.function_pattern,
            limit: params.limit,
            cursor: params.cursor,
            event_id: params.event_id,
        };

        match ChronosEventsReadService::read(&ctx, input).await {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("Session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(ServiceError::InvalidCursorPayload) => Ok(CallToolResult::error(text_content(
                "invalid cursor: expected an opaque cursor previously returned in next_cursor",
            ))),
            Err(ServiceError::EvidenceDecodeFailed {
                session_id,
                seq,
                payload_tag,
            }) => Ok(CallToolResult::error(text_content(format!(
                "evidence at seq {seq} of session '{session_id}' could not be decoded                  (payload tag {payload_tag:?}); the read failed closed and no cursor was issued"
            )))),
            Err(ServiceError::EvidenceReadStalled { session_id, position }) => {
                Ok(CallToolResult::error(text_content(format!(
                    "evidence read stalled for session '{session_id}' at position {position}:                      the log reported more data without advancing"
                ))))
            }
            Err(ServiceError::NoExecutionLog(s)) => Ok(CallToolResult::error(text_content(
                format!("session '{s}' owns no ExecutionLog"),
            ))),
            Err(ServiceError::EvidenceUnavailableDueToRetention { retained_from }) => {
                Ok(CallToolResult::error(text_content(format!(
                    "evidence unavailable: this session's history is retained only from seq \
{retained_from}, so an id from the retired range cannot be reported as absent"
                ))))
            }
            Err(ServiceError::ExecutionLogUnavailable { session_id, reason }) => {
                Ok(CallToolResult::error(text_content(format!(
                    "ExecutionLog unavailable for session '{session_id}': {reason}"
                ))))
            }
            Err(ServiceError::CursorStale {
                requested_next_seq,
                retained_from_seq,
            }) => {
                // REC-C1.6: structured envelope — keep the existing text content
                // (chronos-sandbox restart_uat R2 already parses it) AND attach a
                // second json content item carrying the two numbers. The agent can
                // then re-anchor deliberately without having to scrape text.
                let mut content = text_content(format!(
                    "Cursor at seq {requested_next_seq} is stale; the earliest available position is \
{retained_from_seq}. This is retention, not evidence loss: re-anchor deliberately."
                ));
                content.extend(json_content(&serde_json::json!({
                    "error": "cursor_stale",
                    "requested_next_seq": requested_next_seq,
                    "retained_from_seq": retained_from_seq,
                })));
                Ok(CallToolResult::error(content))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "execution_log_read",
        description = "Execution Explorer read path over a session's authoritative ExecutionLog. Select the view via `mode`: `poll` returns a bounded batch of new events past the cursor plus the next cursor; `summarize` returns time-bucketed counts for the whole log; `rollup` returns per-invocation (per-thread) counts; `causality` reports whether causality hints are available for the session. Aggregates are NOT evidence — use `events_read` for paged event evidence. Fails closed: a session with no readable log returns an error, never an empty result."
    )]
    async fn execution_log_read(
        &self,
        params: Parameters<ExecutionLogReadParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;
        let session = chronos_domain::SessionId::new(params.session_id.clone());

        // Resolve the cursor against the requested session so a cursor
        // minted elsewhere is refused rather than silently reinterpreted.
        let cursor = match params.cursor.as_deref() {
            Some(encoded) => {
                match chronos_services::events_cursor::EventsCursorV1::decode_for_session(
                    encoded, &session,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok(CallToolResult::error(text_content(format!(
                            "invalid cursor for session '{}': {e}",
                            params.session_id
                        ))))
                    }
                }
            }
            None => chronos_services::events_cursor::EventsCursorV1::start(session.clone()),
        };

        let outcome = match params.mode {
            ExecutionLogReadKind::Poll => self
                .read_path
                .poll_batch(&params.session_id, &cursor, params.limit)
                .map(|batch| {
                    // `MockEvent` is intentionally not `Serialize` (it is
                    // a test-shaped adapter), so map it explicitly rather
                    // than widening a test type's public contract.
                    let events: Vec<serde_json::Value> = batch
                        .events
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "seq": e.seq.get(),
                                "kind": e.kind,
                                "payload": e.payload,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "mode": "poll",
                        "session_id": params.session_id,
                        "event_count": events.len(),
                        "events": events,
                        "next_cursor": batch.next_cursor.encode(),
                    })
                }),
            ExecutionLogReadKind::Summarize => {
                if params.bucket_size_ns == 0 {
                    return Ok(CallToolResult::error(text_content(
                        "bucket_size_ns must be > 0 (a zero-width bucket is not a time bucket)",
                    )));
                }
                self.read_path
                    .summarize(&params.session_id, &cursor, params.bucket_size_ns)
                    .map(|summary| {
                        serde_json::json!({
                            "mode": "summarize",
                            "session_id": params.session_id,
                            "bucket_size_ns": params.bucket_size_ns,
                            "total_events": summary.total_events,
                            "bucket_count": summary.bucket_count,
                            "mean_per_bucket": summary.mean_per_bucket,
                        })
                    })
            }
            ExecutionLogReadKind::Rollup => {
                self.read_path
                    .rollup(&params.session_id, &cursor)
                    .map(|rollup| {
                        serde_json::json!({
                            "mode": "rollup",
                            "session_id": params.session_id,
                            // The counts below are per GROUP, and this says
                            // what the group is. Without it, `invocation_count`
                            // reads as a count of invocations while the read
                            // path can only count threads — an agent would
                            // draw a conclusion the numbers cannot support.
                            "grouping_key": rollup.grouping_key.as_str(),
                            "grouping_is_identity": rollup.grouping_key.is_identity(),
                            "invocation_count": rollup.invocation_count,
                            "total_events": rollup.total_events,
                            "mean_per_invocation": rollup.mean_per_invocation,
                            "max_per_invocation": rollup.max_per_invocation,
                        })
                    })
            }
            ExecutionLogReadKind::Causality => {
                // Causality needs the engine half as well as the log half.
                // Only this mode takes the engine lock: the other three are
                // pure log reads and must not serialise behind it.
                //
                // REC-C1.7: gate before reading `engines`, exactly as
                // `execution_query`, `state_query` and `trace_slice` do.
                // This arm used to be the one reader of the map that did
                // not, so it answered from whatever a live drain happened
                // to leave behind — a drain that delivered a subset of the
                // log produced an answer here that disagreed with the
                // gated tools' for the same session in the same process.
                // The gate rebuilds from the log, so after it the engine
                // below is log-derived by construction rather than by
                // assumption.
                //
                // `gate_projection` (not `gate_projection_for_wire`) so a
                // failure stays on this tool's own error channel: every
                // other failure below is an `isError` envelope, and a
                // missing log is exactly what the tool description
                // promises to report that way. The registry's own reason
                // travels through unedited, so "no log here" is never
                // dressed up as an empty session.
                if let Err(e) = self.gate_projection(&params.session_id).await {
                    return Ok(CallToolResult::error(text_content(format!("{e}"))));
                }

                // `QueryEngine` is not `Clone`, so the engine is borrowed
                // under the guard rather than copied out of the map.
                let engines = self.engines.lock().await;
                // An engine that indexes no events cannot support a
                // "hints are available" claim. `causality_index_is_configured`
                // cannot be the test: `build_engine` always calls
                // `with_causality`, so it is structurally true for every
                // projection `IndexBuilder` produced, empty ones included,
                // and `causality_status` would promote that to `Wired`.
                // Gating makes "a readable log" imply "a projection
                // exists", so the honest signal left for "are there hints
                // to offer" is whether the projection holds evidence.
                let engine = engines
                    .get(&params.session_id)
                    .filter(|e| e.event_count() > 0);
                let engine_loaded = engine.is_some();
                self.read_path
                    .causality_status(&params.session_id, engine)
                    .map(|status| {
                        serde_json::json!({
                            "mode": "causality",
                            "session_id": params.session_id,
                            "status": format!("{status:?}"),
                            "engine_loaded": engine_loaded,
                        })
                    })
            }
        };

        match outcome {
            Ok(value) => Ok(CallToolResult::success(json_content(&value))),
            // SCALE_BUDGETS §5 D2: a budget stop is the one read failure whose
            // numbers an agent can act on — how far the walk got and the
            // cursor to resume from. So it gets the same treatment the
            // `cursor_stale` arm above already gives a re-anchorable error:
            // readable text AND a structured envelope with the numbers. A bare
            // `"{e}"` here would be honest but unusable, and an agent that
            // could not see the resume anchor would retry the whole
            // aggregation and stall on the same ceiling again.
            Err(chronos_services::read_path::ReadPathError::Budget(b)) => {
                let mut content = text_content(b.to_string());
                content.extend(json_content(&serde_json::json!({
                    "error": b.reason(),
                    "operation": b.operation,
                    "limit_field": b.limit.field(),
                    "limit_secs": b.limit_secs,
                    "max_events": b.max_events,
                    "elapsed_ms": b.elapsed.as_millis() as u64,
                    "events_scanned": b.events_scanned,
                    "pages_scanned": b.pages_scanned,
                    "next_seq": b.next_seq,
                    "partial": true,
                    "resume": "re-issue the same mode with cursor anchored at next_seq",
                })));
                Ok(CallToolResult::error(content))
            }
            // Fail-closed: the registry's own reason is surfaced, never
            // replaced by an empty-success envelope.
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    #[tool(
        name = "observe",
        description = "v2 dispatcher for observation/subscription/instrumentation. Select the operation via `verb` (create | list | update | delete | query). `verb=create` registers a tripwire or uprobe subscription; `verb=list` enumerates subscriptions and drains fired events (destructive); `verb=query` is a non-destructive snapshot; `verb=delete` unregisters a subscription by id; `verb=update` is reserved (rejected with `unsupported`). Supersedes the v1 `tripwire_create`, `tripwire_list`, `tripwire_delete`, `tripwire_query`, and `probe_inject` tools. See docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md (line 14)."
    )]
    async fn observe(
        &self,
        params: Parameters<ObserveParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = params.0;

        // Translate ObserveConditionWire → ObserveCondition. Tripwire
        // variants carry a serde_json::Value that must be re-parsed into
        // TripwireConditionType and then converted via .into_condition().
        let typed_condition = match params.condition {
            Some(ObserveConditionWire::Tripwire { condition, label }) => {
                let parsed_type: TripwireConditionType = match serde_json::from_value(condition) {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(CallToolResult::error(text_content(format!(
                            "observe: invalid tripwire condition JSON: {}",
                            e
                        ))));
                    }
                };
                match parsed_type.into_condition() {
                    Ok(c) => Some(chronos_services::output::ObserveCondition::Tripwire {
                        condition: c,
                        label,
                    }),
                    Err(bad) => {
                        return Ok(CallToolResult::error(text_content(format!(
                            "observe: unknown event_type '{}'. Valid types: syscall_enter, syscall_exit, function_entry, function_exit, variable_write, memory_write, signal_delivered, breakpoint_hit, thread_create, thread_exit, exception_thrown.",
                            bad
                        ))));
                    }
                }
            }
            Some(ObserveConditionWire::Uprobe {
                binary_path,
                symbol_name,
                pid,
                label,
            }) => Some(chronos_services::output::ObserveCondition::Uprobe {
                binary_path,
                symbol_name,
                pid,
                label,
            }),
            None => None,
        };

        // Translate ObserveScopeWire → ObserveScope.
        let typed_scope = match params.scope {
            Some(ObserveScopeWire::Session { session_id }) => {
                Some(chronos_services::output::ObserveScope::Session { session_id })
            }
            Some(ObserveScopeWire::Global) => Some(chronos_services::output::ObserveScope::Global),
            None => None,
        };

        // Translate ObserveRequestedEvidenceWire → ObserveRequestedEvidence.
        let typed_evidence = match params.requested_evidence {
            Some(ObserveRequestedEvidenceWire::EventTypes { event_types }) => {
                Some(chronos_services::output::ObserveRequestedEvidence::EventTypes { event_types })
            }
            Some(ObserveRequestedEvidenceWire::Properties { names }) => {
                Some(chronos_services::output::ObserveRequestedEvidence::Properties { names })
            }
            None => None,
        };

        // Build the ProbeContext for the dispatcher.
        let probe_ctx = chronos_services::probe::ProbeContext {
            live_probes: &self.live_probes,
            execution_logs: &self.execution_logs,
            engines: &self.engines,
            session_languages: &self.session_languages,
            tripwire_manager: &self.tripwire_manager,
            active_session: &self.active_session,
            uprobe_injector: &self.uprobe_injector,
            native_probe_factory: &self.native_probe_factory,
        };

        let ctx = ObserveContext {
            tripwire_manager: &self.tripwire_manager,
            probe: &probe_ctx,
            uprobe_counter: &self.uprobe_counter,
        };

        let input = ObserveInput {
            verb: params.verb,
            subscription_id: params.subscription_id,
            condition: typed_condition,
            action: params.action,
            retention: params.retention,
            requested_evidence: typed_evidence,
            scope: typed_scope,
            cursor: params.cursor,
            label: params.label,
        };

        match ChronosObserveService::observe(&ctx, input).await {
            Ok(out) => Ok(CallToolResult::success(json_content(
                &serde_json::to_value(&out)
                    .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
            ))),
            Err(ServiceError::Unsupported(s)) => Ok(CallToolResult::error(text_content(format!(
                "observe: unsupported: {}",
                s
            )))),
            Err(ServiceError::TripwireNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("observe: tripwire '{}' not found", s),
            ))),
            Err(ServiceError::InvalidTripwireIdFormat(s)) => Ok(CallToolResult::error(
                text_content(format!("observe: invalid tripwire id format: '{}'", s)),
            )),
            Err(ServiceError::LockPoisoned) => Ok(CallToolResult::error(text_content(
                "observe: lock poisoned",
            ))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("observe: {e}")))),
        }
    }

    /// Shared dispatcher for `session_compare` v2 + the two v1 shims
    /// (`compare_sessions` → kind=divergence, `performance_regression_audit`
    /// → kind=regression). All three tool wrappers funnel through here so the
    /// validation and error mapping stay identical; only the response wire
    /// shape differs, and `wire` selects it.
    async fn dispatch_session_compare(
        ctx: &chronos_services::session_compare::SessionCompareContext,
        params: SessionCompareParams,
        wire: SessionCompareWire,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let input = chronos_services::output::SessionCompareInput {
            kind: parse_session_compare_kind(&params.kind)?,
            session_a: Some(params.session_a.clone()),
            session_b: Some(params.session_b.clone()),
            baseline_session_id: Some(params.session_a),
            target_session_id: Some(params.session_b),
            top_n: params.top_n,
        };
        match chronos_services::session_compare::ChronosSessionCompareService::compare(ctx, input) {
            Ok(out) => match wire.to_value(out) {
                Ok(v) => Ok(CallToolResult::success(json_content(&v))),
                Err(e) => Ok(CallToolResult::error(text_content(format!(
                    "Serialization error: {}",
                    e
                )))),
            },
            Err(ServiceError::SessionNotFound(s)) => Ok(CallToolResult::error(text_content(
                format!("session '{}' not found", s),
            ))),
            Err(ServiceError::InvalidInput(s)) => Ok(CallToolResult::error(text_content(s))),
            Err(e) => Ok(CallToolResult::error(text_content(format!("{e}")))),
        }
    }

    // ========================================================================
    // M8 — Counterexample MCP wrappers (m8-03)
    // ========================================================================

    /// `counterexample_shrink` — run proptest's shrink loop on a failing
    /// hypothesis. Returns the freshly-persisted bundle summary and the
    /// full-bundle DTO (events_count + minimised payload).
    #[tool(
        name = "counterexample_shrink",
        description = "Minimize a failing hypothesis by running proptest's shrink loop on the captured trace events. Returns the bundle summary plus a CounterexampleBundleDto carrying events_count and the minimised payload shape. m8-03 ships with Just(value) strategies (no real shrinking yet); rounds_used reports the round count the runner actually used."
    )]
    async fn counterexample_shrink(
        &self,
        params: Parameters<CounterexampleShrinkParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;

        // Build the wire -> HypothesisInput bridge.
        let target = chronos_services::hypothesis_test::HypothesisInput {
            session_id: p.target_hypothesis.session_id,
            kind: p.target_hypothesis.kind,
            scope: p.target_hypothesis.scope,
            comparison: p.target_hypothesis.comparison,
            constant: p.target_hypothesis.constant,
            property_target: p.target_hypothesis.property_target,
            predicate: p.target_hypothesis.predicate,
            caller: p.target_hypothesis.caller,
            callee: p.target_hypothesis.callee,
            max_depth: p.target_hypothesis.max_depth.map(|d| d as usize),
        };
        let hyp_ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let counterexample_ctx = chronos_services::counterexample::CounterexampleContext {
            repository: std::sync::Arc::clone(&self.counterexample_repository),
            hypothesis_ctx: &hyp_ctx,
        };
        let input = chronos_services::counterexample::CounterexampleShrinkInput::Shrink {
            property_kind: p.property_kind,
            target_hypothesis: target,
            max_rounds: p
                .max_rounds
                .unwrap_or(chronos_services::counterexample::DEFAULT_SHRINK_MAX_ROUNDS),
            seed: p.seed,
        };
        match chronos_services::counterexample::ChronosCounterexampleService::shrink(
            &counterexample_ctx,
            input,
        )
        .await
        {
            Ok(out) => {
                let v = serialize_counterexample_output(out);
                Ok(CallToolResult::success(json_content(&v)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "counterexample_shrink failed: {e}"
            )))),
        }
    }

    /// `counterexample_get` — retrieve a previously persisted bundle by id.
    #[tool(
        name = "counterexample_get",
        description = "Read a persisted counterexample bundle from the redb counterexample_bundles table by bundle_id. Returns CounterexampleGetOutputDto { bundle, has_full_bundle }. Returns Err(LoadFailed) when no such bundle exists."
    )]
    async fn counterexample_get(
        &self,
        params: Parameters<CounterexampleGetParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let hyp_ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let counterexample_ctx = chronos_services::counterexample::CounterexampleContext {
            repository: std::sync::Arc::clone(&self.counterexample_repository),
            hypothesis_ctx: &hyp_ctx,
        };
        match chronos_services::counterexample::ChronosCounterexampleService::get(
            &counterexample_ctx,
            &p.bundle_id,
        ) {
            Ok(out) => {
                let v = serialize_counterexample_output(out);
                Ok(CallToolResult::success(json_content(&v)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "counterexample_get failed: {e}"
            )))),
        }
    }

    /// `counterexample_list` — list bundle summaries matching optional filters.
    #[tool(
        name = "counterexample_list",
        description = "List counterexample bundle summaries from redb, optionally filtered by workspace_id, property_kind, since_ms, until_ms, and limit. Returns CounterexampleListOutputDto { bundles, next_cursor }. m8-05 (B2): forward pagination — when the page is full (len == limit), next_cursor is the bundle_id to pass back as `cursor` for the next page; otherwise next_cursor is null."
    )]
    async fn counterexample_list(
        &self,
        params: Parameters<CounterexampleListParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let hyp_ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let counterexample_ctx = chronos_services::counterexample::CounterexampleContext {
            repository: std::sync::Arc::clone(&self.counterexample_repository),
            hypothesis_ctx: &hyp_ctx,
        };
        let filter = chronos_services::counterexample::CounterexampleListFilter {
            workspace_id: p.workspace_id,
            property_kind: p.property_kind,
            since_ms: p.since_ms,
            until_ms: p.until_ms,
            limit: p.limit.unwrap_or(50),
            cursor: p.cursor,
        };
        match chronos_services::counterexample::ChronosCounterexampleService::list(
            &counterexample_ctx,
            filter,
        ) {
            Ok(out) => {
                let v = serialize_counterexample_output(out);
                Ok(CallToolResult::success(json_content(&v)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "counterexample_list failed: {e}"
            )))),
        }
    }

    /// m8-05 (B3): lightweight accessor — return the persisted
    /// `events_count` of a bundle without re-emitting the summary.
    /// Saves an LLM round-trip + a full-bundle-deserialize when all
    /// it needs is the count. Returns
    /// `CounterexampleEventsCountOutputDto { bundle_id, events_count }`.
    /// Errors with `LoadFailed` when the bundle_id does not exist.
    #[tool(
        name = "counterexample_events_count",
        description = "Return the events_count of a persisted counterexample bundle without re-emitting the full summary. m8-05 (B3) accessor; m8-05 R3: the count is the one persisted at save() time, not a live re-read of the engine. Input: bundle_id. Output: {bundle_id, events_count}."
    )]
    async fn counterexample_events_count(
        &self,
        params: Parameters<CounterexampleEventsCountParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let hyp_ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let counterexample_ctx = chronos_services::counterexample::CounterexampleContext {
            repository: std::sync::Arc::clone(&self.counterexample_repository),
            hypothesis_ctx: &hyp_ctx,
        };
        match chronos_services::counterexample::ChronosCounterexampleService::events_count(
            &counterexample_ctx,
            &p.bundle_id,
        ) {
            Ok(out) => {
                let v = serialize_counterexample_output(out);
                Ok(CallToolResult::success(json_content(&v)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "counterexample_events_count failed: {e}"
            )))),
        }
    }

    /// m9-91: closes m9-02-R4. Returns the events stream for a
    /// counterexample bundle. The corresponding storage primitive
    /// (`counterexample_storage::bundle_events_or_legacy`) has shipped
    /// since m9-02; m9-91 adds the MCP surface. The response envelope
    /// is `CounterexampleBundleEventsOutputDto { bundle_id, events_count,
    /// returned_events, next_offset }`:
    ///
    /// - `events_count` is the total persisted (independent of paging).
    /// - `returned_events` is the slice (empty if offset is past the end).
    /// - `next_offset` is `Some(n)` to continue paging, or `None` when
    ///   no more events remain.
    ///
    /// Errors with `LoadFailed` when the bundle_id does not exist.
    #[tool(
        name = "counterexample_bundle_events",
        description = "Return the events stream of a persisted counterexample bundle (m9-91 — closes m9-02-R4). Reads via the m9-02 v3 side-table. Input: {bundle_id, limit?, offset?}. Output: {bundle_id, events_count, returned_events, next_offset}. `events_count` is the total persisted (not the slice size); `next_offset == null` when no more events remain or the offset was already past the end."
    )]
    async fn counterexample_bundle_events(
        &self,
        params: Parameters<CounterexampleBundleEventsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let hyp_ctx = chronos_services::hypothesis_test::HypothesisTestContext {
            engines: &self.engines,
        };
        let counterexample_ctx = chronos_services::counterexample::CounterexampleContext {
            repository: std::sync::Arc::clone(&self.counterexample_repository),
            hypothesis_ctx: &hyp_ctx,
        };
        match chronos_services::counterexample::ChronosCounterexampleService::events(
            &counterexample_ctx,
            &p.bundle_id,
            p.limit,
            p.offset,
        ) {
            Ok(out) => {
                let v = serialize_counterexample_output(out);
                Ok(CallToolResult::success(json_content(&v)))
            }
            Err(e) => Ok(CallToolResult::error(text_content(format!(
                "counterexample_bundle_events failed: {e}"
            )))),
        }
    }
}

/// Serialize a `CounterexampleOutput` into the m8-03 wire DTO envelope.
///
/// - `Got` → `CounterexampleGetOutputDto` (without `full`).
/// - `Saved` → `{ saved: summary }` envelope (m8-04 close-time work will
///   re-use this for the persist path).
/// - `Listed` → `CounterexampleListOutputDto`.
/// - `Shrunk` → `CounterexampleShrinkOutputDto` with the full-bundle
///   payload populated (events_count=0 placeholder; minimised payload
///   shape taken from `target_hypothesis.session_id`).
fn serialize_counterexample_output(
    out: chronos_services::counterexample::CounterexampleOutput,
) -> serde_json::Value {
    use chronos_services::counterexample::CounterexampleOutput as COut;
    use chronos_services::output::{
        CounterexampleBundleDto, CounterexampleBundleEventsOutputDto,
        CounterexampleBundleSummaryDto, CounterexampleEventsCountOutputDto,
        CounterexampleGetOutputDto, CounterexampleListOutputDto, CounterexampleMinimisedDto,
        CounterexampleShrinkOutputDto,
    };

    match out {
        COut::Got { summary } => {
            let has_full = summary.has_full_bundle;
            let bundle = CounterexampleBundleSummaryDto {
                bundle_id: summary.bundle_id,
                property_kind: summary.property_kind,
                workspace_id: summary.workspace_id,
                created_at_ms: summary.created_at_ms,
                rounds_used: summary.rounds_used,
                has_full_bundle: summary.has_full_bundle,
            };
            serde_json::to_value(CounterexampleGetOutputDto {
                bundle,
                has_full_bundle: has_full,
            })
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        }
        COut::Saved {
            summary,
            events_count,
        } => {
            // m8-04 (B5 rework): the `{"saved": <summary>}` stopgap is
            // replaced with the `{bundle, events_count}` shape that
            // `Shrunk` and `Got` already use. Reuses the existing
            // `CounterexampleBundleSummaryDto` for the `bundle` field;
            // `minimised`/`minimised_kind` are intentionally omitted
            // from this variant (Saved has no minimised payload —
            // that's a Shrunk-only field).
            let bundle = CounterexampleBundleSummaryDto {
                bundle_id: summary.bundle_id,
                property_kind: summary.property_kind,
                workspace_id: summary.workspace_id,
                created_at_ms: summary.created_at_ms,
                rounds_used: summary.rounds_used,
                has_full_bundle: summary.has_full_bundle,
            };
            serde_json::json!({
                "bundle": bundle,
                "events_count": events_count,
            })
        }
        COut::Listed {
            summaries,
            next_cursor,
        } => {
            let bundles = summaries
                .into_iter()
                .map(|s| CounterexampleBundleSummaryDto {
                    bundle_id: s.bundle_id,
                    property_kind: s.property_kind,
                    workspace_id: s.workspace_id,
                    created_at_ms: s.created_at_ms,
                    rounds_used: s.rounds_used,
                    has_full_bundle: s.has_full_bundle,
                })
                .collect::<Vec<_>>();
            serde_json::to_value(CounterexampleListOutputDto {
                bundles,
                next_cursor,
            })
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        }
        COut::Shrunk {
            bundle,
            rounds_used,
            minimised_constant,
            minimised_predicate,
            minimised_call_path,
            events_count,
        } => {
            // Build the minimised payload discriminated union.
            let (minimised, minimised_kind) = match bundle.property_kind {
                chronos_services::output::HypothesisKind::Invariant => {
                    let p = minimised_constant.clone().unwrap_or_default();
                    let d = CounterexampleMinimisedDto {
                        constant: Some(p),
                        predicate: None,
                        caller: None,
                        callee: None,
                        max_depth: None,
                    };
                    (Some(d), Some("constant".to_string()))
                }
                chronos_services::output::HypothesisKind::Existence => {
                    let p = minimised_predicate.clone().unwrap_or_else(|| {
                        chronos_services::output::ExistencePredicate::EventTypeEquals {
                            event_type: String::new(),
                        }
                    });
                    let d = CounterexampleMinimisedDto {
                        constant: None,
                        predicate: Some(p),
                        caller: None,
                        callee: None,
                        max_depth: None,
                    };
                    (Some(d), Some("predicate".to_string()))
                }
                chronos_services::output::HypothesisKind::CallPath => {
                    let (caller, callee, max_depth) = minimised_call_path
                        .clone()
                        .unwrap_or_else(|| (String::new(), String::new(), None));
                    let d = CounterexampleMinimisedDto {
                        constant: None,
                        predicate: None,
                        caller: Some(caller),
                        callee: Some(callee),
                        max_depth: max_depth.map(|d| d as u64),
                    };
                    (Some(d), Some("call_path".to_string()))
                }
            };
            let summary = CounterexampleBundleSummaryDto {
                bundle_id: bundle.bundle_id.clone(),
                property_kind: bundle.property_kind,
                workspace_id: bundle.workspace_id.clone(),
                created_at_ms: bundle.created_at_ms,
                rounds_used,
                has_full_bundle: bundle.has_full_bundle,
            };
            let full = CounterexampleBundleDto {
                summary: summary.clone(),
                events_count, // m8-04: was hardcoded 0 in m8-03
                minimised,
                minimised_kind,
            };
            serde_json::to_value(CounterexampleShrinkOutputDto {
                bundle: summary,
                full,
                rounds_used,
            })
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        }
        COut::EventsCount {
            bundle_id,
            events_count,
        } => {
            // m8-05 (B3): just `{bundle_id, events_count}`. No summary,
            // no events payload, no minimised. The lightweight accessor
            // exists so an LLM agent that only wants the count doesn't
            // have to deserialize the full bundle.
            serde_json::to_value(CounterexampleEventsCountOutputDto {
                bundle_id,
                events_count,
            })
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        }
        COut::Events {
            bundle_id,
            events_count,
            returned_events,
            next_offset,
        } => {
            // m9-91: closes m9-02-R4. The events stream accessor for a
            // counterexample bundle. Returns `{bundle_id, events_count,
            // returned_events, next_offset}` — events_count is the
            // total persisted on disk (independent of the slice),
            // `next_offset == None` means no more events remain.
            serde_json::to_value(CounterexampleBundleEventsOutputDto {
                bundle_id,
                events_count,
                returned_events,
                next_offset,
            })
            .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        }
    }
}

// mod tests is NOT cfg-gated (intentional pre-existing structure). Helper
// functions inside `mod tests` are still flagged dead_code when not built
// with `--cfg test`; the prior fixup commit instead annotated each helper
// with #[allow(dead_code)] where invoked.
mod tests {
    use super::*;

    #[test]
    fn test_event_type_from_snake_case_round_trip() {
        // MS-EVT-TYPED: single-owner mapping in chronos-domain; all 21
        // variants round-trip Display -> from_snake_case -> identity.
        let all = [
            EventType::SyscallEnter,
            EventType::SyscallExit,
            EventType::FunctionEntry,
            EventType::FunctionExit,
            EventType::VariableWrite,
            EventType::MemoryWrite,
            EventType::SignalDelivered,
            EventType::BreakpointHit,
            EventType::ThreadCreate,
            EventType::ThreadExit,
            EventType::ExceptionThrown,
            EventType::VariableRead,
            EventType::MemoryAlloc,
            EventType::MemoryFree,
            EventType::MemoryRead,
            EventType::ThreadSwitch,
            EventType::WatchTrigger,
            EventType::ExceptionCaught,
            EventType::InvocationIncomplete,
            EventType::Custom,
            EventType::Unknown,
        ];
        assert_eq!(
            all.len(),
            21,
            "EventType variant count changed; update this list"
        );
        for et in all {
            let name = et.to_string();
            assert_eq!(
                EventType::from_snake_case(&name),
                Some(et),
                "round-trip failed for {name}"
            );
        }
        assert_eq!(EventType::from_snake_case("unknown_type"), None);
    }

    #[test]
    fn test_server_new() {
        let _server = ChronosServer::new();
    }

    #[test]
    fn test_server_default() {
        let _server = ChronosServer::default();
    }

    // ========================================================================
    // m9-82: degraded-store disclosure (closes FIND-M9-75).
    //
    // `ChronosServer::is_degraded()` reports whether the underlying store
    // is in-memory (true) or file-backed (false). The session-persistence
    // tool envelopes (save_session / list_sessions / load_session /
    // delete_session / drop_session) include a top-level `degraded` field
    // matching this accessor so MCP callers can confirm the runtime is
    // not persisting to disk.
    // ========================================================================

    #[test]
    fn test_server_is_degraded_true_for_in_memory_test_store() {
        // Under cfg(test), ChronosServer::new() builds the store from
        // SessionStore::in_memory() (see try_open_default_store under
        // cfg(test) at ~line 1500). is_degraded() must therefore be true.
        let server = ChronosServer::new();
        assert!(
            server.is_degraded(),
            "test server builds from in_memory store; expected is_degraded() == true",
        );
    }

    #[test]
    fn test_session_envelope_injects_degraded_at_top_level() {
        // m9-82: helper preserves the existing object shape and only
        // adds the `degraded` key. REQ-M9-82-04 (wire-shape additivity).
        let original = serde_json::json!({
            "session_id": "abc",
            "status": "saved",
            "event_count": 7,
        });
        let wrapped = session_envelope(true, original.clone());
        let obj = wrapped.as_object().expect("must be a JSON object");
        assert_eq!(obj.get("degraded"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(obj.get("session_id"), original.get("session_id"));
        assert_eq!(obj.get("status"), original.get("status"));
        assert_eq!(obj.get("event_count"), original.get("event_count"));
        // Same key count minus zero (no fields lost) plus one (degraded).
        assert_eq!(obj.len(), original.as_object().unwrap().len() + 1);
    }

    #[test]
    fn test_session_envelope_wraps_non_object_defensively() {
        // m9-82: if a future caller passes a bare array/string/number,
        // the helper must still produce a top-level object so the wire
        // contract is preserved.
        let wrapped = session_envelope(false, serde_json::json!([1, 2, 3]));
        let obj = wrapped.as_object().expect("must be a JSON object");
        assert_eq!(obj.get("degraded"), Some(&serde_json::Value::Bool(false)));
        assert!(obj.get("result").is_some());
    }

    #[tokio::test]
    async fn test_list_sessions_envelope_includes_degraded_true() {
        // m9-82: under cfg(test) the server's store is in-memory, so the
        // list_sessions JSON envelope must carry `degraded: true` at the
        // top level. Closes REQ-M9-82-03 scenario `list_sessions_includes_degraded`.
        let server = ChronosServer::new();
        // Pre-condition: the accessor agrees with what the envelope must say.
        assert!(server.is_degraded());

        let list_result = server
            .list_sessions(Parameters(NoParams {}))
            .await
            .expect("list_sessions call");
        assert_ne!(list_result.is_error, Some(true));

        // Round-trip the content through serde_json::Value so we can
        // assert on the parsed JSON shape rather than the Debug
        // representation. The tool emits exactly one text content block.
        let v: serde_json::Value =
            serde_json::to_value(&list_result.content[0]).expect("content is JSON");
        let obj = v
            .get("text")
            .and_then(|t| t.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .expect("content must round-trip to a JSON object envelope");
        let obj = obj.as_object().expect("top-level must be a JSON object");
        assert_eq!(
            obj.get("degraded"),
            Some(&serde_json::Value::Bool(true)),
            "list_sessions envelope must include `degraded: true` for an in-memory store",
        );
        // REQ-M9-82-04: existing fields are preserved.
        assert!(obj.contains_key("session_count"));
        assert!(obj.contains_key("sessions"));
    }

    #[tokio::test]
    async fn test_save_session_envelope_includes_degraded() {
        // m9-82: save_session envelope must also carry the flag. Use the
        // existing helper shape (build + save) so the assertion exercises
        // the real envelope construction path, not a hand-built one.
        let server = ChronosServer::new();
        let sid = "m9-82-save-envelope".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];
        server
            .build_and_store_engine(&sid, events, Language::C)
            .await;

        let result = server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid.clone(),
                language: "native".to_string(),
                target: "/bin/m9-82".to_string(),
            }))
            .await
            .expect("save_session call");
        assert_ne!(result.is_error, Some(true));

        let v: serde_json::Value =
            serde_json::to_value(&result.content[0]).expect("content is JSON");
        let obj = v
            .get("text")
            .and_then(|t| t.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .expect("content must round-trip to a JSON object envelope");
        let obj = obj.as_object().expect("top-level must be a JSON object");
        assert_eq!(
            obj.get("degraded"),
            Some(&serde_json::Value::Bool(true)),
            "save_session envelope must include `degraded: true` for an in-memory store",
        );
        assert_eq!(obj.get("session_id"), Some(&serde_json::Value::String(sid)));
        assert_eq!(
            obj.get("status"),
            Some(&serde_json::Value::String("saved".to_string()))
        );
    }

    // ========================================================================
    // SF5 — Symbol Subscription Tests (Phase 12)
    // ========================================================================
    // SF4 tool tests
    // ========================================================================

    /// Helper: build a server with a pre-loaded session from synthetic events.
    #[allow(dead_code)]
    async fn server_with_session(events: Vec<TraceEvent>) -> (ChronosServer, String) {
        let server = ChronosServer::new();
        let session_id = "test-session-sf4".to_string();
        server
            .build_and_store_engine(&session_id, events, Language::C)
            .await;
        (server, session_id)
    }

    #[allow(dead_code)]
    fn make_fn_entry(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    #[allow(dead_code)]
    fn make_fn_exit(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::FunctionExit,
            loc,
            EventData::Empty,
        )
    }

    // ========================================================================
    // SF5 Persistence Tool Tests (T16)
    // ========================================================================

    #[allow(dead_code)]
    fn make_fn_event(id: u64, ts: u64, tid: u64, func: &str) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        let loc = SourceLocation::new("", 0, func, 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::FunctionEntry,
            loc,
            EventData::Function {
                name: func.to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    /// A unique temporary directory, so parallel tests never share a store path.
    #[allow(dead_code)]
    fn unique_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "chronos-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// Minimal `SessionMetadata` for store-level assertions (m9-75).
    #[allow(dead_code)]
    fn test_session_metadata(session_id: &str) -> chronos_store::SessionMetadata {
        chronos_store::SessionMetadata {
            session_id: session_id.to_string(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: 1,
            duration_ms: 0,
            tail_sealed: false,
            sealed_at: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_session_roundtrip() {
        let server = ChronosServer::new();
        let sid = "test-persist-session".to_string();
        let events = vec![
            make_fn_event(0, 100, 1, "main"),
            make_fn_event(1, 200, 1, "helper"),
        ];

        // Build engine manually
        server
            .build_and_store_engine(&sid, events.clone(), Language::C)
            .await;

        // Save session
        let save_result = server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(save_result.is_error, Some(true));
        let text = format!("{:?}", save_result.content);
        assert!(text.contains("saved") || text.contains("event_count"));

        // Load session
        let load_result = server
            .load_session(Parameters(LoadSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(load_result.is_error, Some(true));
        let text = format!("{:?}", load_result.content);
        assert!(text.contains("loaded") || text.contains("event_count"));
    }

    /// Discriminante for the `projection_meta` eviction.
    ///
    /// `projection_meta` is populated by `ensure_projection` and by nothing
    /// else, so before this test it had no coverage at all: the previous
    /// version of this test set the session up through `build_and_store_engine`,
    /// which writes `engines` and `session_languages` but never
    /// `projection_meta`, so it could not have observed the map even by
    /// accident. It asserted "all in-memory state is gone" over three of the
    /// four session maps.
    ///
    /// The entry is inserted directly rather than by calling `ensure_projection`,
    /// which needs a real `SessionExecutionLog`. What is under test is the
    /// eviction, not the shape of `ProjectionMeta`.
    ///
    /// Reverting the fix — dropping the `projection_meta` eviction from
    /// `cleanup_session_memory` — makes this fail with the entry still
    /// present.
    #[tokio::test]
    async fn test_cleanup_session_memory_removes_all_state() {
        let server = ChronosServer::new();
        let sid = "cleanup-test-session".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Build engine (registers in engines + session_languages)
        server
            .build_and_store_engine(&sid, events, Language::Python)
            .await;

        // Stand in for `ensure_projection`, the only writer of this map.
        server.projection_meta.lock().await.insert(
            sid.clone(),
            ProjectionMeta {
                session_id: chronos_domain::SessionId::new(sid.clone()),
                projected_from: chronos_domain::EventSeq::ZERO,
                projected_through: Some(chronos_domain::EventSeq::ZERO),
                completeness: projection::ProjectionCompleteness::Full,
                source: projection::ProjectionSource::ExecutionLog,
            },
        );

        // Verify it's registered
        assert!(server.engines.lock().await.contains_key(&sid));
        assert!(server.session_languages.lock().await.contains_key(&sid));
        assert!(server.projection_meta.lock().await.contains_key(&sid));

        // Cleanup
        server.cleanup_session_memory(&sid).await;

        // Verify all in-memory state is gone
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
        assert!(!server.connected_sessions.lock().unwrap().contains(&sid));
        assert!(
            !server.projection_meta.lock().await.contains_key(&sid),
            "cleanup must evict projection_meta too, otherwise every session \
             recovered from the store leaks an entry that outlives it"
        );
    }

    /// Positive control for the neighbouring map: a projection entry for a
    /// *different* session must survive another session's cleanup. Without
    /// this, the guard above could be satisfied by clearing the whole map.
    // --- REC-C1.7 projection gate ---
    //
    // `ensure_projection` is the only writer of `projection_meta`, and before
    // these tests nothing in the workspace called it. It is not a cold path:
    // `execution_query`, `state_query` and `trace_slice` all go through
    // `gate_projection_for_wire` on every request, so the gate that keeps
    // TRUTH-001 closed — the QueryEngine is a reconstructible projection of
    // the log, never a second authority — had no coverage at all.
    //
    // Each test below registers a real but empty `SessionExecutionLog`, so
    // `projection::build_engine` answers `ProjectionCompleteness::Empty`.
    // That is a truthful answer rather than a stub: an empty log really does
    // describe a session with no records, and `meta_is_full` deliberately
    // accepts it. Only `Truncated` is refused, because there the engine would
    // silently miss retired history.
    //
    // The fixture is inlined rather than extracted into a helper: this module
    // is not cfg-gated, so a helper used only by `#[tokio::test]` functions
    // reads as dead code in the ordinary build, and silencing that would mean
    // adding an `#[allow(dead_code)]` like the ones already in this file.

    #[tokio::test]
    async fn ensure_projection_registers_the_session_in_both_maps() {
        let sid = "rec-c1-7-both-maps";
        let server = ChronosServer::new();
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-7-both-maps-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        server
            .execution_logs
            .register(
                chronos_services::session_log::SessionExecutionLog::create_for_tests(
                    &log_dir,
                    chronos_log::SessionId::new(sid),
                )
                .expect("test execution log"),
            )
            .expect("register the test log");

        let meta = server
            .ensure_projection(sid)
            .await
            .expect("a session with a log must project");

        assert_eq!(
            meta.completeness,
            projection::ProjectionCompleteness::Empty,
            "an empty log must project as Empty, not as Full"
        );
        assert!(
            server.engines.lock().await.contains_key(sid),
            "a projection without a registered engine is unreachable by the \
             services that read `engines`"
        );
        assert!(
            server.projection_meta.lock().await.contains_key(sid),
            "the gate reads `projection_meta` first; an entry that is not there \
             cannot be gated"
        );
    }

    /// Discriminante for the claim that eviction is safe.
    ///
    /// `cleanup_session_memory` drops the `projection_meta` entry, and the
    /// justification recorded there is that a cache miss is the normal path and
    /// rebuilds from the log. That justification was a comment and nothing
    /// else, which is the same failure mode as the leak it was written to
    /// justify: an invariant asserted in prose with no test behind it.
    ///
    /// Here the cache entry is dropped while the log stays registered. If a
    /// missing meta were ever treated as "not projected, refuse", this would
    /// fail instead of rebuilding.
    #[tokio::test]
    async fn ensure_projection_rebuilds_after_the_cache_entry_is_dropped() {
        let sid = "rec-c1-7-rebuild-on-miss";
        let server = ChronosServer::new();
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-7-rebuild-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        server
            .execution_logs
            .register(
                chronos_services::session_log::SessionExecutionLog::create_for_tests(
                    &log_dir,
                    chronos_log::SessionId::new(sid),
                )
                .expect("test execution log"),
            )
            .expect("register the test log");

        let first = server
            .ensure_projection(sid)
            .await
            .expect("first projection");
        assert!(server.projection_meta.lock().await.contains_key(sid));

        // Drop only the cache, keeping the log: a miss must rebuild.
        server.projection_meta.lock().await.remove(sid);
        assert!(!server.projection_meta.lock().await.contains_key(sid));

        let rebuilt = server
            .ensure_projection(sid)
            .await
            .expect("a cache miss must rebuild from the log, not report not-found");

        assert_eq!(rebuilt.completeness, first.completeness);
        assert!(
            server.projection_meta.lock().await.contains_key(sid),
            "the rebuild must repopulate the cache, not answer without it"
        );
        assert!(
            server.engines.lock().await.contains_key(sid),
            "the rebuild must repopulate the engine as well; inserting only the \
             meta would break the mirror in the other direction"
        );
    }

    /// The gate accepts a genuinely empty projection and refuses only a
    /// truncated one. Pinned because the distinction is easy to get backwards:
    /// refusing `Empty` would break every query against an idle session, and
    /// accepting `Truncated` would let a query answer as if retired history
    /// had never existed.
    #[tokio::test]
    async fn gate_projection_accepts_an_empty_projection() {
        let sid = "rec-c1-7-empty-accepted";
        let server = ChronosServer::new();
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-7-empty-accepted-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        server
            .execution_logs
            .register(
                chronos_services::session_log::SessionExecutionLog::create_for_tests(
                    &log_dir,
                    chronos_log::SessionId::new(sid),
                )
                .expect("test execution log"),
            )
            .expect("register the test log");

        let meta = server
            .gate_projection(sid)
            .await
            .expect("an empty session is a real answer, not a gate failure");
        assert_eq!(meta.completeness, projection::ProjectionCompleteness::Empty);
    }

    /// The refusal that is the point of the gate, end to end.
    ///
    /// A projection built from a log whose early segments were retired must
    /// not let a query through, because the engine would answer as if that
    /// history had never existed. `chronos-services` covers the decision
    /// itself — `meta_is_full` on all three completeness states, and the fact
    /// that it never disagrees with `require_full_history` — and that coverage
    /// builds a truncated projection from a real log through
    /// `advance_retained_from` plus `compact_retired`.
    ///
    /// What is checked here is the wiring: that `gate_projection` really does
    /// refuse rather than pass the meta through. The truncated meta is seeded
    /// into the cache, which is the same shape a long-lived server holds after
    /// retention advances, and it also exercises the cache-hit fast path.
    #[tokio::test]
    async fn gate_projection_refuses_a_truncated_projection() {
        let sid = "rec-c1-7-gate-refuses-truncated";
        let server = ChronosServer::new();

        server.projection_meta.lock().await.insert(
            sid.to_string(),
            ProjectionMeta {
                session_id: chronos_domain::SessionId::new(sid),
                projected_from: chronos_domain::EventSeq::new(5),
                projected_through: Some(chronos_domain::EventSeq::new(9)),
                completeness: projection::ProjectionCompleteness::Truncated {
                    retained_from: chronos_domain::EventSeq::new(5),
                },
                source: projection::ProjectionSource::ExecutionLog,
            },
        );

        match server.gate_projection(sid).await {
            Err(ServiceError::EvidenceUnavailableDueToRetention { retained_from }) => {
                assert_eq!(
                    retained_from, 5,
                    "the error must name the boundary so the caller can say \
                     which history is missing"
                );
            }
            Err(other) => panic!("expected a retention error, got {other:?}"),
            Ok(meta) => panic!(
                "the gate passed a truncated projection through: {:?}",
                meta.completeness
            ),
        }
    }

    /// Positive control for the refusal above: a session whose projection is
    /// still complete must keep passing. Without this, the guard could be
    /// satisfied by refusing everything.
    #[tokio::test]
    async fn gate_projection_still_passes_a_complete_projection() {
        let sid = "rec-c1-7-gate-passes-full";
        let server = ChronosServer::new();
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-7-gate-passes-full-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        server
            .execution_logs
            .register(
                chronos_services::session_log::SessionExecutionLog::create_for_tests(
                    &log_dir,
                    chronos_log::SessionId::new(sid),
                )
                .expect("test execution log"),
            )
            .expect("register the test log");
        server.projection_meta.lock().await.insert(
            sid.to_string(),
            ProjectionMeta {
                session_id: chronos_domain::SessionId::new(sid),
                projected_from: chronos_domain::EventSeq::ZERO,
                projected_through: Some(chronos_domain::EventSeq::new(4)),
                completeness: projection::ProjectionCompleteness::Full,
                source: projection::ProjectionSource::ExecutionLog,
            },
        );

        let meta = server
            .gate_projection(sid)
            .await
            .expect("a complete projection must pass the gate");
        assert_eq!(meta.completeness, projection::ProjectionCompleteness::Full);
    }

    /// The server-built engine must index exactly the events the projection
    /// would keep.
    ///
    /// `build_and_store_engine` and `chronos_services::projection::build_engine`
    /// are the two ways a `QueryEngine` comes into existence, and REC-C1.7 only
    /// holds if they agree. They now share one definition of noise — the filter
    /// here calls `projection::is_noisy` — but sharing the definition is only
    /// worth anything if the *result* is checked, so this asserts the surviving
    /// set directly against `is_noisy` rather than against a hardcoded count.
    ///
    /// Before the shared filter existed, the server kept a byte-identical copy
    /// of the predicate with no test on this side at all. Re-inlining a
    /// divergent copy here would leave the count wrong and turn this red.
    #[tokio::test]
    async fn build_and_store_engine_indexes_exactly_the_non_noisy_events() {
        use chronos_domain::{EventData, EventType, SourceLocation};

        let server = ChronosServer::new();
        let sid = "noise-filter-parity";

        // Two kinds of infrastructure noise the tracer emits: a raw register
        // snapshot carried as a Custom event, and an unclassified ptrace stop.
        let register_snapshot = TraceEvent::new(
            90,
            MonotonicNs::from(90),
            1,
            EventType::Custom,
            SourceLocation::default(),
            EventData::Registers(Default::default()),
        );
        let unknown_stop = TraceEvent::new(
            91,
            MonotonicNs::from(91),
            1,
            EventType::Unknown,
            SourceLocation::default(),
            EventData::Empty,
        );

        let mut events = vec![make_fn_event(0, 100, 1, "main"), register_snapshot];
        events.push(make_fn_event(1, 101, 1, "work"));
        events.push(unknown_stop);
        events.push(make_fn_event(2, 102, 1, "tail"));

        let expected: Vec<u64> = events
            .iter()
            .filter(|e| !projection::is_noisy(e))
            .map(|e| e.event_id)
            .collect();
        assert_eq!(
            expected,
            vec![0, 1, 2],
            "the fixture is wrong if the noise filter keeps something else"
        );

        server
            .build_and_store_engine(sid, events, Language::Rust)
            .await;

        let engines = server.engines.lock().await;
        let engine = engines.get(sid).expect("engine registered for the session");
        let mut kept: Vec<u64> = engine.events().iter().map(|e| e.event_id).collect();
        kept.sort_unstable();
        assert_eq!(
            kept, expected,
            "the server-built engine must index exactly what the shared noise \
             filter keeps, or it is a second authority next to the projection"
        );
    }

    // ─── REC-C1.5: rebuild(log) == maintained_projection(log) ───────────────
    //
    // `chronos_services::projection::build_engine` and
    // `ChronosServer::build_and_store_engine` are the only two ways a
    // `QueryEngine` comes into existence in this process, and REC-C1.5 asks
    // for the equivalence between them. Nothing asserted it. The test above
    // checks the noise predicate on the server side only, against a hardcoded
    // id list, and never writes a log at all — so the two builders were
    // covered separately and never against each other. The nearest thing
    // outside this file,
    // `chronos-sandbox/tests/rec_c1_7_projection_restart_equivalence.rs`,
    // does not close the gap either: it calls the gate, so both sides of its
    // comparison are projections built by `build_engine`.
    //
    // Every comparison below reads `engines` **directly**, never through
    // `gate_projection` / `ensure_projection`. Those are not neutral
    // observers: on a cache miss `ensure_projection` overwrites the `engines`
    // entry (`server.rs:776`) before the caller can look at it, so gating in
    // between would compare a rebuild against a rebuild and the maintained
    // engine under test would already have been replaced by its own
    // comparison.
    //
    // Ordering is the load-bearing assertion and nothing here sorts. A sorted
    // comparison passes for any two engines holding the same event *set*,
    // which is exactly the class of divergence this requirement is about:
    // `build_and_store_engine` receives events carrying no `seq` and keeps
    // the caller's order, while `build_engine` derives order from log
    // position.

    /// A fixture that populates every index `IndexBuilder` builds —
    /// `temporal` (all events), `shadow` (function and memory addresses),
    /// `causality` (variable and memory writes), `performance`
    /// (`FunctionEntry` call counts) — plus one event both paths must
    /// independently drop as noise.
    ///
    /// The returned order is the caller's to choose: the tests below write it
    /// to the log in exactly this order, and log position is what
    /// `build_engine` derives its ordering from.
    #[allow(dead_code)]
    fn rec_c1_5_fixture() -> Vec<TraceEvent> {
        use chronos_domain::SourceLocation;

        // ids 10/20/30/40 are ascending, so `fixture()` is also the
        // event_id-ascending ordering the merge path preserves.
        vec![
            TraceEvent::new(
                10,
                MonotonicNs::from(1_000),
                1,
                EventType::FunctionEntry,
                SourceLocation::new("main.c", 10, "main", 0x1000),
                EventData::Function {
                    name: "main".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
            TraceEvent::new(
                20,
                MonotonicNs::from(2_000),
                1,
                EventType::VariableWrite,
                SourceLocation::new("main.c", 11, "main", 0x2000),
                EventData::Variable(VariableInfo::local("counter", "1", "i32", 0x2000)),
            ),
            TraceEvent::new(
                30,
                MonotonicNs::from(3_000),
                1,
                EventType::MemoryWrite,
                SourceLocation::new("main.c", 12, "main", 0x2000),
                EventData::Memory {
                    address: 0x2000,
                    size: 8,
                    data: Some(vec![1, 2, 3, 4]),
                },
            ),
            TraceEvent::new(
                40,
                MonotonicNs::from(4_000),
                1,
                EventType::FunctionEntry,
                SourceLocation::new("work.c", 20, "work", 0x3000),
                EventData::Function {
                    name: "work".to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            ),
            // ptrace register snapshot: `Custom`/`Registers` is noise, and
            // both builders must drop it on their own.
            TraceEvent::new(
                50,
                MonotonicNs::from(5_000),
                1,
                EventType::Custom,
                SourceLocation::default(),
                EventData::Registers(Default::default()),
            ),
        ]
    }

    /// Append `events` to `log` in the given order, as JSON-encoded
    /// `TraceEvent` records — the producer contract `events_log_read::decode`
    /// (shared with `projection::build_engine`) reads back.
    #[allow(dead_code)]
    fn append_events_to_log(
        log: &chronos_services::session_log::SessionExecutionLog,
        session_id: &chronos_log::SessionId,
        events: &[TraceEvent],
    ) {
        for event in events {
            let payload = chronos_log::ExecutionPayload::new(
                serde_json::to_vec(event).expect("encode trace event"),
                "trace_event",
            );
            log.append(chronos_log::NewExecutionRecord {
                kind: chronos_log::ExecutionKind::Raw,
                session_id: session_id.clone(),
                monotonic_ns: event.timestamp_ns.get(),
                payload,
                invocation_id: None,
                parent_invocation_id: None,
                symbol_id: None,
                captured_at_unix_ns: None,
            })
            .expect("append record to the test execution log");
        }
    }

    /// Event ids in engine order — the thing `merge` is allowed to change.
    #[allow(dead_code)]
    fn engine_ids(engine: &QueryEngine) -> Vec<u64> {
        engine.events().iter().map(|e| e.event_id).collect()
    }

    /// A structural fingerprint of the answers a `QueryEngine` gives through
    /// each of its four indices, so the two engines can be compared on more
    /// than their event slice.
    ///
    /// **This is not index equality, and cannot be.** `QueryEngine` exposes no
    /// accessor for `shadow_index` / `temporal_index` / `causality_index` /
    /// `performance_index` (the only one is
    /// `causality_index_is_configured()`, a bool), and the four index types
    /// derive `Debug, Clone, Default` but not `PartialEq`
    /// (`chronos-domain/src/index/{shadow,temporal,causality,performance}.rs`).
    /// So this compares the *answers those indices produce*, which is strictly
    /// weaker than comparing the indices: an index could hold a structurally
    /// different state that these five probes happen not to expose, and the
    /// comparison would still pass. Recorded as the known hole rather than
    /// papered over.
    ///
    /// One normalisation, applied to the perf rows only: see the comment on
    /// the sort below. The event slice is never sorted anywhere in this file's
    /// new tests — engine order is the property under test.
    #[allow(dead_code)]
    fn index_answer_fingerprint(engine: &QueryEngine, session: &str) -> serde_json::Value {
        use chronos_domain::query::{CausalityQuery, PerfQuery, TraceQuery};
        use chronos_domain::TimestampNs;

        // temporal: a time-range query only uses the index when both ends are set.
        let temporal = engine.execute(
            &TraceQuery::new(session)
                .time_range(TimestampNs::from(0), TimestampNs::from(10_000))
                .pagination(100, 0),
        );
        // shadow: `get_memory_at` resolves the address through shadow and then
        // binary-searches the event vec, so this also depends on the engine's
        // events being id-sorted.
        let memory = engine.get_memory_at(0x2000, TimestampNs::from(10_000));
        // causality: full lineage of the written address.
        let causality = engine.query_causality(
            &CausalityQuery::new(session)
                .by_address(0x2000)
                .with_full_lineage(),
        );
        // performance: call counts, ordered by call count.
        let performance = engine.query_perf(&PerfQuery::new(session).top(10).sort_by_calls());

        // The one field here whose order is not a property of the engine:
        // `PerformanceIndex::top_functions_by_calls`
        // (`chronos-domain/src/index/performance.rs:136`) collects from a
        // `HashMap` and then stable-sorts by `Reverse(call_count)`, so a tie
        // in call_count is broken by that map's own iteration order — which
        // differs between two independently constructed indexes even when
        // both are correct. Comparing that order fails at random. Sorting by
        // `address`, a total key with one row per function, keeps the
        // comparison sensitive to a missing or extra function while dropping
        // an ordering the engine never chose.
        let mut performance = serde_json::to_value(performance).expect("perf serialises");
        if let Some(functions) = performance
            .get_mut("functions")
            .and_then(|f| f.as_array_mut())
        {
            functions.sort_by_key(|f| f.get("address").and_then(|a| a.as_u64()).unwrap_or(0));
        }

        serde_json::json!({
            "temporal_range": serde_json::to_value(temporal).expect("temporal serialises"),
            "shadow_memory_at": format!("{memory:?}"),
            "causality": serde_json::to_value(causality).expect("causality serialises"),
            "performance": performance,
            "causality_index_configured": engine.causality_index_is_configured(),
        })
    }

    /// REC-C1.5: the equivalence, on a log whose append order is also
    /// ascending in `event_id` — the ordering a tracer's own event ids
    /// produce.
    ///
    /// `build_and_store_engine` is driven twice so both of its branches run:
    /// the from-scratch build (`server.rs:869-881`) on the first drain, and
    /// the cumulative `merge` refresh (`server.rs:858-866`) on the second,
    /// which carries only the events the first drain had not seen. The log
    /// holds all five records; the maintained engine must end up holding the
    /// four non-noisy ones, in the same order `build_engine` derives.
    ///
    /// The noise event is present in the log *and* in the second drain, so
    /// each side has to drop it on its own rather than inheriting the other's
    /// decision.
    #[tokio::test]
    async fn rec_c1_5_rebuild_equals_maintained_projection_over_a_real_log() {
        let sid = "rec-c1-5-equivalence";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-5-equivalence-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        let events = rec_c1_5_fixture();
        append_events_to_log(&log, &session_id, &events);
        log.flush().expect("flush the appended records");

        // Drain 1 -> from-scratch branch. Drain 2 -> merge branch.
        server
            .build_and_store_engine(sid, events[..2].to_vec(), Language::Rust)
            .await;
        server
            .build_and_store_engine(sid, events[2..].to_vec(), Language::Rust)
            .await;

        let rebuilt = projection::build_engine(&log)
            .expect("rebuild from the log")
            .engine;
        let engines = server.engines.lock().await;
        let maintained = engines
            .get(sid)
            .expect("build_and_store_engine must have registered an engine");

        assert_eq!(
            maintained.event_count(),
            4,
            "the five-record log must project to four events; the Custom/Registers \
             record is noise and neither builder may keep it"
        );
        assert_eq!(
            maintained.event_count(),
            rebuilt.event_count(),
            "the maintained projection and the rebuild disagree on how many events \
             they hold"
        );

        // Ids first, in order: this is the assertion that would fail on an
        // ordering divergence, and its message is readable.
        assert_eq!(
            engine_ids(maintained),
            engine_ids(&rebuilt),
            "event ids differ between the maintained projection and the rebuild, \
             in order"
        );
        // Then the events themselves, not just their ids.
        assert_eq!(
            maintained.events(),
            rebuilt.events(),
            "the same event ids do not mean the same events in the same order"
        );

        // Index-backed answers, as far as the public surface allows.
        assert_eq!(
            index_answer_fingerprint(maintained, sid),
            index_answer_fingerprint(&rebuilt, sid),
            "the two engines answer differently through their indices"
        );
    }

    /// REC-C1.5, second half: the equivalence also holds for a log whose
    /// append order is not ascending in `event_id`.
    ///
    /// `rebuild(log)` is defined by log position, so it returns the events in
    /// the order they were written: ids `[30, 10, 40, 20]`. The from-scratch
    /// branch of `build_and_store_engine` hands the caller's order straight to
    /// `QueryEngine::with_indices` and therefore also returns
    /// `[30, 10, 40, 20]`.
    ///
    /// The `merge` branch used not to. `QueryEngine::merge` deduped the
    /// incoming events and then sorted `events` by `event_id`
    /// unconditionally, so one cumulative refresh was enough to reorder the
    /// maintained engine to `[10, 20, 30, 40]` while the rebuild stayed at
    /// `[30, 10, 40, 20]`. Same four events, different order: the divergence
    /// this test now pins as *absent*.
    ///
    /// Log/drain order is canonical for a query engine — the services layer
    /// derives causal edges from the vector order it read from the log — and
    /// `merge` no longer re-sorts, so a merge-grown engine and a rebuilt one
    /// answer the same log in the same order. This test is what keeps that
    /// true for the non-ascending case specifically, which the first test
    /// above cannot reach (its fixture ascends).
    #[tokio::test]
    async fn rec_c1_5_rebuild_equals_maintained_projection_when_log_order_is_not_event_id_ascending(
    ) {
        let sid = "rec-c1-5-order-divergence";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-5-order-divergence-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        // Same fixture, written to the log in a non-ascending order.
        let fixture = rec_c1_5_fixture();
        let written = vec![
            fixture[2].clone(),
            fixture[0].clone(),
            fixture[3].clone(),
            fixture[1].clone(),
            fixture[4].clone(),
        ];
        assert_eq!(
            written.iter().map(|e| e.event_id).collect::<Vec<_>>(),
            vec![30, 10, 40, 20, 50],
            "the fixture must actually be written out of event_id order, or this \
             test proves nothing"
        );
        append_events_to_log(&log, &session_id, &written);
        log.flush().expect("flush the appended records");

        let rebuilt = projection::build_engine(&log)
            .expect("rebuild from the log")
            .engine;
        assert_eq!(
            engine_ids(&rebuilt),
            vec![30, 10, 40, 20],
            "build_engine must order by log position, dropping the noise record"
        );

        // First drain: the from-scratch branch, fed the same order the log
        // holds. This one agrees with the rebuild.
        server
            .build_and_store_engine(sid, written.clone(), Language::Rust)
            .await;
        {
            let engines = server.engines.lock().await;
            let maintained = engines
                .get(sid)
                .expect("engine registered on the first drain");
            assert_eq!(
                maintained.events(),
                rebuilt.events(),
                "the from-scratch branch must hand the caller's order through \
                 unchanged, exactly as build_engine hands the log order through"
            );
        }

        // Second drain: a cumulative refresh over events the engine already
        // has — the shape `probe_drain` / `session_snapshot` produce. Every
        // incoming id is a duplicate, so nothing is appended; the only thing
        // that could still change the order is a merge that re-sorts.
        server
            .build_and_store_engine(sid, written, Language::Rust)
            .await;

        let engines = server.engines.lock().await;
        let maintained = engines.get(sid).expect("engine still registered");

        // The event SET is unchanged, and in the SAME order as the rebuild:
        // one cumulative refresh must not reorder a projection of the same
        // evidence. Ids first, because that is what an ordering divergence
        // shows up in.
        assert_eq!(
            engine_ids(maintained),
            vec![30, 10, 40, 20],
            "after one cumulative refresh the maintained engine must still be in \
             log order; a re-sort by event_id here is the REC-C1.5 divergence \
             returning"
        );
        assert_eq!(
            engine_ids(&rebuilt),
            vec![30, 10, 40, 20],
            "the rebuild is defined by log position and must not have moved"
        );
        assert_eq!(
            maintained.events(),
            rebuilt.events(),
            "REC-C1.5: rebuild(log) and maintained_projection(log) must be the same \
             events in the same order, for a log whose append order is not \
             event_id-ascending"
        );
    }

    /// The `Causality` arm of `execution_log_read` answers from the
    /// log-derived projection, never from what a drain left behind.
    ///
    /// This test used to be
    /// `rec_c1_5_ensure_projection_replaces_the_maintained_engine_recorded_defect`
    /// and it asserted the opposite. The `engines` slot had two writers:
    /// `build_and_store_engine` (`server.rs:835`) writes `engines` and
    /// `session_languages` but never `projection_meta`, so the engine a
    /// drain leaves is invisible to the gate, while `ensure_projection`
    /// (`server.rs:750`) rebuilds from the log and replaces it. A drain
    /// that delivered two of the log's four events therefore left a
    /// two-event engine in the map, and the `Causality` arm — then the
    /// only reader of `engines` that did not gate first — answered from
    /// those two while `execution_query` / `state_query` / `trace_slice`
    /// gated and got all four. Two tools, two engines, one session, one
    /// process, selected by which tool the agent happened to call. The old
    /// doc comment said it outright: "RECORDED DEFECT, not a
    /// specification", to be rewritten once the slot stopped being
    /// contested. It now is.
    ///
    /// The old test also never drove the wire. It asserted the map's
    /// state transitions and stopped there, so nothing in it could tell an
    /// agent that two tools disagreed. This one calls the tool.
    ///
    /// Deliberately kept from the old version, because both remain true
    /// and are load-bearing: `build_and_store_engine` must not publish
    /// `projection_meta` (it is the live-capture path, and a
    /// partial drain-sourced engine that could pass the gate would be a
    /// worse lie than the one this milestone removed), and the
    /// drain-sourced engine legitimately holds exactly the two events the
    /// drain delivered.
    #[tokio::test]
    async fn rec_c1_5_execution_log_read_causality_answers_from_the_log_derived_projection() {
        let sid = "rec-c1-5-causality-gated";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-5-causality-gated-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        // The log holds all four non-noisy events...
        let events = rec_c1_5_fixture();
        append_events_to_log(&log, &session_id, &events);
        log.flush().expect("flush the appended records");

        // ...but a drain delivered only the first two.
        server
            .build_and_store_engine(sid, events[..2].to_vec(), Language::Rust)
            .await;

        {
            let engines = server.engines.lock().await;
            let drained = engines.get(sid).expect("engine registered by the drain");
            assert_eq!(
                drained.event_count(),
                2,
                "the drain only ever saw two events, so the drain-sourced engine \
                 holds two — this is the engine the arm used to answer from"
            );
        }
        assert!(
            !server.projection_meta.lock().await.contains_key(sid),
            "build_and_store_engine must not publish projection metadata: it is the \
             live-capture path, and a partial drain-sourced engine that could pass \
             the gate would be a worse lie than the one being removed here"
        );

        // What a gated tool would build, built independently here so the
        // comparison below is against the log and not against the cache.
        let independent = projection::build_engine(&log)
            .expect("the log is readable, so an independent rebuild must succeed")
            .engine;

        // The agent calls the tool. It must gate, rebuild, and answer from
        // the projection — not report the two events the drain left.
        let result = server
            .execution_log_read(Parameters(ExecutionLogReadParams {
                session_id: sid.to_string(),
                mode: ExecutionLogReadKind::Causality,
                cursor: None,
                bucket_size_ns: default_bucket_size_ns(),
                limit: default_limit(),
            }))
            .await
            .expect("a readable log must answer, not fail the tool call");

        assert_ne!(
            result.is_error,
            Some(true),
            "a session with a readable log must answer: {:?}",
            result.content
        );
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("engine_loaded"),
            "causality mode must still report engine_loaded, got: {text}"
        );
        assert!(
            text.contains("Wired"),
            "answering from a populated log-derived projection means hints are \
             genuinely available; reporting Unsupported here would mean the arm \
             declined to use the projection the gate just built, got: {text}"
        );

        // The engine the tool answered from is the log-derived one. It is
        // also, event for event, what `execution_query`, `state_query` and
        // `trace_slice` see, because all four read this single map entry.
        {
            let engines = server.engines.lock().await;
            let answered_from = engines
                .get(sid)
                .expect("the gate must leave an engine behind");
            assert_eq!(
                answered_from.event_count(),
                4,
                "the Causality arm must answer from a projection of the whole log, \
                 not from the two events the drain delivered — this is the \
                 REC-C1.5 divergence returning"
            );
            assert_eq!(
                engine_ids(answered_from),
                engine_ids(&independent),
                "same ids, in the order the log defines"
            );
            assert_eq!(
                answered_from.events(),
                independent.events(),
                "REC-C1.5: the engine the arm answered from must be the log's \
                 projection, not a lookalike that happens to have the same count"
            );
        }

        // Kept from the old version: the log is intact, so gating this
        // session yields a Full projection and not the retention refusal.
        // The arm now depends on the gate, so it inherits the refusal too,
        // and this is the assertion that says the refusal did not fire here.
        let meta = server
            .projection_meta
            .lock()
            .await
            .get(sid)
            .cloned()
            .expect("answering at all means the gate published projection metadata");
        assert_eq!(
            meta.completeness,
            projection::ProjectionCompleteness::Full,
            "the log is intact; this is not a retention case"
        );
    }

    /// A drain-sourced engine is not authority, and the tool says so.
    ///
    /// The other half of the gate change. A session can hold an engine that
    /// no log backs: `build_and_store_engine` writes `engines` with no
    /// `projection_meta` and no log behind it, which is the pure
    /// live-capture shape. `ReadPathService::causality_status` needs a
    /// readable log, so this shape has never produced a status — it failed
    /// closed — and gating the arm does not change that, which is exactly
    /// what is pinned here: the drain-sourced engine must never be reported
    /// as if a log vouched for it.
    ///
    /// The fixture is the tempting one on purpose: a real, non-empty
    /// engine sitting in the map. A test with no engine at all would pass
    /// under any implementation, including a hypothetical rule that reports
    /// `Unsupported` whenever the engine is missing.
    #[tokio::test]
    async fn execution_log_read_causality_fails_closed_when_only_a_drain_sourced_engine_exists() {
        let server = ChronosServer::new();
        let sid = "causality-drain-sourced-only";

        // The live-capture shape: a drain delivered the fixture and left a
        // real engine behind. No log was ever registered for this session.
        let events = rec_c1_5_fixture();
        server
            .build_and_store_engine(sid, events, Language::Rust)
            .await;
        {
            let engines = server.engines.lock().await;
            let drained = engines.get(sid).expect("the drain registered an engine");
            assert_eq!(
                drained.event_count(),
                4,
                "fixture sanity: four non-noisy events, so the engine a logless \
                 session is holding is genuinely populated"
            );
        }

        let result = server
            .execution_log_read(Parameters(ExecutionLogReadParams {
                session_id: sid.to_string(),
                mode: ExecutionLogReadKind::Causality,
                cursor: None,
                bucket_size_ns: default_bucket_size_ns(),
                limit: default_limit(),
            }))
            .await
            .expect("this tool reports failures as an isError envelope, not a transport error");

        assert_eq!(
            result.is_error,
            Some(true),
            "with no registered log the engine in the map is not authority and must \
             not be answered from: {:?}",
            result.content
        );
        let text = format!("{:?}", result.content);
        assert!(
            !text.contains("engine_loaded"),
            "an error envelope must not carry a causality status derived from an \
             engine no log backs: {text}"
        );
        assert!(
            !text.contains("Wired") && !text.contains("Unsupported"),
            "no status may be reported at all for a session with no log: {text}"
        );

        // The gate refuses; it does not evict. `build_and_store_engine`'s
        // output is the live capture path's own state, and a reader that
        // cannot use it must not destroy it.
        assert!(
            server.engines.lock().await.contains_key(sid),
            "the gate must leave the capture path's engine untouched"
        );
    }

    /// A session with no log never gets a projection.
    ///
    /// The error is `ExecutionLogUnavailable`, not `SessionNotFound`, and that
    /// distinction is deliberate: a session loaded from the session store
    /// exists but has no `ExecutionLog` registered yet, so reporting
    /// "session not found" would be false. The variant carries the reason, so
    /// the wire message says which of the two situations occurred.
    ///
    /// An empty success here would be worse than either: it would be
    /// indistinguishable from a real empty session, which *is* a valid answer.
    #[tokio::test]
    async fn gate_projection_reports_an_unavailable_log_for_a_session_without_one() {
        let server = ChronosServer::new();

        let err = server
            .gate_projection("session-that-was-never-registered")
            .await
            .expect_err("a session with no log must not project");

        match err {
            ServiceError::ExecutionLogUnavailable { session_id, .. } => {
                assert_eq!(session_id, "session-that-was-never-registered");
            }
            other => panic!("expected ExecutionLogUnavailable, got {other:?}"),
        }
    }

    // ---- ADR-0004 at the wire boundary --------------------------------
    //
    // The tests above pin the gate's *decision*. The ones below pin what
    // an agent actually reads: the `rmcp::ErrorData` message
    // `gate_projection_for_wire` builds, and the meta it hands back.
    //
    // That layer needs its own pins because the retention guarantee
    // reaches the caller through a single `format!("...: {}", e)`. The
    // boundary is stated in the message only because that arm renders
    // `ServiceError` with `Display`. Rewrite it as a constant, or as
    // `{e:?}`, and the boundary stops being stated: the agent is told
    // the query failed, is not told which history is missing, and every
    // test in the block above still passes. A Silent Lie with nothing to
    // catch it — which is the defect class, not a hypothetical.
    //
    // Deliberately NOT asserted: `ErrorData::code`. Both failure modes
    // are mapped to `internal_error` today, and that classification is a
    // wire-contract decision rather than a fact about retention, so
    // these tests pin the *content* of the message and leave the code
    // free to move under a decision taken on purpose.

    /// The headline ADR-0004 guarantee: a refused gate tells the agent
    /// *which* history is missing, not merely that it failed.
    ///
    /// The truncation is real, not seeded. Two flushed segments are
    /// written and the first is retired through `advance_retained_from` +
    /// `compact_retired`, so `projection::build_engine` derives
    /// `Truncated { retained_from: 5 }` from the log's own watermark.
    /// (`chronos-log`'s memory backend carries no retention watermark at
    /// all, so the segmented adapter behind `create_for_tests` is the
    /// only way to reach this state.) The boundary the wire must state is
    /// therefore derived from durable evidence, not written into a test
    /// fixture — a test that seeded `retained_from: 5` would pass just as
    /// happily if the log itself disagreed.
    ///
    /// A truncated session still has a readable tail, so this is a
    /// refusal about history, not an absence of data: the second segment
    /// is what makes the distinction real.
    #[tokio::test]
    async fn wire_gate_names_the_retention_boundary_for_a_truncated_projection() {
        let sid = "rec-c1-7-wire-gate-names-retention-boundary";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "{sid}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        let events: Vec<TraceEvent> = (1..=10u64).map(rec_c1_7_later_event).collect();
        append_events_to_log(&log, &session_id, &events[..5]);
        log.flush().expect("flush the first segment");
        append_events_to_log(&log, &session_id, &events[5..]);
        log.flush().expect("flush the second segment");

        log.advance_retained_from(chronos_domain::EventSeq::new(5))
            .expect("retire the first segment");
        log.compact_retired().expect("reclaim the retired segment");

        let err = server
            .gate_projection_for_wire(sid)
            .await
            .expect_err("a truncated projection must not reach the agent as a query answer");

        let message: &str = err.message.as_ref();
        assert!(
            message.contains("seq 5"),
            "the wire must state the retention boundary so the agent can say which \
             history is missing; got: {message}"
        );
        // "retention"/"retained", spelled out: a bare "reten" prefix would be
        // satisfied by neither word and would quietly pass.
        let lowered = message.to_lowercase();
        assert!(
            lowered.contains("retention") || lowered.contains("retained"),
            "the wire must identify retention as the cause, not just that some gate \
             failed; got: {message}"
        );
    }

    /// A session the registry never heard of must be reported as such,
    /// naming the session.
    ///
    /// The `expect_err` is half the guarantee. `Empty` is a *pass* at
    /// this gate — an idle session is a real answer — so "this session
    /// has no log" and "this session has no records" only stay
    /// distinguishable while the former stays an error. The positive
    /// control for that half is `wire_gate_accepts_an_empty_session`
    /// below; a wire that returned an empty meta for a session with no
    /// log at all would satisfy this test's message assertions and still
    /// have invented data, which is why the two are written together.
    #[tokio::test]
    async fn wire_gate_names_the_session_when_no_log_is_registered() {
        let server = ChronosServer::new();
        let sid = "rec-c1-7-wire-gate-names-missing-log-session";

        let err = server
            .gate_projection_for_wire(sid)
            .await
            .expect_err("a session with no registered log must not be reported as an empty one");

        let message: &str = err.message.as_ref();
        assert!(
            message.contains(sid),
            "the wire must name the session it could not read; got: {message}"
        );
        assert!(
            message.to_lowercase().contains("executionlog"),
            "the wire must say the log is unavailable, rather than that the session \
             is missing or empty; got: {message}"
        );
        // Not a retention story: a caller reading "retained" here would
        // go looking for a boundary that does not exist. Spelled out for
        // the same reason as the positive check above — a prefix that
        // matches neither word would make this pass unconditionally.
        let lowered = message.to_lowercase();
        assert!(
            !(lowered.contains("retention") || lowered.contains("retained")),
            "a missing log must not read as a retention condition; got: {message}"
        );
    }

    /// Positive control: a complete projection reaches the agent as
    /// `Ok(meta)`, not as a message.
    ///
    /// Without this, every other pin in this group could be satisfied by
    /// a `gate_projection_for_wire` that refuses unconditionally — the
    /// guarantee would then be "the agent is always told something",
    /// which is not what ADR-0004 asks for. The meta is built from a real
    /// log with no retirement, so the ranges asserted here are the log's
    /// own rather than a fixture's.
    #[tokio::test]
    async fn wire_gate_returns_the_meta_for_a_full_projection() {
        let sid = "rec-c1-7-wire-gate-passes-full";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "{sid}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        let events: Vec<TraceEvent> = (1..=3u64).map(rec_c1_7_later_event).collect();
        append_events_to_log(&log, &session_id, &events);
        log.flush().expect("flush the appended records");

        let meta = server
            .gate_projection_for_wire(sid)
            .await
            .expect("a complete projection must pass the gate to the caller");

        assert_eq!(meta.completeness, projection::ProjectionCompleteness::Full);
        assert_eq!(
            meta.session_id.as_str(),
            sid,
            "the meta handed to the caller must be this session's"
        );
        assert_eq!(meta.projected_from, chronos_domain::EventSeq::ZERO);
        assert_eq!(
            meta.projected_through,
            Some(chronos_domain::EventSeq::new(2)),
            "`projected_through` is the last seq the projection read, so the \
             agent can see how much history it is actually being answered about"
        );
    }

    /// The second positive control, and the counterpart to
    /// `wire_gate_names_the_session_when_no_log_is_registered`.
    ///
    /// An empty session is a real answer: it has no records, so it has no
    /// history to be missing, and the gate returns its meta instead of
    /// refusing. Refusing it would break every query against an idle
    /// session. Accepting it is only safe because it is
    /// distinguishable from "no log registered" — which is the pair of
    /// tests that say so.
    #[tokio::test]
    async fn wire_gate_accepts_an_empty_session() {
        let sid = "rec-c1-7-wire-gate-accepts-empty";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "{sid}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");
        // No records appended: the log exists and holds nothing, which is
        // the case a missing log must not be allowed to imitate.

        let meta = server
            .gate_projection_for_wire(sid)
            .await
            .expect("an empty session is a real answer, not a gate failure");

        assert_eq!(meta.completeness, projection::ProjectionCompleteness::Empty);
        assert_eq!(meta.session_id.as_str(), sid);
        assert_eq!(
            meta.projected_through, None,
            "nothing was read, so no seq may be claimed as read through"
        );
        assert_eq!(meta.projected_from, chronos_domain::EventSeq::ZERO);
    }

    /// Register a log holding `events`, and return the (log, SessionId) pair
    /// so a test can append more evidence afterwards the way a live probe
    /// keeps doing.
    #[cfg(test)]
    #[allow(dead_code)]
    fn rec_c1_7_log_with(
        tag: &str,
        sid: &str,
        events: &[TraceEvent],
    ) -> (
        ChronosServer,
        chronos_services::session_log::SessionExecutionLog,
        chronos_log::SessionId,
    ) {
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "{tag}-{sid}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");
        append_events_to_log(&log, &session_id, events);
        log.flush().expect("flush the appended records");
        (server, log, session_id)
    }

    /// A second, non-noisy event whose `event_id` is above the fixture's, so
    /// it is genuinely new evidence rather than a re-delivery.
    #[cfg(test)]
    #[allow(dead_code)]
    fn rec_c1_7_later_event(id: u64) -> TraceEvent {
        use chronos_domain::SourceLocation;
        TraceEvent::new(
            id,
            MonotonicNs::from(id * 100),
            1,
            EventType::FunctionEntry,
            SourceLocation::new("main.c", 10 + id as u32, "later", 0x1000 + id),
            EventData::Function {
                name: format!("fn_{id}"),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        )
    }

    /// The drain exactly as the live-capture path performs it: the events are
    /// read back out of the session's own `ExecutionLog`, not out of a ring
    /// buffer. `ProbeService::stop` and `ProbeService::session_snapshot` both
    /// hand `build_and_store_engine` the result of this call, which is why
    /// the drained evidence is provably already in the log.
    #[cfg(test)]
    #[allow(dead_code)]
    fn rec_c1_7_native_drain(
        log: &chronos_services::session_log::SessionExecutionLog,
    ) -> Vec<TraceEvent> {
        chronos_services::canonical_drain::read_all_raw_events(log)
            .expect("a readable log must drain")
            .events
    }

    /// REC-C1.7, the defect: a drain mutated a published projection in place
    /// and the gate kept certifying the result.
    ///
    /// The sequence is the production one, in order: a query publishes the
    /// projection (`gate_projection` -> `ensure_projection`), the probe keeps
    /// capturing so the log grows, then the operator calls
    /// `session_snapshot`, whose drain reaches `build_and_store_engine`.
    ///
    /// `build_and_store_engine` used to consult `engines` only, so the drain
    /// took the cumulative-`merge` branch against the engine the gate had
    /// just built *from the log*, and `projection_meta` — the gate's cache —
    /// kept its entry. The entry is not bookkeeping: `Full` means "the engine
    /// matches the log verbatim" and `projected_through` is "the last seq the
    /// projection read" (projection.rs:48-49, 64-65). After the drain both
    /// claims are unearned: the engine was written by `merge`, a function the
    /// projection never runs, and the certified range stops short of the
    /// log's tail. Every gated reader — `execution_query`, `state_query`,
    /// `trace_slice`, `execution_log_read` — then fast-paths on that meta
    /// (server.rs:751-753) and never rebuilds.
    ///
    /// Asserted as the absence of the corruption rather than as the
    /// reproduction of it, so the test stays green in a fixed tree. Its
    /// non-vacuity was established by reverting the guard and watching this
    /// exact assertion fail; the failure message below is the one it printed.
    #[tokio::test]
    async fn rec_c1_7_a_drain_may_not_leave_a_meta_certifying_a_merge_made_engine() {
        let sid = "rec-c1-7-drain-over-published";
        let (server, log, session_id) =
            rec_c1_7_log_with("rec-c1-7-drain", sid, &rec_c1_5_fixture());

        // 1. A gated query publishes the projection: `Full`, certified
        //    through the log's tail as it stands now.
        let published = server
            .gate_projection(sid)
            .await
            .expect("a readable log must project");
        let tail_at_projection = log.handle().tail_seq();
        assert_eq!(
            published.completeness,
            projection::ProjectionCompleteness::Full,
            "fixture sanity: an untruncated log projects as Full"
        );
        assert_eq!(
            published.projected_through, tail_at_projection,
            "fixture sanity: the published meta certifies the log's current tail"
        );

        // 2. The probe keeps capturing. This is the ordinary case for a live
        //    session between two `session_snapshot` calls.
        let later = [rec_c1_7_later_event(60), rec_c1_7_later_event(70)];
        append_events_to_log(&log, &session_id, &later);
        log.flush().expect("flush the newer records");
        let tail_after_capture = log.handle().tail_seq();
        assert!(
            tail_after_capture > tail_at_projection,
            "fixture sanity: the log must have grown, or the certified range \
             would still reach its tail and this test would prove nothing"
        );

        // 3. `session_snapshot` drains — out of the log, as the live path does.
        let drain = rec_c1_7_native_drain(&log);
        server
            .build_and_store_engine(sid, drain, Language::Rust)
            .await;

        // The gate's cache must not still be vouching for this engine. Before
        // the fix the meta survived here, and its `projected_through` was
        // still the pre-capture tail: it claimed `Full` over a range the
        // projection never read, for an engine `merge` had rewritten.
        let surviving = server.projection_meta.lock().await.get(sid).cloned();
        assert!(
            surviving.is_none(),
            "a drain over a session with a published projection must invalidate \
             that projection, not merge into the engine it certifies. A surviving \
             entry is a gate that fast-paths on stale coverage: {:?}",
            surviving
        );

        // And the next gated query — the one an agent actually makes — has to
        // produce a meta that certifies the log as it is *now*.
        let refreshed = server
            .gate_projection(sid)
            .await
            .expect("the log is still readable, so the next query must rebuild");
        assert_eq!(
            refreshed.projected_through, tail_after_capture,
            "the rebuilt meta must certify the log's current tail, not the tail \
             that was current when the drain happened"
        );
        let rebuilt = projection::build_engine(&log)
            .expect("independent rebuild")
            .engine;
        let engines = server.engines.lock().await;
        let served = engines.get(sid).expect("the rebuild left an engine");
        assert_eq!(
            served.events(),
            rebuilt.events(),
            "the engine a gated reader is served must be the log's projection"
        );
    }

    /// The invariant itself, asserted as the disjunction rather than as "it
    /// works".
    ///
    /// After any drain, for any session: either `projection_meta` has no
    /// entry, or `engines[sid]` still equals `projection::build_engine(&log)`.
    /// The second arm is not decorative — it is what a session with no
    /// published projection must satisfy, and it is the arm a "just drop the
    /// engine too" fix would break. Both arms are checked explicitly so this
    /// test cannot pass by accident on either one.
    #[tokio::test]
    async fn rec_c1_7_after_a_drain_the_meta_is_gone_or_the_engine_is_the_log_projection() {
        let sid = "rec-c1-7-invariant";
        let (server, log, session_id) =
            rec_c1_7_log_with("rec-c1-7-invariant", sid, &rec_c1_5_fixture());

        // Publish, grow, drain — the same production sequence as above.
        server
            .gate_projection(sid)
            .await
            .expect("a readable log must project");
        let later = [rec_c1_7_later_event(60), rec_c1_7_later_event(70)];
        append_events_to_log(&log, &session_id, &later);
        log.flush().expect("flush the newer records");
        let drain = rec_c1_7_native_drain(&log);
        server
            .build_and_store_engine(sid, drain, Language::Rust)
            .await;

        let published_still = server.projection_meta.lock().await.contains_key(sid);
        let rebuilt = projection::build_engine(&log)
            .expect("independent rebuild")
            .engine;
        let engines = server.engines.lock().await;
        let engine_is_the_projection = match engines.get(sid) {
            Some(engine) => engine.events() == rebuilt.events(),
            // No engine at all cannot be read behind the gate, so it cannot
            // violate the invariant; `ensure_projection` will build one.
            None => false,
        };

        assert!(
            !published_still || engine_is_the_projection,
            "INVARIANT BROKEN: projection_meta is published for '{sid}' but the \
             engine it certifies is not projection::build_engine(&log). Either the \
             meta must be invalidated by the drain, or the engine must still be \
             the log's projection. published={published_still} \
             engine_is_projection={engine_is_the_projection}"
        );

        // The disjunction is only meaningful if the second arm is reachable,
        // so pin which arm this sequence actually took.
        assert!(
            !published_still,
            "the drain must have invalidated the published projection; if this arm \
             ever flips, the assertion above is no longer testing the invalidation"
        );
    }

    /// The positive control: a drain for a session with **no** published
    /// projection still does its cumulative merge.
    ///
    /// This is the pure live-capture shape — `session_start` + `probe_stop`,
    /// and the browser probe, whose events come from the CDP adapter and have
    /// no `ExecutionLog` at all (FIND-C2.2-04) and therefore can never hold a
    /// `projection_meta` entry. It is also UAT-M0-02
    /// (`m0-02-make-snapshots-cumulative`), and this project has repeatedly
    /// found fixes that were satisfied by refusing drains outright. This is
    /// the test that says the guard is narrow: the merge branch must remain
    /// reachable and must keep accumulating.
    ///
    /// Non-vacuity: making the guard too broad — skipping the merge whenever
    /// the engine is not a projection *by construction* rather than whenever a
    /// projection is published — turns this red on the `event_count`, and the
    /// existing
    /// `rec_c1_5_rebuild_equals_maintained_projection_over_a_real_log` goes
    /// red with it.
    #[tokio::test]
    async fn rec_c1_7_a_drain_with_no_published_projection_still_merges_cumulatively() {
        let sid = "rec-c1-7-positive-control";

        // A logless session: exactly what `probe_stop` on a fresh
        // `session_start` leaves, and what the browser probe always is.
        let server = ChronosServer::new();
        assert!(
            server.projection_meta.lock().await.is_empty(),
            "fixture sanity: nothing is projected before the first drain"
        );

        // First drain: the from-scratch branch.
        server
            .build_and_store_engine(sid, vec![make_fn_event(0, 100, 1, "main")], Language::C)
            .await;
        {
            let engines = server.engines.lock().await;
            assert_eq!(
                engines
                    .get(sid)
                    .expect("first drain registers")
                    .event_count(),
                1,
                "the from-scratch branch must index the events it was given"
            );
        }

        // Second and third drains: the cumulative merge, which is the whole
        // point of UAT-M0-02.
        server
            .build_and_store_engine(sid, vec![make_fn_event(1, 200, 1, "helper")], Language::C)
            .await;
        server
            .build_and_store_engine(sid, vec![make_fn_event(2, 300, 1, "main")], Language::C)
            .await;

        let engines = server.engines.lock().await;
        let engine = engines
            .get(sid)
            .expect("the cumulative merge must leave the engine registered");
        assert_eq!(
            engine.event_count(),
            3,
            "three cumulative drains for a session with no published projection \
             must accumulate three events. An engine stuck at 1 means the fix \
             refused the drain, which is the failure mode this control exists \
             to catch"
        );
        assert_eq!(
            engine_ids(engine),
            vec![0, 1, 2],
            "the cumulative merge must keep the delivery order, so the session \
             answers the history it actually captured"
        );
        assert!(
            !server.projection_meta.lock().await.contains_key(sid),
            "the live-capture path still must not publish projection metadata: a \
             drain-sourced engine that could pass the gate would be a worse lie \
             than the one REC-C1.7 removed"
        );
    }

    /// The content half of the defect, and the reason invalidation is the
    /// right shape rather than "merge is close enough".
    ///
    /// On the happy path — an append-only log, unique `event_id`s, a drain
    /// that reads the whole log — `merge` happens to reconstruct the same
    /// event list a rebuild would, because the drained vector is a prefix
    /// extension of the projected one. That coincidence is not a guarantee,
    /// and this log state shows why: the log holds two `Raw` records with the
    /// same `event_id`.
    ///
    /// `build_engine` pushes every record it decodes (projection.rs:138-144)
    /// and never deduplicates, so the log's record list is the truth and
    /// holds both. `merge` is the id-union, first-wins
    /// (engine.rs:207-213), so it silently drops one. A gate that still
    /// holds `Full` would then certify an engine that is missing evidence the
    /// log demonstrably has — and the missing event is gone from the only
    /// structure a reader will consult, because the next gated query
    /// fast-paths on the meta and never rebuilds.
    ///
    /// Honest scope: the in-tree native producer allocates `event_id`s from a
    /// monotonic per-session counter, so it does not currently emit a repeat.
    /// A repeated id is still a legal `ExecutionLog` — nothing in the log
    /// contract makes ids unique — and the point of the test is the
    /// *guarantee*, not the producer: after a drain, either the meta is gone
    /// or the engine equals `build_engine(&log)`. Same conclusion if ids
    /// collide, and if history retires (`retained_from` advancing after the
    /// projection would leave the engine holding records the log no longer
    /// has, under a `Full` that `build_engine` would itself refuse).
    #[tokio::test]
    async fn rec_c1_7_merge_is_not_a_projection_when_the_log_repeats_an_event_id() {
        let sid = "rec-c1-7-repeated-id";
        let server = ChronosServer::new();
        let session_id = chronos_log::SessionId::new(sid);
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-7-repeated-id-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let log = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            session_id.clone(),
        )
        .expect("test execution log");
        server
            .execution_logs
            .register(log.clone())
            .expect("register the test log");

        // One record to begin with, so the repeat lands *after* the
        // projection is published. That ordering is the whole test: a repeat
        // already in the log when the projection runs is in the projected
        // engine too, and a later drain re-adds nothing. The corruption needs
        // the repeat to arrive while the session is live, between two
        // snapshots — the same shape the two tests above use.
        let first = make_fn_event(10, 100, 1, "twin");
        append_events_to_log(&log, &session_id, &[first]);
        log.flush().expect("flush the first record");

        // 1. Publish: the engine holds one event, `Full` over seq 0.
        server
            .gate_projection(sid)
            .await
            .expect("a readable log must project");
        {
            let engines = server.engines.lock().await;
            assert_eq!(
                engines
                    .get(sid)
                    .expect("the gate left an engine")
                    .event_count(),
                1,
                "fixture sanity: the projection of a one-record log holds one event"
            );
        }

        // 2. The probe captures a second record that reuses the id. Nothing
        //    in the `ExecutionLog` contract makes `event_id` unique.
        let twin = make_fn_event(10, 200, 1, "twin");
        append_events_to_log(&log, &session_id, &[twin]);
        log.flush().expect("flush the repeated record");

        let truth = projection::build_engine(&log).expect("rebuild").engine;
        assert_eq!(
            truth.event_count(),
            2,
            "fixture sanity: build_engine keeps every record it decodes, so a log \
             with two same-id records projects to two events. If this ever fails \
             the producer-side dedup changed and this test is no longer testing \
             what it claims"
        );

        // 3. The drain reads both records back, and `merge` is asked to fold
        //    them into an engine that already holds id 10.
        let drain = rec_c1_7_native_drain(&log);
        assert_eq!(
            drain.len(),
            2,
            "fixture sanity: the drain must deliver both records, or the merge \
             never sees the repeat and this test proves nothing"
        );
        server
            .build_and_store_engine(sid, drain, Language::Rust)
            .await;

        // The evidence is genuinely lost from the engine, and this is true
        // whichever arm of the fix was taken: the engine is not rebuilt on
        // the drain, and `merge` is first-wins on `event_id`, so the second
        // record never enters it. Without the meta being invalidated this is
        // unrecoverable — the next gated query fast-paths and never rebuilds.
        {
            let engines = server.engines.lock().await;
            let held = engines
                .get(sid)
                .expect("an engine is present")
                .event_count();
            assert!(
                held < truth.event_count(),
                "fixture sanity: this test exists because merge cannot represent a \
                 repeated event_id, so the engine must be short a record the log \
                 has. engine={held} log={}",
                truth.event_count()
            );
        }

        let published_still = server.projection_meta.lock().await.contains_key(sid);
        let engines = server.engines.lock().await;
        let engine_is_the_projection = engines
            .get(sid)
            .is_some_and(|e| e.events() == truth.events());

        assert!(
            !published_still || engine_is_the_projection,
            "INVARIANT BROKEN on a log that repeats an event_id: a gate holding \
             `Full` is certifying an engine that merge built by id-union, so it \
             is missing a record build_engine has, and the gate will never rebuild \
             it. published={published_still} \
             engine_is_projection={engine_is_the_projection}"
        );
    }

    #[tokio::test]
    async fn test_cleanup_session_memory_keeps_other_sessions_projections() {
        let server = ChronosServer::new();
        let kept = "kept-session".to_string();
        let dropped = "dropped-session".to_string();

        let meta_for = |sid: &str| ProjectionMeta {
            session_id: chronos_domain::SessionId::new(sid),
            projected_from: chronos_domain::EventSeq::ZERO,
            projected_through: Some(chronos_domain::EventSeq::ZERO),
            completeness: projection::ProjectionCompleteness::Full,
            source: projection::ProjectionSource::ExecutionLog,
        };
        server
            .projection_meta
            .lock()
            .await
            .insert(kept.clone(), meta_for(&kept));
        server
            .projection_meta
            .lock()
            .await
            .insert(dropped.clone(), meta_for(&dropped));

        server.cleanup_session_memory(&dropped).await;

        assert!(!server.projection_meta.lock().await.contains_key(&dropped));
        assert!(
            server.projection_meta.lock().await.contains_key(&kept),
            "evicting one session must not disturb another's projection"
        );
    }

    #[tokio::test]
    async fn test_delete_session_also_cleans_memory() {
        let server = ChronosServer::new();
        let sid = "delete-cleanup-session".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Build and save session
        server
            .build_and_store_engine(&sid, events, Language::C)
            .await;
        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid.clone(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        // Engine should be in memory
        assert!(server.engines.lock().await.contains_key(&sid));

        // Delete from store
        let result = server
            .delete_session(Parameters(DeleteSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));

        // Memory should be cleaned up too
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
    }

    #[tokio::test]
    async fn test_list_sessions_after_save() {
        let server = ChronosServer::new();
        let sid1 = "list-test-1".to_string();
        let sid2 = "list-test-2".to_string();
        let events = vec![make_fn_event(0, 100, 1, "main")];

        // Save two sessions
        server
            .build_and_store_engine(&sid1, events.clone(), Language::C)
            .await;
        server
            .build_and_store_engine(&sid2, events.clone(), Language::C)
            .await;

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid1.clone(),
                language: "native".to_string(),
                target: "/bin/test1".to_string(),
            }))
            .await
            .unwrap();

        server
            .save_session(Parameters(SaveSessionParams {
                session_id: sid2.clone(),
                language: "native".to_string(),
                target: "/bin/test2".to_string(),
            }))
            .await
            .unwrap();

        // List sessions
        let list_result = server.list_sessions(Parameters(NoParams {})).await.unwrap();
        assert_ne!(list_result.is_error, Some(true));
        let text = format!("{:?}", list_result.content);
        assert!(text.contains("session_count") || text.contains("sessions"));
    }

    /// FIND-M9-69-MCP-STORE-ISOLATION: under `cfg(test)`, `ChronosServer::new()`
    /// must be hermetic — it must not read from or write to the developer's real
    /// `$HOME/.local/share/chronos/sessions.redb`. Before the fix, `new()` opened
    /// that database, so a fresh server started out listing whatever sessions the
    /// developer happened to have, and `list_sessions` hard-failed on the first
    /// stale-schema record. That coupling is exactly how
    /// `test_list_sessions_after_save` failed deterministically on a populated
    /// machine while passing on a clean one.
    #[tokio::test]
    async fn test_default_test_server_does_not_read_the_developer_store() {
        // Escape hatch: a test that deliberately opts into a real file store is
        // selected by CHRONOS_DB_PATH. Hermeticity is only asserted for the
        // default configuration.
        if std::env::var("CHRONOS_DB_PATH").is_ok() {
            eprintln!("CHRONOS_DB_PATH set; skipping hermeticity assertion");
            return;
        }

        let a = ChronosServer::new();
        let b = ChronosServer::new();

        // (1) A fresh test server starts empty, not with the developer's sessions.
        let listed_before = a.store.list_sessions().expect("list_sessions must succeed");
        assert!(
            listed_before.is_empty(),
            "fresh test server must start with an empty store, found {} session(s): \
             the test store is reading the developer's $HOME database",
            listed_before.len()
        );

        // (2) Two independently constructed servers must not share a store.
        let sid = "isolation-test".to_string();
        a.build_and_store_engine(&sid, vec![make_fn_event(0, 100, 1, "main")], Language::C)
            .await;
        a.save_session(Parameters(SaveSessionParams {
            session_id: sid.clone(),
            language: "native".to_string(),
            target: "/bin/isolation".to_string(),
        }))
        .await
        .unwrap();

        assert_eq!(
            a.store.list_sessions().expect("list_sessions").len(),
            1,
            "server a should see its own session"
        );
        assert_eq!(
            b.store.list_sessions().expect("list_sessions").len(),
            0,
            "server b must not see server a's session (stores must be isolated)"
        );
    }

    /// m9-75: the store path resolution is a pure function of the two
    /// environment values, so the policy is testable without mutating the
    /// process environment.
    #[test]
    fn test_default_store_path_prefers_db_path_and_falls_back_to_home() {
        assert_eq!(
            crate::composition::default_store_path(Some("/tmp/explicit.redb"), Some("/home/dev")),
            std::path::PathBuf::from("/tmp/explicit.redb")
        );
        assert_eq!(
            crate::composition::default_store_path(None, Some("/home/dev")),
            std::path::PathBuf::from("/home/dev/.local/share/chronos/sessions.redb")
        );
        assert_eq!(
            crate::composition::default_store_path(Some(""), Some("/home/dev")),
            std::path::PathBuf::from("/home/dev/.local/share/chronos/sessions.redb"),
            "an empty CHRONOS_DB_PATH must not become a relative store path"
        );
        assert_eq!(
            crate::composition::default_store_path(None, None),
            std::path::PathBuf::from("./.local/share/chronos/sessions.redb")
        );
    }

    /// m9-75: only an explicit opt-in enables the in-memory fallback.
    #[test]
    fn test_allow_in_memory_fallback_is_strict() {
        for yes in ["1", "true", "TRUE", " yes ", "Yes"] {
            assert!(
                crate::composition::allow_in_memory_fallback(Some(yes)),
                "{yes:?} must opt in"
            );
        }
        for no in ["", "0", "false", "no", "maybe", "2", "on"] {
            assert!(
                !crate::composition::allow_in_memory_fallback(Some(no)),
                "{no:?} must not opt in"
            );
        }
        assert!(!crate::composition::allow_in_memory_fallback(None));
    }

    /// m9-75: a path whose parent does not exist yet is created, not rejected.
    #[test]
    fn test_open_store_at_creates_missing_parent_directories() {
        let dir = unique_test_dir("m9-75-missing-parent");
        let path = dir.join("nested").join("deeper").join("sessions.redb");
        let store = crate::composition::open_session_store_at(&path, false)
            .expect("a fresh path must be created");
        store
            .save_session(
                test_session_metadata("fresh"),
                &[make_fn_event(0, 100, 1, "main")],
            )
            .expect("the freshly created store must accept a session");
        assert!(path.exists(), "the store file must exist after opening it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// m9-75 (closes `FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE`):
    /// a store that exists but cannot be opened is an error, and the only way
    /// past it is the explicit opt-in, which really does give an in-memory store.
    ///
    /// The "cannot be opened" fixture is a store already open in this process:
    /// redb allows one `Database` per path per process, so the second open fails
    /// deterministically. Before m9-75 the server turned that failure into a
    /// silent in-memory store, i.e. `session_save` reported success while
    /// `session_list` stayed empty.
    #[test]
    fn test_open_store_at_fails_closed_instead_of_degrading_silently() {
        let dir = unique_test_dir("m9-75-fail-closed");
        let path = dir.join("sessions.redb");

        // Positive control: the file store works and persists.
        let live = crate::composition::open_session_store_at(&path, false)
            .expect("first open must succeed");
        live.save_session(
            test_session_metadata("persisted"),
            &[make_fn_event(0, 100, 1, "main")],
        )
        .expect("file-backed save must succeed");

        // (1) Fail closed: the locked store must be reported, not replaced.
        let err = match crate::composition::open_session_store_at(&path, false) {
            Ok(_) => panic!("a store that cannot be opened must not be silently replaced"),
            Err(e) => e,
        };
        assert_eq!(err.path(), path.as_path());
        assert!(
            err.to_string().contains("CHRONOS_ALLOW_IN_MEMORY_FALLBACK"),
            "the error must name the opt-in: {err}"
        );
        assert!(
            std::error::Error::source(&err).is_some(),
            "the underlying store error must be preserved"
        );

        // (2) The documented opt-in still degrades, loudly: the store is
        // genuinely in-memory, so what it accepts is never persisted.
        let degraded = crate::composition::open_session_store_at(&path, true)
            .expect("the explicit opt-in must still start");
        degraded
            .save_session(
                test_session_metadata("ephemeral"),
                &[make_fn_event(0, 100, 1, "main")],
            )
            .expect("in-memory save must succeed");
        assert_eq!(
            degraded.list_sessions().expect("list_sessions").len(),
            1,
            "the degraded store sees its own session"
        );

        // (3) Differential: once the file store is free it holds exactly the
        // session written to it and none of the degraded one's.
        drop(degraded);
        drop(live);
        let reopened = crate::composition::open_session_store_at(&path, false)
            .expect("the store must be openable again");
        let sessions: Vec<String> = reopened
            .list_sessions()
            .expect("list_sessions")
            .into_iter()
            .map(|m| m.session_id)
            .collect();
        assert!(
            sessions.contains(&"persisted".to_string()),
            "the file store must keep its own session, found {sessions:?}"
        );
        assert!(
            !sessions.contains(&"ephemeral".to_string()),
            "the degraded store's session must not leak into the file store, found {sessions:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_save_session_not_found_in_memory() {
        let server = ChronosServer::new();
        let result = server
            .save_session(Parameters(SaveSessionParams {
                session_id: "this-does-not-exist".to_string(),
                language: "native".to_string(),
                target: "/bin/test".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn test_load_session_not_found_in_store() {
        let server = ChronosServer::new();
        let result = server
            .load_session(Parameters(LoadSessionParams {
                session_id: "this-does-not-exist-in-store".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_resource_limits_default_values() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_events, 1_000_000);
        assert_eq!(limits.timeout_secs, 60);
    }

    #[test]
    fn test_resource_limits_custom_values() {
        let limits = ResourceLimits {
            max_events: 500_000,
            timeout_secs: 120,
        };
        assert_eq!(limits.max_events, 500_000);
        assert_eq!(limits.timeout_secs, 120);
    }

    // ========================================================================
    // SF6 — Inspection Tools Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_test_python_frame_with_locals(
        id: u64,
        ts: u64,
        tid: u64,
        locals: Vec<chronos_domain::VariableInfo>,
    ) -> TraceEvent {
        TraceEvent::python_call_with_locals(
            id,
            MonotonicNs::from(ts),
            tid,
            "my_module.my_func",
            "/path/to/script.py",
            10,
            locals,
        )
    }

    #[allow(dead_code)]
    fn make_test_memory_event(
        id: u64,
        ts: u64,
        tid: u64,
        address: u64,
        size: usize,
        data: Vec<u8>,
    ) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::MemoryWrite,
            SourceLocation::from_address(address),
            EventData::Memory {
                address,
                size,
                data: Some(data),
            },
        )
    }

    #[tokio::test]
    async fn test_debug_get_variables_python() {
        let locals = vec![chronos_domain::VariableInfo::new(
            "count",
            "42",
            "int",
            0x1000,
            chronos_domain::VariableScope::Local,
        )];
        let events = vec![make_test_python_frame_with_locals(0, 100, 1, locals)];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: sid,
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("count") && text.contains("42"));
    }

    #[tokio::test]
    async fn test_debug_get_variables_empty() {
        // PythonFrame with None locals
        let event = TraceEvent::python_call(
            0,
            MonotonicNs::from(100),
            1,
            "my_module.my_func",
            "/path/to/script.py",
            10,
        );
        let events = vec![event];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: sid,
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("variables"));
    }

    #[tokio::test]
    async fn test_debug_get_variables_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_get_variables(Parameters(DebugGetVariablesParams {
                session_id: "nonexistent".to_string(),
                event_id: 0,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    // ========================================================================
    // SF7 — Phase 11 Missing Tools Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_register_event(
        id: u64,
        ts: u64,
        tid: u64,
        regs: chronos_domain::RegisterState,
    ) -> TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation};
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::Custom,
            SourceLocation::from_address(regs.rip),
            EventData::Registers(regs),
        )
    }

    #[tokio::test]
    async fn test_debug_diff_variables_changed() {
        use chronos_domain::VariableScope;
        let locals_a = vec![chronos_domain::VariableInfo::new(
            "x",
            "10",
            "i32",
            0x1000,
            VariableScope::Local,
        )];
        let locals_b = vec![chronos_domain::VariableInfo::new(
            "x",
            "20",
            "i32",
            0x1000,
            VariableScope::Local,
        )];
        let events = vec![
            TraceEvent::python_call_with_locals(
                0,
                MonotonicNs::from(100),
                1,
                "f",
                "test.py",
                10,
                locals_a,
            ),
            TraceEvent::python_call_with_locals(
                1,
                MonotonicNs::from(200),
                1,
                "f",
                "test.py",
                15,
                locals_b,
            ),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .debug_diff(Parameters(DebugDiffParams {
                session_id: sid,
                event_id_a: 0,
                event_id_b: 1,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("variables_changed") && text.contains("x"));
    }

    #[tokio::test]
    async fn test_debug_diff_session_not_found() {
        let server = ChronosServer::new();
        let result = server
            .debug_diff(Parameters(DebugDiffParams {
                session_id: "nonexistent".to_string(),
                event_id_a: 0,
                event_id_b: 1,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
    }

    // ========================================================================
    // Phase 25 — drop_session Tool Tests
    // ========================================================================

    #[tokio::test]
    async fn test_drop_session_removes_from_memory() {
        let server = ChronosServer::new();
        let sid = "drop-test-session".to_string();
        let events = vec![make_fn_entry(0, 100, 1, "main")];

        // Build engine (registers in engines + session_languages)
        server
            .build_and_store_engine(&sid, events, Language::Python)
            .await;

        // Verify it's registered in memory
        assert!(server.engines.lock().await.contains_key(&sid));
        assert!(server.session_languages.lock().await.contains_key(&sid));

        // Drop the session
        let result = server
            .drop_session(Parameters(DropSessionParams {
                session_id: sid.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("dropped"), "Should contain 'dropped' status");
        assert!(
            text.contains("Persistent storage not affected"),
            "Should mention storage not affected"
        );

        // Verify all in-memory state is gone
        assert!(!server.engines.lock().await.contains_key(&sid));
        assert!(!server.session_languages.lock().await.contains_key(&sid));
        assert!(!server.connected_sessions.lock().unwrap().contains(&sid));
    }

    #[tokio::test]
    async fn test_drop_session_not_found_is_idempotent() {
        let server = ChronosServer::new();

        // Drop non-existent session - should return success with not_found status
        let result = server
            .drop_session(Parameters(DropSessionParams {
                session_id: "nonexistent-session".to_string(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not_found"),
            "Should contain 'not_found' status"
        );
    }

    // ========================================================================
    // compare_sessions tests
    // ========================================================================

    #[tokio::test]
    async fn test_compare_sessions_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: "nonexistent-a".to_string(),
                session_b: "nonexistent-b".to_string(),
            }))
            .await
            .unwrap();

        // Should return an error because sessions don't exist in store
        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("session A not found") || text.contains("not found"));
    }

    #[tokio::test]
    async fn test_compare_sessions_identical() {
        use chronos_domain::{EventData, SourceLocation};
        let server = ChronosServer::new();

        // Build and save two identical sessions
        let make_event = |id: u64, func: &str| {
            let loc = SourceLocation::new("test.rs", 1, func, 0x1000 + id);
            TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                EventType::FunctionEntry,
                loc,
                EventData::Function {
                    name: func.to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        };

        let events = vec![make_event(0, "main"), make_event(1, "helper")];
        let sid_a = "cmp-test-a".to_string();
        let sid_b = "cmp-test-b".to_string();

        // Save both sessions to the store
        let meta_a = SessionMetadata {
            session_id: sid_a.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        let meta_b = SessionMetadata {
            session_id: sid_b.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 200,
            tail_sealed: false,
            sealed_at: None,
        };
        server.store.save_session(meta_a, &events).unwrap();
        server.store.save_session(meta_b, &events).unwrap();

        let result = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: sid_a.clone(),
                session_b: sid_b.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("similarity_pct"));
        assert!(text.contains("common_count"));
        assert!(text.contains("summary"));
        // Identical events → high similarity
        assert!(text.contains("100") || text.contains("highly similar"));
    }

    // ========================================================================
    // session_compare / session_explain tests (m7-03)
    // ========================================================================

    /// Re-uses the helpers from the v1 tests above; saves one session
    /// so `session_compare{kind=divergence}` can be exercised end-to-end.
    #[tokio::test]
    async fn test_session_compare_divergence_kind_routes_via_shim() {
        use chronos_domain::{EventData, SourceLocation};
        let server = ChronosServer::new();

        let make_event = |id: u64, func: &str| {
            let loc = SourceLocation::new("test.rs", 1, func.to_string(), 0x1000 + id);
            TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                EventType::FunctionEntry,
                loc,
                EventData::Function {
                    name: func.to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        };

        let events = vec![make_event(0, "main"), make_event(1, "helper")];
        let sid_a = "sc-div-a".to_string();
        let sid_b = "sc-div-b".to_string();
        let meta = SessionMetadata {
            session_id: sid_a.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        let meta_b = SessionMetadata {
            session_id: sid_b.clone(),
            ..meta.clone()
        };
        server.store.save_session(meta, &events).unwrap();
        server.store.save_session(meta_b, &events).unwrap();

        let result = server
            .session_compare(Parameters(SessionCompareParams {
                kind: "divergence".to_string(),
                session_a: sid_a.clone(),
                session_b: sid_b.clone(),
                top_n: None,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        // v2 divergence returns Divergence variant with provenance + similarity_pct
        assert!(
            text.contains("divergence") || text.contains("Divergence"),
            "expected divergence variant in output: {text}"
        );
        assert!(text.contains("similarity_pct"));
        assert!(text.contains("provenance"));
    }

    /// m9-74 (`FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE`): m7-03 rerouted the two
    /// deprecated v1 tools through the `session_compare` dispatcher, and with
    /// them changed what they return — the tagged `SessionCompareOutput`
    /// envelope instead of the flat v1 result their names, parameters and
    /// descriptions still promise. `SessionCompareOutput`'s own doc comment
    /// says "MCP shims drop the `provenance` field so existing v1 callers see
    /// the same JSON they did before m7-03", so the flat shape is the declared
    /// contract, not a preference. This pins the split: shims flat, v2 enveloped.
    #[tokio::test]
    async fn test_v1_shims_return_flat_result_while_v2_returns_envelope() {
        use chronos_domain::{EventData, SourceLocation};

        /// Pull the JSON payload out of a tool call's content block.
        fn tool_json(result: &CallToolResult) -> serde_json::Value {
            let content = serde_json::to_value(&result.content).expect("content serializes");
            let text = content[0]["text"].as_str().expect("text content");
            serde_json::from_str(text).expect("tool payload is JSON")
        }

        let server = ChronosServer::new();
        let make_event = |id: u64, func: &str| {
            let loc = SourceLocation::new("test.rs", 1, func.to_string(), 0x3000 + id);
            TraceEvent::new(
                id,
                MonotonicNs::from(id * 100),
                1,
                EventType::FunctionEntry,
                loc,
                EventData::Function {
                    name: func.to_string(),
                    signature: None,
                    symbol_id: None,
                    invocation_id: None,
                    parent_invocation_id: None,
                },
            )
        };

        // One shared event plus one event each: a genuine divergence.
        let events_a = vec![make_event(0, "main"), make_event(1, "helper")];
        let events_b = vec![make_event(0, "main"), make_event(1, "other")];
        let sid_a = "wire-a".to_string();
        let sid_b = "wire-b".to_string();
        let meta = SessionMetadata {
            session_id: sid_a.clone(),
            created_at: 0,
            language: "native".to_string(),
            target: "/bin/test".to_string(),
            event_count: events_a.len(),
            duration_ms: 100,
            tail_sealed: false,
            sealed_at: None,
        };
        let meta_b = SessionMetadata {
            session_id: sid_b.clone(),
            event_count: events_b.len(),
            ..meta.clone()
        };
        server.store.save_session(meta, &events_a).unwrap();
        server.store.save_session(meta_b, &events_b).unwrap();

        // v1 shim #1 — `compare_sessions` must be flat.
        let shim = server
            .compare_sessions(Parameters(CompareSessionsParams {
                session_a: sid_a.clone(),
                session_b: sid_b.clone(),
            }))
            .await
            .unwrap();
        assert_ne!(shim.is_error, Some(true));
        let v = tool_json(&shim);
        assert_eq!(v["session_a_id"], serde_json::json!(sid_a));
        assert_eq!(v["session_b_id"], serde_json::json!(sid_b));
        assert!(v["similarity_pct"].is_number(), "flat result fields: {v}");
        assert!(
            v.get("provenance").is_none(),
            "v1 shim leaked the v2 envelope (provenance): {v}"
        );
        assert!(
            v.get("result").is_none() && v.get("kind").is_none(),
            "v1 shim leaked the v2 envelope (result/kind): {v}"
        );

        // v1 shim #2 — `performance_regression_audit` must be flat.
        let shim = server
            .performance_regression_audit(Parameters(PerformanceRegressionAuditParams {
                baseline_session_id: sid_a.clone(),
                target_session_id: sid_b.clone(),
                top_n: Some(5),
            }))
            .await
            .unwrap();
        assert_ne!(shim.is_error, Some(true));
        let v = tool_json(&shim);
        assert_eq!(v["baseline_session_id"], serde_json::json!(sid_a));
        assert_eq!(v["target_session_id"], serde_json::json!(sid_b));
        assert!(
            v["functions_analyzed"].is_number(),
            "flat result fields: {v}"
        );
        assert!(
            v.get("provenance").is_none(),
            "v1 shim leaked the v2 envelope (provenance): {v}"
        );

        // v2 keeps the tagged envelope.
        let v2 = server
            .session_compare(Parameters(SessionCompareParams {
                kind: "divergence".to_string(),
                session_a: sid_a,
                session_b: sid_b,
                top_n: None,
            }))
            .await
            .unwrap();
        assert_ne!(v2.is_error, Some(true));
        let v = tool_json(&v2);
        assert_eq!(v["kind"], "divergence");
        assert!(
            v.get("provenance").is_some(),
            "v2 must carry provenance: {v}"
        );
        assert!(
            v["result"]["session_a_id"].is_string(),
            "v2 result nests the flat shape: {v}"
        );
    }

    /// `session_compare` with an unknown kind returns the parser-level error.
    #[tokio::test]
    async fn test_session_compare_unknown_kind_rejected() {
        let server = ChronosServer::new();
        let result = server
            .session_compare(Parameters(SessionCompareParams {
                kind: "wat".to_string(),
                session_a: "x".to_string(),
                session_b: "y".to_string(),
                top_n: None,
            }))
            .await;
        // The parser returns `rmcp::ErrorData` (not `CallToolResult`), so the
        // outer Result is Err; the wrapper short-circuits with `?`.
        assert!(
            result.is_err(),
            "expected parser-level rejection, got {:?}",
            result
        );
    }

    /// `session_explain{kind=facts}` returns a FactsBundle variant.
    /// Uses an empty-session store path: load_session returns SessionNotFound,
    /// so the wrapper maps it to a tool error (not a parse error).
    #[tokio::test]
    async fn test_session_explain_session_not_found_returns_tool_error() {
        let server = ChronosServer::new();
        let result = server
            .session_explain(Parameters(SessionExplainParams {
                session_id: "no-such-session".to_string(),
                kind: "facts".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not found"),
            "expected 'not found' in error: {text}"
        );
    }

    // ========================================================================
    // SF10 — Browser/WASM Probe Tool Tests
    // ========================================================================

    #[tokio::test]
    async fn test_browser_probe_start_chrome_not_available() {
        let server = ChronosServer::new();

        // Force Chrome to be unavailable by using an invalid path
        std::env::set_var("CHROME_PATH", "/nonexistent/chrome/path/for/test");
        let result = server
            .browser_probe_start(Parameters(BrowserProbeStartParams {
                url: "http://example.com".to_string(),
                headless: true,
                chrome_path: None,
            }))
            .await
            .unwrap();
        std::env::remove_var("CHROME_PATH");

        // Result is either "Chrome is not available" (if is_chrome_available fails)
        // or "Failed to start browser probe: Chrome not found" (if spawn fails)
        let text = format!("{:?}", result.content);
        if result.is_error == Some(true) {
            assert!(
                text.contains("Chrome") || text.contains("chrome"),
                "Should mention Chrome issue, got: {}",
                text
            );
        }
        // If Chrome IS available on this system, the probe might succeed — that's OK
    }

    #[tokio::test]
    async fn test_browser_probe_stop_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: "nonexistent-browser-session".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not found"),
            "Should mention 'not found', got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_browser_probe_drain_session_not_found() {
        let server = ChronosServer::new();

        let result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: "nonexistent-browser-session".to_string(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("not found"),
            "Should mention 'not found', got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_browser_probe_lifecycle_with_mock_session() {
        // Test the full lifecycle: create mock session → drain → stop
        // This exercises the MCP integration without requiring real Chrome
        let server = ChronosServer::new();
        let session_id = "test-browser-mock-session".to_string();

        // Manually inject a mock browser probe session (simulating browser_probe_start)
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                backend: adapter.clone(),
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Drain events from the mock session (should return 0 events since no Chrome)
        let drain_result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_ne!(drain_result.is_error, Some(true), "drain should succeed");
        let drain_text = format!("{:?}", drain_result.content);
        assert!(
            drain_text.contains("total_buffered"),
            "Should have total_buffered field"
        );
        assert!(drain_text.contains("0"), "Should have 0 buffered events");

        // Stop the mock session
        let stop_result = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        assert_ne!(stop_result.is_error, Some(true), "stop should succeed");
        let stop_text = format!("{:?}", stop_result.content);
        assert!(
            stop_text.contains("stopped"),
            "Should contain 'stopped' status"
        );
        assert!(
            stop_text.contains("total_events"),
            "Should have total_events field"
        );

        // Verify session is removed
        assert!(
            !server
                .live_browser_probes
                .lock()
                .unwrap()
                .contains_key(&session_id),
            "Session should be removed after stop"
        );
    }

    #[tokio::test]
    async fn test_browser_probe_drain_with_offset_limit() {
        let server = ChronosServer::new();
        let session_id = "test-browser-offset-limit".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                backend: adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Drain with offset=0, limit=0 (should return empty)
        let result = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 0,
                offset: 0,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let text = format!("{:?}", result.content);
        assert!(text.contains("returned"), "Should have returned field");

        // Cleanup
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams { session_id }))
            .await;
    }

    #[tokio::test]
    async fn test_browser_probe_stop_removes_from_active_session() {
        let server = ChronosServer::new();
        let session_id = "test-browser-active-cleanup".to_string();

        // Inject mock session and set as active
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                backend: adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);

            // Set as active session
            let mut active = server.active_session.lock().await;
            *active = Some(session_id.clone());
        }

        // Stop should remove from live_browser_probes
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        // Verify session is gone from live_browser_probes
        assert!(
            !server
                .live_browser_probes
                .lock()
                .unwrap()
                .contains_key(&session_id),
            "Session should be removed from live_browser_probes"
        );

        // Note: browser_probe_stop does NOT clear active_session — that's by design
        // (active_session tracks the last used session for query tools)
    }

    #[tokio::test]
    async fn test_browser_probe_double_stop_returns_error() {
        let server = ChronosServer::new();
        let session_id = "test-browser-double-stop".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                backend: adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // First stop should succeed
        let first = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();
        assert_ne!(first.is_error, Some(true));

        // Second stop should return error (session already removed)
        let second = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(
            second.is_error,
            Some(true),
            "Second stop should be an error"
        );
        let text = format!("{:?}", second.content);
        assert!(text.contains("not found"), "Should mention not found");
    }

    #[tokio::test]
    async fn test_browser_probe_drain_after_stop_returns_error() {
        let server = ChronosServer::new();
        let session_id = "test-browser-drain-after-stop".to_string();

        // Inject mock session
        {
            let adapter = Arc::new(BrowserAdapter::new());
            let capture_session = CaptureSession::new(
                0,
                Language::WebAssembly,
                CaptureConfig::new("http://test.local"),
            );
            let probe = BrowserProbeSession {
                backend: adapter,
                session: capture_session,
                session_id: session_id.clone(),
                url: "http://test.local".to_string(),
            };
            server
                .live_browser_probes
                .lock()
                .unwrap()
                .insert(session_id.clone(), probe);
        }

        // Stop the session
        let _ = server
            .browser_probe_stop(Parameters(BrowserProbeStopParams {
                session_id: session_id.clone(),
            }))
            .await
            .unwrap();

        // Drain after stop should return error
        let drain = server
            .browser_probe_drain(Parameters(BrowserProbeDrainParams {
                session_id: session_id.clone(),
                limit: 1000,
                offset: 0,
            }))
            .await
            .unwrap();
        assert_eq!(
            drain.is_error,
            Some(true),
            "Drain after stop should be an error"
        );
    }

    /// m1-08 — the daemon round is a no-op for backends without an
    /// attached log. Verifies we don't accidentally fabricate
    /// compaction activity on probes that opted out of the log.
    #[tokio::test(flavor = "current_thread")]
    async fn m1_08_auto_compaction_round_skips_backends_without_log() {
        use chronos_domain::ports::NativeProbeController;
        use chronos_domain::{CaptureConfig, CaptureSession, Language};
        use chronos_native::native_probe_controller::NativeProbeControllerImpl;
        use chronos_native::probe_backend::NativeProbeBackend;

        let server = Arc::new(ChronosServer::new());

        // Register a backend with NO execution log attached.
        let backend_no_log = std::sync::Arc::new(NativeProbeBackend::new());
        // Build the minimum LiveProbeSession: the daemon only
        // reads `controller.execution_log()`, so we stub the other
        // fields with dummies that compile.
        let dummy_session = CaptureSession::new(0, Language::Rust, CaptureConfig::new("noop"));
        // REC-C1.2a: a session always owns a log (the type is not `Option`), so
        // the pre-C1.2a "no log attached" fixture is not representable, and the
        // old trailing comment claiming the postcondition was "no log was
        // created" was simply wrong -- this test creates the log itself, on
        // purpose, one line above. What the round must tolerate is a log with
        // nothing to compact.
        //
        // That is the postcondition worth asserting, and it is observable:
        // `run_one_compaction_round` reports what it reclaimed, and a round
        // over an empty log must reclaim nothing. The old body ran the round
        // and then asserted nothing at all, on the grounds that absence cannot
        // be observed -- but `reclaimed_paths` and the cumulative metrics are
        // exactly how absence is observed here.
        let log_dir = std::env::temp_dir().join(format!(
            "rec-c1-2a-compaction-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned_for_round = chronos_services::session_log::SessionExecutionLog::create_for_tests(
            &log_dir,
            chronos_log::SessionId::new("rec-c1-2a-compaction"),
        )
        .expect("test log");
        let controller_no_log: Box<dyn NativeProbeController> =
            Box::new(NativeProbeControllerImpl::new(
                backend_no_log,
                chronos_domain::session_id::SessionId::from("no-log-session"),
                dummy_session.clone(),
            ));
        let live = LiveProbeSession {
            controller: controller_no_log,
            session: dummy_session,
            language: Language::Rust,
            target: "noop".to_string(),
            attached: false,
            uprobe_handle: None,
            ebpf_attachment: None,
            execution_log: owned_for_round,
        };
        {
            let mut probes = server.live_probes.lock().unwrap();
            probes.insert("no-log-session".to_string(), live);
        }

        // A round over a log with nothing to compact must not fabricate
        // activity. The session stays registered after the round, so the
        // metrics are readable through it.
        ChronosServer::run_one_compaction_round(&server).await;

        let after = {
            let probes = server.live_probes.lock().unwrap();
            let live = probes
                .get("no-log-session")
                .expect("the session is still registered after the round");
            live.execution_log
                .compaction_metrics()
                .expect("compaction metrics after the round")
        };
        assert_eq!(
            after.segments_reclaimed, 0,
            "a round over a log with nothing to compact must not reclaim segments"
        );
        assert_eq!(
            after.compaction_passes, 0,
            "no compaction pass may have run: there was nothing retired to compact"
        );
    }

    /// m2-08 — `probe_drain_log` surfaces each event's `data` (which carries
    /// the `EventData::Function` identity fields) in the JSON output, without
    /// dropping the existing flat projection. Uses the in-process slot seam to
    /// attach a real `SegmentedExecutionLog` holding an identity-bearing
    /// record, so no ptrace is needed.
    #[tokio::test(flavor = "current_thread")]
    async fn m2_08_probe_drain_log_surfaces_event_data_identity() {
        use chronos_domain::{CaptureConfig, CaptureSession, SourceLocation};
        use chronos_log::{
            ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
            SessionId as LogSessionId,
        };
        use chronos_native::native_probe_controller::NativeProbeControllerImpl;
        use chronos_native::probe_backend::NativeProbeBackend;
        use std::sync::Arc;

        let session_key = "sess-drain-identity".to_string();

        // Build the log session id the backend expects ("native-<key>").
        let log_session_id = format!("native-{}", session_key);
        let dir = std::env::temp_dir().join(format!(
            "chronos-m2-08-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let sym = chronos_domain::SymbolId::new("main", None, chronos_domain::Language::C);
        let inv = chronos_domain::InvocationId::now();
        let parent = chronos_domain::InvocationId::now();
        let ev = TraceEvent::new(
            7,
            MonotonicNs::from(700),
            1,
            EventType::FunctionEntry,
            SourceLocation::new("main.c", 3, "main", 0x1000),
            EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: Some(sym),
                invocation_id: Some(inv),
                parent_invocation_id: Some(parent),
            },
        );

        // Open a log, attach it to the backend, and append the record.
        let backend = std::sync::Arc::new(NativeProbeBackend::new());
        let concrete = Arc::new(
            SegmentedExecutionLog::open(
                LogSessionId::new(&log_session_id),
                SegmentedConfig::with_dir(&dir),
            )
            .unwrap(),
        );
        concrete
            .append(NewExecutionRecord {
                kind: chronos_log::ExecutionKind::Raw,

                session_id: LogSessionId::new(&log_session_id),
                monotonic_ns: 700,
                payload: ExecutionPayload::new(serde_json::to_vec(&ev).unwrap(), "trace_event"),
                invocation_id: Some(inv),
                parent_invocation_id: Some(parent),
                symbol_id: Some(sym),
                captured_at_unix_ns: None,
            })
            .unwrap();
        concrete.flush().ok();
        // REC-C3.3.2 — wire the writer through `attach_execution_log`
        // with an `Arc<dyn ExecutionLogProvider>`. The legacy test
        // slot is gone.
        let provider: Arc<dyn chronos_domain::ports::execution_log::ExecutionLogProvider> =
            Arc::new(chronos_log::provider::SegmentedExecutionLogProvider::new(
                LogSessionId::new(&log_session_id),
                concrete.clone(),
            ));
        backend.attach_execution_log(provider);

        // Register the backend as a live probe session.
        //
        // REC-C1.2: the session OWNS the log. The backend keeps its clone only
        // for writing, and the read path reads through the session — there is no
        // backend read fallback. So this fixture must hand the session the log,
        // exactly as `ProbeService::start` does.
        let server = Arc::new(ChronosServer::new());
        let dummy_session =
            CaptureSession::new(0, chronos_domain::Language::C, CaptureConfig::new("noop"));
        let owned_log = chronos_services::session_log::SessionExecutionLog::from_segmented_log(
            LogSessionId::new(&log_session_id),
            concrete.clone(),
            Some(dir.clone()),
        );
        let live = LiveProbeSession {
            controller: Box::new(NativeProbeControllerImpl::new(
                backend,
                chronos_domain::session_id::SessionId::from(log_session_id.clone()),
                dummy_session.clone(),
            )),
            session: dummy_session,
            language: chronos_domain::Language::C,
            target: "noop".to_string(),
            attached: false,
            uprobe_handle: None,
            ebpf_attachment: None,
            execution_log: owned_log,
        };
        server
            .live_probes
            .lock()
            .unwrap()
            .insert(session_key.clone(), live);

        let result = server
            .probe_drain_log(Parameters(ProbeDrainLogParams {
                session_id: session_key,
                since: None,
                limit: 256,
            }))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        // Flat projection preserved.
        assert!(s.contains("event_id"), "flat event_id missing: {s}");
        assert!(s.contains("FunctionEntry"), "kind missing: {s}");
        // New: data object surfaced with identity.
        assert!(s.contains("data"), "data field missing: {s}");
        assert!(s.contains("invocation_id"), "invocation_id missing: {s}");
        assert!(
            s.contains(&inv.to_string()),
            "invocation_id value missing: {s}"
        );
        assert!(s.contains("parent_invocation_id"), "parent missing: {s}");
        assert!(s.contains("symbol_id"), "symbol_id missing: {s}");
        assert!(s.contains("main"), "function name missing: {s}");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ========================================================================
    // M3 — Mutation Lens Tests
    // ========================================================================

    #[allow(dead_code)]
    fn make_var_write(id: u64, ts: u64, tid: u64, name: &str, value: &str) -> TraceEvent {
        use chronos_domain::{SourceLocation, VariableInfo, VariableScope};
        let loc = SourceLocation::new("", 0, "", 0x1000 + id);
        TraceEvent::new(
            id,
            MonotonicNs::from(ts),
            tid,
            EventType::VariableWrite,
            loc,
            EventData::Variable(VariableInfo::new(
                name,
                value,
                "string",
                0x7FFE0000 + id,
                VariableScope::Local,
            )),
        )
    }

    #[tokio::test]
    async fn test_mutation_lens_no_session_returns_error() {
        let server = ChronosServer::new();
        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: "nonexistent".to_string(),
                target: None,
                limit: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_mutation_lens_with_two_writes_returns_one_transition() {
        let events = vec![
            make_var_write(0, 100, 1, "x", "1"),
            make_var_write(1, 200, 1, "x", "2"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: sid,
                target: None,
                limit: None,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(
            s.contains("count") && s.contains("transitions"),
            "expected valid output: {s}"
        );
        assert!(
            s.contains("target") && s.contains("x"),
            "expected target 'x' in output: {s}"
        );
    }

    #[tokio::test]
    async fn test_mutation_lens_filters_by_target() {
        let events = vec![
            make_var_write(0, 100, 1, "x", "1"),
            make_var_write(1, 200, 1, "y", "10"),
            make_var_write(2, 300, 1, "x", "2"),
            make_var_write(3, 400, 1, "y", "20"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .mutation_lens(Parameters(MutationLensParams {
                session_id: sid,
                target: Some("x".to_string()),
                limit: None,
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        // Should get exactly 1 transition for x (two writes: 1->2)
        assert!(s.contains("count"), "expected count field: {s}");
        // Should NOT contain y transitions
        assert!(
            !s.contains("target") || !s.contains("y"),
            "target filter should exclude y: {s}"
        );
    }

    // ========================================================================
    // M3 — Causal Slice Tests
    // ========================================================================

    #[tokio::test]
    async fn test_causal_slice_no_session_returns_error() {
        let server = ChronosServer::new();
        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: "nonexistent".to_string(),
                sink_event_id: 0,
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_causal_slice_sink_not_found_returns_error() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "compute"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: sid,
                sink_event_id: 999, // Does not exist
            }))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(s.contains("not found"), "expected 'not found' error: {s}");
    }

    #[tokio::test]
    async fn test_causal_slice_returns_included_path_for_sink() {
        let events = vec![
            make_fn_entry(0, 100, 1, "main"),
            make_fn_entry(1, 200, 1, "compute"),
            make_fn_exit(2, 300, 1, "compute"),
            make_fn_exit(3, 400, 1, "main"),
        ];
        let (server, sid) = server_with_session(events).await;

        let result = server
            .causal_slice(Parameters(CausalSliceParams {
                session_id: sid,
                sink_event_id: 3, // main exit
            }))
            .await
            .unwrap();

        assert_ne!(result.is_error, Some(true));
        let s = format!("{:?}", result.content[0]);
        assert!(
            s.contains("included") && s.contains("missing") && s.contains("depth"),
            "expected causal slice output with included/missing/depth: {s}"
        );
        // Sink event 3 should be in the included set.
        assert!(s.contains("3"), "expected sink event 3 in output: {s}");
    }
}

/// ServerHandler implementation with custom server identity.
/// This overrides the auto-generated one from #[tool_router(server_handler)]
/// to provide correct name/version instead of rmcp defaults.
#[rmcp::tool_handler(
    name = "chronos-mcp",
    version = "0.1.0",
    instructions = "Time-travel debugging server for AI agents. Use session_start to capture program execution, then query with events_read, execution_query (kind=call_stack / race_detect / execution_summary / call_graph / hotspot / saliency), causal_slice (kind=causality / variable_origin / crash / memory_audit), state_query, observe, and session_compare. The v1 read aliases were removed in C5.3.2; only the v2 tools above are callable."
)]
impl rmcp::handler::server::ServerHandler for ChronosServer {}

// ---- MS-CAP-DISCOVERY: toolset / capability-awareness tests ----

#[cfg(test)]
mod cap_discovery_tests {
    use super::*;

    #[test]
    fn active_toolset_defaults_to_auto() {
        // Without CHRONOS_ACTIVE_TOOLSET, server uses "auto".
        let server = ChronosServer::new();
        assert_eq!(server.active_toolset(), "auto");
    }

    #[test]
    fn native_toolset_count_leq_25() {
        // REQ-CAP-005: native toolset must have ≤ 25 tools.
        assert!(
            NATIVE_TOOL_NAMES.len() <= 25,
            "native toolset has {} tools, must be ≤ 25",
            NATIVE_TOOL_NAMES.len()
        );
    }

    #[test]
    fn all_tool_names_matches_router_count() {
        // C5.3 (REC-C5): 41 tools remained after deleting the 22 deprecated
        // alias handlers (was 63). REC-C3.3 (Tren B slice F) added
        // `capture_session`, bringing the total to 42. Sourced from the
        // live `#[tool]`
        // router so this assertion cannot drift relative to the actual
        // tool registrations in this file.
        let from_router = ChronosServer::tool_router().list_all().len();
        assert_eq!(
            ALL_TOOL_NAMES.len(),
            from_router,
            "ALL_TOOL_NAMES length ({actual}) disagrees with the live \
             #[tool] router count ({from_router}). The \
             toolset_sync_check::all_tool_names_matches_router test \
             should have caught this; investigate before relaxing \
             either side.",
            actual = ALL_TOOL_NAMES.len(),
        );
    }

    #[tokio::test]
    async fn execution_log_read_fails_closed_for_unknown_session() {
        // M10: the read path must never turn "no log" into an empty
        // success. A caller that sees `events: []` with no error will
        // conclude the session is quiet, which is a false negative about
        // evidence — the most dangerous failure mode for a read path.
        let server = ChronosServer::new();
        let result = server
            .execution_log_read(Parameters(ExecutionLogReadParams {
                session_id: "definitely-not-a-session".to_string(),
                mode: ExecutionLogReadKind::Poll,
                cursor: None,
                bucket_size_ns: default_bucket_size_ns(),
                limit: 10,
            }))
            .await
            .expect("tool call returns a result, not a transport error");

        assert_eq!(
            result.is_error,
            Some(true),
            "unknown session must fail closed, not return an empty envelope"
        );
        let text = format!("{:?}", result.content);
        assert!(
            !text.contains("\"event_count\":0"),
            "fail-closed response must not carry a zero-count success shape, got: {text}"
        );
    }

    #[tokio::test]
    async fn execution_log_read_rejects_cursor_from_another_session() {
        // A cursor is minted for one session; replaying it against another
        // would silently reinterpret positions. The tool must refuse
        // instead of anchoring to the wrong log.
        let server = ChronosServer::new();
        let foreign = chronos_services::events_cursor::EventsCursorV1::start(
            chronos_domain::SessionId::new("session-a".to_string()),
        );
        let result = server
            .execution_log_read(Parameters(ExecutionLogReadParams {
                session_id: "session-b".to_string(),
                mode: ExecutionLogReadKind::Poll,
                cursor: Some(foreign.encode()),
                bucket_size_ns: default_bucket_size_ns(),
                limit: 10,
            }))
            .await
            .expect("tool call returns a result");
        assert_eq!(
            result.is_error,
            Some(true),
            "a cursor minted for another session must be rejected"
        );
    }

    #[tokio::test]
    async fn execution_log_read_causality_reports_unsupported_without_engine() {
        // Causality over a *readable* log but with no engine loaded is a
        // legitimate status, not a failure: the agent learns that hints are
        // unavailable instead of seeing a spurious error. This must stay
        // distinguishable from the fail-closed case above, where there is
        // no log to speak of at all.
        use chronos_services::session_log::SessionExecutionLog;
        let session = chronos_domain::SessionId::new("m10-causality-probe".to_string());
        let dir =
            std::env::temp_dir().join(format!("m10_causality_{}", uuid::Uuid::new_v4().simple()));
        let log = SessionExecutionLog::create_for_tests(&dir, session.clone())
            .expect("create a real log for the probe");
        let server = ChronosServer::new();
        // Register through the very registry the production server built,
        // so the read path and `events_read` observe the same single
        // authority. No engine is ever registered for this session.
        server
            .execution_log_registry()
            .register(log)
            .expect("register log");

        let result = server
            .execution_log_read(Parameters(ExecutionLogReadParams {
                session_id: session.as_str().to_string(),
                mode: ExecutionLogReadKind::Causality,
                cursor: None,
                bucket_size_ns: default_bucket_size_ns(),
                limit: default_limit(),
            }))
            .await
            .expect("tool call returns a result");

        assert_ne!(
            result.is_error,
            Some(true),
            "causality over a readable log must answer, not fail: {:?}",
            result.content
        );
        let text = format!("{:?}", result.content);
        assert!(
            text.contains("engine_loaded"),
            "causality mode must report engine_loaded, got: {text}"
        );
        assert!(
            text.contains("Unsupported"),
            "with no engine loaded the status is Unsupported, got: {text}"
        );
    }

    #[test]
    fn tool_availability_map_covers_every_registered_tool() {
        // The map is built by inserting one entry per `ALL_TOOL_NAMES`
        // member, so its size cannot diverge from the toolset: the real
        // invariant is coverage, not a hand-maintained count. Pin the
        // count only as a cheap canary, and derive it from the router so
        // adding a `#[tool]` handler cannot leave it stale.
        let server = ChronosServer::new();
        let map = server.build_tool_availability(ALL_TOOL_NAMES, Some("rust"));
        assert_eq!(
            map.len(),
            ChronosServer::tool_router().list_all().len(),
            "tool_availability must cover every tool the router exposes",
        );
        for name in ALL_TOOL_NAMES {
            assert!(
                map.contains_key(*name),
                "tool '{name}' is missing from tool_availability; a tool that \
                 is registered but not discoverable is invisible to clients",
            );
        }
    }

    #[test]
    fn tool_availability_debug_get_variables_requires_language() {
        // C5.3 (REC-C5): `evaluate_expression` was retired. `debug_get_variables`
        // remains and is the canonical language-required tool; it was in the
        // `tool_required_capabilities` `language/any` bucket before C5.3.
        let server = ChronosServer::new();
        let map = server.build_tool_availability(ALL_TOOL_NAMES, Some("rust"));
        let avail = map.get("debug_get_variables").unwrap();
        assert!(
            !avail.required_capabilities.is_empty(),
            "debug_get_variables should require capabilities"
        );
        assert!(
            avail
                .required_capabilities
                .iter()
                .any(|r| r.contains("language")),
            "debug_get_variables requires a language capability"
        );
    }

    #[test]
    fn native_toolset_marks_browser_tools_unavailable() {
        let server = ChronosServer::with_toolset("native");

        assert!(!server.is_tool_listed("browser_probe_start"));
        assert!(!server.is_tool_listed("browser_probe_stop"));
        assert!(!server.is_tool_listed("browser_probe_drain"));
        // But native tools ARE listed.
        assert!(server.is_tool_listed("events_read"));
        assert!(server.is_tool_listed("capabilities"));
    }

    #[test]
    fn auto_toolset_lists_all_tools() {
        let server = ChronosServer::with_toolset("auto");

        assert!(server.is_tool_listed("browser_probe_start"));
        assert!(server.is_tool_listed("evaluate_expression"));
        assert!(server.is_tool_listed("events_read"));
    }

    #[test]
    fn garbage_toolset_defaults_to_auto() {
        // REQ-CAP-004: unrecognised toolset treated as auto.
        let server = ChronosServer::with_toolset("garbage");

        assert!(server.is_tool_listed("browser_probe_start"));
        assert!(server.is_tool_listed("evaluate_expression"));
    }

    #[test]
    fn python_toolset_excludes_browser_probe() {
        let server = ChronosServer::with_toolset("python");

        assert!(!server.is_tool_listed("browser_probe_start"));
        assert!(!server.is_tool_listed("browser_probe_stop"));
        assert!(!server.is_tool_listed("browser_probe_drain"));
    }

    #[test]
    fn native_toolset_excludes_language_tools() {
        let server = ChronosServer::with_toolset("native");

        assert!(!server.is_tool_listed("evaluate_expression"));
        assert!(!server.is_tool_listed("debug_get_variables"));
        assert!(!server.is_tool_listed("debug_get_memory"));
    }
}

/// Build-time assertion: every entry in `ALL_TOOL_NAMES` is declared via
/// `#[tool(name = "...")]` in this file, and vice versa.
///
/// Closes FIND-DEBT-002 (coupling) and the original MS-CAP-DISCOVERY
/// followup concern: a future cycle that adds a new `#[tool]` without
/// adding it to `ALL_TOOL_NAMES` (or trims `ALL_TOOL_NAMES` without
/// removing the corresponding `#[tool]`) will fail this test when
/// `cargo test -p chronos-mcp` is run, instead of drifting silently at
/// runtime.
///
/// Implementation note: rmcp 1.5's `#[tool_router]` macro generates a
/// `pub fn tool_router()` method on the impl block that returns a
/// `ToolRouter<Self>`. Calling `.list_all()` on it yields the live
/// `Vec<Tool>` populated from every `#[tool]` attribute in this file.
/// That router list is the single source of truth; `ALL_TOOL_NAMES` is
/// a const mirror used by `build_tool_availability` and friends.
///
/// Spec: REQ-CAP-007 `AllToolNamesBuildAssertion`.
///
/// When adding/removing `#[tool(name = …)]` registrations, extend or trim
/// `ALL_TOOL_NAMES` accordingly.
#[cfg(test)]
mod toolset_sync_check {
    use super::*;

    /// Return the live list of `#[tool]`-registered names by introspecting
    /// the macro-generated `ToolRouter`. This is the single source of
    /// truth for what tools the server exposes.
    fn router_tool_names() -> Vec<String> {
        ChronosServer::tool_router()
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect()
    }

    /// Assert that the router's tool list and `ALL_TOOL_NAMES` are in
    /// lockstep. Replaces the old `REGISTERED_TOOLS` mirror and the two
    /// `all_tool_names_covers_registrations` / `all_tool_names_have_registration`
    /// tests that depended on a hand-maintained duplicate.
    #[test]
    fn all_tool_names_matches_router() {
        let from_router: std::collections::HashSet<_> = router_tool_names().into_iter().collect();
        let from_const: std::collections::HashSet<_> =
            ALL_TOOL_NAMES.iter().copied().map(String::from).collect();

        let missing_from_const: Vec<_> = from_router.difference(&from_const).collect();
        let extra_in_const: Vec<_> = from_const.difference(&from_router).collect();

        assert!(
            missing_from_const.is_empty() && extra_in_const.is_empty(),
            "ALL_TOOL_NAMES is out of sync with the #[tool] router.\n\
             Tools declared via #[tool] but missing from ALL_TOOL_NAMES: {missing_from_const:?}\n\
             Entries in ALL_TOOL_NAMES but with no corresponding #[tool]: {extra_in_const:?}\n\
             Update ALL_TOOL_NAMES to keep capability discovery in sync."
        );
    }

    /// Asserts the router has at least 42 tools — a tripwire against
    /// accidental bulk deletion of `#[tool]` registrations.
    ///
    /// C5.3 (REC-C5) shrank the wire surface from 63 → 41 tools;
    /// REC-C3.3 (Tren B slice F) added `capture_session` (42). The
    /// floor mirrors the current budget (AC-32-2).
    #[test]
    fn router_has_expected_minimum_tool_count() {
        let count = router_tool_names().len();
        assert!(
            count >= 42,
            "Router has only {count} tools, expected at least 42. \
             Did someone delete a chunk of #[tool] registrations?"
        );
    }
}
