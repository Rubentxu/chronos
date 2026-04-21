//! Go debug target implementation.
//!
//! Uses Delve RPC on port 2345 for debugging Go programs.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Delve JSON-RPC client for communicating with Delve debugger.
#[derive(Debug)]
struct DelveClient {
    stream: TcpStream,
    next_id: u64,
}

impl DelveClient {
    /// Connect to Delve server at localhost:port.
    fn new(port: u16) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self { stream, next_id: 1 })
    }

    /// Send a JSON-RPC request and wait for response.
    fn send_rpc(&mut self, method: &str, params: serde_json::Value) -> std::io::Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({
            "id": id,
            "method": method,
            "params": params
        });

        let request_str = serde_json::to_string(&request).unwrap();
        let request_bytes = request_str.as_bytes();

        // Send Content-Length header + body
        let header = format!("Content-Length: {}\r\n\r\n", request_bytes.len());
        self.stream.write_all(header.as_bytes())?;
        self.stream.write_all(request_bytes)?;
        self.stream.flush()?;

        // Read response headers
        let mut header_buf = [0u8; 1];
        let mut headers = Vec::new();
        loop {
            self.stream.read_exact(&mut header_buf)?;
            if header_buf[0] == b'\r' {
                // Peek at next byte
                let mut peek_buf = [0u8; 1];
                self.stream.read_exact(&mut peek_buf)?;
                if header_buf[0] == b'\r' && peek_buf[0] == b'\n' {
                    // Got \r\n\r\n, check for Content-Length
                    let mut body_start = Vec::new();
                    // Read until \r\n\r\n
                    loop {
                        let mut buf = [0u8; 1];
                        self.stream.read_exact(&mut buf)?;
                        if buf[0] == b'\r' {
                            let mut peek = [0u8; 1];
                            self.stream.read_exact(&mut peek)?;
                            if peek[0] == b'\n' {
                                break;
                            }
                            body_start.push(b'\r');
                            body_start.push(peek[0]);
                        } else {
                            body_start.push(buf[0]);
                        }
                    }
                    // Parse Content-Length from headers
                    let headers_str = String::from_utf8_lossy(&headers);
                    let content_length = headers_str
                        .lines()
                        .find(|l| l.starts_with("Content-Length: "))
                        .and_then(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    // Read body
                    let mut body = vec![0u8; content_length];
                    self.stream.read_exact(&mut body)?;
                    let response: serde_json::Value = serde_json::from_slice(&body)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    return Ok(response);
                }
                headers.push(b'\r');
                headers.push(peek_buf[0]);
            } else if header_buf[0] == b'\n' {
                // Check if this is the end of headers
                let len = headers.len();
                if len >= 2 && headers[len-2] == b'\r' && headers[len-1] == b'\n' {
                    break;
                }
                headers.push(header_buf[0]);
            } else {
                headers.push(header_buf[0]);
            }
        }

        Err(std::io::Error::other("Malformed HTTP response"))
    }

    /// Attach to a process by PID.
    fn attach(&mut self, pid: u32) -> std::io::Result<()> {
        let response = self.send_rpc("RPCServer.AttachProcess", serde_json::json!({ "pid": pid }))?;
        if response.get("error").is_some() {
            let err = response["error"].as_str().unwrap_or("unknown error");
            return Err(std::io::Error::other(err));
        }
        Ok(())
    }

    /// Create a breakpoint at the given address.
    fn set_breakpoint(&mut self, address: u64) -> std::io::Result<u64> {
        let response = self.send_rpc(
            "RPCServer.CreateBreakpoint",
            serde_json::json!({
                "breakpoint": {
                    "addr": address
                }
            }),
        )?;
        if let Some(err) = response.get("error") {
            return Err(std::io::Error::other(err.to_string()));
        }
        // Return breakpoint ID
        let id = response["result"]["breakpoint"]["id"].as_u64().unwrap_or(0);
        Ok(id)
    }

    /// Continue execution and wait for a stop event.
    fn wait_stopped(&mut self) -> std::io::Result<BreakpointHit> {
        let response = self.send_rpc("RPCServer.Command", serde_json::json!({ "name": "continue" }))?;
        if let Some(err) = response.get("error") {
            return Err(std::io::Error::other(err.to_string()));
        }

        // Parse stopped event
        if let Some(result) = response.get("result") {
            if let Some(event) = result.get("event").and_then(|e| e.as_str()) {
                if event == "stopped" {
                    // Extract process/thread info from the response
                    let pid = result.get("process_info")
                        .and_then(|pi| pi.get("pid"))
                        .and_then(|p| p.as_u64())
                        .unwrap_or(0) as u32;
                    let tid = result.get("threads")
                        .and_then(|t| t.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|t| t.get("id"))
                        .and_then(|id| id.as_u64())
                        .unwrap_or(0) as u32;
                    let address = result.get("breakpoint_info")
                        .and_then(|bi| bi.get("breakpoints"))
                        .and_then(|bps| bps.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|bp| bp.get("addr"))
                        .and_then(|a| a.as_u64())
                        .unwrap_or(0);

                    return Ok(BreakpointHit { pid, tid, address });
                }
            }
        }

        Err(std::io::Error::other("Unexpected response"))
    }

    /// Resume execution.
    fn resume(&mut self) -> std::io::Result<()> {
        let response = self.send_rpc("RPCServer.Command", serde_json::json!({ "name": "continue" }))?;
        if let Some(err) = response.get("error") {
            return Err(std::io::Error::other(err.to_string()));
        }
        Ok(())
    }

    /// Detach from the debugged process.
    fn detach(&mut self, kill: bool) -> std::io::Result<()> {
        let response = self.send_rpc("RPCServer.Detach", serde_json::json!({ "kill": kill }))?;
        if let Some(err) = response.get("error") {
            return Err(std::io::Error::other(err.to_string()));
        }
        Ok(())
    }
}

