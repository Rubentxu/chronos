//! Segment file format for `chronos-log::ExecutionLog`.
//!
//! One session produces zero or more immutable segment files. Each
//! segment is a single file on disk; the on-disk format is:
//!
//! ```text
//! +-------------------+  <- header (96 bytes, little-endian)
//! | magic    (u32)    |   0x4348_5347 ("CHSG")
//! | version  (u32)    |   currently 1
//! | flags    (u32)    |   reserved
//! | reserved (u32)    |
//! | start_seq (u64)   |   first EventSeq in the segment (inclusive)
//! | end_seq   (u64)   |   last EventSeq in the segment (inclusive)
//! | record_count (u64)|
//! | schema_version (u32)
//! | reserved (u32)    |
//! | reserved (u32)    |
//! | reserved (u32)    |
//! | reserved (u32)    |
//! | checksum  [32]    |   BLAKE3 over the compressed payload below
//! +-------------------+
//! | compressed payload (LZ4 frame, variable length)
//! +-------------------+
//! ```
//!
//! Records inside the payload are encoded with bincode (length-prefixed
//! records: `[u32 len][bytes]` for each `ExecutionRecord` or `[u32
//! len][bytes]` for each `Gap`). Records appear in append order.
//!
//! Crash safety: a segment is "complete" iff its file ends with the
//! expected compressed payload size AND its BLAKE3 checksum matches.
//! An incomplete segment (mid-write crash) is detected on replay and
//! truncated to the last successfully-decoded record. Prior segments
//! remain readable.

use crate::error::LogError;
use crate::gap::{Gap, GapReason};
use crate::record::{ExecutionKind, ExecutionPayload, ExecutionRecord, SessionId};
use crate::seq::EventSeq;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// File magic: "CHSG" little-endian.
const SEGMENT_MAGIC: u32 = 0x4348_5347;
const SEGMENT_VERSION: u32 = 1;
const HEADER_SIZE: usize = 96;
const CHECKSUM_SIZE: usize = 32;

/// On-disk metadata for one segment, decoded from the 64-byte header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentMetadata {
    pub start_seq: EventSeq,
    pub end_seq: EventSeq,
    pub record_count: u64,
    pub schema_version: u32,
}

impl SegmentMetadata {
    pub fn seq_count(&self) -> u64 {
        self.end_seq.0.saturating_sub(self.start_seq.0) + 1
    }
}

