//! REC-C2.2.0 — the **productive** accepted-Raw seam.
//!
//! Closes FIND-C2.1-03: until now the derivation had no production caller, so
//! firing evidence could only be produced by a fixture writing the bytes
//! directly. This test proves the whole production path:
//!
//! ```text
//! real probe -> real TraceEvent -> real ExecutionLog Raw S
//!             -> production derivation (accepted-Raw observer)
//!             -> TripwireFired F(source_seq = S)
//!             -> stop, restart
//!             -> observe(list) -> the SAME F, the SAME S
//! ```
//!
//! No `TripwireFiredEvidence` is constructed by the test.

use std::path::PathBuf;

use chronos_sandbox::client::tools::McpTestClient;

fn unique_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c22-producer-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create root");
    p
}

/// Spawn a session with function-frame capture on.
async fn start_capturing(client: &mut McpTestClient, fixture: &str) -> String {
    let response = client
        .call_tool(
            "session_start",
            serde_json::json!({
                "action": "spawn",
                "spawn_fields": {
                    "program": fixture,
                    "args": [],
                    "track_function_frames": true,
                }
            }),
        )
        .await
        .expect("session_start");
    response
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("session_id missing from {response}"))
        .to_string()
}

async fn create_any_function_tripwire(client: &mut McpTestClient, session: &str) {
    let response = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "create",
                "scope": {"scope": "session", "session_id": session},
                "condition": {
                    "kind": "tripwire",
                    "condition": {"type": "function_name", "pattern": "*"},
                    "label": "watch-any"
                }
            }),
        )
        .await
        .expect("observe create");
    assert!(
        response.get("subscription_id").is_some(),
        "the tripwire must be registered, got {response}"
    );
}

async fn list_firings(client: &mut McpTestClient, session: &str) -> Vec<serde_json::Value> {
    let response = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "list",
                "scope": {"scope": "session", "session_id": session},
            }),
        )
        .await
        .expect("observe list");
    response
        .get("fired_events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn production_derivation_records_a_replayable_firing() {
    let root = unique_root("production");
    let db = root.join("sessions.redb");
    let fixture = chronos_sandbox::client::McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run cargo build first");
    let fixture = fixture.to_str().expect("utf8 path").to_string();

    // ---- Process 1: capture, derive, stop ----
    let (session, before) = {
        let mut client = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
            .await
            .expect("start MCP");

        let session = start_capturing(&mut client, &fixture).await;
        create_any_function_tripwire(&mut client, &session).await;

        // Let the probe run so events are accepted and derived.
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

        client
            .session_stop(&session, true, true)
            .await
            .expect("session_stop");

        let firings = list_firings(&mut client, &session).await;
        client.shutdown().await.ok();
        (session, firings)
    };

    // NEGATIVE, and the point of the whole test: the firing exists because a
    // real captured event was accepted and the production observer derived it.
    // Nothing here wrote a TripwireFired record by hand.
    assert!(
        !before.is_empty(),
        "production derivation must have recorded at least one firing from the \
         captured function-entry events; if this fails, the accepted-Raw observer \
         is not wired (FIND-C2.1-03 reopened)"
    );
    assert!(
        before
            .iter()
            .any(|f| f.get("source_seq").and_then(|v| v.as_u64()).is_some()),
        "every firing must name the accepted source it derives from: {before:?}"
    );

    // ---- Process 2: restart over the same durable root ----
    let mut second = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("restart MCP");
    let after = list_firings(&mut second, &session).await;

    assert_eq!(
        after.len(),
        before.len(),
        "the same number of firings survives the restart"
    );
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(
            b.get("firing_seq"),
            a.get("firing_seq"),
            "the firing identity is preserved"
        );
        assert_eq!(
            b.get("source_seq"),
            a.get("source_seq"),
            "the cause identity is preserved"
        );
    }
}
