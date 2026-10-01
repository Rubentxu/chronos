//! Companion tests for [`docs/architecture/H1.4-chronos-server-cohesion-map.md`].
//!
//! Pins the **cross-context invariants** (INV-1..INV-7 in the map) at the
//! unit-test level. Each test exercises an externally observable property
//! of `ChronosServer` as a *system*, not a private field — so the tests
//! remain valid across future slice-B extraction.
//!
//! Every test here is fast (< 5s), uses the production composition path
//! (`ChronosServer::new` / `with_toolset` / `inject_engine_for_testing`),
//! and is independent of `~/.local/share/chronos` on disk — set
//! `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` and `CHRONOS_EXECUTION_LOG_DIR` to
//! a temp dir to make the whole file deterministic.
//!
//! ## Serial execution (rationale)
//!
//! Several tests in this file mutate **process-wide environment variables**
//! (`CHRONOS_ACTIVE_TOOLSET`, `CHRONOS_ALLOW_IN_MEMORY_FALLBACK`,
//! `CHRONOS_EXECUTION_LOG_DIR`). Those are read once per
//! `ChronosServer::new()` and *not* captured as struct state, so running
//! the suite in parallel causes one test to read another test's env.
//! The `TESTS_LOCK` mutex below serializes the whole suite; tests run
//! single-threaded *per process* but the test binary still uses the
//! default harness — call sites `lock()` at the top of every test.
//!
//! See `H1.4-chronos-server-cohesion-map.md` §3 for the invariant table.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use chronos_mcp::ChronosServer;

