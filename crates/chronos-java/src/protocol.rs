//! Minimal JDWP client for reading debugger events.
//!
//! JDWP (Java Debug Wire Protocol) is the protocol used between a JVM
//! and a debugger. This module implements the subset needed to receive
//! method entry/exit and exception events.

use crate::error::JavaError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// JDWP command packet header size (11 bytes).
const JDWP_HEADER_SIZE: usize = 11;

/// JDWP handshake bytes: "JDWP-Handshake"
pub const JDWP_HANDSHAKE: &[u8] = b"JDWP-Handshake";

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
    pub(crate) stream: TcpStream,
    pub(crate) next_id: u32,
    /// ID sizes for tagged IDs (objectID, threadID, frameID). Default 8 for 64-bit JVM.
    pub id_sizes: [u8; 2],
}

impl JdwpClient {
    /// Connect to a JVM's JDWP port.
    pub async fn connect(port: u16) -> Result<Self, JavaError> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| JavaError::JdwpProtocol(format!("Failed to connect: {}", e)))?;
        // Default to 8 bytes for 64-bit JVM IDs. Actual value can be read
        // from VirtualMachine.IDSizes after handshake if needed.
        Ok(Self {
            stream,
            next_id: 1,
            id_sizes: [8, 8],
        })
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

        if recv != JDWP_HANDSHAKE {
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

    /// Send a JDWP command and return the reply data payload.
    ///
    /// Builds a command packet, sends it, reads the reply header,
    /// checks for errors, and returns the data portion.
    ///
    /// # Arguments
    /// * `cmd_set` - The JDWP command set byte
    /// * `cmd` - The JDWP command byte
    /// * `data` - The command data payload
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The reply data payload on success
    /// * `Err(JavaError)` - If the command failed
    pub async fn send_command(
        &mut self,
        cmd_set: u8,
        cmd: u8,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, JavaError> {
        let id = self.next_id;
        self.next_id += 1;

        let packet = build_command_packet(cmd_set, cmd, id, &data);
        send_packet(&mut self.stream, &packet).await?;

        // Read the 11-byte reply header
        let mut header = [0u8; JDWP_HEADER_SIZE];
        self.stream
            .read_exact(&mut header)
            .await
            .map_err(|e| JavaError::JdwpProtocol(format!("Read reply header failed: {}", e)))?;

        // Parse header fields
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let _reply_id = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        let flags = header[8];
        let error_code = u16::from_be_bytes([header[9], header[10]]);

        // Check if this is a reply packet (flags should be 0x80)
        if flags != 0x80 {
            return Err(JavaError::JdwpProtocol(format!(
                "Expected reply packet (flags=0x80), got flags=0x{:02x}",
                flags
            )));
        }

        // Check error code (0 = success)
        if error_code != 0 {
            return Err(JavaError::JdwpProtocol(format!(
                "JDWP command error: error_code={}",
                error_code
            )));
        }

        // Read the data portion
        let data_len = (length as usize).saturating_sub(JDWP_HEADER_SIZE);
        let mut reply_data = vec![0u8; data_len];
        if data_len > 0 {
            self.stream
                .read_exact(&mut reply_data)
                .await
                .map_err(|e| JavaError::JdwpProtocol(format!("Read reply data failed: {}", e)))?;
        }

        Ok(reply_data)
    }

    /// Get all thread IDs from the JVM using VirtualMachine.AllThreads.
    ///
    /// Command set=1 (VirtualMachine), Command=6 (AllThreads)
    pub async fn all_threads(&mut self) -> Result<Vec<u64>, JavaError> {
        let reply = self.send_command(1, 6, vec![]).await?;
        parse_all_threads(&reply)
    }

    /// Get the name of a thread using ThreadReference.Name.
    ///
    /// Command set=11 (ThreadReference), Command=1 (Name)
    pub async fn thread_name(&mut self, thread_id: u64) -> Result<String, JavaError> {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&thread_id.to_be_bytes());
        let reply = self.send_command(11, 1, data).await?;
        parse_thread_name(&reply)
    }

    /// Get stack frames for a thread using ThreadReference.Frames.
    ///
    /// Command set=4 (ThreadReference), Command=6 (Frames)
    /// Request: threadID(8) + startFrame(4) + length(4)
    /// Use start=-1 and length=-1 to get all frames.
    pub async fn frames(
        &mut self,
        thread_id: u64,
        start: i32,
        length: i32,
    ) -> Result<Vec<FrameInfo>, JavaError> {
        let mut data = Vec::with_capacity(16);
        data.extend_from_slice(&thread_id.to_be_bytes());
        data.extend_from_slice(&start.to_be_bytes());
        data.extend_from_slice(&length.to_be_bytes());
        let reply = self.send_command(4, 6, data).await?;
        parse_frames(&reply)
    }

    /// Get values of local variables in a stack frame using StackFrame.GetValues.
    ///
    /// Command set=6 (StackFrame), Command=2 (GetValues)
    /// Request: threadID(8) + frameID(8) + slots_count(4) + per slot: slot(4) + sigbyte(1)
    /// Response: count(4) + per value: tag(1) + value_bytes
    ///
    /// This method queries slots 0..slot_count with signature 'I' (int).
    pub async fn frame_values(
        &mut self,
        thread_id: u64,
        frame_id: u64,
        slot_count: u32,
    ) -> Result<Vec<String>, JavaError> {
        let mut data = Vec::with_capacity(17 + slot_count as usize * 5);
        data.extend_from_slice(&thread_id.to_be_bytes());
        data.extend_from_slice(&frame_id.to_be_bytes());
        data.extend_from_slice(&slot_count.to_be_bytes());

        // Add slots 0..slot_count with signature 'I' (int)
        for slot in 0..slot_count {
            data.extend_from_slice(&slot.to_be_bytes());
            data.push(b'I'); // signature byte for int
        }

        let reply = self.send_command(6, 2, data).await?;

        // Parse reply: count(4) + per value: tag(1) + value_bytes
        if reply.len() < 4 {
            return Err(JavaError::JdwpProtocol(
                "GetValues response too short".to_string(),
            ));
        }

        let count = u32::from_be_bytes([reply[0], reply[1], reply[2], reply[3]]) as usize;
        let mut offset = 4;
        let mut values = Vec::with_capacity(count);

        for _ in 0..count {
            let (val_str, consumed) = parse_tagged_value(&reply[offset..])
                .map_err(|e| JavaError::JdwpProtocol(format!("GetValues value parse error: {}", e)))?;
            values.push(val_str);
            offset += consumed;
        }

        Ok(values)
    }

    /// Get all loaded classes from the JVM using VirtualMachine.AllClasses.
    ///
    /// Command set=1 (VirtualMachine), Command=9 (AllClasses)
    /// Returns a list of class signatures that can be used to look up classes.
    pub async fn all_classes(&mut self) -> Result<Vec<ClassInfo>, JavaError> {
        let reply = self.send_command(1, 9, vec![]).await?;
        parse_all_classes(&reply)
    }

    /// Get static field values for a reference type using ReferenceType.GetValues.
    ///
    /// Command set=2 (ReferenceType), Command=6 (GetValues)
    /// Request: referenceTypeID(8) + fieldsCount(4) + fieldID(8) * fieldsCount
    /// Response: valuesCount(4) + tagged-value * valuesCount
    pub async fn get_static_field_values(
        &mut self,
        ref_type_id: u64,
        field_ids: &[u64],
    ) -> Result<Vec<String>, JavaError> {
        let mut data = Vec::with_capacity(12 + field_ids.len() * 8);
        data.extend_from_slice(&ref_type_id.to_be_bytes());
        data.extend_from_slice(&(field_ids.len() as u32).to_be_bytes());
        for field_id in field_ids {
            data.extend_from_slice(&field_id.to_be_bytes());
        }

        let reply = self.send_command(2, 6, data).await?;

        // Parse reply: valuesCount(4) + tagged-value * valuesCount
        if reply.len() < 4 {
            return Err(JavaError::JdwpProtocol(
                "GetValues response too short".to_string(),
            ));
        }

        let count = u32::from_be_bytes([reply[0], reply[1], reply[2], reply[3]]) as usize;
        let mut offset = 4;
        let mut values = Vec::with_capacity(count);

        for _ in 0..count {
            let (val_str, consumed) = parse_tagged_value(&reply[offset..])
                .map_err(|e| JavaError::JdwpProtocol(format!("GetValues value parse error: {}", e)))?;
            values.push(val_str);
            offset += consumed;
        }

        Ok(values)
    }

    /// Get fields for a reference type using ReferenceType.Fields.
    ///
    /// Command set=2 (ReferenceType), Command=4 (Fields)
    /// Request: referenceTypeID(8)
    /// Response: int count + per field: fieldID(8) + name + signature + genericSignature + modifiers(4) + genericSignature
    pub async fn reference_type_fields(
        &mut self,
        ref_type_id: u64,
    ) -> Result<Vec<FieldInfo>, JavaError> {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&ref_type_id.to_be_bytes());
        let reply = self.send_command(2, 4, data).await?;
        parse_fields(&reply)
    }
}

