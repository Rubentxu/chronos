//! Minimal JDWP client for reading debugger events.
//!
//! JDWP (Java Debug Wire Protocol) is the protocol used between a JVM
//! and a debugger. This module implements the subset needed to receive
//! method entry/exit and exception events.

use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::JavaError;

/// JDWP command packet header size (11 bytes).
const JDWP_HEADER_SIZE: usize = 11;

/// JDWP handshake bytes: "JDWP-Handshake"
const JDWP_HANDSHAKE: &[u8] = b"JDWP-Handshake";

/// JDWP event kinds used in EventRequest.Set.
pub mod event_kind {
    pub const METHOD_ENTRY: u8 = 40;
    pub const METHOD_EXIT: u8 = 41;
    pub const EXCEPTION: u8 = 56;
    pub const BREAKPOINT: u8 = 2;
    pub const STEP: u8 = 1;
    pub const GOROUTINE_STOP: u8 = 100;
}

/// A connected JDWP client session.
pub struct JdwpClient {
    stream: TcpStream,
    next_id: u32,
}

impl JdwpClient {
    /// Connect to a JVM's JDWP port.
    pub async fn connect(port: u16) -> Result<Self, JavaError> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| JavaError::JdwpProtocol(format!("Failed to connect: {}", e)))?;
        Ok(Self { stream, next_id: 1 })
    }

    /// Perform the JDWP handshake.
    ///
    /// Sends "JDWP-Handshake" and expects the same bytes back.
    pub async fn handshake(&mut self) -> Result<(), JavaError> {
        self.stream
            .write_all(JDWP_HANDSHAKE)
            .await
            .map_err(|e| JavaError::JdwpHandshake(format!("Send failed: {}", e)))?;

        let mut recv = [0u8; 14];
        self.stream
            .read_exact(&mut recv)
            .await
            .map_err(|e| JavaError::JdwpHandshake(format!("Recv failed: {}", e)))?;

        if &recv != JDWP_HANDSHAKE {
            return Err(JavaError::JdwpHandshake(
                "Handshake bytes mismatch".to_string(),
            ));
        }
        Ok(())
    }

    /// Send a VirtualMachine.Resume command to resume the JVM.
    pub async fn resume(&mut self) -> Result<(), JavaError> {
        // VirtualMachine.Resume command set (1), command 9
        let id = self.next_id;
        self.next_id += 1;
        let packet = build_command_packet(1, 9, id, &[]);
        send_packet(&mut self.stream, &packet).await?;
        // For now, we don't parse the reply — assume success
        Ok(())
    }

    /// Set an event request for the given JDWP event kind.
    ///
    /// This uses the EventRequest.Set command (command set 43, command 1).
    /// SuspendPolicy=ALL (1) means all threads suspend when the event occurs.
    pub async fn set_event_request(&mut self, event_kind: u8) -> Result<(), JavaError> {
        let id = self.next_id;
        self.next_id += 1;

        // EventRequest.Set payload:
        // event_kind (1 byte), suspend_policy (1 byte), modifiers_count (4 bytes)
        // We use no modifiers (modifiers_count = 0)
        let payload = [
            event_kind, // Event kind
            1,          // SuspendPolicy: ALL
            0, 0, 0, 0, // Modifiers count: 0
        ];

        let packet = build_command_packet(43, 1, id, &payload);
        send_packet(&mut self.stream, &packet).await?;
        // For now, skip parsing the reply
        Ok(())
    }

    /// Read the next JDWP event from the connection.
    ///
    /// JDWP events are sent as reply packets (type=2) with data formatted
    /// according to the event kind.
    pub async fn read_event(&mut self) -> Result<JdwpEvent, JavaError> {
        loop {
            // Read the packet header first
            let mut header = [0u8; JDWP_HEADER_SIZE];
            self.stream
                .read_exact(&mut header)
                .await
                .map_err(|e| JavaError::JdwpProtocol(format!("Read header failed: {}", e)))?;

            // Parse length (4 bytes, big-endian) and id (4 bytes, big-endian)
            let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
            let _id = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
            let flags = header[8];
            let _error_code = u16::from_be_bytes([header[9], header[10]]);

            // Skip packets that aren't event packets (e.g., command replies)
            if flags != 0x80 {
                // Read remaining bytes and continue to next packet
                let remaining = (length as usize).saturating_sub(JDWP_HEADER_SIZE);
                if remaining > 0 {
                    let mut discard = vec![0u8; remaining];
                    self.stream
                        .read_exact(&mut discard)
                        .await
                        .map_err(|e| JavaError::JdwpProtocol(format!("Discard failed: {}", e)))?;
                }
                continue;
            }

            // Read the rest of the packet data
            let data_len = (length as usize).saturating_sub(JDWP_HEADER_SIZE);
            let mut data = vec![0u8; data_len];
            if data_len > 0 {
                self.stream
                    .read_exact(&mut data)
                    .await
                    .map_err(|e| JavaError::JdwpProtocol(format!("Read data failed: {}", e)))?;
            }

            return parse_jdwp_event(&data);
        }
    }
}

