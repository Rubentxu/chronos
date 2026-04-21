//! DAP (Debug Adapter Protocol) TCP client for Python debugpy.
//!
//! DAP uses Content-Length framing for messages.
//! The header is `Content-Length: <N>\r\n\r\n` followed by N bytes of JSON.

use crate::error::PythonAdapterError;
use std::io::Read;
use serde_json::Value;

/// DAP message sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seq(u64);

impl Seq {
    /// Get the next sequence number and increment.
    pub fn get_next(&mut self) -> u64 {
        let current = self.0;
        self.0 += 1;
        current
    }
}

impl Default for Seq {
    fn default() -> Self {
        Self(1)
    }
}

/// A DAP event received from the debuggee.
#[derive(Debug, Clone)]
pub struct DapEvent {
    /// Event type name (e.g., "stopped", "output", "terminated")
    pub event: String,
    /// Event body payload
    pub body: Value,
}

/// DAP client that communicates over TCP with debugpy.
pub struct DapClient {
    stream: std::net::TcpStream,
    seq: Seq,
    read_buf: String,
}

impl DapClient {
    /// Connect to debugpy DAP server at the given address.
    pub fn connect(addr: &str) -> Result<Self, PythonAdapterError> {
        let stream = std::net::TcpStream::connect(addr)
            .map_err(|e| PythonAdapterError::ConnectionFailed(e.to_string()))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .ok();
        Ok(Self {
            stream,
            seq: Seq::default(),
            read_buf: String::new(),
        })
    }

    /// Send a DAP request and wait for a response.
    pub fn send_request(
        &mut self,
        command: &str,
        args: Value,
    ) -> Result<Value, PythonAdapterError> {
        let request = serde_json::json!({
            "seq": self.seq.get_next(),
            "type": "request",
            "command": command,
            "arguments": args,
        });

        let json_str = serde_json::to_string(&request)?;
        self.send_raw(&json_str)?;

        // Read response
        let response: Value = self.read_message()?;
        if response.get("success").and_then(|s| s.as_bool()) == Some(false) {
            let msg = response.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
            return Err(PythonAdapterError::ProtocolError(msg.to_string()));
        }
        let result = response
            .get("body")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(result)
    }

    /// Read the next DAP message (request, response, or event).
    pub fn read_message(&mut self) -> Result<Value, PythonAdapterError> {
        // Read until we have a complete Content-Length delimited message
        loop {
            // Look for Content-Length header
            if let Some(pos) = self.read_buf.find("Content-Length:") {
                if let Some(end) = self.read_buf[pos..].find("\r\n\r\n") {
                    let header_end = pos + end + 4;
                    let header = &self.read_buf[pos..header_end];
                    let length = Self::parse_content_length(header)?;

                    let msg_start = header_end;
                    if self.read_buf.len() >= msg_start + length {
                        // Extract the message and drain buffer
                        let raw_msg = self.read_buf[msg_start..msg_start + length].to_string();
                        self.read_buf.drain(..msg_start + length);

                        let msg: Value = serde_json::from_str(&raw_msg)?;
                        return Ok(msg);
                    }
                }
            }

            // Need more data
            let mut buf = [0u8; 4096];
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                return Err(PythonAdapterError::ConnectionFailed(
                    "Connection closed".to_string(),
                ));
            }
            self.read_buf.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
    }

    /// Parse Content-Length header value.
    fn parse_content_length(header: &str) -> Result<usize, PythonAdapterError> {
        for line in header.lines() {
            if line.starts_with("Content-Length:") {
                let value = line.trim_start_matches("Content-Length:").trim();
                return value
                    .parse::<usize>()
                    .map_err(|_| PythonAdapterError::ProtocolError("Invalid Content-Length".into()));
            }
        }
        Err(PythonAdapterError::ProtocolError(
            "Missing Content-Length header".into(),
        ))
    }

    /// Send raw JSON string with DAP framing.
    fn send_raw(&mut self, json_str: &str) -> Result<(), PythonAdapterError> {
        let bytes = json_str.as_bytes();
        let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
        use std::io::Write;
        self.stream.write_all(header.as_bytes())?;
        self.stream.write_all(bytes)?;
        Ok(())
    }

    /// Initialize DAP session: send initialize and attach requests.
    pub fn initialize(&mut self, pid: u32) -> Result<(), PythonAdapterError> {
        // initialize request
        let _ = self.send_request(
            "initialize",
            serde_json::json!({
                "adapterID": "chronos",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
            }),
        )?;

        // attach request (debugpy-specific)
        let _ = self.send_request(
            "attach",
            serde_json::json!({
                "pid": pid,
                "waitOnExit": false,
            }),
        )?;

        // configurationDone request
        let _ = self.send_request("configurationDone", serde_json::json!({}))?;

        Ok(())
    }

    /// Read the next DAP event (blocking).
    /// Returns None when the session has terminated.
    pub fn next_event(&mut self) -> Result<Option<DapEvent>, PythonAdapterError> {
        loop {
            let msg = self.read_message()?;

            // Check if this is an event message
            let msg_type = msg.get("type").and_then(|t| t.as_str());
            if msg_type == Some("event") {
                let event = msg
                    .get("event")
                    .and_then(|e| e.as_str())
                    .unwrap_or("")
                    .to_string();
                let body = msg.get("body").cloned().unwrap_or(Value::Null);

                // "terminated" event means end of session
                if event == "terminated" {
                    return Ok(None);
                }

                return Ok(Some(DapEvent { event, body }));
            }

            // Otherwise keep reading
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seq_default() {
        let seq = Seq::default();
        assert_eq!(seq.0, 1);
    }

    #[test]
    fn test_seq_get_next() {
        let mut seq = Seq::default();
        assert_eq!(seq.get_next(), 1);
        assert_eq!(seq.get_next(), 2);
        assert_eq!(seq.get_next(), 3);
    }

    #[test]
    fn test_parse_content_length() {
        let header = "Content-Length: 42\r\n\r\n";
        let len = DapClient::parse_content_length(header).unwrap();
        assert_eq!(len, 42);
    }

    #[test]
    fn test_parse_content_length_missing() {
        let header = "Content-Type: application/json\r\n\r\n";
        let result = DapClient::parse_content_length(header);
        assert!(result.is_err());
    }

    // Integration tests that need real debugpy are marked ignored
    #[test]
    #[ignore]
    fn test_connect_to_debugpy() {
        // requires: debugpy running on localhost:5678
        let client = DapClient::connect("localhost:5678");
        assert!(client.is_ok());
    }
}