/// Information about a loaded class.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// The class signature (e.g., "java/lang/String").
    pub signature: String,
    /// The class ID in JDWP.
    pub class_id: u64,
}

/// Information about a field in a class.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    /// The field ID in JDWP.
    pub field_id: u64,
    /// The field name.
    pub name: String,
    /// The field signature (e.g., "I" for int, "Ljava/lang/String;" for String).
    pub signature: String,
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

/// Parse VirtualMachine.AllThreads reply data.
///
/// Format: int count + count × threadID(8 bytes each)
pub fn parse_all_threads(data: &[u8]) -> Result<Vec<u64>, JavaError> {
    if data.len() < 4 {
        return Err(JavaError::JdwpProtocol(
            "AllThreads response too short".to_string(),
        ));
    }

    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    let mut thread_ids = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 8 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllThreads response: truncated thread ID".to_string(),
            ));
        }
        let thread_id = read_u64(&data[offset..offset + 8]);
        thread_ids.push(thread_id);
        offset += 8;
    }

    Ok(thread_ids)
}

/// Parse ThreadReference.Name reply data.
///
/// Format: String (length-prefixed UTF-8: int32 length + bytes)
pub fn parse_thread_name(data: &[u8]) -> Result<String, JavaError> {
    if data.len() < 4 {
        return Err(JavaError::JdwpProtocol(
            "ThreadName response too short".to_string(),
        ));
    }

    let length = i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + length {
        return Err(JavaError::JdwpProtocol(
            "ThreadName response: truncated string".to_string(),
        ));
    }

    let name = String::from_utf8_lossy(&data[4..4 + length]).to_string();
    Ok(name)
}

