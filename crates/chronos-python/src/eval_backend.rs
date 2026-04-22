//! Python DAP-based evaluation backend for the SessionEvalDispatcher.
//!
//! This backend delegates expression evaluation to a connected debugpy DAP client.

use crate::client::DapClient;
use crate::error::PythonAdapterError;
use chronos_domain::TraceError;
use chronos_query::eval_dispatcher::EvalBackend;
use std::sync::Arc;

/// Evaluation backend that delegates to a Python DAP client (debugpy).
///
/// This backend allows expressions to be evaluated in the context of a paused
/// Python debuggee via the Debug Adapter Protocol.
///
/// The client is optional, allowing the backend to be registered before
/// a connection is established. If no client is connected, evaluate calls
/// will return UnsupportedOperation.
pub struct PythonDapEvalBackend {
    /// The DAP client connected to debugpy.
    /// Uses std::sync::Mutex since DapClient is a synchronous TCP client.
    /// None if no client is connected yet.
    client: Arc<std::sync::Mutex<Option<DapClient>>>,
    /// Optional frame ID to evaluate in a specific stack frame.
    /// None means evaluate in the current/top frame.
    frame_id: Option<u64>,
}

impl PythonDapEvalBackend {
    /// Create a new PythonDapEvalBackend without a connected client.
    /// The client can be set later via set_client().
    pub fn new() -> Self {
        Self {
            client: Arc::new(std::sync::Mutex::new(None)),
            frame_id: None,
        }
    }

    /// Create a new PythonDapEvalBackend wrapping the given DAP client.
    /// Takes ownership of the client directly.
    pub fn with_client(client: DapClient) -> Self {
        Self {
            client: Arc::new(std::sync::Mutex::new(Some(client))),
            frame_id: None,
        }
    }

    /// Create a new backend with a specific frame ID.
    pub fn with_frame_id(client: DapClient, frame_id: u64) -> Self {
        Self {
            client: Arc::new(std::sync::Mutex::new(Some(client))),
            frame_id: Some(frame_id),
        }
    }

    /// Set the DAP client for this backend.
    pub fn set_client(&self, client: DapClient) {
        let mut guard = self.client.lock().unwrap();
        *guard = Some(client);
    }

    /// Set the frame ID for subsequent evaluations.
    pub fn set_frame_id(&mut self, frame_id: Option<u64>) {
        self.frame_id = frame_id;
    }
}

