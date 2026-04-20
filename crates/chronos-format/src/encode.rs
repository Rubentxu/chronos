//! Encode trace events to binary format.

use crate::TraceHeader;
use chronos_domain::TraceEvent;
use std::io::{self, Write};

/// Encode a trace header to bytes.
pub fn encode_header(header: &TraceHeader) -> Vec<u8> {
    bincode::serialize(header).expect("header serialization should never fail")
}

/// Encode a single trace event to bytes (without compression).
pub fn encode_event(event: &TraceEvent) -> Vec<u8> {
    bincode::serialize(event).expect("event serialization should never fail")
}

/// Encode a trace event with LZ4 compression.
pub fn encode_event_compressed(event: &TraceEvent) -> Vec<u8> {
    let raw = encode_event(event);
    lz4_flex::compress_prepend_size(&raw)
}

/// Write a length-prefixed event to a writer.
pub fn write_event<W: Write>(writer: &mut W, event: &TraceEvent) -> io::Result<()> {
    let data = encode_event(event);
    let len = data.len() as u64;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};

    #[test]
    fn test_encode_decode_header() {
        let header = TraceHeader::new("session-1", "rust", "/usr/bin/test", 1234);
        let encoded = encode_header(&header);
        let decoded: TraceHeader = bincode::deserialize(&encoded).unwrap();
        assert_eq!(decoded.session_id, "session-1");
        assert_eq!(decoded.language, "rust");
        assert_eq!(decoded.pid, 1234);
        assert_eq!(decoded.magic, crate::CHRONOS_MAGIC);
    }

    #[test]
    fn test_encode_decode_event() {
        let event = TraceEvent::new(
            42,
            12345,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("main.rs", 10, "main", 0x401000),
            EventData::Empty,
        );
        let encoded = encode_event(&event);
        let decoded: TraceEvent = bincode::deserialize(&encoded).unwrap();
        assert_eq!(decoded.event_id, 42);
        assert_eq!(decoded.event_type, EventType::FunctionEntry);
        assert_eq!(decoded.function_name(), Some("main"));
    }

    #[test]
    fn test_compressed_event() {
        let event = TraceEvent::function_entry(1, 100, 1, "test", 0x1000);
        let compressed = encode_event_compressed(&event);
        // Compressed should be non-empty
        assert!(!compressed.is_empty());

        // Can decompress back
        let decompressed = lz4_flex::decompress_size_prepended(&compressed).unwrap();
        let decoded: TraceEvent = bincode::deserialize(&decompressed).unwrap();
        assert_eq!(decoded.event_id, 1);
    }
}
