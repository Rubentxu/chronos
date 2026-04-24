//! Session comparison tests — verify compare_sessions functionality.

use chronos_sandbox::McpTestClient;

/// Integration test for compare_sessions.
///
/// This test verifies that two sessions running the same program produce
/// similar results with no divergences.
///
/// Run with: cargo test -p chronos-sandbox -- --ignored
#[tokio::test]
#[ignore]
async fn test_compare_sessions() {
    let mut client = McpTestClient::start().await.unwrap();

    // Start first session with test_add
    let session_a = client.probe_start("test_add").await.unwrap();

    // Give it time to collect events
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Drain events from session A
    let _events_a = client.probe_drain(&session_a).await.unwrap();

    // Start second session with test_add
    let session_b = client.probe_start("test_add").await.unwrap();

    // Give it time to collect events
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Drain events from session B
    let _events_b = client.probe_drain(&session_b).await.unwrap();

    // Compare — they should be similar (same program, same fixture)
    let report = client.compare_sessions(&session_a, &session_b).await.unwrap();

    // Verify the report indicates similarity
    assert!(!report.has_divergences(), "Sessions should not have divergences for same program");

    // Clean up
    client.probe_stop(&session_a).await.unwrap();
    client.probe_stop(&session_b).await.unwrap();
}