/// A stack frame with its ID and location.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    /// The frame ID.
    pub frame_id: u64,
    /// The location (typeTag:1 + classID:8 + methodID:8 + index:8)
    pub location: u64,
}

/// Parse ThreadReference.Frames reply data.
///
/// Format: int count + per frame: frameID(8) + location(typeTag:1 + classID:8 + methodID:8 + index:8)
pub fn parse_frames(data: &[u8]) -> Result<Vec<FrameInfo>, JavaError> {
    if data.len() < 4 {
        return Err(JavaError::JdwpProtocol(
            "Frames response too short".to_string(),
        ));
    }

    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 8 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Frames response: truncated frame ID".to_string(),
            ));
        }
        let frame_id = read_u64(&data[offset..offset + 8]);
        offset += 8;

        if offset + 8 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Frames response: truncated location".to_string(),
            ));
        }
        // Location is 1 byte typeTag + 8 bytes of ID + 8 bytes for index = 17 bytes
        // But typically it's packed as: typeTag(1) + classID(8) + methodID(8) + index(8) = 25 bytes
        // We only read the first 8 bytes as a simplified location identifier
        let location = read_u64(&data[offset..offset + 8]);
        offset += 8;

        frames.push(FrameInfo { frame_id, location });
    }

    Ok(frames)
}