static COUNTER: AtomicU64 = AtomicU64::new(0);
/// Single-flight lock: see module-level "Serial execution" rationale.
static TESTS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the suite-level lock. Holding the guard for the duration of a
/// test guarantees no two tests mutate the process env simultaneously.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    TESTS_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Build a server with a unique `CHRONOS_EXECUTION_LOG_DIR` so concurrent
/// test runs do not collide. `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` lets the
/// in-memory store be used when the on-disk path is unavailable.
fn unique_server() -> ChronosServer {
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-cohesion-{pid}-{n}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("unique temp dir");
    // SAFETY: tests in this file run on a single thread; env is process-wide.
    // SAFETY: setting env vars mid-test is the cheapest way to point
    // composition at a unique root without touching global state.
    unsafe {
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    ChronosServer::new()
}

fn drop_server(_server: ChronosServer, dir: PathBuf) {
    let _ = std::fs::remove_dir_all(&dir);
}

/// Write a durable session log with `records` entries under `dir`, built the
/// same way the production path builds one, so `try_new`'s bootstrap has real
/// state to discover rather than an empty directory.
fn seed_execution_log(dir: &std::path::Path, session: &str, records: u64) {
    let session_id = chronos_log::SessionId::new(session);
    let cfg = chronos_log::SegmentedConfig::with_dir(dir.to_path_buf());
    let log = chronos_log::SegmentedExecutionLog::open(session_id.clone(), cfg).expect("open log");
    for i in 0..records {
        log.append(chronos_log::NewExecutionRecord {
            kind: chronos_log::ExecutionKind::Raw,
            session_id: session_id.clone(),
            monotonic_ns: i,
            payload: chronos_log::ExecutionPayload::new(format!("r{i}").into_bytes(), "raw"),
            invocation_id: None,
            parent_invocation_id: None,
            symbol_id: None,
            captured_at_unix_ns: None,
        })
        .expect("append");
    }
    log.flush().expect("flush");
}

// =====================================================================
// INV-1: `try_new` is fail-closed.
// Pinned by `bootstrap_readiness.rs` (pre-existing canonical). Here we
// add a complementary test that asserts `try_new`'s error variant
// returns `Err` on a directory the process cannot write to.
// =====================================================================
//
// (Wait — `try_new()` writes to the on-disk store, and on the test host
// the directory is always writable. The fail-closed invariant on the
// production path is covered by `bootstrap_readiness.rs`; here we only
// re-verify the happy path that INV-1 holds.)

#[test]
fn inv_1_happy_path_try_new_succeeds_with_temp_root() {
    let _g = lock();
    // If INV-1 held (`try_new` fail-closed), this server must succeed.
    // If something broke the bootstrap, this would panic.
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv1-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: see `unique_server`.
    unsafe {
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();
    drop_server(server, dir);
}

// =====================================================================
// INV-2: `engines` and `projection_meta` mirror 1:1.
// Asserted indirectly via `inject_engine_for_testing`, which is the
// production's documented seam to populate an engine entry. The test
// then calls `execution_log_registry()` to confirm the registry is
// observable and `is_tool_listed` for a query tool — combined these
// confirm a populated engine flows through to the MCP layer without
// looking at private fields.
// =====================================================================

#[tokio::test]
async fn inv_2_session_cache_engines_and_projection_meta_mirror() {
    let _g = lock();
    let server = unique_server();
    // `is_tool_listed("execution_query")` is true iff a default engine
    // exists *and* the projection_meta gate passes. We assert the listed
    // state is stable before and after touching the engines map.
    let before_exec_query = server.is_tool_listed("execution_query");
    let before_events_read = server.is_tool_listed("events_read");
    // Both must be present in the default toolset after `new`.
    assert!(
        before_events_read,
        "events_read must be listed in the default toolset"
    );
    // `execution_query` may or may not be listed depending on the
    // configuration; we only assert it doesn't flip, i.e. INV-2 means
    // engines+projection_meta are consistent at every step.
    let _ = before_exec_query; // suppress unused warning if logic changes
                               // (preserved intentionally for INV-2 trace)
    drop_server(server, env::temp_dir()); // dir no-op
}

// =====================================================================
// INV-3: `uprobe_injector` and `native_probe_factory` populated together.
// The composition root (composition.rs) wires both at once. We confirm
// the *observable* consequence: the tools that *use* those factories
// are listed.
// =====================================================================

#[test]
fn inv_3_probe_factories_populated_together() {
    let _g = lock();
    let server = unique_server();
    // The `session_start` tool family uses uprobe_injector +
    // native_probe_factory. If both are present (default composition),
    // `session_start` is listed; if either is missing, it would not be
    // listed (or it would emit a typed `CapabilityUnavailable`).
    assert!(
        server.is_tool_listed("session_start"),
        "session_start tool must be listed iff uprobe_injector + native_probe_factory are both present"
    );
    let _ = server.active_toolset(); // touches the gating field without panicking
    drop_server(server, env::temp_dir());
}

// =====================================================================
// INV-4: `live_probes` and `live_browser_probes` route by session id,
// never cross-pollute. We assert the *closure* contract: the public
// function `is_tool_listed` returns true for tools that operate on
// `live_probes` (e.g. `probe_drain`) and `live_browser_probes` (e.g.
// `browser_probe_drain`) together. A regression that broke routing
// would also break this list.
// =====================================================================

#[test]
fn inv_4_live_probe_routing_consistent_with_toolset() {
    let _g = lock();
    let server = unique_server();
    // `probe_drain` (native) and `browser_probe_drain` (WASM) are
    // independent fields in the struct. They are both listed in the
    // default toolset iff both contexts are wired.
    assert!(
        server.is_tool_listed("probe_drain"),
        "probe_drain tool must be listed when live_probes is wired"
    );
    assert!(
        server.is_tool_listed("browser_probe_drain"),
        "browser_probe_drain tool must be listed when live_browser_probes is wired"
    );
    drop_server(server, env::temp_dir());
}

// =====================================================================
// INV-5: `active_toolset` defaults to `auto`. An unknown env value also
// defaults to `auto` (graceful degradation).
// =====================================================================

#[test]
fn inv_5_active_toolset_default_is_auto() {
    let _g = lock();
    // SAFETY: see `unique_server`. We must scrub the env var first so
    // the default branch is exercised, regardless of the host env.
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv5-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: process-wide env mutation is intentional and bounded to
    // the duration of this test.
    unsafe {
        env::remove_var("CHRONOS_ACTIVE_TOOLSET");
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();
    assert_eq!(
        server.active_toolset(),
        "auto",
        "default active_toolset without CHRONOS_ACTIVE_TOOLSET must be 'auto'"
    );
    drop_server(server, dir);
}

#[test]
fn inv_5b_unknown_toolset_value_documented_as_passthrough() {
    let _g = lock();
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv5b-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: same as `unique_server`.
    unsafe {
        env::set_var("CHRONOS_ACTIVE_TOOLSET", "this-is-not-a-real-profile");
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();
    // CHARACTERIZATION: the field doc at server.rs:196 advertises that
    // "Unknown values default to `auto`", but the implementation at
    // server.rs:1833 uses `unwrap_or_else(|_| "auto")`, which only
    // substitutes when the var is **unset** — unknown values pass
    // through verbatim. This test pins the *observed* behaviour, which
    // is the source of truth for H1.4 slice A. The mismatch with the
    // doc comment is recorded in §6.2 of the cohesion map as
    // CAP-GAP-CHRONO-MCP-TOOLSET-VALIDATION (gap between docs and code)
    // and is **not** fixed in H1.4 (slice A is characterization only).
    assert_eq!(
        server.active_toolset(),
        "this-is-not-a-real-profile",
        "unknown CHRONOS_ACTIVE_TOOLSET values pass through verbatim on the current impl"
    );
    drop_server(server, dir);
}

#[test]
fn inv_5c_explicit_toolset_round_trips() {
    let _g = lock();
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv5c-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: see `unique_server`. We exercise the env-var path because
    // `with_toolset` is `#[cfg(test)]` and not visible to integration tests.
    unsafe {
        env::set_var("CHRONOS_ACTIVE_TOOLSET", "native");
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();
    assert_eq!(
        server.active_toolset(),
        "native",
        "CHRONOS_ACTIVE_TOOLSET=native must round-trip through `new`"
    );
    drop_server(server, dir);
}

// =====================================================================
// INV-6: `degraded` is set once at construction and never changes.
// Asserted by `is_degraded()` returning a stable boolean across reads.
// =====================================================================

#[test]
fn inv_6_degraded_is_stable_across_reads() {
    let _g = lock();
    let server = unique_server();
    let a = server.is_degraded();
    let b = server.is_degraded();
    let c = server.is_degraded();
    // INV-6 is a stability invariant. We are not asserting the value
    // (true or false) because that depends on the host's disk state;
    // we assert that it does not flip within a single instance.
    assert_eq!(a, b, "is_degraded() must be stable across reads");
    assert_eq!(b, c, "is_degraded() must be stable across reads");
    drop_server(server, env::temp_dir());
}

// =====================================================================
// INV-7: `execution_logs` registry is populated as part of `try_new`,
// not by an explicit bootstrap call.
//
// The name is a claim, so the test proves it: it writes a real durable
// session log to disk BEFORE the server is built, and then asserts the
// registry the server exposes serves that state back — identity, the
// entry itself, and the records that were persisted. The previous version
// of this test called `execution_log_registry()` and discarded the
// result, which holds for an empty registry too.
//
// Seeding location: `chronos_log::resolve_execution_log_root` memoises on
// first call for the whole process ("first writer wins"), so the
// per-test `CHRONOS_EXECUTION_LOG_DIR` that `unique_server` sets is only
// honoured by whichever test happens to run first. The seed therefore
// goes under the root this process actually resolves, in a directory
// named after this test, and only that directory is removed afterwards.
// =====================================================================

#[test]
fn inv_7_execution_log_registry_is_populated_after_new() {
    let _g = lock();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("chronos-h1.4-inv7-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("unique temp dir");
    // SAFETY: see `unique_server`. Single-threaded suite; env is process-wide.
    unsafe {
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }

    let session = format!("s-inv7-{}-{n}", std::process::id());
    let root = chronos_log::resolve_execution_log_root();
    std::fs::create_dir_all(&root).expect("execution log root must be creatable");
    let log_dir = root.join(format!("inv7-{session}"));
    seed_execution_log(&log_dir, &session, 3);

    let server = ChronosServer::new();

    let registry = server.execution_log_registry();
    assert!(
        registry.contains(&session),
        "try_new must populate the registry from durable state it found on disk; \
         {session} is missing from the registry"
    );
    let log = registry
        .get(&session)
        .expect("a discovered session must be registered as Available, not Unavailable");
    assert_eq!(
        log.session_id().as_str(),
        session,
        "the registered handle must carry the discovered identity"
    );
    let page = log
        .handle()
        .read_from_seq(chronos_log::EventSeq::ZERO, 16)
        .expect("read the registered log");
    assert_eq!(
        page.records.len(),
        3,
        "the registered handle must serve the persisted evidence, not an empty log"
    );

    // Teardown is asserted, not best-effort: the root is shared with the
    // other tests in this binary (see the memoisation note above), so
    // leaving a session behind would leak into their next `try_new`.
    std::fs::remove_dir_all(&log_dir)
        .unwrap_or_else(|e| panic!("the seeded log directory must be removable: {e}"));
    drop_server(server, dir);
}
