//! JSON-RPC client for MCP sandbox communication.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::time::timeout;

use super::error::McpSandboxError;
use super::process::{McpReader, McpWriter};

/// Global counter for RPC request IDs.
static RPC_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Internal RPC client combining writer and reader.
pub(crate) struct RpcClient {
    writer: McpWriter,
    reader: McpReader,
}

impl RpcClient {
    /// Create a new RPC client from stdio handles.
    pub fn new(writer: McpWriter, reader: McpReader) -> Self {
        Self { writer, reader }
    }

    /// Initialize the MCP connection by sending the initialize request.
    ///
    /// This must be called before any other method calls.
    /// The MCP server expects an initialize request before accepting tool calls.
    pub async fn initialize(&mut self) -> Result<(), McpSandboxError> {
        let id = RPC_ID_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "chronos-sandbox",
                    "version": "0.1.0"
                }
            }
        });

        let request_str = serde_json::to_string(&request)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;
        let mut line = request_str.clone();
        line.push('\n');

        self.writer
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;
        self.writer
            .flush()
            .await
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        tracing::debug!("Sent MCP initialize request");

        // Read the response to initialize request
        let response = timeout(Duration::from_secs(30), self.read_response())
            .await
            .map_err(|_| McpSandboxError::TimeoutError("initialize response".to_string(), Duration::from_secs(30)))??;

        tracing::debug!(response = ?response, "Received initialize response");

        // Read and discard the "initialized" notification from server
        // This is a JSON-RPC notification (no id) that the server sends after initialize
        let mut notification_line = String::new();
        match timeout(Duration::from_secs(5), self.reader.read_line(&mut notification_line)).await {
            Ok(Ok(_)) => {
                tracing::debug!(line = %notification_line.trim(), "Received MCP initialized notification");
            }
            Ok(Err(e)) => {
                tracing::warn!("Error reading initialized notification: {}", e);
            }
            Err(_) => {
                // Timeout waiting for notification - this is okay, some servers don't send it immediately
                tracing::debug!("Timeout waiting for initialized notification - continuing anyway");
            }
        }

        Ok(())
    }

    /// Send an RPC call and wait for response with default 30-second timeout.
    ///
    /// Note: For MCP tools, use `call_tool` instead.
    pub async fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpSandboxError> {
        self.call_with_timeout(method, params, Duration::from_secs(30)).await
    }

    /// Call an MCP tool using the tools/call protocol.
    ///
    /// This sends the tool call via the MCP tools/call method and returns
    /// the parsed result from the tool.
    ///
    /// The MCP tool response wraps the actual result in `result.content[0].text`
    /// as a JSON string, so we unwrap it here.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });
        let response = self.call_with_timeout("tools/call", params, Duration::from_secs(30)).await?;

        // The tools/call response has the form:
        // {"result": {"content": [{"type": "text", "text": "..."}], "isError": false}}
        // We need to extract the text and parse it as JSON
        let result_obj = response
            .get("result")
            .ok_or_else(|| McpSandboxError::RpcError("Missing result in tool response".to_string()))?;

        let content = result_obj
            .get("content")
            .ok_or_else(|| McpSandboxError::RpcError("Missing content in tool response".to_string()))?;

        let content_array = content
            .as_array()
            .ok_or_else(|| McpSandboxError::RpcError("content is not an array".to_string()))?;

        if content_array.is_empty() {
            return Err(McpSandboxError::RpcError("content array is empty".to_string()));
        }

        // Check if the response is an error
        if let Some(is_error) = result_obj.get("isError").and_then(|v| v.as_bool()) {
            if is_error {
                // Extract error message from content
                let first_content = &content_array[0];
                let text = first_content
                    .get("text")
                    .ok_or_else(|| McpSandboxError::RpcError("Missing text in error content".to_string()))?;
                let text_str = text
                    .as_str()
                    .ok_or_else(|| McpSandboxError::RpcError("text is not a string".to_string()))?;
                return Err(McpSandboxError::RpcError(text_str.to_string()));
            }
        }

        let first_content = &content_array[0];
        let text = first_content
            .get("text")
            .ok_or_else(|| McpSandboxError::RpcError("Missing text in content".to_string()))?;

        let text_str = text
            .as_str()
            .ok_or_else(|| McpSandboxError::RpcError("text is not a string".to_string()))?;

        // Parse the inner JSON string
        let inner_value: serde_json::Value = serde_json::from_str(text_str)
            .map_err(|e| McpSandboxError::RpcError(format!("Failed to parse tool result: {}", e)))?;

        Ok(inner_value)
    }

    /// Send an RPC call and wait for response with custom timeout.
    pub async fn call_with_timeout(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout_duration: Duration,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let id = RPC_ID_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        // Write the request
        let request_str = serde_json::to_string(&request)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;
        let mut line = request_str.clone();
        line.push('\n');

        self.writer
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;
        self.writer
            .flush()
            .await
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        tracing::debug!(method = method, id = id, "Sent RPC request");

        // Read the response with custom timeout
        let response = timeout(timeout_duration, self.read_response())
            .await
            .map_err(|_| McpSandboxError::TimeoutError(format!("method={}", method), timeout_duration))??;

        tracing::debug!(method = method, "Received RPC response");

        Ok(response)
    }

    /// Read a JSON-RPC response line from stdout.
    async fn read_response(&mut self) -> Result<serde_json::Value, McpSandboxError> {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        if line.is_empty() {
            return Err(McpSandboxError::UnexpectedOutput("Empty response".to_string()));
        }

        let response: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        // Check for error responses
        if let Some(error) = response.get("error") {
            return Err(McpSandboxError::RpcError(error.to_string()));
        }

        Ok(response)
    }
}

/// Wrapper for stdin that can send RPC calls.
pub struct McpWriterHandle(McpWriter);

impl McpWriterHandle {
    /// Create a new writer handle from stdin.
    pub fn new(writer: McpWriter) -> Self {
        Self(writer)
    }

    /// Get mutable reference to the underlying writer.
    pub fn writer_mut(&mut self) -> &mut McpWriter {
        &mut self.0
    }
}

/// Wrapper for stdout reader.
pub struct McpReaderHandle(McpReader);

impl McpReaderHandle {
    /// Create a new reader handle.
    pub fn new(reader: McpReader) -> Self {
        Self(reader)
    }

    /// Get mutable reference to the underlying reader.
    pub fn reader_mut(&mut self) -> &mut McpReader {
        &mut self.0
    }
}
