//! End-to-end connectivity tests — verify MCP client connects to real chronos-mcp binary.

use chronos_sandbox::client::tools::McpTestClient;

#[tokio::test]
async fn test_mcp_server_starts_and_responds() {
    let client = McpTestClient::start().await;
    assert!(client.is_ok(), "Failed to start MCP server: {:?}", client.err());

    let mut session = client.unwrap();

    // Give the server a moment to initialize
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Try a simple call - list_sessions
    // The server should respond (it may return an error if the database isn't set up,
    // but we should get a valid response, not a connection failure)
    let result = session.list_sessions().await;
    // We expect a response (even if error due to database), not a connection failure
    // The error would be something like "Database error" rather than "connection closed"
    if let Err(e) = &result {
        let err_str = format!("{:?}", e);
        // If it's a database error, that's expected in test environment
        // Just verify we got a response, not a connection issue
        assert!(
            !err_str.contains("connection closed") && !err_str.contains("timeout"),
            "Connection lost: {}",
            err_str
        );
    }

    session.shutdown().await.ok();
}
