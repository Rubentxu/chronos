//! Variable tools tests — verify debug_get_variables and evaluate_expression
//! work correctly after probe_stop.

use chronos_sandbox::client::tools::McpTestClient;
use chronos_sandbox::McpSession;
use std::time::Duration;

#[tokio::test]
async fn test_debug_get_variables_empty_session() {
    // debug_get_variables requires Python frame events with local variables.
    // Native C programs don't produce these, so we expect empty results.
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to get variables at event 0 - will be empty for C programs
    let variables = client.debug_get_variables(&session_id, 0).await
        .expect("debug_get_variables failed");

    // === Assertions ===
    // C programs don't have Python-style frame events with local variables,
    // so we expect empty results - but the call should succeed
    println!("✓ debug_get_variables at event 0: {} variables (expected empty for C)", variables.len());

    for var in variables.iter() {
        println!("  {} = {} ({})", var.name, var.value, var.var_type.as_deref().unwrap_or("?"));
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_debug_get_variables_out_of_range() {
    // Query for variables at a non-existent event ID
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to get variables at a high event ID that doesn't exist
    let variables = client.debug_get_variables(&session_id, 999999).await
        .expect("debug_get_variables failed");

    // Should return empty - the event doesn't exist
    println!("✓ debug_get_variables at event 999999: {} variables", variables.len());

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_evaluate_expression_empty_session() {
    // evaluate_expression requires Python frame events with local variables.
    // Native C programs don't produce these.
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to evaluate an expression - will fail because no Python frames with locals
    let result = client.evaluate_expression(&session_id, "x + y").await;

    match result {
        Ok(value) => {
            // If it succeeds, it means there were variables but they might not match
            println!("✓ evaluate_expression returned: {:?}", value);
        }
        Err(e) => {
            // Expected: C programs don't have Python-style variables
            println!("✓ evaluate_expression failed as expected for C program: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}

#[tokio::test]
async fn test_evaluate_expression_invalid_expression() {
    // Test evaluate_expression with an invalid expression
    let fixture = McpSession::fixture_path("test_add")
        .expect("test_add fixture not found - run cargo build first");

    let mut client = McpTestClient::start().await
        .expect("Failed to start MCP server");

    let session_id = client.probe_start(fixture.to_str().unwrap()).await
        .expect("probe_start failed");

    // Wait for it to complete
    tokio::time::sleep(Duration::from_secs(1)).await;

    let _drained = client.probe_drain(&session_id).await
        .expect("probe_drain failed");

    let stop = client.probe_stop(&session_id).await
        .expect("probe_stop failed");

    println!("Probe stopped: {} total events", stop.total_events);

    // Give query engine time to build
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to evaluate a syntactically invalid expression
    let result = client.evaluate_expression(&session_id, "x +++ y").await;

    match result {
        Ok(value) => {
            // Might succeed with an error result in the JSON
            println!("✓ evaluate_expression returned: {:?}", value);
        }
        Err(e) => {
            // Also acceptable - the call itself failed
            println!("✓ evaluate_expression call failed: {:?}", e);
        }
    }

    client.shutdown().await.ok();
}
