//! Python DAP-based evaluation backend.
//!
//! This backend delegates expression evaluation to a connected debugpy DAP client.
//!
//! NOTE: The EvalBackend trait implementation has been removed as part of the
//! SessionEvalDispatcher removal. This struct is retained for potential future
//! use with direct expression evaluation.

use crate::client::DapClient;
use std::sync::Arc;

/// Evaluation backend that delegates to a Python DAP client (debugpy).
///
/// This backend allows expressions to be evaluated in the context of a paused
/// Python debuggee via the Debug Adapter Protocol.
///
/// The client is optional, allowing the backend to be registered before
/// a connection is established.
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
