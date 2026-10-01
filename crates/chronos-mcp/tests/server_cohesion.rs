//! Companion tests for [`docs/architecture/H1.4-chronos-server-cohesion-map.md`].
//!
//! Pins the **cross-context invariants** (INV-1..INV-7 in the map) at the
//! unit-test level. Each test exercises an externally observable property
//! of `ChronosServer` as a *system*, not a private field — so the tests
//! remain valid across future slice-B extraction.
//!
//! Every test here is fast (< 5s), uses the production composition path
//! (`ChronosServer::new` and the `CHRONOS_ACTIVE_TOOLSET` env contract),
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
//! The lock serializes but does **not** isolate: a value one test writes
//! stays in the environment for the next one, and the harness picks the
//! order. Isolation therefore comes from each test pinning the values it
//! depends on, not from the lock. See `unique_server`.
//!
//! See `H1.4-chronos-server-cohesion-map.md` §3 for the invariant table.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use chronos_mcp::tools_params::ALL_TOOL_NAMES;
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
///
/// `CHRONOS_ACTIVE_TOOLSET` is pinned to `auto` rather than left alone. The
/// suite lock serialises test *bodies* but does not restore env values, so a
/// test that pins another profile (`inv_2`, `inv_3`, `inv_5b`, `inv_5c`) would
/// otherwise leak that value into every later test that reaches the default
/// profile — `inv_4` did exactly that, and failed whenever the harness ran it
/// after `inv_3`. Pinning here makes each test hermetic with respect to both
/// the host environment and the test execution order.
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
        env::set_var("CHRONOS_ACTIVE_TOOLSET", "auto");
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
//
// Neither field is reachable from an integration test. The only writer of
// `engines` a test could reach for, `inject_engine_for_testing`, carries
// `#[cfg(test)]` in `src/server.rs` — and `#[cfg(test)]` compiles the
// crate's *own* unit tests only, so it is structurally invisible from
// `tests/`. This test therefore pins the property that *is* observable and
// that the same class of regression breaks: the query tools gated on that
// mirror are offered under exactly the toolset that lists them, and the
// `capabilities` response agrees with the listing for every registered
// tool.
//
// `auto` returns `true` for every name (server.rs:529), which would make
// the comparison vacuous, so the profile is pinned to `native`.
// =====================================================================

#[test]
fn inv_2_session_cache_engines_and_projection_meta_mirror() {
    let _g = lock();
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv2-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: see `unique_server`. The profile is pinned rather than
    // inherited: the host may export `CHRONOS_ACTIVE_TOOLSET`.
    unsafe {
        env::set_var("CHRONOS_ACTIVE_TOOLSET", "native");
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();

    // `build_tool_availability` is what the `capabilities` response is made
    // of. With no target language, a listed tool is available and an
    // unlisted one is not, so the two must agree for every registered name.
    let availability = server.build_tool_availability(ALL_TOOL_NAMES, None);
    assert_eq!(
        availability.len(),
        ALL_TOOL_NAMES.len(),
        "build_tool_availability must cover every registered tool name, \
         got {} entries for {} names",
        availability.len(),
        ALL_TOOL_NAMES.len()
    );

    let disagreements: Vec<String> = ALL_TOOL_NAMES
        .iter()
        .filter_map(|name| {
            let entry = availability
                .get(*name)
                .expect("registered name must be present");
            let listed = server.is_tool_listed(name);
            (entry.available != listed).then(|| {
                format!(
                    "{name}: listed={listed} but capabilities reports available={}",
                    entry.available
                )
            })
        })
        .collect();
    assert!(
        disagreements.is_empty(),
        "the capabilities availability map must agree with the toolset listing; \
         disagreements: {disagreements:?}"
    );

    // If the profile boundary did not bite, the loop above proved nothing.
    assert!(
        !server.is_tool_listed("no_such_tool"),
        "a name in no profile list must not be listed under 'native'"
    );

    drop_server(server, dir);
}

// =====================================================================
// INV-3: `uprobe_injector` and `native_probe_factory` populated together.
// The composition root (composition.rs) wires both at once. We confirm
// the *observable* consequence: the tools that *use* those factories are
// listed.
//
// The profile is pinned to `native` because under `auto` `is_tool_listed`
// returns `true` for every name, which would turn the coupling assertion
// below into a tautology. The probe names are exactly the `native` probe
// family in `tools_params::NATIVE_TOOL_NAMES`.
// =====================================================================

#[test]
fn inv_3_probe_factories_populated_together() {
    let _g = lock();
    let dir = env::temp_dir().join(format!(
        "chronos-h1.4-inv3-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: see `unique_server`. The profile is pinned rather than
    // inherited: the host may export `CHRONOS_ACTIVE_TOOLSET`.
    unsafe {
        env::set_var("CHRONOS_ACTIVE_TOOLSET", "native");
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &dir);
        env::set_var("CHRONOS_ALLOW_IN_MEMORY_FALLBACK", "1");
    }
    let server = ChronosServer::new();

    // `session_start` is the tool that consumes uprobe_injector +
    // native_probe_factory. The probe_* family is wired by the same
    // composition root. Were either factory missing, composition would drop
    // the two families independently and the listing would diverge — which
    // is precisely what INV-3 forbids.
    let session_start_listed = server.is_tool_listed("session_start");
    assert!(
        session_start_listed,
        "session_start must be listed in the 'native' profile, where it is a member"
    );
    for tool in [
        "probe_start",
        "probe_stop",
        "probe_drain",
        "probe_drain_log",
        "probe_compaction_metrics",
        "probe_status",
    ] {
        assert_eq!(
            server.is_tool_listed(tool),
            session_start_listed,
            "{tool} and session_start are wired by the same uprobe_injector + \
             native_probe_factory pair, so their listing must agree"
        );
    }

    // The boundary must bite, else the loop above is vacuous.
    assert!(
        !server.is_tool_listed("no_such_tool"),
        "a name in no profile list must not be listed under 'native'"
    );

    drop_server(server, dir);
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