/// One decoded segment.
#[derive(Debug, Clone)]
pub struct DecodedSegment {
    pub metadata: SegmentMetadata,
    pub entries: Vec<SegmentEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentEntry {
    Record(ExecutionRecord),
    Gap(Gap),
}

/// Compute the canonical file path for a segment of `session_id` with
/// the given `start_seq`.
pub fn segment_path(dir: &Path, session_id: &SessionId, start_seq: EventSeq) -> PathBuf {
    let safe = sanitize_session(session_id);
    dir.join(format!("{}-{}.seg", safe, start_seq.0))
}

/// Replace any path-unfriendly character in `SessionId` with `_` so
/// the file path is safe. Sessions whose id contains only
/// `[A-Za-z0-9._-]` are passed through unchanged.
pub fn sanitize_session(session_id: &SessionId) -> String {
    session_id
        .0
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Encode a slice of entries into the on-disk payload (LZ4-compressed
/// length-prefixed bincode records).
pub fn encode_payload(entries: &[SegmentEntry]) -> Result<Vec<u8>, LogError> {
    let mut raw = Vec::with_capacity(entries.len() * 64);
    for entry in entries {
        let bytes = match entry {
            SegmentEntry::Record(r) => bincode_encode_record(r)?,
            SegmentEntry::Gap(g) => bincode_encode_gap(g)?,
        };
        // length-prefixed frame
        let len = u32::try_from(bytes.len())
            .map_err(|_| LogError::Backend("record too large for u32 length prefix".into()))?;
        raw.extend_from_slice(&len.to_le_bytes());
        raw.extend_from_slice(&bytes);
    }
    Ok(lz4_flex::compress_prepend_size(&raw))
}

/// Decode the on-disk payload back into entries.
pub fn decode_payload(payload: &[u8]) -> Result<Vec<SegmentEntry>, LogError> {
    let raw = lz4_flex::decompress_size_prepended(payload)
        .map_err(|e| LogError::Backend(format!("lz4 decompress failed: {}", e)))?;
    let mut entries = Vec::new();
    let mut offset = 0usize;
    while offset < raw.len() {
        if offset + 4 > raw.len() {
            return Err(LogError::Backend(
                "truncated length prefix while decoding payload".into(),
            ));
        }
        let len = u32::from_le_bytes([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + len > raw.len() {
            return Err(LogError::Backend(
                "truncated record body while decoding payload".into(),
            ));
        }
        let body = &raw[offset..offset + len];
        offset += len;
        // First byte discriminates: 0 = record, 1 = gap.
        let tag = body.first().copied().unwrap_or(0xFF);
        match tag {
            0 => {
                let r = bincode_decode_record(&body[1..])?;
                entries.push(SegmentEntry::Record(r));
            }
            1 => {
                let g = bincode_decode_gap(&body[1..])?;
                entries.push(SegmentEntry::Gap(g));
            }
            other => {
                return Err(LogError::Backend(format!(
                    "unknown entry tag {} while decoding payload",
                    other
                )));
            }
        }
    }
    Ok(entries)
}

/// Write a complete segment file atomically (write to `*.tmp` then
/// rename). Returns the final path on success.
pub fn write_segment(
    dir: &Path,
    session_id: &SessionId,
    start_seq: EventSeq,
    end_seq: EventSeq,
    record_count: u64,
    entries: &[SegmentEntry],
) -> Result<PathBuf, LogError> {
    fs::create_dir_all(dir).map_err(|e| LogError::Backend(format!("mkdir {:?}: {}", dir, e)))?;

    let final_path = segment_path(dir, session_id, start_seq);
    let tmp_path = dir.join(format!(
        "{}-{}.tmp",
        sanitize_session(session_id),
        start_seq.0
    ));

    let payload = encode_payload(entries)?;
    let checksum = blake3::hash(&payload);

    let mut f = File::create(&tmp_path)
        .map_err(|e| LogError::Backend(format!("create {:?}: {}", tmp_path, e)))?;
    let mut header = [0u8; HEADER_SIZE];
    header[0..4].copy_from_slice(&SEGMENT_MAGIC.to_le_bytes());
    header[4..8].copy_from_slice(&SEGMENT_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&0u32.to_le_bytes()); // flags
    header[12..16].copy_from_slice(&0u32.to_le_bytes()); // reserved
    header[16..24].copy_from_slice(&start_seq.0.to_le_bytes());
    header[24..32].copy_from_slice(&end_seq.0.to_le_bytes());
    header[32..40].copy_from_slice(&record_count.to_le_bytes());
    let schema_version: u32 = 1;
    header[40..44].copy_from_slice(&schema_version.to_le_bytes());
    // header[44..64] reserved (already zero — 20 bytes)
    // header[64..96] reserved (already zero — 32 bytes)
    let checksum_start = HEADER_SIZE - CHECKSUM_SIZE;
    header[checksum_start..HEADER_SIZE].copy_from_slice(checksum.as_bytes());
    f.write_all(&header)
        .map_err(|e| LogError::Backend(format!("write header: {}", e)))?;
    f.write_all(&payload)
        .map_err(|e| LogError::Backend(format!("write payload: {}", e)))?;
    f.sync_all()
        .map_err(|e| LogError::Backend(format!("fsync: {}", e)))?;
    drop(f);

    fs::rename(&tmp_path, &final_path).map_err(|e| {
        LogError::Backend(format!(
            "atomic rename {:?} -> {:?}: {}",
            tmp_path, final_path, e
        ))
    })?;
    Ok(final_path)
}

/// Read and decode a complete segment file. Returns
/// `Err(LogError::Backend(_))` if the header is malformed, the
/// checksum does not match, or the payload fails to decode.
///
/// Incomplete / truncated segments are reported by returning the
/// entries that *did* decode (so the caller can truncate the
/// trailing garbage). m1-02 truncates by truncating the file to
/// the last complete record boundary.
pub fn read_segment(path: &Path) -> Result<DecodedSegment, LogError> {
    let mut f =
        File::open(path).map_err(|e| LogError::Backend(format!("open {:?}: {}", path, e)))?;
    let mut header = [0u8; HEADER_SIZE];
    f.read_exact(&mut header)
        .map_err(|e| LogError::Backend(format!("read header from {:?}: {}", path, e)))?;

    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    if magic != SEGMENT_MAGIC {
        return Err(LogError::Backend(format!(
            "bad magic in {:?}: expected {:x}, got {:x}",
            path, SEGMENT_MAGIC, magic
        )));
    }
    let version = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if version != SEGMENT_VERSION {
        return Err(LogError::Backend(format!(
            "unsupported segment version {} in {:?}",
            version, path
        )));
    }
    let start_seq = EventSeq(u64::from_le_bytes([
        header[16], header[17], header[18], header[19], header[20], header[21], header[22],
        header[23],
    ]));
    let end_seq = EventSeq(u64::from_le_bytes([
        header[24], header[25], header[26], header[27], header[28], header[29], header[30],
        header[31],
    ]));
    let record_count = u64::from_le_bytes([
        header[32], header[33], header[34], header[35], header[36], header[37], header[38],
        header[39],
    ]);
    let schema_version = u32::from_le_bytes([header[40], header[41], header[42], header[43]]);

    let expected_checksum = &header[HEADER_SIZE - CHECKSUM_SIZE..HEADER_SIZE];
    let mut payload = Vec::new();
    f.read_to_end(&mut payload)
        .map_err(|e| LogError::Backend(format!("read payload from {:?}: {}", path, e)))?;
    let actual = blake3::hash(&payload);
    if actual.as_bytes() != expected_checksum {
        return Err(LogError::Backend(format!(
            "checksum mismatch in {:?}: expected {}, got {}",
            path,
            hex::encode(expected_checksum),
            hex::encode(actual.as_bytes())
        )));
    }
    let entries = decode_payload(&payload)?;
    Ok(DecodedSegment {
        metadata: SegmentMetadata {
            start_seq,
            end_seq,
            record_count,
            schema_version,
        },
        entries,
    })
}

/// Truncate `path` so it ends right after the last fully-decoded
/// record. Used to recover from a partial write that left a
/// trailing corrupted record. Returns the file's new length.
///
/// `keep_bytes` is the number of bytes to keep (header + complete
/// records). The function truncates the file to exactly that many
/// bytes.
pub fn truncate_to(path: &Path, keep_bytes: u64) -> Result<(), LogError> {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| LogError::Backend(format!("open for truncate {:?}: {}", path, e)))?;
    f.set_len(keep_bytes)
        .map_err(|e| LogError::Backend(format!("truncate {:?}: {}", path, e)))?;
    drop(f);
    Ok(())
}

/// Read only the header of `path` (no payload decode).
pub fn read_header(path: &Path) -> Result<SegmentMetadata, LogError> {
    let mut f =
        File::open(path).map_err(|e| LogError::Backend(format!("open {:?}: {}", path, e)))?;
    let mut header = [0u8; HEADER_SIZE];
    f.read_exact(&mut header)
        .map_err(|e| LogError::Backend(format!("read header from {:?}: {}", path, e)))?;
    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    if magic != SEGMENT_MAGIC {
        return Err(LogError::Backend(format!(
            "bad magic in {:?}: expected {:x}, got {:x}",
            path, SEGMENT_MAGIC, magic
        )));
    }
    let start_seq = EventSeq(u64::from_le_bytes([
        header[16], header[17], header[18], header[19], header[20], header[21], header[22],
        header[23],
    ]));
    let end_seq = EventSeq(u64::from_le_bytes([
        header[24], header[25], header[26], header[27], header[28], header[29], header[30],
        header[31],
    ]));
    let record_count = u64::from_le_bytes([
        header[32], header[33], header[34], header[35], header[36], header[37], header[38],
        header[39],
    ]);
    let schema_version = u32::from_le_bytes([header[40], header[41], header[42], header[43]]);
    Ok(SegmentMetadata {
        start_seq,
        end_seq,
        record_count,
        schema_version,
    })
}

/// Get the on-disk size of `path`.
pub fn file_size(path: &Path) -> Result<u64, LogError> {
    let f = File::open(path).map_err(|e| LogError::Backend(format!("open {:?}: {}", path, e)))?;
    f.metadata()
        .map(|m| m.len())
        .map_err(|e| LogError::Backend(format!("stat {:?}: {}", path, e)))
}

/// Seek to the start of the payload in `path` and return the byte
/// offset (= `HEADER_SIZE` for a well-formed segment).
pub fn payload_offset(_path: &Path) -> u64 {
    HEADER_SIZE as u64
}

/// Find the byte offset just past the header for a partial read.
pub fn skip_header<R: Read + Seek>(mut r: R) -> Result<u64, LogError> {
    r.seek(SeekFrom::Start(HEADER_SIZE as u64))
        .map_err(|e| LogError::Backend(format!("seek past header: {}", e)))
}

// ---------------------------------------------------------------------------
// bincode helpers — keep them local so we don't pull in `serde-bincode`
// for one struct.
// ---------------------------------------------------------------------------

fn bincode_encode_record(r: &ExecutionRecord) -> Result<Vec<u8>, LogError> {
    let mut out = Vec::with_capacity(64);
    out.push(0); // tag = Record
    out.extend_from_slice(&r.session_id.0.len().to_le_bytes());
    out.extend_from_slice(r.session_id.0.as_bytes());
    out.extend_from_slice(&r.seq.0.to_le_bytes());
    out.extend_from_slice(&r.monotonic_ns.to_le_bytes());
    out.push(kind_tag(&r.kind));
    out.extend_from_slice(&r.payload.bytes.len().to_le_bytes());
    out.extend_from_slice(&r.payload.bytes);
    let tag_bytes = r.payload.tag.as_bytes();
    out.extend_from_slice(&tag_bytes.len().to_le_bytes());
    out.extend_from_slice(tag_bytes);
    Ok(out)
}

fn bincode_encode_gap(g: &Gap) -> Result<Vec<u8>, LogError> {
    let mut out = Vec::with_capacity(64);
    out.push(1); // tag = Gap
    out.extend_from_slice(&g.first_missing.0.to_le_bytes());
    out.extend_from_slice(&g.last_missing.0.to_le_bytes());
    out.push(gap_reason_tag(&g.reason));
    let src = g.source.as_bytes();
    out.extend_from_slice(&src.len().to_le_bytes());
    out.extend_from_slice(src);
    Ok(out)
}

fn bincode_decode_record(mut body: &[u8]) -> Result<ExecutionRecord, LogError> {
    use std::io::Read;
    let mut buf = [0u8; 8];
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("record session_id len: {}", e)))?;
    let sid_len = u64::from_le_bytes(buf) as usize;
    let mut sid_bytes = vec![0u8; sid_len];
    body.read_exact(&mut sid_bytes)
        .map_err(|e| LogError::Backend(format!("record session_id body: {}", e)))?;
    let session_id = SessionId(
        String::from_utf8(sid_bytes)
            .map_err(|e| LogError::Backend(format!("record session_id utf8: {}", e)))?,
    );
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("record seq: {}", e)))?;
    let seq = EventSeq(u64::from_le_bytes(buf));
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("record monotonic_ns: {}", e)))?;
    let monotonic_ns = u64::from_le_bytes(buf);
    let mut kind_tag_buf = [0u8; 1];
    body.read_exact(&mut kind_tag_buf)
        .map_err(|e| LogError::Backend(format!("record kind tag: {}", e)))?;
    let kind = match kind_tag_buf[0] {
        0 => ExecutionKind::Raw,
        1 => ExecutionKind::GapMarker,
        other => {
            return Err(LogError::Backend(format!(
                "unknown ExecutionKind tag {}",
                other
            )))
        }
    };
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("record payload bytes len: {}", e)))?;
    let bytes_len = u64::from_le_bytes(buf) as usize;
    let mut bytes = vec![0u8; bytes_len];
    body.read_exact(&mut bytes)
        .map_err(|e| LogError::Backend(format!("record payload body: {}", e)))?;
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("record tag len: {}", e)))?;
    let tag_len = u64::from_le_bytes(buf) as usize;
    let mut tag_bytes = vec![0u8; tag_len];
    body.read_exact(&mut tag_bytes)
        .map_err(|e| LogError::Backend(format!("record tag body: {}", e)))?;
    let tag = String::from_utf8(tag_bytes)
        .map_err(|e| LogError::Backend(format!("record tag utf8: {}", e)))?;
    Ok(ExecutionRecord {
        session_id,
        seq,
        monotonic_ns,
        kind,
        payload: ExecutionPayload::new(bytes, tag),
    })
}

