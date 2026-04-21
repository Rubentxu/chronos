//! Trace file reader and writer.

use crate::{decode, encode, TraceHeader};
use chronos_domain::TraceEvent;
use std::io::{self, BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;

// File format:
// [header_len: u64 LE] [header: bincode] [event1_len: u64 LE] [event1: bincode] ...

/// Writes trace events to a .chronos file.
pub struct TraceFileWriter {
    writer: BufWriter<std::fs::File>,
    header: TraceHeader,
    event_count: u64,
    min_timestamp: Option<u64>,
    max_timestamp: Option<u64>,
}

impl TraceFileWriter {
    /// Create a new trace file writer.
    pub fn create(path: &Path, header: TraceHeader) -> io::Result<Self> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header placeholder (will be overwritten on finalize)
        let header_data = encode::encode_header(&header);
        let header_len = header_data.len() as u64;
        writer.write_all(&header_len.to_le_bytes())?;
        writer.write_all(&header_data)?;
        writer.flush()?;

        Ok(Self {
            writer,
            header,
            event_count: 0,
            min_timestamp: None,
            max_timestamp: None,
        })
    }

    /// Write a single event to the trace file.
    pub fn write_event(&mut self, event: &TraceEvent) -> io::Result<()> {
        encode::write_event(&mut self.writer, event)?;
        self.event_count += 1;

        let ts = event.timestamp_ns;
        self.min_timestamp = Some(self.min_timestamp.map_or(ts, |m: u64| m.min(ts)));
        self.max_timestamp = Some(self.max_timestamp.map_or(ts, |m: u64| m.max(ts)));

        // Flush every 1000 events to avoid large memory buffers
        if self.event_count.is_multiple_of(1000) {
            self.writer.flush()?;
        }

        Ok(())
    }

    /// Write multiple events.
    pub fn write_events(&mut self, events: &[TraceEvent]) -> io::Result<()> {
        for event in events {
            self.write_event(event)?;
        }
        Ok(())
    }

    /// Finalize the trace file: rewrite header with statistics and flush.
    pub fn finalize(mut self) -> io::Result<TraceHeader> {
        self.writer.flush()?;

        let mut header = self.header;
        header.event_count = self.event_count;
        header.start_timestamp_ns = self.min_timestamp.unwrap_or(0);
        header.end_timestamp_ns = self.max_timestamp.unwrap_or(0);

        // Seek back to start and rewrite the header with updated stats
        let header_data = encode::encode_header(&header);
        let header_len = header_data.len() as u64;
        {
            let file = self.writer.get_mut();
            file.seek(std::io::SeekFrom::Start(0))?;
            file.write_all(&header_len.to_le_bytes())?;
            file.write_all(&header_data)?;
            file.flush()?;
        }

        Ok(header)
    }

    /// Returns the number of events written so far.
    pub fn event_count(&self) -> u64 {
        self.event_count
    }
}

/// Reads trace events from a .chronos file.
#[derive(Debug)]
pub struct TraceFileReader {
    header: TraceHeader,
    events_offset: u64,
}

impl TraceFileReader {
    /// Open a trace file and read its header.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read header length
        let mut len_buf = [0u8; 8];
        reader.read_exact(&mut len_buf)?;
        let header_len = u64::from_le_bytes(len_buf) as usize;

        // Read header
        let mut header_data = vec![0u8; header_len];
        reader.read_exact(&mut header_data)?;

        let header = decode::decode_header(&header_data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Validate magic
        if header.magic != crate::CHRONOS_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid magic: expected {:?}, got {:?}",
                    crate::CHRONOS_MAGIC,
                    header.magic
                ),
            ));
        }

        let events_offset = 8 + header_len as u64;

        Ok(Self {
            header,
            events_offset,
        })
    }

    /// Get the trace header.
    pub fn header(&self) -> &TraceHeader {
        &self.header
    }

    /// Read all events from the file.
    pub fn read_all_events(&self) -> io::Result<Vec<TraceEvent>> {
        let file = std::fs::File::open(self.get_path()?)?;
        let mut reader = BufReader::new(file);

        // Skip to events
        let mut skip_buf = vec![0u8; self.events_offset as usize];
        reader.read_exact(&mut skip_buf)?;

        let mut events = Vec::with_capacity(self.header.event_count as usize);
        while let Some(event) = decode::read_event(&mut reader)? {
            events.push(event);
        }

        Ok(events)
    }

    /// Read events within a specific ID range.
    pub fn read_event_range(&self, start_id: u64, end_id: u64) -> io::Result<Vec<TraceEvent>> {
        let all = self.read_all_events()?;
        Ok(all
            .into_iter()
            .filter(|e| e.event_id >= start_id && e.event_id < end_id)
            .collect())
    }

    fn get_path(&self) -> io::Result<&Path> {
        // We need to store the path. For now, this is a limitation.
        // In practice, the caller keeps the path.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Use open_with_path instead",
        ))
    }
}

