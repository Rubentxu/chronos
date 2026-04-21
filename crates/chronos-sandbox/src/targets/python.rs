//! Python debug target implementation.
//!
//! Uses debugpy DAP (Debug Adapter Protocol) on port 5678.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use chronos_python::DapClient;
use std::sync::{Arc, Mutex};

/// Python debug target using debugpy DAP protocol.
#[derive(Clone)]
pub struct PythonTarget {
    attached: Arc<Mutex<bool>>,
    pid: Arc<Mutex<Option<u32>>>,
    port: u16,
    client: Arc<Mutex<Option<DapClient>>>,
}

impl std::fmt::Debug for PythonTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonTarget")
            .field("attached", &*self.attached.lock().unwrap())
            .field("pid", &*self.pid.lock().unwrap())
            .field("port", &self.port)
            .finish()
    }
}

impl Default for PythonTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonTarget {
    /// Creates a new PythonTarget with default debugpy port (5678).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port: 5678,
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates a new PythonTarget with a custom port.
    pub fn with_port(port: u16) -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port,
            client: Arc::new(Mutex::new(None)),
        }
    }
}

impl DebugTarget for PythonTarget {
    fn attach(&self, pid: u32) -> Result<(), SandboxError> {
        if *self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }

        // Connect to debugpy DAP server
        let addr = format!("127.0.0.1:{}", self.port);
        let mut client = DapClient::connect(&addr)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to connect to debugpy: {}", e)))?;

        // Initialize DAP session with the target PID
        client.initialize(pid)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to initialize DAP: {}", e)))?;

        *self.attached.lock().unwrap() = true;
        *self.pid.lock().unwrap() = Some(pid);
        *self.client.lock().unwrap() = Some(client);

        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with debugpy
        let mut cmd = Command::new("python");
        cmd.arg("-m");
        cmd.arg("debugpy");
        cmd.arg("--listen").arg(format!("127.0.0.1:{}", self.port));
        cmd.arg(program);
        cmd.args(args);

        let child = cmd.spawn().map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;
        let pid = child.id();

        // Wait briefly for debugpy to start
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Connect to the debugpy server
        let addr = format!("127.0.0.1:{}", self.port);
        let mut client = DapClient::connect(&addr)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to connect to debugpy: {}", e)))?;

        // Initialize without attaching to a specific PID (spawn mode)
        let _ = client.send_request("initialize", serde_json::json!({
            "adapterID": "chronos",
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "pathFormat": "path",
        }));

        let _ = client.send_request("attach", serde_json::json!({
            "waitOnExit": false,
        }));

        let _ = client.send_request("configurationDone", serde_json::json!({}));

        *self.attached.lock().unwrap() = true;
        *self.pid.lock().unwrap() = Some(pid);
        *self.client.lock().unwrap() = Some(client);

        Ok(pid)
    }

    fn is_attached(&self) -> bool {
        *self.attached.lock().unwrap()
    }

    fn set_breakpoint(&self, address: u64) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            // address is treated as a line number in DAP
            let response = client.send_request(
                "setBreakpoints",
                serde_json::json!({
                    "source": {
                        "name": "main.py",
                        "path": "main.py"
                    },
                    "lines": [address],
                    "breakpoints": [{
                        "line": address
                    }]
                }),
            ).map_err(|e| SandboxError::DebugTargetConnectFailed(format!("setBreakpoints failed: {}", e)))?;

            // Check if breakpoint was verified
            let breakpoints = response.get("breakpoints").and_then(|b| b.as_array());
            if let Some(bps) = breakpoints {
                for bp in bps {
                    if bp.get("verified").and_then(|v| v.as_bool()) != Some(true) {
                        return Err(SandboxError::DebugTargetConnectFailed(
                            "breakpoint not verified".to_string(),
                        ));
                    }
                }
            }
            Ok(())
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no client connection".to_string()))
        }
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            // Wait for stopped event
            loop {
                let event = client.next_event()
                    .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("next_event failed: {}", e)))?;

                match event {
                    Some(evt) if evt.event == "stopped" => {
                        let pid = *self.pid.lock().unwrap();
                        let tid = evt.body.get("threadId").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                        let _reason = evt.body.get("reason").and_then(|r| r.as_str()).unwrap_or("");
                        let line = evt.body.get("line").and_then(|l| l.as_u64()).unwrap_or(0);

                        return Ok(BreakpointHit {
                            pid: pid.unwrap_or(0),
                            tid,
                            address: line,
                        });
                    }
                    Some(_) => {
                        // Other events, continue waiting
                        continue;
                    }
                    None => {
                        return Err(SandboxError::DebugTargetConnectFailed(
                            "debugpy session terminated".to_string(),
                        ));
                    }
                }
            }
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no client connection".to_string()))
        }
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            let _ = client.send_request("continue", serde_json::json!({}))
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("continue failed: {}", e)))?;
            Ok(())
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no client connection".to_string()))
        }
    }

    fn detach(&self) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Ok(());
        }

        // Drop the client (closes connection)
        *self.client.lock().unwrap() = None;
        *self.attached.lock().unwrap() = false;
        *self.pid.lock().unwrap() = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_target_default_port() {
        let target = PythonTarget::new();
        assert!(!target.is_attached());
        assert_eq!(target.port, 5678);
    }

    #[test]
    fn test_python_target_with_port() {
        let target = PythonTarget::with_port(12345);
        assert_eq!(target.port, 12345);
    }

    #[test]
    fn test_python_target_attach_not_attached() {
        let target = PythonTarget::new();
        // Trying to set breakpoint without attach should fail
        let result = target.set_breakpoint(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_python_target_wait_not_attached() {
        let target = PythonTarget::new();
        let result = target.wait();
        assert!(result.is_err());
    }

    #[test]
    fn test_python_target_resume_not_attached() {
        let target = PythonTarget::new();
        let result = target.resume();
        assert!(result.is_err());
    }

    #[test]
    fn test_python_target_detach_when_not_attached() {
        let target = PythonTarget::new();
        // Detaching when not attached should be ok (no-op)
        let result = target.detach();
        assert!(result.is_ok());
        assert!(!target.is_attached());
    }

    #[test]
    #[ignore = "requires debugpy and a real Python process"]
    fn test_python_target_debugpy_integration() {
        // This test requires debugpy to be installed
        // and a real Python process to debug. Marked as ignored by default.
        let target = PythonTarget::new();
        // Try to spawn a simple Python program under debugpy
        let result = target.spawn("echo", &["hello"]);
        if result.is_ok() {
            let detach_result = target.detach();
            assert!(detach_result.is_ok());
        }
    }
}
