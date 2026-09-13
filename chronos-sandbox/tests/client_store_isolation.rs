//! Client store isolation — every `McpTestClient` gets its own store.
//!
//! Regression coverage for `FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT`.
//! Before this, `McpTestClient::start` inherited the ambient environment, so
//! every client in all 35 sandbox suites opened the *same*
//! `$HOME/.local/share/chronos/sessions.redb`: one multi-megabyte store holding
//! every session the developer had ever saved. `save_session` then had to
//! serialize a full session's events into that shared file behind a fixed 30 s
//! client timeout, and on a slow run it lost the race and failed the test for
//! reasons that had nothing to do with the code under test.
//!
//! These tests are about the *contract* rather than the timing: a client's
//! store is private unless the caller explicitly shares one.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

/// Drive a minimal probe lifecycle and save it, returning the session id.
async fn save_one_session(client: &mut McpTestClient, tag: &str) -> String {
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed");

    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client
        .probe_drain(&session_id)
        .await
        .expect("probe_drain failed");

    let stop = client
        .probe_stop(&session_id)
        .await
        .expect("probe_stop failed");
    assert!(stop.total_events > 0, "probe captured no events");

    // Give the query engine time to build before persisting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    client
        .save_session(&session_id, tag)
        .await
        .expect("save_session failed");

    session_id
}

/// Two clients started through `start()` must never share a store, and neither
/// may point at the developer's real database.
#[tokio::test]
async fn test_each_client_gets_its_own_store_path() {
    let a = McpTestClient::start()
        .await
        .expect("Failed to start MCP server (client A)");
    let b = McpTestClient::start()
        .await
        .expect("Failed to start MCP server (client B)");

    let path_a = a.db_path().expect("client A has no db path").to_path_buf();
    let path_b = b.db_path().expect("client B has no db path").to_path_buf();

    assert_ne!(
        path_a, path_b,
        "two clients must not be started against the same store"
    );

    // Ambient-hostile check: when the runner exports CHRONOS_DB_PATH (a
    // developer who has one configured), the value is inherited by the test
    // process, but no client may actually open it. Measured by running this test
    // as `CHRONOS_DB_PATH=<scratch>/decoy.redb cargo test -p chronos-sandbox
    // --test client_store_isolation`; the decoy file must never appear.
    if let Ok(ambient) = std::env::var("CHRONOS_DB_PATH") {
        let ambient = std::path::PathBuf::from(ambient);
        assert!(
            !ambient.exists(),
            "a sandbox client opened the ambient CHRONOS_DB_PATH {} instead of a \
             private store",
            ambient.display()
        );
        println!(
            "✓ ambient CHRONOS_DB_PATH {} was ignored",
            ambient.display()
        );
    }

    let default = McpTestClient::default_db_path();
    assert_ne!(
        path_a, default,
        "client A must not use the developer's real store"
    );
    assert_ne!(
        path_b, default,
        "client B must not use the developer's real store"
    );

    assert!(
        path_a.starts_with(std::env::temp_dir()),
        "client store should live under the temp dir, got {}",
        path_a.display()
    );

    // Measured, not argued: the client records a path, but only the *server*
    // decides which store it opens. `open_default_store` falls back to an
    // in-memory store when the file cannot be opened, so requiring the file to
    // exist on disk is what proves the server really opened `db_path()`.
    // Without this, a regression that drops `CHRONOS_DB_PATH` from the child
    // environment leaves every assertion above green (FIND-M9-73-SILENT-
    // IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE).
    for (label, path) in [("A", &path_a), ("B", &path_b)] {
        assert!(
            path.exists(),
            "server {label} never opened the store the client recorded at {}: the \
             server fell back to an in-memory store instead",
            path.display()
        );
        assert!(
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > 0,
            "server {label} opened an empty store file at {}",
            path.display()
        );
    }

    println!(
        "✓ client A store: {}\n✓ client B store: {}",
        path_a.display(),
        path_b.display()
    );
}

/// A session saved by one client must be invisible to another client, and a
/// fresh client must see an empty store rather than the developer's sessions.
#[tokio::test]
async fn test_saved_session_is_invisible_to_another_client() {
    let mut a = McpTestClient::start()
        .await
        .expect("Failed to start MCP server (client A)");

    let session_id = save_one_session(&mut a, "isolation_a").await;

    let seen_by_a = a.list_sessions().await.expect("list_sessions (A) failed");
    assert!(
        seen_by_a.iter().any(|s| s.session_id == session_id),
        "client A should see the session it saved"
    );

    let store_a = a.db_path().expect("client A has no db path").to_path_buf();
    assert!(
        store_a.exists(),
        "client A's server never opened {}: it is running on an in-memory store, \
         so 'invisible to B' proves nothing",
        store_a.display()
    );

    let mut b = McpTestClient::start()
        .await
        .expect("Failed to start MCP server (client B)");
    let store_b = b.db_path().expect("client B has no db path").to_path_buf();
    assert!(
        store_b.exists(),
        "client B's server never opened {}: it is running on an in-memory store, \
         so an empty session list is not evidence of isolation",
        store_b.display()
    );
    let seen_by_b = b.list_sessions().await.expect("list_sessions (B) failed");

    assert!(
        !seen_by_b.iter().any(|s| s.session_id == session_id),
        "client B must not see client A's session (shared store regression)"
    );
    assert!(
        seen_by_b.is_empty(),
        "a freshly started client must see an empty store, saw {} session(s)",
        seen_by_b.len()
    );

    println!("✓ client A sees 1 session, client B sees 0");
}

/// Sharing is still possible, but only on request: `start_with_db_path` makes
/// two clients open the same store, which is what `ce12` and the session
/// edge-case suite rely on.
#[tokio::test]
async fn test_explicit_db_path_still_shares_one_store() {
    let dir = std::env::temp_dir().join(format!(
        "chronos-shared-store-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("failed to create shared store dir");
    let db_path = dir.join("sessions.redb");

    let mut a = McpTestClient::start_with_db_path(db_path.clone())
        .await
        .expect("Failed to start MCP server (client A, shared store)");
    let session_id = save_one_session(&mut a, "shared_store").await;
    a.shutdown().await.ok();

    let mut b = McpTestClient::start_with_db_path(db_path.clone())
        .await
        .expect("Failed to start MCP server (client B, shared store)");

    assert_eq!(
        b.db_path(),
        Some(db_path.as_path()),
        "explicit path should be reported verbatim"
    );
    assert!(
        db_path.exists(),
        "shared store {} was never created: the servers fell back to in-memory \
         stores, so sharing is not actually being exercised",
        db_path.display()
    );

    let seen_by_b = b.list_sessions().await.expect("list_sessions (B) failed");
    assert!(
        seen_by_b.iter().any(|s| s.session_id == session_id),
        "clients sharing an explicit path must see each other's sessions"
    );

    drop(b);
    let _ = std::fs::remove_dir_all(&dir);
    println!("✓ explicit shared store still works");
}
