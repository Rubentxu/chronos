//! Error types for the MCP sandbox client.

use std::fmt;
use std::time::Duration;

/// Errors that can occur when interacting with the MCP sandbox.
#[derive(Debug)]
pub enum McpSandboxError {
    /// Failed to spawn the MCP server process.
    SpawnFailed(String),
    /// JSON-RPC protocol error.
    RpcError(String),
    /// Operation timed out after the specified duration.
    TimeoutError(String, Duration),
    /// The MCP server process crashed.
    ServerCrashed(String),
    /// Received unexpected output from the server.
    UnexpectedOutput(String),
    /// Retry exhausted after multiple attempts.
    RetryExhausted(String),
    /// Server health check failed.
    HealthCheckFailed(String),
}

impl fmt::Display for McpSandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpSandboxError::SpawnFailed(msg) => write!(f, "Failed to spawn MCP server: {}", msg),
            McpSandboxError::RpcError(msg) => write!(f, "RPC error: {}", msg),
            McpSandboxError::TimeoutError(msg, dur) => write!(f, "Operation timed out after {:?}: {}", dur, msg),
            McpSandboxError::ServerCrashed(msg) => write!(f, "Server crashed: {}", msg),
            McpSandboxError::UnexpectedOutput(msg) => write!(f, "Unexpected output: {}", msg),
            McpSandboxError::RetryExhausted(msg) => write!(f, "Retry exhausted: {}", msg),
            McpSandboxError::HealthCheckFailed(msg) => write!(f, "Health check failed: {}", msg),
        }
    }
}

impl std::error::Error for McpSandboxError {}

impl From<std::io::Error> for McpSandboxError {
    fn from(err: std::io::Error) -> Self {
        McpSandboxError::SpawnFailed(err.to_string())
    }
}
