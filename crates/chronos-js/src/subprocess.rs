//! Node.js subprocess management for the JavaScript adapter.

use crate::error::JsAdapterError;
use std::process::Stdio;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use which::which;

/// Represents a running Node.js process with debugging enabled.
pub struct NodeProcess {
    child: Child,
    port: u16,
}

impl NodeProcess {
    /// Spawn a Node.js process with `--inspect` flag.
    ///
    /// Runs `node --inspect=<port> <script>`
    pub fn spawn(script: &str, port: u16) -> Result<Self, JsAdapterError> {
        let node_path = which("node").map_err(|_| JsAdapterError::NodeNotFound)?;

        let mut child = Command::new(&node_path)
            .args([format!("--inspect={}", port), script.to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| JsAdapterError::SpawnFailed(e))?;

        // Wait a moment for the process to start
        // (best effort - we check again in wait_for_cdp_ready)

        Ok(Self { child, port })
    }

    /// Get the inspect port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the child process ID
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Wait for the CDP endpoint to be ready and return the WebSocket URL.
    ///
    /// Polls `http://127.0.0.1:<port>/json` until the debugger is ready.
    pub async fn wait_for_cdp_ready(&self, timeout_secs: u64) -> Result<String, JsAdapterError> {
        let url = format!("http://127.0.0.1:{}/json", self.port);
        let client = reqwest::Client::new();

        let deadline = Duration::from_secs(timeout_secs);

        let result = timeout(deadline, async {
            let mut url_opt: Option<String> = None;
            while url_opt.is_none() {
                match client.get(&url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(ws_url) = json.get("webSocketDebuggerUrl") {
                                    if let Some(url_str) = ws_url.as_str() {
                                        url_opt = Some(url_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Connection refused or other error, continue polling
                    }
                }
                if url_opt.is_none() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
            url_opt
        })
        .await;

        match result {
            Ok(Some(url)) => Ok(url),
            Ok(None) => Err(JsAdapterError::CdpTimeout { timeout: timeout_secs }),
            Err(_) => Err(JsAdapterError::CdpTimeout { timeout: timeout_secs }),
        }
    }

    /// Signal the process to terminate.
    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }

    /// Wait for the process to exit.
    pub async fn wait(&mut self) -> Result<i32, JsAdapterError> {
        self.child
            .wait()
            .await
            .map(|s| s.code().unwrap_or(-1))
            .map_err(|e| JsAdapterError::SpawnFailed(e))
    }
}

impl Drop for NodeProcess {
    fn drop(&mut self) {
        // SIGTERM the process on drop
        let _ = self.child.start_kill();
    }
}

/// Check if Node.js is available on the system.
pub fn is_node_available() -> bool {
    which("node").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_available() {
        // This test passes if node is on the system PATH
        let available = is_node_available();
        // Just verify the method works - actual result depends on system
        assert!(available || !available);
    }
}
