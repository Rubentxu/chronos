//! Error types for the JavaScript CDP adapter.

#[derive(Debug, thiserror::Error)]
pub enum JsAdapterError {
    #[error("Node.js not found in PATH")]
    NodeNotFound,

    #[error("Failed to spawn Node.js: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Node.js process error: {0}")]
    ProcessError(String),

    #[error("CDP endpoint not ready after {timeout}s")]
    CdpTimeout { timeout: u64 },

    #[error("WebSocket connection failed: {0}")]
    WebSocketFailed(String),

    #[error("CDP command failed: {method} - {error}")]
    CdpCommandFailed { method: String, error: String },

    #[error("CDP protocol error: {0}")]
    CdpProtocol(String),

    #[error("HTTP request failed: {0}")]
    HttpFailed(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