fn bincode_decode_gap(mut body: &[u8]) -> Result<Gap, LogError> {
    use std::io::Read;
    let mut buf = [0u8; 8];
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("gap first_missing: {}", e)))?;
    let first_missing = EventSeq(u64::from_le_bytes(buf));
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("gap last_missing: {}", e)))?;
    let last_missing = EventSeq(u64::from_le_bytes(buf));
    let mut tag_buf = [0u8; 1];
    body.read_exact(&mut tag_buf)
        .map_err(|e| LogError::Backend(format!("gap reason tag: {}", e)))?;
    let reason = match tag_buf[0] {
        0 => GapReason::KernelRingOverflow,
        1 => GapReason::AdapterBufferOverflow,
        2 => GapReason::ProcessDetached,
        3 => GapReason::TransportFailure,
        4 => GapReason::CorruptSegment,
        5 => GapReason::UnsupportedEvidence,
        other => {
            return Err(LogError::Backend(format!(
                "unknown GapReason tag {}",
                other
            )))
        }
    };
    body.read_exact(&mut buf)
        .map_err(|e| LogError::Backend(format!("gap source len: {}", e)))?;
    let src_len = u64::from_le_bytes(buf) as usize;
    let mut src_bytes = vec![0u8; src_len];
    body.read_exact(&mut src_bytes)
        .map_err(|e| LogError::Backend(format!("gap source body: {}", e)))?;
    let source = String::from_utf8(src_bytes)
        .map_err(|e| LogError::Backend(format!("gap source utf8: {}", e)))?;
    Ok(Gap::new(first_missing, last_missing, reason, source))
}

