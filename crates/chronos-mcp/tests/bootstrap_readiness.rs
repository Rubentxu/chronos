//! UAT-R5 (REC-C1.5 closure) — readiness invariant of `ChronosServer::try_new`.
//!
//! ## Invariant
//!
//! A fresh `ChronosServer` must be **immediately ready** for `events_read`
//! on any session whose `ExecutionLog` already exists on disk under the
//! configured root. The registry must be populated as part of
//! `try_new`/`new`, NOT by an explicit bootstrap call from the caller.
//!
//! If a caller had to manually invoke
//! `chronos_services::execution_log_bootstrap::bootstrap_execution_logs`
//! before the first read, the binary restart path would silently regress
//! to "session is `Unavailable` until somebody restarts the binary"
//! (the bug the bootstrap-on-startup step is explicitly meant to close).
//!
//! ## What we prove
//!
//! The on-disk repository already has two `ExecutionLog`s for `rec-c1.5-r5-a`
//! and `rec-c1.5-r5-b`. We point the production locator at the tempdir
//! (`CHRONOS_EXECUTION_LOG_DIR`), then build `ChronosServer::new` — exactly
//! the entrypoint the production binary uses.
//!
//! After construction, WITHOUT any subsequent `bootstrap_execution_logs`
//! call, the registry must contain both sessions. If it doesn't,
//! `try_new` did not perform the bootstrap it advertises, and the binary
//! restart would surface `SessionNotFound` on every read until somebody
//! restarted again.
//!
//! ## Scope
//!
//! This is an in-process unit test: it does NOT spawn the binary as a
//! subprocess, because the readiness invariant is a property of the
//! library entry point. The companion restart_uat.rs R3 covers the
//! full process-restart round-trip end to end.

use std::env;
use std::path::PathBuf;

use chronos_log::SessionId;
use chronos_mcp::ChronosServer;
use chronos_services::session_log::SessionExecutionLog;

fn unique_root() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = env::temp_dir().join(format!(
        "chronos-rec-c1.5-r5-{pid}-{n}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("create tempdir root");
    dir
}

/// Build a `SessionExecutionLog` for `session_id` at `<root>/<session_id>/`.
///
/// Writes a `execution-log.manifest.json` and an initial empty segment —
/// the exact shape `bootstrap_execution_logs` discovers and reopens.
fn seed_execution_log(root: &std::path::Path, session_id: &str) -> SessionExecutionLog {
    let session = SessionId::new(session_id.to_string());
    let dir = root.join(session_id);
    std::fs::create_dir_all(&dir).expect("create session dir");
    SessionExecutionLog::create(&dir, session).expect("create SessionExecutionLog")
}

#[test]
fn new_immediately_populates_registry_from_disk() {
    let root = unique_root();

    // Seed two sessions under the same root so the assertion can require
    // BOTH to be present, not just one.
    let _seeded_a = seed_execution_log(&root, "rec-c1.5-r5-a");
    let _seeded_b = seed_execution_log(&root, "rec-c1.5-r5-b");

    // Pretend we are the production binary: the env var names the root
    // that `chronos_log::resolve_execution_log_root()` reads.
    //
    // We also point `CHRONOS_DB_PATH` at a fresh temp redb so this test
    // never tries to open the developer's real store (which can fail with
    // DatabaseAlreadyOpen when other test runs in the workspace share
    // the same on-disk file — a coupling `FIND-M9-69` explicitly closed).
    //
    // SAFETY: env::set_var is unsafe from Rust 2024; this test owns its
    // own keys and removes them before returning.
    let db_path = root.join("test-only-sessions.redb");
    unsafe {
        env::set_var("CHRONOS_EXECUTION_LOG_DIR", &root);
        env::set_var("CHRONOS_DB_PATH", &db_path);
    }

    // try_new is the entrypoint every production caller must use.
    // It MUST succeed (no panic, no error) AND must have populated the
    // registry as a side effect of the bootstrap call inside.
    let server = ChronosServer::try_new()
        .expect("try_new must succeed on a clean tempdir with seeded logs");

    let registry = server.execution_log_registry();
    let registry_len = registry.len();
    assert_eq!(
        registry_len, 2,
        "try_new must have bootstrapped exactly the two seeded sessions; \
         got {registry_len} registered (SessionNotFound regression — bootstrap \
         did not run, caller is expected to call it manually)"
    );
    assert!(
        registry.contains("rec-c1.5-r5-a"),
        "session A missing from registry after try_new"
    );
    assert!(
        registry.contains("rec-c1.5-r5-b"),
        "session B missing from registry after try_new"
    );

    // Clean up the env vars so they don't leak into the next test.
    unsafe {
        env::remove_var("CHRONOS_EXECUTION_LOG_DIR");
        env::remove_var("CHRONOS_DB_PATH");
    }
}
