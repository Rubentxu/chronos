//! Browser adapter error types.

use thiserror::Error;

/// Errors that can occur during browser-based WASM debugging.
#[derive(Debug, Clone, Error)]
pub enum BrowserError {
    #[error("Chrome not found: {0}")]
    ChromeNotFound(String),

    #[error("CDP connection failed: {0}")]
    CdpConnectionFailed(String),

    #[error("CDP command error: {method} - {message}")]
    CdpCommandError {
        method: String,
        message: String,
    },

    #[error("WASM module not found: {0}")]
    WasmModuleNotFound(String),

    #[error("No WASM modules detected")]
    NoWasmModules,

    #[error("Breakpoint error: {0}")]
    BreakpointError(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("IO error: {0}")]
    Io(String),
}

impl BrowserError {
    /// Returns true if this error indicates the Chrome process is not found.
    pub fn is_chrome_not_found(&self) -> bool {
        matches!(self, BrowserError::ChromeNotFound(_))
    }

    /// Returns true if this error is related to CDP communication.
    pub fn is_cdp_error(&self) -> bool {
        matches!(
            self,
            BrowserError::CdpConnectionFailed(_) | BrowserError::CdpCommandError { .. }
        )
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for BrowserError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        BrowserError::WebSocket(e.to_string())
    }
}

impl From<serde_json::Error> for BrowserError {
    fn from(e: serde_json::Error) -> Self {
        BrowserError::Json(e.to_string())
    }
}

impl From<std::io::Error> for BrowserError {
    fn from(e: std::io::Error) -> Self {
        BrowserError::Io(e.to_string())
    }
}
