//! Decode trace events from binary format.

use crate::TraceHeader;
use chronos_domain::TraceEvent;
use std::io::{self, Read};

/// Decode a trace header from bytes.
pub fn decode_header(data: &[u8]) -> Result<TraceHeader, String> {
    bincode::deserialize(data).map_err(|e| format!("Failed to decode header: {}", e))
}

/// Decode a single trace event from bytes.
pub fn decode_event(data: &[u8]) -> Result<TraceEvent, String> {
    bincode::deserialize(data).map_err(|e| format!("Failed to decode event: {}", e))
}

/// Decode a compressed event.
#[allow(dead_code)] // Used by future compressed trace format
pub fn decode_event_compressed(data: &[u8]) -> Result<TraceEvent, String> {
    let decompressed = lz4_flex::decompress_size_prepended(data)
        .map_err(|e| format!("LZ4 decompression failed: {}", e))?;
    decode_event(&decompressed)
}

/// Read a single length-prefixed event from a reader.
pub fn read_event<R: Read>(reader: &mut R) -> io::Result<Option<TraceEvent>> {
    let mut len_buf = [0u8; 8];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u64::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data)?;

    let event = decode_event(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode;

    #[test]
    fn test_roundtrip_header() {
        let header = crate::TraceHeader::new("s1", "c", "./test", 100);
        let encoded = encode::encode_header(&header);
        let decoded = decode_header(&encoded).unwrap();
        assert_eq!(decoded.session_id, "s1");
    }

    #[test]
    fn test_roundtrip_event() {
        let event = chronos_domain::TraceEvent::function_entry(1, 100, 1, "foo", 0x2000);
        let encoded = encode::encode_event(&event);
        let decoded = decode_event(&encoded).unwrap();
        assert_eq!(decoded.event_id, 1);
    }

    #[test]
    fn test_read_write_roundtrip() {
        let event = chronos_domain::TraceEvent::function_entry(5, 500, 1, "bar", 0x3000);
        let mut buf = Vec::new();
        encode::write_event(&mut buf, &event).unwrap();

        let mut cursor = io::Cursor::new(buf);
        let decoded = read_event(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded.event_id, 5);
    }

    #[test]
    fn test_read_empty_stream() {
        let mut cursor = io::Cursor::new(Vec::<u8>::new());
        let result = read_event(&mut cursor).unwrap();
        assert!(result.is_none());
    }
}
