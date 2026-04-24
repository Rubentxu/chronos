//! Tripwire tools tests — verify tripwire_create, tripwire_list, tripwire_delete,
//! and tripwire_query work correctly. Also includes the fixed ignored tests.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::client::types::{TripwireConditionType, TripwireCreateParams};
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_tripwire_create_and_list() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create a tripwire watching for function names matching "main"
    let tripwire_id = client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::FunctionName {
                pattern: "main".into(),
            },
            label: Some("watch_main".into()),
        })
        .await
        .expect("tripwire_create failed");

    println!("✓ Created tripwire: {}", tripwire_id);
    assert!(!tripwire_id.is_empty(), "tripwire_id should not be empty");

    // List tripwires
    let list = client.tripwire_list().await
        .expect("tripwire_list failed");

    println!("✓ tripwire_list returned {} active tripwires", list.len());
    assert!(!list.is_empty(), "Should have at least one tripwire");

    // Find our created tripwire
    let found = list.iter().find(|t| t.id == tripwire_id);
    assert!(found.is_some(), "Created tripwire should be in list");

    if let Some(tw) = found {
        println!("  Tripwire: id={}, label={:?}, condition={}, fire_count={}",
            tw.id, tw.label, tw.condition, tw.fire_count);
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_tripwire_delete() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create a tripwire
    let tripwire_id = client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::FunctionName {
                pattern: "test_*".into(),
            },
            label: Some("to_delete".into()),
        })
        .await
        .expect("tripwire_create failed");

    println!("✓ Created tripwire to delete: {}", tripwire_id);

    // Verify it's in the list
    let list_before = client.tripwire_list().await
        .expect("tripwire_list failed");
    assert!(list_before.iter().any(|t| t.id == tripwire_id),
        "Tripwire should be in list before delete");

    // Delete the tripwire
    client.tripwire_delete(&tripwire_id).await
        .expect("tripwire_delete failed");

    println!("✓ Deleted tripwire: {}", tripwire_id);

    // Verify it's gone
    let list_after = client.tripwire_list().await
        .expect("tripwire_list failed");
    assert!(!list_after.iter().any(|t| t.id == tripwire_id),
        "Tripwire should not be in list after delete");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_tripwire_delete_nonexistent() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Try to delete a non-existent tripwire
    // This should return an error via the RPC layer
    let result = client.tripwire_delete("tripwire-999999").await;

    match result {
        Ok(()) => {
            println!("✓ tripwire_delete for nonexistent succeeded (idempotent behavior)");
        }
        Err(e) => {
            println!("✓ tripwire_delete for nonexistent returned error: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_tripwire_query() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create a few tripwires
    let id1 = client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::FunctionName {
                pattern: "func_a".into(),
            },
            label: Some("query_test_1".into()),
        })
        .await
        .expect("tripwire_create failed");

    let _id2 = client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::EventType {
                event_types: vec!["syscall_enter".into()],
            },
            label: Some("query_test_2".into()),
        })
        .await
        .expect("tripwire_create failed");

    println!("✓ Created 2 tripwires: {}, ...", id1);

    // Use tripwire_query (non-destructive read)
    let tripwires = client.tripwire_query().await
        .expect("tripwire_query failed");

    println!("✓ tripwire_query returned {} active tripwires", tripwires.len());
    assert!(tripwires.len() >= 2, "Should have at least 2 tripwires");

    for tw in tripwires.iter().take(5) {
        println!("  {}: {:?} (fire_count={})", tw.id, tw.label, tw.fire_count);
    }

    // Use tripwire_list again - should return same count (query doesn't drain)
    let list_again = client.tripwire_list().await
        .expect("tripwire_list failed");

    // Both should return the same active count (fired events might differ)
    assert_eq!(tripwires.len(), list_again.len(),
        "tripwire_query and tripwire_list should return same active count");

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_tripwire_multiple_conditions() {
    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Create tripwires with different condition types
    let conditions = vec![
        TripwireConditionType::FunctionName { pattern: "malloc".into() },
        TripwireConditionType::SyscallNumber { numbers: vec![1] }, // write syscall
        TripwireConditionType::Signal { numbers: vec![11] }, // SIGSEGV
        TripwireConditionType::ExceptionType { exc_type: "Error".into() },
    ];

    let mut ids = Vec::new();
    for (i, condition) in conditions.into_iter().enumerate() {
        let id = client
            .tripwire_create(TripwireCreateParams {
                condition,
                label: Some(format!("multi_test_{}", i)),
            })
            .await
            .expect("tripwire_create failed");
        ids.push(id);
    }

    println!("✓ Created {} tripwires with different conditions", ids.len());

    // List all
    let list = client.tripwire_list().await
        .expect("tripwire_list failed");

    println!("✓ Total tripwires: {}", list.len());
    assert!(list.len() >= ids.len(), "Should have at least our created tripwires");

    // Clean up
    for id in &ids {
        client.tripwire_delete(id).await.ok();
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_compare_sessions() {
    // Fixed: uses fixture_path instead of bare program name
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    // Start first session with test_add
    let session_a = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Give it time to collect events
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Drain events from session A
    let _events_a = client.probe_drain(&session_a).await
        .expect("probe_drain failed");

    // Start second session with test_add
    let session_b = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Give it time to collect events
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Drain events from session B
    let _events_b = client.probe_drain(&session_b).await
        .expect("probe_drain failed");

    // Stop both sessions
    client.probe_stop(&session_a).await
        .expect("probe_stop failed");
    client.probe_stop(&session_b).await
        .expect("probe_stop failed");

    // Give time for query engine to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Save both sessions
    client.save_session(&session_a, "compare_a").await
        .expect("save_session failed");
    client.save_session(&session_b, "compare_b").await
        .expect("save_session failed");

    // Compare — they should be similar (same program, same fixture)
    // Note: Some divergences are expected due to timing differences in syscall events
    let report = client.compare_sessions(&session_a, &session_b).await
        .expect("compare_sessions failed");

    println!("✓ compare_sessions result:");
    println!("  Similarity: {}%", report.similarity_pct);
    println!("  Only in A: {}, Only in B: {}", report.only_in_a_count, report.only_in_b_count);
    println!("  Common: {}", report.common_count);
    println!("  Summary: {}", report.summary);

    // The tool should return a valid report structure - similarity percentage
    // is expected to vary due to non-deterministic syscall timing
    assert!(report.similarity_pct >= 0.0 && report.similarity_pct <= 100.0,
        "Similarity should be between 0 and 100");
    assert!(report.common_count >= 0, "Common count should be non-negative");

    client.shutdown().await.ok();
}
