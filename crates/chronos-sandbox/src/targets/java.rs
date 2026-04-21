//! Java debug target implementation.
//!
//! Uses JDWP (Java Debug Wire Protocol) on port 5005.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// JDWP packet types
const JDWP_PACKET_TYPE_CMD: u8 = 0x0;
const JDWP_PACKET_TYPE_REPLY: u8 = 0x80;

/// JDWP command sets
mod cmd_set {
    pub const VIRTUAL_MACHINE: u8 = 1;
    pub const EVENT_REQUEST: u8 = 15;
}

/// JDWP commands
mod cmd {
    pub mod virtual_machine {
        pub const VERSION: u8 = 1;
        pub const RESUME: u8 = 9;
    }
    pub mod event_request {
        pub const SET: u8 = 1;
    }
}

/// JDWP event kinds
mod event_kind {
    pub const BREAKPOINT: u8 = 2;
    pub const METHOD_ENTRY: u8 = 3;
}

/// JDWP suspend policy
mod suspend_policy {
    #[allow(dead_code)]
    pub const NONE: u8 = 0;
    #[allow(dead_code)]
    pub const THREAD: u8 = 1;
    pub const ALL: u8 = 2;
}

/// JDWP debug client for communicating with JVM over JDWP.
#[derive(Debug)]
struct JdwpClient {
    stream: TcpStream,
    next_id: u32,
}

