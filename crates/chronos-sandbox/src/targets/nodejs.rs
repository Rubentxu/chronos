//! Node.js debug target implementation.
//!
//! Uses Chrome DevTools Protocol (CDP) on port 9229 via WebSocket.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::{Arc, Mutex};

/// Node.js debug target using CDP over WebSocket.
#[derive(Debug, Clone)]
pub struct NodeJsTarget {
    attached: Arc<Mutex<bool>>,
    pid: Arc<Mutex<Option<u32>>>,
    port: u16,
    /// Tokio runtime for async CDP operations. Only Some when attached.
    /// Stored as Arc<Runtime> so it can be cloned and reused across CDP calls.
    runtime: Arc<Mutex<Option<Arc<tokio::runtime::Runtime>>>>,
    /// WebSocket URL for CDP connection.
    ws_url: String,
}

impl Default for NodeJsTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeJsTarget {
    /// Creates a new NodeJsTarget with default CDP port (9229).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port: 9229,
            runtime: Arc::new(Mutex::new(None)),
            ws_url: format!("ws://127.0.0.1:{}/json", 9229),
        }
    }

    /// Creates a new NodeJsTarget with a custom port.
    pub fn with_port(port: u16) -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port,
            runtime: Arc::new(Mutex::new(None)),
            ws_url: format!("ws://127.0.0.1:{}/json", port),
        }
    }

    /// Internal: connect to CDP WebSocket and enable debugger.
    /// Uses the runtime stored in `self.runtime` (via `handle.block_on()`) so the
    /// same runtime can be reused for subsequent CDP calls (set_breakpoint, wait, resume).
    fn connect_cdp(&self, rt: Arc<tokio::runtime::Runtime>) -> Result<(), SandboxError> {
        use tokio_tungstenite::{connect_async, tungstenite::Message};
        use futures_util::{SinkExt, StreamExt};

        let ws_url = self.ws_url.clone();
        let handle = rt.handle().clone();

        handle.block_on(async {
            // Connect to the CDP WebSocket endpoint
            let (ws_stream, _) = connect_async(&ws_url)
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("WebSocket connect failed: {}", e)))?;

            let (mut write, mut read) = ws_stream.split();

            // Enable the debugger
            let enable_msg = serde_json::json!({
                "id": 1,
                "method": "Debugger.enable",
                "params": {}
            });
            write.send(Message::Text(enable_msg.to_string()))
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to send: {}", e)))?;

            // Enable runtime
            let runtime_enable = serde_json::json!({
                "id": 2,
                "method": "Runtime.enable",
                "params": {}
            });
            write.send(Message::Text(runtime_enable.to_string()))
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to send: {}", e)))?;

            // Drain responses
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    // Check if we got responses to our enable requests
                                    if let Some(id) = json.get("id").and_then(|i| i.as_i64()) {
                                        if id == 1 || id == 2 {
                                            continue;
                                        }
                                    }
                                    // If it's an event, we can stop draining
                                    if json.get("method").is_some() {
                                        break;
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => break,
                            Some(Ok(Message::Binary(_))) | Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) | Some(Err(_)) => break,
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                        // Timeout after 2 seconds
                        break;
                    }
                }
            }

            Ok::<(), SandboxError>(())
        })?;

        Ok(())
    }
}