/// Extended reader that stores the file path.
pub struct TraceFileReaderWithMetadata {
    reader: TraceFileReader,
    path: std::path::PathBuf,
}

impl TraceFileReaderWithMetadata {
    /// Open a trace file.
    pub fn open(path: &Path) -> io::Result<Self> {
        let reader = TraceFileReader::open(path)?;
        Ok(Self {
            reader,
            path: path.to_path_buf(),
        })
    }

    /// Get the trace header.
    pub fn header(&self) -> &TraceHeader {
        self.reader.header()
    }

    /// Read all events.
    pub fn read_all_events(&self) -> io::Result<Vec<TraceEvent>> {
        let file = std::fs::File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        let mut skip_buf = vec![0u8; self.reader.events_offset as usize];
        reader.read_exact(&mut skip_buf)?;

        let mut events = Vec::new();
        while let Some(event) = decode::read_event(&mut reader)? {
            events.push(event);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{EventData, EventType, SourceLocation};
    use tempfile::NamedTempFile;

    fn make_event(id: u64, ts: u64) -> TraceEvent {
        TraceEvent::new(
            id,
            ts,
            1,
            EventType::FunctionEntry,
            SourceLocation::new("test.rs", id as u32, "func", 0x1000 + id),
            EventData::Empty,
        )
    }

    fn temp_path(tag: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("test_{}.chronos", tag));
        (dir, path)
    }

    #[test]
    fn test_write_and_read_trace_file() {
        let (_dir, path) = temp_path("basic");

        // Write
        let header = TraceHeader::new("s1", "rust", "./test", 42);
        let mut writer = TraceFileWriter::create(&path, header).unwrap();

        writer.write_event(&make_event(0, 100)).unwrap();
        writer.write_event(&make_event(1, 200)).unwrap();
        writer.write_event(&make_event(2, 300)).unwrap();

        let final_header = writer.finalize().unwrap();
        assert_eq!(final_header.event_count, 3);
        assert_eq!(final_header.start_timestamp_ns, 100);
        assert_eq!(final_header.end_timestamp_ns, 300);

        // Read
        let reader = TraceFileReaderWithMetadata::open(&path).unwrap();
        assert_eq!(reader.header().session_id, "s1");
        assert_eq!(reader.header().event_count, 3);

        // Debug: check file size
        let file_meta = std::fs::metadata(&path).unwrap();
        eprintln!(
            "DEBUG: file size = {} bytes, events_offset = {}",
            file_meta.len(),
            reader.reader.events_offset
        );

        let events = reader.read_all_events().unwrap();
        eprintln!("DEBUG: events.len() = {}", events.len());
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, 0);
        assert_eq!(events[1].timestamp_ns, 200);
        assert_eq!(events[2].event_id, 2);
    }

    #[test]
    fn test_write_many_events() {
        let (_dir, path) = temp_path("many");

        let header = TraceHeader::new("s2", "c", "./big_test", 99);
        let mut writer = TraceFileWriter::create(&path, header).unwrap();

        for i in 0..5000 {
            writer.write_event(&make_event(i, i as u64 * 100)).unwrap();
        }
        let final_header = writer.finalize().unwrap();
        assert_eq!(final_header.event_count, 5000);

        let reader = TraceFileReaderWithMetadata::open(&path).unwrap();
        let events = reader.read_all_events().unwrap();
        assert_eq!(events.len(), 5000);
    }

    #[test]
    fn test_invalid_magic() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write a valid header structure but with wrong magic
        let mut bad_header = crate::TraceHeader::new("s", "c", "./t", 1);
        bad_header.magic = [b'X', b'X', b'X', b'X']; // Invalid magic
        let header_data = bincode::serialize(&bad_header).unwrap();
        let header_len = header_data.len() as u64;

        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&header_len.to_le_bytes()).unwrap();
        file.write_all(&header_data).unwrap();

        let result = TraceFileReader::open(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid magic"));
    }
}