impl JdwpClient {
    /// Connect to JDWP server and perform handshake.
    fn new(port: u16) -> std::io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), std::time::Duration::from_secs(5))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;

        let mut client = Self { stream, next_id: 1 };

        // Perform JDWP handshake
        client.handshake()?;

        Ok(client)
    }

    /// Perform JDWP handshake: send "JDWP-Handshake", expect it back.
    fn handshake(&mut self) -> std::io::Result<()> {
        const JDWP_HANDSHAKE: &[u8; 14] = b"JDWP-Handshake";

        self.stream.write_all(JDWP_HANDSHAKE)?;

        let mut response = [0u8; 14];
        self.stream.read_exact(&mut response)?;

        if &response == JDWP_HANDSHAKE {
            Ok(())
        } else {
            Err(std::io::Error::other("JDWP handshake failed: unexpected response"))
        }
    }

    /// Read a u32 in big-endian byte order.
    fn read_u32(&mut self) -> std::io::Result<u32> {
        let mut buf = [0u8; 4];
        self.stream.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    /// Write a u32 in big-endian byte order.
    fn write_u32(&mut self, v: u32) -> std::io::Result<()> {
        self.stream.write_all(&v.to_be_bytes())
    }

    /// Send a JDWP command packet and read the reply.
    fn send_command(&mut self, cmd_set: u8, cmd: u8, data: &[u8]) -> std::io::Result<Vec<u8>> {
        let id = self.next_id;
        self.next_id += 1;

        // Calculate packet length (header is 11 bytes: 4 length + 4 id + 1 flags + 1 cmd_set + 1 cmd)
        let len = 11 + data.len();

        // Write header
        self.write_u32(len as u32)?;
        self.write_u32(id)?;
        self.stream.write_all(&[JDWP_PACKET_TYPE_CMD, cmd_set, cmd])?;

        // Write data
        if !data.is_empty() {
            self.stream.write_all(data)?;
        }
        self.stream.flush()?;

        // Read reply
        let reply_len = self.read_u32()?;
        let _reply_id = self.read_u32()?;
        let flags = {
            let mut buf = [0u8; 1];
            self.stream.read_exact(&mut buf)?;
            buf[0]
        };

        // Read error code if this is a reply packet
        if flags == JDWP_PACKET_TYPE_REPLY {
            let error_code = {
                let mut buf = [0u8; 2];
                self.stream.read_exact(&mut buf)?;
                u16::from_be_bytes(buf)
            };
            if error_code != 0 {
                return Err(std::io::Error::other(format!("JDWP error code: {}", error_code)));
            }
        }

        // Read remaining data
        let data_len = reply_len as usize - 11;
        let mut response = vec![0u8; data_len];
        if data_len > 0 {
            self.stream.read_exact(&mut response)?;
        }

        Ok(response)
    }

    /// Get the JVM version information.
    fn get_version(&mut self) -> std::io::Result<String> {
        let data = self.send_command(cmd_set::VIRTUAL_MACHINE, cmd::virtual_machine::VERSION, &[])?;

        // Parse version string (length-prefixed)
        let len = u16::from_be_bytes([data[0], data[1]]) as usize;
        let version = String::from_utf8_lossy(&data[2..2 + len]).to_string();

        Ok(version)
    }

    /// Set an event request for breakpoints.
    fn set_breakpoint(&mut self, event_kind: u8, line_number: u32) -> std::io::Result<u32> {
        // Build event request set command
        // Format: [suspendPolicy:1][eventKind:1][modifiersCount:4][modifiers...]
        let suspend_policy = suspend_policy::ALL;
        let modifiers_count = 1u32;

        // Modifier: LineOnly modifier
        // [modifierKind:2][sourceId:4][lineNumber:4]
        // modifierKind for LineOnly is 3
        let line_only_kind = 3u16.to_be_bytes();
        let source_id = 0u32.to_be_bytes();
        let line = line_number.to_be_bytes();

        let mut modifiers = Vec::new();
        modifiers.extend_from_slice(&line_only_kind);
        modifiers.extend_from_slice(&source_id);
        modifiers.extend_from_slice(&line);

        let mut data = Vec::new();
        data.push(suspend_policy);
        data.push(event_kind);
        data.extend_from_slice(&modifiers_count.to_be_bytes());
        data.extend_from_slice(&modifiers);

        let response = self.send_command(cmd_set::EVENT_REQUEST, cmd::event_request::SET, &data)?;

        // Response contains requestId (u32)
        let request_id = u32::from_be_bytes([response[0], response[1], response[2], response[3]]);

        Ok(request_id)
    }

    /// Resume the JVM.
    fn resume(&mut self) -> std::io::Result<()> {
        let _ = self.send_command(cmd_set::VIRTUAL_MACHINE, cmd::virtual_machine::RESUME, &[])?;
        Ok(())
    }

    /// Wait for an event packet and return the breakpoint hit info.
    fn wait_for_event(&mut self) -> std::io::Result<BreakpointHit> {
        loop {
            let event_data = self.read_event_packet()?;

            // Parse event packet format:
            // [suspendPolicy:1][events:4][...event data...]
            if event_data.len() < 5 {
                continue;
            }

            let suspend_policy = event_data[0];
            let num_events = u32::from_be_bytes([event_data[1], event_data[2], event_data[3], event_data[4]]) as usize;

            let mut offset = 5;
            for _ in 0..num_events {
                if offset >= event_data.len() {
                    break;
                }

                let event_kind = event_data[offset];
                offset += 1;

                // Read requestId (u32)
                if offset + 4 > event_data.len() {
                    break;
                }
                let _request_id = u32::from_be_bytes([event_data[offset], event_data[offset+1], event_data[offset+2], event_data[offset+3]]);
                offset += 4;

                // Read location (threadId:8 + classId:8 + methodId:8 + bytecode_index:8 + line:4)
                if offset + 28 > event_data.len() {
                    break;
                }
                let _thread_id = u64::from_be_bytes([event_data[offset], event_data[offset+1], event_data[offset+2], event_data[offset+3],
                                                       event_data[offset+4], event_data[offset+5], event_data[offset+6], event_data[offset+7]]);
                offset += 8;

                if event_kind == event_kind::BREAKPOINT || event_kind == event_kind::METHOD_ENTRY {
                    // We got a breakpoint or method entry event
                    let line_offset = 8 + 8 + 8 + 8; // skip classId + methodId + bytecode_index
                    if offset + line_offset + 4 <= event_data.len() {
                        let line = u32::from_be_bytes([event_data[offset + line_offset],
                                                       event_data[offset + line_offset + 1],
                                                       event_data[offset + line_offset + 2],
                                                       event_data[offset + line_offset + 3]]);

                        return Ok(BreakpointHit {
                            pid: 0, // We'd need to get this from the VM
                            tid: 0,
                            address: line as u64,
                        });
                    }
                }
            }

            // Resume after handling event if suspend policy was ALL
            if suspend_policy == suspend_policy::ALL {
                let _ = self.resume();
            }
        }
    }

    /// Read an event packet from the stream.
    fn read_event_packet(&mut self) -> std::io::Result<Vec<u8>> {
        let len = self.read_u32()?;
        let id = self.read_u32()?;
        let flags = {
            let mut buf = [0u8; 1];
            self.stream.read_exact(&mut buf)?;
            buf[0]
        };

        // For events (not replies), flags will be 0x0
        // For replies, flags will be 0x80

        // Read remaining data
        let data_len = len as usize - 11;
        let mut data = vec![0u8; data_len];
        if data_len > 0 {
            self.stream.read_exact(&mut data)?;
        }

        // Combine header info for processing
        let mut packet = Vec::with_capacity(11 + data_len);
        packet.extend_from_slice(&len.to_be_bytes());
        packet.extend_from_slice(&id.to_be_bytes());
        packet.push(flags);
        packet.extend_from_slice(&data);

        Ok(packet)
    }
}