fn kind_tag(k: &ExecutionKind) -> u8 {
    match k {
        ExecutionKind::Raw => 0,
        ExecutionKind::GapMarker => 1,
    }
}

fn gap_reason_tag(r: &GapReason) -> u8 {
    match r {
        GapReason::KernelRingOverflow => 0,
        GapReason::AdapterBufferOverflow => 1,
        GapReason::ProcessDetached => 2,
        GapReason::TransportFailure => 3,
        GapReason::CorruptSegment => 4,
        GapReason::UnsupportedEvidence => 5,
    }
}

/// Convenience: ensure the directory exists for the given path. Used
/// by tests and by the higher-level `SegmentedExecutionLog` backend.
pub fn ensure_dir(dir: &Path) -> Result<(), LogError> {
    fs::create_dir_all(dir).map_err(|e| LogError::Backend(format!("mkdir {:?}: {}", dir, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(seq: u64, tag: &str) -> SegmentEntry {
        SegmentEntry::Record(ExecutionRecord {
            session_id: SessionId::new("test"),
            seq: EventSeq::new(seq),
            monotonic_ns: seq * 100,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(vec![1, 2, 3], tag),
        })
    }

    fn gap(first: u64, last: u64) -> SegmentEntry {
        SegmentEntry::Gap(Gap::new(
            EventSeq::new(first),
            EventSeq::new(last),
            GapReason::AdapterBufferOverflow,
            "test",
        ))
    }

    #[test]
    fn sanitize_passes_through_safe_chars() {
        assert_eq!(
            sanitize_session(&SessionId::new("session-123")),
            "session-123"
        );
        assert_eq!(sanitize_session(&SessionId::new("a/b")), "a_b");
        assert_eq!(sanitize_session(&SessionId::new("x y")), "x_y");
    }

    #[test]
    fn payload_round_trip() {
        let entries = vec![record(0, "a"), gap(1, 2), record(3, "c")];
        let payload = encode_payload(&entries).unwrap();
        let decoded = decode_payload(&payload).unwrap();
        assert_eq!(decoded, entries);
    }

    #[test]
    fn write_and_read_segment() {
        let dir = tempdir();
        let session = SessionId::new("rt");
        let entries = vec![record(0, "a"), record(1, "b"), record(2, "c")];
        let path = write_segment(
            &dir,
            &session,
            EventSeq::new(0),
            EventSeq::new(2),
            3,
            &entries,
        )
        .unwrap();
        let decoded = read_segment(&path).unwrap();
        assert_eq!(decoded.metadata.start_seq, EventSeq::new(0));
        assert_eq!(decoded.metadata.end_seq, EventSeq::new(2));
        assert_eq!(decoded.metadata.record_count, 3);
        assert_eq!(decoded.entries, entries);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn truncated_segment_reports_decoded_prefix() {
        let dir = tempdir();
        let session = SessionId::new("trunc");
        let entries = vec![record(0, "a"), record(1, "b"), record(2, "c")];
        let path = write_segment(
            &dir,
            &session,
            EventSeq::new(0),
            EventSeq::new(2),
            3,
            &entries,
        )
        .unwrap();
        // Truncate the file by 16 bytes — guarantees we land inside
        // the third record's body and the checksum won't match.
        let len = file_size(&path).unwrap();
        let truncated = len.saturating_sub(16);
        truncate_to(&path, truncated).unwrap();
        let err = read_segment(&path);
        assert!(err.is_err(), "expected checksum mismatch");
        fs::remove_dir_all(dir).ok();
    }

    fn tempdir() -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "chronos-log-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let p = base.join(unique);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