/// Parse a JDWP tagged value from a buffer.
///
/// Tags:
/// - 'I' → i32 (4 bytes big-endian)
/// - 'J' → i64 (8 bytes big-endian)
/// - 'Z' → bool (1 byte: 0=false, 1=true)
/// - 'D' → f64 (8 bytes big-endian IEEE 754)
/// - 's' → String object (objectID; return placeholder)
/// - Unknown tag → error
///
/// Returns the string representation and the number of bytes consumed.
pub fn parse_tagged_value(data: &[u8]) -> Result<(String, usize), JavaError> {
    if data.is_empty() {
        return Err(JavaError::JdwpProtocol(
            "Tagged value: empty data".to_string(),
        ));
    }

    let tag = data[0] as char;
    let offset = 1;

    match tag {
        'I' => {
            if data.len() < offset + 4 {
                return Err(JavaError::JdwpProtocol(
                    "Tagged value: I (int) requires 4 bytes".to_string(),
                ));
            }
            let val = i32::from_be_bytes([data[1], data[2], data[3], data[4]]);
            Ok((val.to_string(), offset + 4))
        }
        'J' => {
            if data.len() < offset + 8 {
                return Err(JavaError::JdwpProtocol(
                    "Tagged value: J (long) requires 8 bytes".to_string(),
                ));
            }
            let val = i64::from_be_bytes([
                data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
            ]);
            Ok((val.to_string(), offset + 8))
        }
        'Z' => {
            if data.len() < offset + 1 {
                return Err(JavaError::JdwpProtocol(
                    "Tagged value: Z (bool) requires 1 byte".to_string(),
                ));
            }
            let val = data[1] != 0;
            Ok((val.to_string(), offset + 1))
        }
        'D' => {
            if data.len() < offset + 8 {
                return Err(JavaError::JdwpProtocol(
                    "Tagged value: D (double) requires 8 bytes".to_string(),
                ));
            }
            let val = f64::from_be_bytes([data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8]]);
            Ok((val.to_string(), offset + 8))
        }
        's' | 'L' => {
            // String object or other reference type - return placeholder
            // The object ID follows but we don't dereference it
            if data.len() < offset + 8 {
                return Err(JavaError::JdwpProtocol(
                    "Tagged value: s/L (object) requires 8 bytes for object ID".to_string(),
                ));
            }
            let obj_id = read_u64(&data[offset..offset + 8]);
            Ok((format!("Object@{:x}", obj_id), offset + 8))
        }
        _ => Err(JavaError::JdwpProtocol(format!(
            "Tagged value: unknown tag '{}' (0x{:02x})",
            tag, data[0]
        ))),
    }
}

/// Parse VirtualMachine.AllClasses reply data.
///
/// Format: int count + per class: byte typeTag + classID(8) + signature + genericSignature + status
/// - signature is a string: int length(4) + UTF-8 bytes
pub fn parse_all_classes(data: &[u8]) -> Result<Vec<ClassInfo>, JavaError> {
    if data.len() < 4 {
        return Err(JavaError::JdwpProtocol(
            "AllClasses response too short".to_string(),
        ));
    }

    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    let mut classes = Vec::with_capacity(count);
    for _ in 0..count {
        // Read typeTag (1 byte)
        if offset + 1 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated typeTag".to_string(),
            ));
        }
        let _type_tag = data[offset];
        offset += 1;

        // Read classID (8 bytes)
        if offset + 8 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated classID".to_string(),
            ));
        }
        let class_id = read_u64(&data[offset..offset + 8]);
        offset += 8;

        // Read signature (string: length + bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated signature length".to_string(),
            ));
        }
        let sig_length = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if offset + sig_length > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated signature".to_string(),
            ));
        }
        let signature = String::from_utf8_lossy(&data[offset..offset + sig_length]).to_string();
        offset += sig_length;

        // Skip genericSignature (string) - just read length and skip
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated genericSignature length".to_string(),
            ));
        }
        let gen_sig_length = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        offset += gen_sig_length;

        // Skip status (4 bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "AllClasses response: truncated status".to_string(),
            ));
        }
        offset += 4;

        classes.push(ClassInfo {
            signature,
            class_id,
        });
    }

    Ok(classes)
}

