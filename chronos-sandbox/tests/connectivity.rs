//! Connectivity tests — verify MCP client can be created.

use chronos_sandbox::McpTestClient;

/// Basic test — verifies MCP client can connect to chronos-mcp binary.
#[tokio::test]
async fn test_client_creation() {
    let client = McpTestClient::start().await;
    assert!(client.is_ok(), "Failed to start MCP server: {:?}", client.err());
}