impl DebugTarget for NodeJsTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if *self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }

        // Create and store runtime BEFORE connecting - key invariant: after attach succeeds,
        // self.runtime must be Some so subsequent CDP calls (set_breakpoint, wait, resume) work
        let mut runtime_guard = self.runtime.write().unwrap();
        if runtime_guard.is_some() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to create runtime: {}", e)))?;
        let rt_arc = Arc::new(rt);
        // Clone for connect_cdp to use
        let rt_for_cdp = rt_arc.clone();
        *runtime_guard = Some(rt_arc);
        drop(runtime_guard);

        // Connect to CDP WebSocket using the stored runtime
        if let Err(e) = self.connect_cdp(rt_for_cdp) {
            // Clean up: remove the runtime on failure so invariant holds
            *self.runtime.write().unwrap() = None;
            return Err(e);
        }

        *self.attached.lock().unwrap() = true;
        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with --inspect flag
        let mut cmd = Command::new("node");
        cmd.arg(format!("--inspect={}", self.port));
        cmd.arg(program);
        cmd.args(args);

        let child = cmd.spawn().map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;
        let pid = child.id();

        // Create and store runtime BEFORE connecting - key invariant: after spawn succeeds,
        // self.runtime must be Some so subsequent CDP calls (set_breakpoint, wait, resume) work
        let mut runtime_guard = self.runtime.write().unwrap();
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to create runtime: {}", e)))?;
        let rt_arc = Arc::new(rt);
        let rt_for_cdp = rt_arc.clone();
        *runtime_guard = Some(rt_arc);
        drop(runtime_guard);

        // Wait briefly for Node to start the inspector
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Connect to CDP using the stored runtime
        if let Err(e) = self.connect_cdp(rt_for_cdp) {
            // Log but don't fail - the process might still be running
            tracing::warn!("CDP connection failed (may still work): {}", e);
        }

        *self.attached.lock().unwrap() = true;
        *self.pid.lock().unwrap() = Some(pid);

        Ok(pid)
    }

    fn is_attached(&self) -> bool {
        *self.attached.lock().unwrap()
    }

    fn set_breakpoint(&self, address: u64) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        // CDP WebSocket requires async context - use the stored runtime
        let rt_guard = self.runtime.lock().unwrap();
        if rt_guard.is_none() {
            return Err(SandboxError::DebugTargetConnectFailed(
                "CDP runtime not available".to_string(),
            ));
        }
        let rt_arc = rt_guard.as_ref().unwrap().clone();
        drop(rt_guard);

        let ws_url = self.ws_url.clone();
        let handle = rt_arc.handle().clone();

        handle.block_on(async move {
            use tokio_tungstenite::{connect_async, tungstenite::Message};
            use futures_util::{SinkExt, StreamExt};

            let (ws_stream, _) = connect_async(&ws_url)
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("WebSocket connect failed: {}", e)))?;

            let (mut write, _read) = ws_stream.split();

            // Set breakpoint using line number (address is treated as line)
            let bp_msg = serde_json::json!({
                "id": 100,
                "method": "Debugger.setBreakpoint",
                "params": {
                    "location": {
                        "scriptId": "0",
                        "lineNumber": address
                    }
                }
            });

            write.send(Message::Text(bp_msg.to_string()))
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to set breakpoint: {}", e)))?;

            Ok::<(), SandboxError>(())
        })
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        // Wait for CDP debugger paused event - use the stored runtime
        let rt_guard = self.runtime.lock().unwrap();
        if rt_guard.is_none() {
            return Err(SandboxError::DebugTargetConnectFailed(
                "CDP runtime not available".to_string(),
            ));
        }
        let rt_arc = rt_guard.as_ref().unwrap().clone();
        drop(rt_guard);

        let ws_url = self.ws_url.clone();
        let handle = rt_arc.handle().clone();

        handle.block_on(async move {
            use tokio_tungstenite::{connect_async, tungstenite::Message};
            use futures_util::StreamExt;

            let (ws_stream, _) = connect_async(&ws_url)
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("WebSocket connect failed: {}", e)))?;

            let (_write, mut read) = ws_stream.split();

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    // Check if this is a Debugger.paused event
                                    if json.get("method") == Some(&serde_json::Value::String("Debugger.paused".to_string())) {
                                        let pid = *self.pid.lock().unwrap();
                                        let params = json.get("params").unwrap_or(&serde_json::Value::Null);
                                        let _reason = params.get("reason").and_then(|r| r.as_str()).unwrap_or("breakpoint");
                                        let line = params.get("callFrames")
                                            .and_then(|cf| cf.as_array())
                                            .and_then(|frames| frames.first())
                                            .and_then(|f| f.get("lineNumber"))
                                            .and_then(|ln| ln.as_u64())
                                            .unwrap_or(0);

                                        return Ok(BreakpointHit {
                                            pid: pid.unwrap_or(0),
                                            tid: 0,
                                            address: line,
                                        });
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                return Err(SandboxError::DebugTargetConnectFailed("Connection closed".to_string()));
                            }
                            Some(Ok(Message::Binary(_))) | Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) | Some(Err(_)) => {
                                continue;
                            }
                            None => {
                                return Err(SandboxError::DebugTargetConnectFailed("Connection closed".to_string()));
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                        return Err(SandboxError::DebugTargetConnectFailed("Timeout waiting for breakpoint".to_string()));
                    }
                }
            }
        })
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let rt_guard = self.runtime.lock().unwrap();
        if rt_guard.is_none() {
            return Err(SandboxError::DebugTargetConnectFailed(
                "CDP runtime not available".to_string(),
            ));
        }
        let rt_arc = rt_guard.as_ref().unwrap().clone();
        drop(rt_guard);

        let ws_url = self.ws_url.clone();
        let handle = rt_arc.handle().clone();

        handle.block_on(async move {
            use tokio_tungstenite::{connect_async, tungstenite::Message};
            use futures_util::{SinkExt, StreamExt};

            let (ws_stream, _) = connect_async(&ws_url)
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("WebSocket connect failed: {}", e)))?;

            let (mut write, _read) = ws_stream.split();

            let resume_msg = serde_json::json!({
                "id": 200,
                "method": "Debugger.resume",
                "params": {}
            });

            write.send(Message::Text(resume_msg.to_string()))
                .await
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to resume: {}", e)))?;

            Ok::<(), SandboxError>(())
        })
    }

    fn detach(&self) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Ok(());
        }

        // Drop the runtime if any
        *self.runtime.lock().unwrap() = None;
        *self.attached.lock().unwrap() = false;
        *self.pid.lock().unwrap() = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nodejs_target_default_port() {
        let target = NodeJsTarget::new();
        assert!(!target.is_attached());
        assert_eq!(target.port, 9229);
    }

    #[test]
    fn test_nodejs_target_with_port() {
        let target = NodeJsTarget::with_port(12345);
        assert_eq!(target.port, 12345);
    }

    #[test]
    fn test_nodejs_target_attach_not_attached() {
        let target = NodeJsTarget::new();
        let result = target.set_breakpoint(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_nodejs_target_wait_not_attached() {
        let target = NodeJsTarget::new();
        let result = target.wait();
        assert!(result.is_err());
    }

    #[test]
    fn test_nodejs_target_resume_not_attached() {
        let target = NodeJsTarget::new();
        let result = target.resume();
        assert!(result.is_err());
    }

    #[test]
    fn test_nodejs_target_detach_when_not_attached() {
        let target = NodeJsTarget::new();
        let result = target.detach();
        assert!(result.is_ok());
        assert!(!target.is_attached());
    }

    #[test]
    #[ignore = "requires Node.js and a real process with --inspect"]
    fn test_nodejs_target_cdp_integration() {
        // This test requires Node.js to be installed
        // and a real Node.js process to debug. Marked as ignored by default.
        let target = NodeJsTarget::new();
        let result = target.spawn("echo", &["hello"]);
        if result.is_ok() {
            let detach_result = target.detach();
            assert!(detach_result.is_ok());
        }
    }
}
