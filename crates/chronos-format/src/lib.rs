//! chronos-format: Trace file serialization using bincode + LZ4 compression.
//!
//! For the MVP, we use bincode (binary serde) instead of FlatBuffers.
//! FlatBuffers zero-copy access will be added in Phase 2 for large trace files.

mod decode;
mod encode;
mod trace_file;

pub use decode::decode_event;
pub use encode::encode_event;
pub use trace_file::{TraceFileReader, TraceFileReaderWithMetadata, TraceFileWriter};


/// Magic bytes for .chronos files: "CHR1"
pub const CHRONOS_MAGIC: [u8; 4] = [b'C', b'H', b'R', b'1'];

/// Current format version.
pub const FORMAT_VERSION: u32 = 1;

/// Header of a .chronos trace file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceHeader {
    /// Magic bytes (CHR1).
    pub magic: [u8; 4],
    /// Format version.
    pub version: u32,
    /// Session ID.
    pub session_id: String,
    /// Language of the target.
    pub language: String,
    /// Binary path.
    pub binary_path: String,
    /// PID of the target process.
    pub pid: u32,
    /// Total event count (written when file is finalized).
    pub event_count: u64,
    /// Start timestamp (ns).
    pub start_timestamp_ns: u64,
    /// End timestamp (ns).
    pub end_timestamp_ns: u64,
}

impl TraceHeader {
    /// Create a new header for a capture session.
    pub fn new(
        session_id: impl Into<String>,
        language: impl Into<String>,
        binary_path: impl Into<String>,
        pid: u32,
    ) -> Self {
        Self {
            magic: CHRONOS_MAGIC,
            version: FORMAT_VERSION,
            session_id: session_id.into(),
            language: language.into(),
            binary_path: binary_path.into(),
            pid,
            event_count: 0,
            start_timestamp_ns: 0,
            end_timestamp_ns: 0,
        }
    }
}