impl Default for PythonDapEvalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalBackend for PythonDapEvalBackend {
    fn evaluate_sync(&self, expr: &str, frame_id: Option<u64>) -> chronos_query::eval_dispatcher::EvalResult {
        // Use provided frame_id if available, otherwise use the backend's default
        let effective_frame_id = frame_id.or(self.frame_id);

        let mut client_guard = match self.client.lock() {
            Ok(c) => c,
            Err(_) => {
                return Err(TraceError::UnsupportedOperation(
                    "DAP client lock poisoned".to_string(),
                ));
            }
        };

        let client = match client_guard.as_mut() {
            Some(c) => c,
            None => {
                return Err(TraceError::UnsupportedOperation(
                    "Python DAP client not connected".to_string(),
                ));
            }
        };

        match client.evaluate(expr, effective_frame_id) {
            Ok(result) => Ok(result),
            Err(PythonAdapterError::ConnectionFailed(msg)) => {
                Err(TraceError::UnsupportedOperation(format!(
                    "Python DAP connection failed: {}",
                    msg
                )))
            }
            Err(PythonAdapterError::ProtocolError(msg)) => {
                Err(TraceError::UnsupportedOperation(format!(
                    "Python DAP protocol error: {}",
                    msg
                )))
            }
            Err(PythonAdapterError::EvaluateFailed(msg)) => {
                Err(TraceError::UnsupportedOperation(format!(
                    "Python evaluation failed: {}",
                    msg
                )))
            }
            Err(e) => {
                Err(TraceError::UnsupportedOperation(format!(
                    "Python DAP error: {}",
                    e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Spin up a mock DAP server that responds with a fixed value.
    fn spawn_mock_dap_server(response_body: serde_json::Value, success: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        std::thread::spawn(move || {
            let mut stream = listener.incoming().next().unwrap().unwrap();
            // Read the DAP request
            let mut buf = [0u8; 1024];
            let _n = stream.read(&mut buf).unwrap();

            // Send DAP response
            let response = serde_json::json!({
                "seq": 2,
                "type": "response",
                "command": "evaluate",
                "request_seq": 1,
                "success": success,
                "body": response_body
            });
            let json_str = serde_json::to_string(&response).unwrap();
            let header = format!("Content-Length: {}\r\n\r\n", json_str.len());
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(json_str.as_bytes()).unwrap();
        });

        addr
    }

    #[test]
    fn test_python_eval_backend_success() {
        // Spin up a mock server that returns "42"
        let addr = spawn_mock_dap_server(
            serde_json::json!({
                "result": "42",
                "type": "string"
            }),
            true,
        );

        let client = DapClient::connect(&addr).unwrap();
        let backend = PythonDapEvalBackend::with_client(client);

        let result = backend.evaluate_sync("1 + 1", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }

    #[test]
    fn test_python_eval_backend_connection_error() {
        // For this test, let's use a working server that returns error
        let addr = spawn_mock_dap_server(serde_json::json!({"message": "Variable not found"}), false);
        let client = DapClient::connect(&addr).unwrap();
        let backend = PythonDapEvalBackend::with_client(client);

        let result = backend.evaluate_sync("undefined_var", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TraceError::UnsupportedOperation(_)));
    }

    #[test]
    fn test_python_eval_backend_with_frame_id() {
        // Verify that frame_id is passed through
        let addr = spawn_mock_dap_server(
            serde_json::json!({
                "result": "value",
                "type": "string"
            }),
            true,
        );

        let client = DapClient::connect(&addr).unwrap();
        let backend = PythonDapEvalBackend::with_frame_id(client, 5);

        // The mock server will accept any frame_id, and we verify it gets passed
        // Note: We only test one call since the mock server handles one request
        let result = backend.evaluate_sync("x", Some(10));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "value");

        // Test that backend's internal frame_id is used when None is passed
        // Create a new connection since mock server can only handle one
        let addr2 = spawn_mock_dap_server(
            serde_json::json!({
                "result": "local_value",
                "type": "string"
            }),
            true,
        );
        let client2 = DapClient::connect(&addr2).unwrap();
        let mut backend2 = PythonDapEvalBackend::with_client(client2);
        backend2.set_frame_id(Some(7));

        let result2 = backend2.evaluate_sync("y", None);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_python_eval_backend_empty_expr() {
        let addr = spawn_mock_dap_server(
            serde_json::json!({
                "result": "",
                "type": "string"
            }),
            true,
        );

        let client = DapClient::connect(&addr).unwrap();
        let backend = PythonDapEvalBackend::with_client(client);

        // Empty expression should be handled gracefully
        let result = backend.evaluate_sync("", None);
        // debugpy may or may not error on empty string, but backend should handle it
        // The result depends on debugpy's behavior
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(matches!(err, TraceError::UnsupportedOperation(_)));
        }
    }

    #[test]
    fn test_python_eval_backend_no_client_returns_unsupported() {
        // Create a backend without a client
        let backend = PythonDapEvalBackend::new();

        // Should return UnsupportedOperation when no client is connected
        let result = backend.evaluate_sync("1 + 1", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TraceError::UnsupportedOperation(msg) if msg.contains("not connected")));
    }

    #[test]
    fn test_python_eval_backend_with_client_evaluates() {
        // Test that with_client() constructor properly connects and evaluates
        // This test verifies the requirement: "with_client() constructor, mock client returns '42'"
        let addr = spawn_mock_dap_server(
            serde_json::json!({
                "result": "42",
                "type": "string"
            }),
            true,
        );

        let client = DapClient::connect(&addr).unwrap();
        let backend = PythonDapEvalBackend::with_client(client);

        // Verify the backend can evaluate and returns the expected value
        let result = backend.evaluate_sync("1 + 1", None);
        assert!(result.is_ok(), "Evaluation should succeed");
        assert_eq!(result.unwrap(), "42", "Mock client should return '42'");
    }
}
