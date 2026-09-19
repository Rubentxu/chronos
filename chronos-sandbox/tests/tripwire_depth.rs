//! Tripwire depth tests — verify tripwire behavior in edge cases and depth scenarios.
//!
//! Category TD tests cover tripwire query idempotency, deletion, recreation, and multiple types.
//!
//! CIH-E: every tripwire tool call must pass an explicit `session_id`
//! (canonical scope). The server maps it to `scope=session{id}` and the
//! canonical-evidence observe pipeline resolves the canonical session
//! from the explicit scope; the implicit `active_session` fallback is
//! no longer a supported path.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::{TripwireConditionType, TripwireCreateParams};
use chronos_sandbox::McpSession;
use std::time::Duration;

/// CIH-E: helper that starts a real probe session against the
/// `test_busyloop` fixture so the tripwire tools have a canonical
/// session to scope against. Returns the new `session_id`.
async fn start_real_session(client: &mut McpTestClient) -> String {
    let fixture = McpSession::fixture_path("test_busyloop")
        .expect("test_busyloop fixture not found — run `cargo build` first");

    let session_id = client
        .probe_start(fixture.to_str().unwrap())
        .await
        .expect("probe_start failed for test fixture");

    // Let the probe warm up.
    tokio::time::sleep(Duration::from_millis(300)).await;

    session_id
}

/// TD1: tripwire_query is idempotent — does not consume fired events.
/// Create a tripwire watching for function_entry.
/// Call tripwire_query twice — fired_count should be the same both times.
#[tokio::test]
async fn test_tripwire_query_does_not_consume() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Create a tripwire watching for function entries (scoped).
    let tripwire_id = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::FunctionName {
                    pattern: "main".into(),
                },
                label: Some("watch_main".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect("tripwire_create failed");

    println!("✓ Created tripwire: {}", tripwire_id);

    // First tripwire_query (scoped).
    let tripwires_1 = client
        .tripwire_query(Some(&session_id))
        .await
        .expect("tripwire_query (1) failed");

    let found_1 = tripwires_1.iter().find(|t| t.id == tripwire_id);
    assert!(
        found_1.is_some(),
        "Created tripwire should be in query result"
    );
    let fire_count_1 = found_1.unwrap().fire_count;

    println!("First query: fire_count = {}", fire_count_1);

    // Second tripwire_query — should return same fire_count (not consumed)
    let tripwires_2 = client
        .tripwire_query(Some(&session_id))
        .await
        .expect("tripwire_query (2) failed");

    let found_2 = tripwires_2.iter().find(|t| t.id == tripwire_id);
    assert!(
        found_2.is_some(),
        "Created tripwire should still be in query result"
    );
    let fire_count_2 = found_2.unwrap().fire_count;

    println!("Second query: fire_count = {}", fire_count_2);

    // Assert: fire_count should be identical (not consumed by query)
    assert_eq!(
        fire_count_1, fire_count_2,
        "tripwire_query should not consume fired events (idempotent)"
    );

    client.shutdown().await.ok();
}

/// TD2: tripwire_delete reduces count by 1.
/// Create 3 tripwires; list → count_before (>=3);
/// Delete one; list → count_after;
/// Assert: count_after == count_before - 1.
#[tokio::test]
async fn test_tripwire_count_after_delete() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Create 3 tripwires (scoped).
    let ids = vec![
        client
            .tripwire_create(
                Some(&session_id),
                TripwireCreateParams {
                    condition: TripwireConditionType::FunctionName {
                        pattern: "func_a".into(),
                    },
                    label: Some("delete_test_1".into()),
                    session_id: Some(session_id.clone()),
                },
            )
            .await
            .expect("tripwire_create (1) failed"),
        client
            .tripwire_create(
                Some(&session_id),
                TripwireCreateParams {
                    condition: TripwireConditionType::FunctionName {
                        pattern: "func_b".into(),
                    },
                    label: Some("delete_test_2".into()),
                    session_id: Some(session_id.clone()),
                },
            )
            .await
            .expect("tripwire_create (2) failed"),
        client
            .tripwire_create(
                Some(&session_id),
                TripwireCreateParams {
                    condition: TripwireConditionType::FunctionName {
                        pattern: "func_c".into(),
                    },
                    label: Some("delete_test_3".into()),
                    session_id: Some(session_id.clone()),
                },
            )
            .await
            .expect("tripwire_create (3) failed"),
    ];

    println!("✓ Created 3 tripwires: {:?}", ids);

    // List tripwires (scoped) — count should be >= 3
    let list_before = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("tripwire_list failed");
    let count_before = list_before.len();
    println!("Count before delete: {}", count_before);
    assert!(count_before >= 3, "Should have at least 3 tripwires");

    // Delete the middle one (scoped).
    let id_to_delete = &ids[1];
    client
        .tripwire_delete(Some(&session_id), id_to_delete)
        .await
        .expect("tripwire_delete failed");
    println!("✓ Deleted tripwire: {}", id_to_delete);

    // List again (scoped) — count should be one less
    let list_after = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("tripwire_list failed");
    let count_after = list_after.len();
    println!("Count after delete: {}", count_after);

    assert_eq!(
        count_after,
        count_before - 1,
        "count_after ({}) should == count_before ({}) - 1",
        count_after,
        count_before
    );

    // Verify the deleted one is gone
    assert!(
        !list_after.iter().any(|t| t.id == *id_to_delete),
        "Deleted tripwire should not appear in list"
    );

    client.shutdown().await.ok();
}

