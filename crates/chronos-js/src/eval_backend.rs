//! JavaScript CDP-based evaluation backend.
//!
//! This backend delegates expression evaluation to a connected Chrome DevTools Protocol client.
//!
//! NOTE: The EvalBackend trait implementation has been removed as part of the
//! SessionEvalDispatcher removal. This struct is retained for potential future
//! use with direct expression evaluation.

use crate::cdp_client::CdpClient;
use std::sync::Arc;

/// Evaluation backend that delegates to a JavaScript CDP client (Node.js inspector).
///
/// This backend allows expressions to be evaluated in the context of a paused
/// JavaScript debuggee via the Chrome DevTools Protocol.
///
/// The client is optional, allowing the backend to be registered before
/// a connection is established.
#[allow(dead_code)]
pub struct JsCdpEvalBackend {
    /// The CDP client connected to Node.js inspector.
    /// Uses tokio::sync::Mutex since CdpClient is an async WebSocket client.
    /// None if no client is connected yet.
    client: Arc<tokio::sync::Mutex<Option<CdpClient>>>,
    /// Optional context ID to evaluate in a specific execution context.
    /// None means evaluate in the default context.
    context_id: Option<u64>,
}

impl JsCdpEvalBackend {
    /// Create a new JsCdpEvalBackend without a connected client.
    /// The client can be set later via set_client().
    pub fn new() -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(None)),
            context_id: None,
        }
    }

    /// Create a new JsCdpEvalBackend wrapping the given CDP client.
    /// Since CdpClient::connect is async, this is typically called after
    /// the connection is established in an async context.
    pub fn with_client(client: CdpClient) -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(Some(client))),
            context_id: None,
        }
    }

    /// Create a new backend with a specific context ID.
    pub fn with_context_id(client: CdpClient, context_id: u64) -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(Some(client))),
            context_id: Some(context_id),
        }
    }

    /// Set the context ID for subsequent evaluations.
    pub fn set_context_id(&mut self, context_id: Option<u64>) {
        self.context_id = context_id;
    }
}

impl Default for JsCdpEvalBackend {
    fn default() -> Self {
        Self::new()
    }
}