/// Build a JDWP command packet.
///
/// Format:
/// - length (4 bytes, big-endian): total packet size including header
/// - id (4 bytes, big-endian): packet identifier
/// - flags (1 byte): 0x00 for command packet
/// - command set (1 byte)
/// - command (1 byte)
/// - data (remaining bytes)
fn build_command_packet(command_set: u8, command: u8, id: u32, data: &[u8]) -> Vec<u8> {
    let length = (JDWP_HEADER_SIZE + data.len()) as u32;
    let mut packet = Vec::with_capacity(JDWP_HEADER_SIZE + data.len());
    packet.extend_from_slice(&length.to_be_bytes());
    packet.extend_from_slice(&id.to_be_bytes());
    packet.push(0x00); // flags for command packet
    packet.push(command_set);
    packet.push(command);
    packet.extend_from_slice(data);
    packet
}

/// Send a packet on the stream.
async fn send_packet(stream: &mut TcpStream, packet: &[u8]) -> Result<(), JavaError> {
    stream
        .write_all(packet)
        .await
        .map_err(|e| JavaError::JdwpProtocol(format!("Send failed: {}", e)))?;
    Ok(())
}

/// Parse a JDWP event packet data into a JdwpEvent.
///
/// This is a simplified parser that extracts the key fields.
/// JDWP event data format varies by event kind — we handle METHOD_ENTRY,
/// METHOD_EXIT, and EXCEPTION (kinds 40, 41, 56).
fn parse_jdwp_event(data: &[u8]) -> Result<JdwpEvent, JavaError> {
    if data.is_empty() {
        return Err(JavaError::JdwpProtocol("Empty event data".to_string()));
    }

    let event_kind = data[0];

    // Most JDWP event packets start with:
    // event_kind (1), request_id (4), thread_id (8), ... but the format varies.
    // For METHOD_ENTRY/METHOD_EXIT:
    //   event_kind(1), suspend_policy(1), modifiers(4), thread(8), refType(8), method(8), location(8)
    // For EXCEPTION:
    //   event_kind(1), suspend_policy(1), modifiers(4), thread(8), refType(8), location(8), exception(8), catchLocation(8)

    let mut offset = 1; // skip event_kind

    // Suspend policy (1 byte) + modifiers count (4 bytes)
    offset += 5;

    // Thread ID (8 bytes)
    let thread_id = if offset + 8 <= data.len() {
        read_u64(&data[offset..offset + 8])
    } else {
        0
    };
    offset += 8;

    // For event kinds we care about, the next field is typically a reference type ID (8 bytes)
    // followed by method/location info. We use placeholders for the MVP.
    let class_signature = format!("Class@{:x}", offset);

    // Method name is not directly in the packet — we'd need to do a separate
    // Method.Name command to get it. For now, use a placeholder.
    let method_name = "unknown".to_string();

    // Line number may be in the location field
    let line = None;

    Ok(JdwpEvent {
        kind: event_kind,
        thread_id,
        class_signature,
        method_name,
        line,
    })
}

/// Read a u64 from a byte slice (big-endian).
fn read_u64(bytes: &[u8]) -> u64 {
    let mut val = 0u64;
    for &b in bytes.iter().take(8) {
        val = (val << 8) | u64::from(b);
    }
    val
}

/// Represents a JDWP debugger event.
#[derive(Debug, Clone)]
pub struct JdwpEvent {
    /// The JDWP event kind (METHOD_ENTRY=40, METHOD_EXIT=41, EXCEPTION=56).
    pub kind: u8,
    /// The thread ID where the event occurred.
    pub thread_id: u64,
    /// The class signature (e.g., "Lcom/example/Foo;").
    pub class_signature: String,
    /// The method name.
    pub method_name: String,
    /// The source line number, if available.
    pub line: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jdwp_handshake_bytes() {
        assert_eq!(JDWP_HANDSHAKE, b"JDWP-Handshake");
        assert_eq!(JDWP_HANDSHAKE.len(), 14);
    }

    #[test]
    fn test_jdwp_packet_format() {
        // Build a VirtualMachine.Resume command (command set 1, command 9)
        let packet = build_command_packet(1, 9, 42, &[]);

        // Verify header fields
        let length = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        assert_eq!(length, 11); // header only, no data

        let id = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        assert_eq!(id, 42);

        assert_eq!(packet[8], 0x00); // flags
        assert_eq!(packet[9], 1); // command set
        assert_eq!(packet[10], 9); // command

        // Build a command with data
        let data = [0x01, 0x02, 0x03];
        let packet_with_data = build_command_packet(43, 1, 99, &data);
        let length_with_data =
            u32::from_be_bytes([packet_with_data[0], packet_with_data[1], packet_with_data[2], packet_with_data[3]]);
        assert_eq!(length_with_data, 14); // 11 header + 3 data
    }

    #[test]
    fn test_read_u64() {
        let bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x2B];
        assert_eq!(read_u64(&bytes), 0x12B);
    }
}