/// TD3: tripwire_create with invalid condition returns graceful error (not crash).
/// Send a malformed or empty condition — server should not crash.
#[tokio::test]
async fn test_tripwire_create_invalid_condition_graceful() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Try to create a tripwire with an empty condition.
    // The server uses TripwireConditionType which is an enum with specific variants.
    // An "invalid" condition could be an EventType with empty event_types list.
    let result = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::EventType {
                    event_types: vec![], // Empty list — potentially invalid
                },
                label: Some("invalid_test".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await;

    match result {
        Ok(id) => {
            // Some implementations accept empty conditions gracefully
            println!("✓ tripwire_create with empty condition succeeded: {}", id);
        }
        Err(e) => {
            // Error is also acceptable — graceful handling
            println!(
                "✓ tripwire_create with empty condition returned error: {:?}",
                e
            );
        }
    }

    client.shutdown().await.ok();
}

/// TD4: Create tripwires of different types.
/// Create tripwire watching for function_entry;
/// Create tripwire watching for syscall_enter;
/// tripwire_list → assert at least 2 tripwires exist;
/// Clean up: delete both.
#[tokio::test]
async fn test_tripwire_multiple_types() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Create tripwire for function_entry (using FunctionName pattern)
    let id_func = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::FunctionName {
                    pattern: "malloc".into(),
                },
                label: Some("func_watch".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect("tripwire_create (function) failed");

    // Create tripwire for syscall_enter
    let id_syscall = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::EventType {
                    event_types: vec!["syscall_enter".into()],
                },
                label: Some("syscall_watch".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect("tripwire_create (syscall) failed");

    println!("✓ Created function tripwire: {}", id_func);
    println!("✓ Created syscall tripwire: {}", id_syscall);

    // List tripwires (scoped) — should have at least 2
    let list = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("tripwire_list failed");

    println!("Total tripwires: {}", list.len());
    assert!(list.len() >= 2, "Should have at least 2 tripwires");

    // Verify both are in the list
    let has_func = list.iter().any(|t| t.id == id_func);
    let has_syscall = list.iter().any(|t| t.id == id_syscall);
    assert!(has_func, "Function tripwire should be in list");
    assert!(has_syscall, "Syscall tripwire should be in list");

    // Clean up: delete both (scoped).
    client
        .tripwire_delete(Some(&session_id), &id_func)
        .await
        .expect("tripwire_delete (func) failed");
    client
        .tripwire_delete(Some(&session_id), &id_syscall)
        .await
        .expect("tripwire_delete (syscall) failed");

    println!("✓ Both tripwires deleted");

    client.shutdown().await.ok();
}

/// TD5: tripwire_delete with nonexistent ID is idempotent (no crash).
/// tripwire_delete with id="nonexistent-tripwire-xyz-123"
/// Assert: graceful response (error or success, not crash).
#[tokio::test]
async fn test_tripwire_delete_nonexistent_idempotent() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Try to delete a non-existent tripwire (scoped).
    let result = client
        .tripwire_delete(Some(&session_id), "nonexistent-tripwire-xyz-123")
        .await;

    match result {
        Ok(()) => {
            println!("✓ tripwire_delete for nonexistent succeeded (idempotent)");
        }
        Err(e) => {
            // Error is also acceptable — graceful handling
            println!("✓ tripwire_delete for nonexistent returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

/// TD6: Create, delete, recreate tripwire with same condition.
/// tripwire_create → id_1; tripwire_delete(id_1);
/// tripwire_create same condition → id_2;
/// Assert: id_2 is valid, tripwire works.
#[tokio::test]
async fn test_tripwire_create_delete_recreate() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Create tripwire (scoped).
    let id_1 = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::FunctionName {
                    pattern: "recreate_test".into(),
                },
                label: Some("recreate_original".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect("tripwire_create (1) failed");

    println!("✓ Created tripwire: {}", id_1);

    // Delete it (scoped).
    client
        .tripwire_delete(Some(&session_id), &id_1)
        .await
        .expect("tripwire_delete failed");
    println!("✓ Deleted tripwire: {}", id_1);

    // Recreate with same condition (scoped).
    let id_2 = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::FunctionName {
                    pattern: "recreate_test".into(),
                },
                label: Some("recreate_new".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect("tripwire_create (2) failed");

    println!("✓ Recreated tripwire: {}", id_2);

    // Assert: new ID is valid and tripwire is listable (scoped).
    let list = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("tripwire_list failed");

    let found = list.iter().find(|t| t.id == id_2);
    assert!(
        found.is_some(),
        "Recreated tripwire {} should be in list",
        id_2
    );
    println!("✓ Tripwire {} is valid and queryable", id_2);

    // Clean up (scoped).
    client
        .tripwire_delete(Some(&session_id), &id_2)
        .await
        .expect("tripwire_delete (cleanup) failed");

    client.shutdown().await.ok();
}
