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

/// TD3: `tripwire_create` rejects a condition that can never fire.
///
/// This test used to assert the opposite. It was written to pin a measured
/// defect — the server performed no validation of an empty `event_types` list,
/// returned a tripwire id, and listed a subscription that no event could ever
/// match — so a caller could register a dead watchpoint and pay for it in
/// silence.
///
/// The gap is now closed in `TripwiresService::create`, which rejects a
/// condition that is unsatisfiable by construction. The test follows: it
/// asserts the rejection, that the reason is legible, and that nothing was
/// registered. The last part matters — an error returned *after* registering
/// would leave the dead subscription behind while looking correct.
#[tokio::test]
async fn test_tripwire_create_rejects_unsatisfiable_condition() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    let err = client
        .tripwire_create(
            Some(&session_id),
            TripwireCreateParams {
                condition: TripwireConditionType::EventType {
                    event_types: vec![],
                },
                label: Some("invalid_test".into()),
                session_id: Some(session_id.clone()),
            },
        )
        .await
        .expect_err("an empty event_types list can never match, so it must be refused");

    let message = err.to_string();
    assert!(
        message.to_lowercase().contains("condition"),
        "the error should say the condition is the problem, got: {}",
        message
    );

    // Nothing may be left behind: a rejection that still registered the
    // tripwire would satisfy the assertion above and keep the dead watchpoint.
    let list = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("tripwire_list failed after a rejected create");
    assert!(
        !list
            .iter()
            .any(|t| t.label.as_deref() == Some("invalid_test")),
        "a rejected tripwire must not be registered; list still holds {:?}",
        list
    );

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

/// TD5: `tripwire_delete` rejects unknown ids gracefully, in two
/// distinguishable shapes, and leaves the store untouched.
///
/// The test name keeps the historical "idempotent" wording, but the
/// measured contract is **rejection, not silent success**: an unknown id
/// always produces a structured service error surfaced as
/// `McpSandboxError::RpcError`, never a panic and never a fake `Ok`.
///
/// Measured, and asserted separately because the two shapes take
/// different server paths:
/// - id without the `tripwire-` prefix → `observe: unsupported: observe
///   verb=delete only supports tripwire subscriptions in m7-02; got '<id>'`
///   (prefix guard in `chronos-services/src/observe.rs`)
/// - well-formed but unknown id → `observe: tripwire '<id>' not found`
///   (`TripwiresService::delete` lookup miss)
#[tokio::test]
async fn test_tripwire_delete_nonexistent_idempotent() {
    let mut client = McpTestClient::start()
        .await
        .expect("Failed to start MCP server");

    // CIH-E: drive against a real probe session.
    let session_id = start_real_session(&mut client).await;

    // Shape 1: unknown id that is not even shaped like a tripwire id.
    let malformed_id = "nonexistent-tripwire-xyz-123";
    let err = client
        .tripwire_delete(Some(&session_id), malformed_id)
        .await
        .expect_err("measured: deleting an id without the `tripwire-` prefix is rejected");

    // The error must name the offending id: a generic "unsupported verb"
    // message would mean the request never reached the lookup, which is a
    // different (and useless) outcome for a caller.
    assert!(
        format!("{err:?}").contains(malformed_id),
        "error should echo the rejected id {:?}, got {:?}",
        malformed_id,
        format!("{err:?}")
    );
    println!("✓ Malformed id rejected: {:?}", err);

    // Shape 2: well-formed `tripwire-<n>` id that was never created. This
    // is the case a caller actually hits after a restart or a stale list.
    let unknown_id = "tripwire-9999";
    let err = client
        .tripwire_delete(Some(&session_id), unknown_id)
        .await
        .expect_err("measured: deleting a well-formed but unknown tripwire id is rejected");

    let rendered = format!("{err:?}");
    assert!(
        rendered.contains(unknown_id) && rendered.contains("not found"),
        "error should report {:?} as not found, got {:?}",
        unknown_id,
        rendered
    );
    println!("✓ Unknown but well-formed id rejected: {:?}", err);

    // The failed deletes must not have invented or dropped subscriptions,
    // and the server must still be serving: this scope started empty.
    let list_after = client
        .tripwire_list(Some(&session_id))
        .await
        .expect("server must still answer tripwire_list after rejected deletes");

    assert!(
        list_after.is_empty(),
        "rejected deletes must leave the store untouched; got {:?}",
        list_after
    );

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
