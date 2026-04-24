//! Query tools tests — verify event querying functionality.

use chronos_sandbox::McpTestClient;

/// Stub test — requires chronos-mcp binary to exist.
/// Run with: cargo test -p chronos-sandbox -- --ignored
#[tokio::test]
#[ignore]
async fn test_query_events() {
    let mut client = McpTestClient::start().await.unwrap();
    let session_id = client.probe_start("test_add").await.unwrap();
    let events = client.probe_drain(&session_id).await.unwrap();
    assert!(!events.is_empty());
}