/// Parse ReferenceType.Fields reply data.
///
/// Format: int count + per field: fieldID(8) + name + signature + genericSignature + modifiers(4) + genericSignature
fn parse_fields(data: &[u8]) -> Result<Vec<FieldInfo>, JavaError> {
    if data.len() < 4 {
        return Err(JavaError::JdwpProtocol(
            "Fields response too short".to_string(),
        ));
    }

    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        // Read fieldID (8 bytes)
        if offset + 8 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated fieldID".to_string(),
            ));
        }
        let field_id = read_u64(&data[offset..offset + 8]);
        offset += 8;

        // Read name (string: length + bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated name length".to_string(),
            ));
        }
        let name_length = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if offset + name_length > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated name".to_string(),
            ));
        }
        let name = String::from_utf8_lossy(&data[offset..offset + name_length]).to_string();
        offset += name_length;

        // Read signature (string: length + bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated signature length".to_string(),
            ));
        }
        let sig_length = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if offset + sig_length > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated signature".to_string(),
            ));
        }
        let signature = String::from_utf8_lossy(&data[offset..offset + sig_length]).to_string();
        offset += sig_length;

        // Skip genericSignature (string: length + bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated genericSignature length".to_string(),
            ));
        }
        let gen_sig_length = i32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;
        offset += gen_sig_length;

        // Skip modifiers (4 bytes)
        if offset + 4 > data.len() {
            return Err(JavaError::JdwpProtocol(
                "Fields response: truncated modifiers".to_string(),
            ));
        }
        offset += 4;

        fields.push(FieldInfo {
            field_id,
            name,
            signature,
        });
    }

    Ok(fields)
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
        let length_with_data = u32::from_be_bytes([
            packet_with_data[0],
            packet_with_data[1],
            packet_with_data[2],
            packet_with_data[3],
        ]);
        assert_eq!(length_with_data, 14); // 11 header + 3 data
    }

    #[test]
    fn test_read_u64() {
        let bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x2B];
        assert_eq!(read_u64(&bytes), 0x12B);
    }

    #[test]
    fn test_parse_all_threads_two_threads() {
        // Format: int count(4) + threadID1(8) + threadID2(8)
        // count=2, tid1=0xDEADBEEF, tid2=0xCAFEBABE
        let data = vec![
            0x00, 0x00, 0x00, 0x02, // count = 2
            0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF, // thread ID 1
            0x00, 0x00, 0x00, 0x00, 0xCA, 0xFE, 0xBA, 0xBE, // thread ID 2
        ];

        let result = parse_all_threads(&data);
        assert!(result.is_ok());
        let threads = result.unwrap();
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0], 0xDEADBEEF);
        assert_eq!(threads[1], 0xCAFEBABE);
    }

    #[test]
    fn test_parse_all_threads_empty() {
        // Format: int count(4) = 0
        let data = vec![0x00, 0x00, 0x00, 0x00];

        let result = parse_all_threads(&data);
        assert!(result.is_ok());
        let threads = result.unwrap();
        assert!(threads.is_empty());
    }

    #[test]
    fn test_parse_all_threads_truncated() {
        // Format: int count(4) = 1, but only 4 bytes of thread ID provided
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00];

        let result = parse_all_threads(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_thread_name() {
        // Format: int length(4) + UTF-8 bytes
        // "main" = 4 bytes
        let data = vec![
            0x00, 0x00, 0x00, 0x04, // length = 4
            b'm', b'a', b'i', b'n', // "main"
        ];

        let result = parse_thread_name(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "main");
    }

    #[test]
    fn test_parse_thread_name_long() {
        // "Thread-0" = 8 bytes
        let data = vec![
            0x00, 0x00, 0x00, 0x08, // length = 8
            b'T', b'h', b'r', b'e', b'a', b'd', b'-', b'0',
        ];

        let result = parse_thread_name(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Thread-0");
    }

    #[test]
    fn test_parse_thread_name_truncated() {
        // Format: int length(4) = 4, but only 2 bytes provided
        let data = vec![0x00, 0x00, 0x00, 0x04, b'm', b'a'];

        let result = parse_thread_name(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_all_threads_too_short() {
        // Only 2 bytes provided, not enough for count
        let data = vec![0x00, 0x00];

        let result = parse_all_threads(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_thread_name_too_short() {
        // Only 2 bytes provided, not enough for length
        let data = vec![0x00, 0x00];

        let result = parse_thread_name(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_frames_two_frames() {
        // Format: int count(4) + frame1(8) + location1(8) + frame2(8) + location2(8)
        let data = vec![
            0x00, 0x00, 0x00, 0x02, // count = 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // frame ID 1 = 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0A, // location 1 = 10
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // frame ID 2 = 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, // location 2 = 20
        ];

        let result = parse_frames(&data);
        assert!(result.is_ok());
        let frames = result.unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_id, 1);
        assert_eq!(frames[0].location, 10);
        assert_eq!(frames[1].frame_id, 2);
        assert_eq!(frames[1].location, 20);
    }

    #[test]
    fn test_parse_frames_empty() {
        // Format: int count(4) = 0
        let data = vec![0x00, 0x00, 0x00, 0x00];

        let result = parse_frames(&data);
        assert!(result.is_ok());
        let frames = result.unwrap();
        assert!(frames.is_empty());
    }

    #[test]
    fn test_parse_frames_truncated() {
        // Format: int count(4) = 1, but only 4 bytes of frame ID provided
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00];

        let result = parse_frames(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_frames_too_short() {
        // Only 2 bytes provided, not enough for count
        let data = vec![0x00, 0x00];

        let result = parse_frames(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tagged_value_int() {
        // Tag 'I' + i32 value 42
        let data = vec![b'I', 0x00, 0x00, 0x00, 0x2A];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        assert_eq!(val, "42");
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_parse_tagged_value_int_negative() {
        // Tag 'I' + i32 value -1
        let data = vec![b'I', 0xFF, 0xFF, 0xFF, 0xFF];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, _) = result.unwrap();
        assert_eq!(val, "-1");
    }

    #[test]
    fn test_parse_tagged_value_long() {
        // Tag 'J' + i64 value 0x123456789ABCDEF0 (positive, high bit not set)
        let data = vec![
            b'J', 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
        ];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        assert_eq!(val, "1311768467463790320"); // 0x123456789ABCDEF0 as i64
        assert_eq!(consumed, 9);
    }

    #[test]
    fn test_parse_tagged_value_bool_true() {
        // Tag 'Z' + bool true (1)
        let data = vec![b'Z', 0x01];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        assert_eq!(val, "true");
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_parse_tagged_value_bool_false() {
        // Tag 'Z' + bool false (0)
        let data = vec![b'Z', 0x00];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        assert_eq!(val, "false");
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_parse_tagged_value_double() {
        // Tag 'D' + f64 value 3.14159...
        // 3.14159 as f64 is approximately 0x400921F9F52D3852
        let data = vec![
            b'D', 0x40, 0x09, 0x21, 0xF9, 0xF5, 0x2D, 0x38, 0x52,
        ];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        // Just check that it parses to a numeric-looking string
        assert!(val.parse::<f64>().is_ok());
        assert_eq!(consumed, 9);
    }

    #[test]
    fn test_parse_tagged_value_string_object() {
        // Tag 's' + objectID 0xCAFEBABE
        let data = vec![
            b's', 0x00, 0x00, 0x00, 0x00, 0xCA, 0xFE, 0xBA, 0xBE,
        ];

        let result = parse_tagged_value(&data);
        assert!(result.is_ok());
        let (val, consumed) = result.unwrap();
        assert_eq!(val, "Object@cafebabe");
        assert_eq!(consumed, 9);
    }

    #[test]
    fn test_parse_tagged_value_unknown_tag() {
        // Tag 'X' - unknown
        let data = vec![b'X', 0x00];

        let result = parse_tagged_value(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tagged_value_empty() {
        // Empty data
        let data = vec![];

        let result = parse_tagged_value(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tagged_value_int_truncated() {
        // Tag 'I' but only 2 bytes of data
        let data = vec![b'I', 0x00];

        let result = parse_tagged_value(&data);
        assert!(result.is_err());
    }
}