/// Java debug target using JDWP.
#[derive(Debug, Clone)]
pub struct JavaTarget {
    attached: Arc<Mutex<bool>>,
    pid: Arc<Mutex<Option<u32>>>,
    port: u16,
    client: Arc<Mutex<Option<JdwpClient>>>,
}

impl Default for JavaTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaTarget {
    /// Creates a new JavaTarget with default JDWP port (5005).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port: 5005,
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates a new JavaTarget with a custom port.
    pub fn with_port(port: u16) -> Self {
        Self {
            attached: Arc::new(Mutex::new(false)),
            pid: Arc::new(Mutex::new(None)),
            port,
            client: Arc::new(Mutex::new(None)),
        }
    }
}

impl DebugTarget for JavaTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if *self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed(
                "already attached".to_string(),
            ));
        }

        // Connect to JDWP server
        let mut client = JdwpClient::new(self.port)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("JDWP connection failed: {}", e)))?;

        // Verify connection by getting version
        let _version = client.get_version()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("JDWP version check failed: {}", e)))?;

        *self.client.lock().unwrap() = Some(client);
        *self.attached.lock().unwrap() = true;

        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with JDWP agent
        let jdwp_arg = format!("transport=dt_socket,server=y,suspend=y,address={}", self.port);
        let mut cmd = Command::new(program);
        cmd.arg(format!("-agentlib:jdwp={}", jdwp_arg));
        cmd.args(args);

        let child = cmd
            .spawn()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;

        let pid = child.id();

        // Wait briefly for JVM to start and expose JDWP port
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Try to connect
        match JdwpClient::new(self.port) {
            Ok(mut client) => {
                // Verify connection
                if let Ok(_version) = client.get_version() {
                    *self.client.lock().unwrap() = Some(client);
                    *self.attached.lock().unwrap() = true;
                    *self.pid.lock().unwrap() = Some(pid);
                    return Ok(pid);
                }
            }
            Err(e) => {
                tracing::warn!("JDWP connection attempt failed (program may still run): {}", e);
            }
        }

        // Even if we couldn't connect yet, the process is spawned
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

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            // address is treated as a line number
            let _request_id = client.set_breakpoint(event_kind::BREAKPOINT, address as u32)
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("set_breakpoint failed: {}", e)))?;
            Ok(())
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no JDWP client connection".to_string()))
        }
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            client.wait_for_event()
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("wait_for_event failed: {}", e)))
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no JDWP client connection".to_string()))
        }
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !*self.attached.lock().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut client_guard = self.client.lock().unwrap();
        if let Some(ref mut client) = *client_guard {
            client.resume()
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("resume failed: {}", e)))
        } else {
            Err(SandboxError::DebugTargetConnectFailed("no JDWP client connection".to_string()))
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
    fn test_java_target_default_port() {
        let target = JavaTarget::new();
        assert!(!target.is_attached());
        assert_eq!(target.port, 5005);
    }

    #[test]
    fn test_java_target_with_port() {
        let target = JavaTarget::with_port(12345);
        assert_eq!(target.port, 12345);
    }

    #[test]
    fn test_java_target_attach_not_attached() {
        let target = JavaTarget::new();
        let result = target.set_breakpoint(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_java_target_wait_not_attached() {
        let target = JavaTarget::new();
        let result = target.wait();
        assert!(result.is_err());
    }

    #[test]
    fn test_java_target_resume_not_attached() {
        let target = JavaTarget::new();
        let result = target.resume();
        assert!(result.is_err());
    }

    #[test]
    fn test_java_target_detach_when_not_attached() {
        let target = JavaTarget::new();
        let result = target.detach();
        assert!(result.is_ok());
        assert!(!target.is_attached());
    }

    #[test]
    #[ignore = "requires Java and a JVM with JDWP agent"]
    fn test_java_target_jdwp_integration() {
        // This test requires Java to be installed
        // and a JVM with JDWP agent to debug. Marked as ignored by default.
        let target = JavaTarget::new();
        // Try to spawn a simple Java program under JDWP
        let result = target.spawn("java", &["-version"]);
        if result.is_ok() {
            let detach_result = target.detach();
            assert!(detach_result.is_ok());
        }
    }
}