/// Go debug target using Delve.
#[derive(Debug, Clone)]
pub struct GoTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
    port: u16,
    #[allow(dead_code)]
    client: Arc<RwLock<Option<DelveClient>>>,
}

impl Default for GoTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl GoTarget {
    /// Creates a new GoTarget with default Delve port (2345).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port: 2345,
            client: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a new GoTarget with a custom port.
    pub fn with_port(port: u16) -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port,
            client: Arc::new(RwLock::new(None)),
        }
    }
}

impl DebugTarget for GoTarget {
    fn attach(&self, pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }

        // Connect to Delve server
        let mut client = DelveClient::new(self.port)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to connect to Delve: {}", e)))?;

        client.attach(pid)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to attach: {}", e)))?;

        *self.attached.write().unwrap() = true;
        *self.pid.write().unwrap() = Some(pid);
        *self.client.write().unwrap() = Some(client);

        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with Delve headless
        let mut cmd = Command::new("dlv");
        cmd.arg("debug");
        cmd.arg("--accept-multiclient");
        cmd.arg("--listen").arg(format!(":{}", self.port));
        cmd.arg("--api-version=2");
        cmd.arg("--");
        cmd.args(args);

        // Set the program to debug
        cmd.arg(program);

        let child = cmd.spawn()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Failed to spawn dlv: {}", e)))?;

        // Wait for Delve to be ready (give it a moment to start)
        std::thread::sleep(Duration::from_millis(100));

        // Try to connect
        let client = DelveClient::new(self.port)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("Delve started but connection failed: {}", e)))?;

        *self.attached.write().unwrap() = true;
        *self.pid.write().unwrap() = Some(child.id());
        *self.client.write().unwrap() = Some(client);

        Ok(child.id())
    }

    fn is_attached(&self) -> bool {
        *self.attached.read().unwrap()
    }

    fn set_breakpoint(&self, address: u64) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.write().unwrap();
        if let Some(ref mut client) = *client_guard {
            client.set_breakpoint(address)
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("set_breakpoint failed: {}", e)))?;
        }
        Ok(())
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.write().unwrap();
        if let Some(ref mut client) = *client_guard {
            client.wait_stopped()
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("wait_stopped failed: {}", e)))
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no client connection".to_string()))
        }
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.write().unwrap();
        if let Some(ref mut client) = *client_guard {
            client.resume()
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("resume failed: {}", e)))
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no client connection".to_string()))
        }
    }

    fn detach(&self) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Ok(());
        }

        let mut client_guard = self.client.write().unwrap();
        if let Some(ref mut client) = *client_guard {
            // Detach without killing the process
            let _ = client.detach(false);
        }

        *self.attached.write().unwrap() = false;
        *self.pid.write().unwrap() = None;
        *client_guard = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_target_default_port() {
        let target = GoTarget::new();
        assert!(!target.is_attached());
        assert_eq!(target.port, 2345);
    }

    #[test]
    fn test_go_target_with_port() {
        let target = GoTarget::with_port(54321);
        assert_eq!(target.port, 54321);
    }

    #[test]
    fn test_go_target_attach_not_attached() {
        let target = GoTarget::new();
        // Trying to set breakpoint without attach should fail
        let result = target.set_breakpoint(0x400000);
        assert!(result.is_err());
    }

    #[test]
    fn test_go_target_wait_not_attached() {
        let target = GoTarget::new();
        let result = target.wait();
        assert!(result.is_err());
    }

    #[test]
    fn test_go_target_resume_not_attached() {
        let target = GoTarget::new();
        let result = target.resume();
        assert!(result.is_err());
    }

    #[test]
    fn test_go_target_detach_when_not_attached() {
        let target = GoTarget::new();
        // Detaching when not attached should be ok (no-op)
        let result = target.detach();
        assert!(result.is_ok());
        assert!(!target.is_attached());
    }

    #[test]
    #[ignore = "requires dlv binary and real Go process"]
    fn test_go_target_delve_integration() {
        // This test requires the dlv binary to be installed
        // and a real Go process to debug. Marked as ignored by default.
        let target = GoTarget::new();
        // Try to spawn a simple Go program under Delve
        let result = target.spawn("echo", &["hello"]);
        if result.is_ok() {
            let detach_result = target.detach();
            assert!(detach_result.is_ok());
        }
    }
}
