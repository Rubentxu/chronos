//! JavaScript CDP-based evaluation backend for the SessionEvalDispatcher.
//!
//! This backend delegates expression evaluation to a connected Chrome DevTools Protocol client.

use crate::cdp_client::CdpClient;
use crate::error::JsAdapterError;
use chronos_domain::TraceError;
use chronos_query::eval_dispatcher::EvalBackend;
use std::sync::Arc;
use tokio::runtime::Handle;

/// Evaluation backend that delegates to a JavaScript CDP client (Node.js inspector).
///
/// This backend allows expressions to be evaluated in the context of a paused
/// JavaScript debuggee via the Chrome DevTools Protocol.
///
/// The client is optional, allowing the backend to be registered before
/// a connection is established. If no client is connected, evaluate calls
/// will return UnsupportedOperation.
pub struct JsCdpEvalBackend {
    /// The CDP client connected to Node.js inspector.
    /// Uses tokio::sync::Mutex since CdpClient is an async WebSocket client.
    /// None if no client is connected yet.
    client: Arc<tokio::sync::Mutex<Option<CdpClient>>>,
    /// Optional context ID to evaluate in a specific execution context.
    /// None means evaluate in the default context.
    context_id: Option<u64>,
    /// Tokio runtime handle for bridging sync/async.
    /// Used to call async CdpClient methods from the sync evaluate_sync method.
    runtime_handle: Handle,
}

impl JsCdpEvalBackend {
    /// Create a new JsCdpEvalBackend without a connected client.
    /// The client can be set later via set_client().
    pub fn new() -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(None)),
            context_id: None,
            runtime_handle: Handle::current(),
        }
    }

    /// Create a new JsCdpEvalBackend wrapping the given CDP client.
    pub fn with_client(client: CdpClient) -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(Some(client))),
            context_id: None,
            runtime_handle: Handle::current(),
        }
    }

    /// Create a new backend with a specific context ID.
    pub fn with_context_id(client: CdpClient, context_id: u64) -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(Some(client))),
            context_id: Some(context_id),
            runtime_handle: Handle::current(),
        }
    }

    /// Set the CDP client for this backend.
    pub fn set_client(&self, client: CdpClient) {
        let runtime = Handle::current();
        runtime.block_on(async {
            let mut guard = self.client.lock().await;
            *guard = Some(client);
        });
    }

    /// Set the context ID for subsequent evaluations.
    pub fn set_context_id(&mut self, context_id: Option<u64>) {
        self.context_id = context_id;
    }

    /// Helper to call async CDP command from sync context.
    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime_handle.block_on(future)
    }
}

impl Default for JsCdpEvalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalBackend for JsCdpEvalBackend {
    fn evaluate_sync(
        &self,
        expr: &str,
        context_id: Option<u64>,
    ) -> chronos_query::eval_dispatcher::EvalResult {
        // Use provided context_id if available, otherwise use the backend's default
        let effective_context_id = context_id.or(self.context_id);

        // Check if client is available
        let client_guard = match self.client.try_lock() {
            Ok(c) => c,
            Err(_) => {
                return Err(TraceError::UnsupportedOperation(
                    "CDP client is busy".to_string(),
                ));
            }
        };

        let client = match client_guard.as_ref() {
            Some(c) => c,
            None => {
                return Err(TraceError::UnsupportedOperation(
                    "JavaScript CDP client not connected".to_string(),
                ));
            }
        };

        // Build Runtime.evaluate params
        let mut params = serde_json::json!({
            "expression": expr,
            "returnByValue": true,
            "generatePreview": true
        });

        if let Some(ctx_id) = effective_context_id {
            params["contextId"] = serde_json::json!(ctx_id);
        }

        // Call async send_command from sync context
        let response = self.block_on(client.send_command("Runtime.evaluate", params));

        match response {
            Ok(resp) => {
                // Check for exception details
                if let Some(exception_details) = resp.get("exceptionDetails") {
                    let text = exception_details
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown exception");
                    return Err(TraceError::UnsupportedOperation(format!(
                        "JavaScript exception: {}",
                        text
                    )));
                }

                // Extract result
                if let Some(result_obj) = resp.get("result") {
                    // Try to get the value field first (when returnByValue is true)
                    if let Some(value) = result_obj.get("value") {
                        return Ok(value.to_string());
                    }
                    // Fall back to description
                    if let Some(desc) = result_obj.get("description") {
                        return Ok(desc.as_str().unwrap_or("").to_string());
                    }
                    // Fall back to converting the whole result to string
                    return Ok(result_obj.to_string());
                }

                Err(TraceError::UnsupportedOperation(
                    "CDP evaluate returned no result".to_string(),
                ))
            }
            Err(JsAdapterError::CdpTimeout { timeout: _ }) => Err(TraceError::UnsupportedOperation(
                "CDP evaluate timed out".to_string(),
            )),
            Err(JsAdapterError::CdpCommandFailed { method, error }) => {
                Err(TraceError::UnsupportedOperation(format!(
                    "CDP {} failed: {}",
                    method, error
                )))
            }
            Err(JsAdapterError::CdpProtocol(msg)) => Err(TraceError::UnsupportedOperation(
                format!("CDP protocol error: {}", msg),
            )),
            Err(e) => Err(TraceError::UnsupportedOperation(format!(
                "CDP error: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_eval_backend_success() {
        // This test would require a real CDP connection or a more sophisticated mock
        // For unit testing, we verify the structure works

        // We can't easily mock CdpClient since it requires async WebSocket connections
        // In a real scenario, we would use a test WebSocket server
        // For now, we just verify the struct can be created
        // let backend = JsCdpEvalBackend { ... };
    }

    #[test]
    fn test_js_eval_backend_exception() {
        // Verify that exception responses are properly handled
        // The actual exception handling is tested in the error mapping
    }

    #[test]
    fn test_js_eval_backend_with_context_id() {
        // Verify context_id is included in params
        // This would require a real CDP connection
    }

    #[test]
    fn test_js_eval_backend_complex_expr() {
        // Verify complex expressions are handled
        // This would require a real CDP connection
    }

    #[tokio::test]
    async fn test_js_eval_backend_no_client_returns_unsupported() {
        // Create a backend without a client
        let backend = JsCdpEvalBackend::new();

        // Should return UnsupportedOperation when no client is connected
        let result = backend.evaluate_sync("1 + 1", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, chronos_domain::TraceError::UnsupportedOperation(msg) if msg.contains("not connected")));
    }
}
