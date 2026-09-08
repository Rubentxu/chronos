//! Versioned projection checkpoints.
//!
//! Per ADR-0003 / ADR-0011, graphs and indexes are **versioned
//! projections rebuilt from the ExecutionLog plus optional checkpoints**.
//! This module persists a derived `CallGraph` as a versioned, blake3
//! checksummed JSON artifact so a session need not be fully re-read to
//! recover a projection, and so a future format change can be detected
//! instead of silently mis-parsed.
//!
//! Only full-snapshot checkpoints are supported (no delta/incremental).
//! There is no auto-load on `SegmentedExecutionLog::open` — the v1
//! surface is write / read / replay-equivalence only.
//!
//! Concurrency / durability: the write is atomic (write a `*.tmp` file
//! then rename), mirroring `segment::write_segment`. The blake3 checksum
//! is computed over the canonical JSON body (the envelope without the
//! checksum), so any tampering with the stored graph or version is
//! detected on read.

use crate::call_graph::{call_graph, CallGraph};
use crate::error::LogError;
use crate::memory::InMemoryExecutionLog;
use crate::record::SessionId;

use serde::{Deserialize, Serialize};

/// Current on-disk checkpoint format version.
pub const CALL_GRAPH_CHECKPOINT_VERSION: u32 = 1;

/// File suffix for call-graph checkpoints.
pub const CHECKPOINT_EXTENSION: &str = "chronos-callgraph.json";

/// On-disk JSON envelope: the checksum is kept out of the canonical body
/// so `checksum` can be recomputed over the body independently.
#[derive(Debug, Serialize, Deserialize)]
struct CheckpointBody {
    format_version: u32,
    session: SessionId,
    graph: CallGraph,
}

/// Full on-disk file: body + hex checksum of the body's canonical JSON.
#[derive(Debug, Serialize, Deserialize)]
struct CheckpointFile {
    #[serde(flatten)]
    body: CheckpointBody,
    checksum_hex: String,
}

/// A parsed and integrity-verified call-graph checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphCheckpoint {
    /// Format version this checkpoint was written as.
    pub format_version: u32,
    /// Session the checkpointed graph was derived from.
    pub session: SessionId,
    /// The persisted call graph.
    pub graph: CallGraph,
    /// blake3 checksum over the canonical JSON body.
    pub checksum: [u8; 32],
}

/// A stable file name for a session's call-graph checkpoint.
pub fn checkpoint_path(dir: &std::path::Path, session: &SessionId) -> std::path::PathBuf {
    dir.join(format!("{}.{}", session.as_str(), CHECKPOINT_EXTENSION))
}

fn canonical_body_json(body: &CheckpointBody) -> Result<Vec<u8>, LogError> {
    serde_json::to_vec(body).map_err(|e| LogError::Backend(format!("serialize checkpoint: {}", e)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Persist `graph` for `session` as a versioned, checksummed JSON
/// artifact in `dir`. Writes atomically (`*.tmp` then rename) and
/// returns the final path.
///
/// See REQ-CheckpointWrite.
pub fn write_call_graph_checkpoint(
    dir: &std::path::Path,
    session: &SessionId,
    graph: &CallGraph,
) -> Result<std::path::PathBuf, LogError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| LogError::Backend(format!("mkdir {:?}: {}", dir, e)))?;

    let body = CheckpointBody {
        format_version: CALL_GRAPH_CHECKPOINT_VERSION,
        session: session.clone(),
        graph: graph.clone(),
    };
    let body_bytes = canonical_body_json(&body)?;
    let checksum = blake3::hash(&body_bytes);

    let file = CheckpointFile {
        body,
        checksum_hex: hex(checksum.as_bytes()),
    };
    let file_bytes = serde_json::to_vec(&file)
        .map_err(|e| LogError::Backend(format!("serialize checkpoint file: {}", e)))?;

    let final_path = checkpoint_path(dir, session);
    let tmp_path = dir.join(format!("{}.tmp", session.as_str()));

    std::fs::write(&tmp_path, file_bytes)
        .map_err(|e| LogError::Backend(format!("write {:?}: {}", tmp_path, e)))?;
    std::fs::rename(&tmp_path, &final_path).map_err(|e| {
        LogError::Backend(format!(
            "atomic rename {:?} -> {:?}: {}",
            tmp_path, final_path, e
        ))
    })?;
    Ok(final_path)
}

/// Read and verify a call-graph checkpoint at `path`.
///
/// Returns `Err` if the file is missing / unreadable / not valid JSON,
/// if the stored `format_version` is not
/// `CALL_GRAPH_CHECKPOINT_VERSION`, or if the recomputed checksum does
/// not match the stored one.
///
/// See REQ-CheckpointRead.
pub fn read_call_graph_checkpoint(path: &std::path::Path) -> Result<CallGraphCheckpoint, LogError> {
    let bytes =
        std::fs::read(path).map_err(|e| LogError::Backend(format!("read {:?}: {}", path, e)))?;
    let file: CheckpointFile = serde_json::from_slice(&bytes)
        .map_err(|e| LogError::Backend(format!("parse {:?}: {}", path, e)))?;

    if file.body.format_version != CALL_GRAPH_CHECKPOINT_VERSION {
        return Err(LogError::Backend(format!(
            "unsupported checkpoint format version {} (expected {})",
            file.body.format_version, CALL_GRAPH_CHECKPOINT_VERSION
        )));
    }

    let body_bytes = canonical_body_json(&file.body)?;
    let recomputed = blake3::hash(&body_bytes);
    if hex(recomputed.as_bytes()) != file.checksum_hex {
        return Err(LogError::Backend(format!(
            "checkpoint checksum mismatch on {:?}",
            path
        )));
    }

    Ok(CallGraphCheckpoint {
        format_version: file.body.format_version,
        session: file.body.session,
        graph: file.body.graph,
        checksum: *recomputed.as_bytes(),
    })
}

/// Replay-equivalence check: re-derive the graph for `session` from
/// `log` and report whether it equals `checkpoint.graph`.
///
/// See REQ-ReplayEquivalence.
pub fn graph_matches_checkpoint(
    log: &InMemoryExecutionLog,
    session: &SessionId,
    checkpoint: &CallGraphCheckpoint,
) -> bool {
    call_graph(log, session) == checkpoint.graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_graph::call_graph as derive_graph;
    use crate::record::{ExecutionKind, ExecutionPayload};
    use crate::seq::EventSeq;
    use chronos_domain::{InvocationId, Language, SymbolId};

    fn sid(name: &str) -> SymbolId {
        SymbolId::new(name, None, Language::Rust)
    }

    fn mk(
        session: &SessionId,
        seq: u64,
        inv: Option<InvocationId>,
        parent: Option<InvocationId>,
        sym: Option<SymbolId>,
    ) -> crate::record::ExecutionRecord {
        crate::record::ExecutionRecord {
            session_id: session.clone(),
            seq: EventSeq::new(seq),
            monotonic_ns: seq * 10,
            kind: ExecutionKind::Raw,
            payload: ExecutionPayload::new(Vec::new(), "ev"),
            invocation_id: inv,
            parent_invocation_id: parent,
            symbol_id: sym,
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("chronos-m2-05-{}-{}", tag, std::process::id()))
    }

    fn sample_graph() -> (InMemoryExecutionLog, SessionId, CallGraph) {
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("ckpt-session");
        let a = InvocationId::now();
        let b = InvocationId::now();
        let sa = sid("a");
        let sb = sid("b");
        log.replay_record(&mk(&s, 0, Some(a), None, Some(sa)))
            .unwrap();
        log.replay_record(&mk(&s, 1, Some(b), Some(a), Some(sb)))
            .unwrap();
        let g = derive_graph(&log, &s);
        (log, s, g)
    }

    #[test]
    fn round_trip_non_empty_graph() {
        let dir = temp_dir("roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let (log, s, g) = sample_graph();

        let path = write_call_graph_checkpoint(&dir, &s, &g).unwrap();
        assert!(path.exists(), "checkpoint file exists");
        let ckpt = read_call_graph_checkpoint(&path).unwrap();
        assert_eq!(ckpt.format_version, CALL_GRAPH_CHECKPOINT_VERSION);
        assert_eq!(ckpt.session, s);
        assert_eq!(ckpt.graph, g);
        // Replay-equivalence holds on the unmodified log.
        assert!(graph_matches_checkpoint(&log, &s, &ckpt));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trip_empty_graph() {
        let dir = temp_dir("empty");
        std::fs::create_dir_all(&dir).unwrap();
        let log = InMemoryExecutionLog::new();
        let s = SessionId::new("ckpt-empty");
        let g = derive_graph(&log, &s);
        assert!(g.edges().is_empty());

        let path = write_call_graph_checkpoint(&dir, &s, &g).unwrap();
        let ckpt = read_call_graph_checkpoint(&path).unwrap();
        assert!(ckpt.graph.edges().is_empty());
        assert!(graph_matches_checkpoint(&log, &s, &ckpt));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unsupported_version_errors() {
        let dir = temp_dir("badver");
        std::fs::create_dir_all(&dir).unwrap();
        let (_, s, g) = sample_graph();
        let path = write_call_graph_checkpoint(&dir, &s, &g).unwrap();

        // Rewrite the file with an unsupported version by editing JSON.
        let raw = std::fs::read_to_string(&path).unwrap();
        let tampered = raw.replacen("\"format_version\":1", "\"format_version\":99", 1);
        std::fs::write(&path, tampered).unwrap();
        let err = read_call_graph_checkpoint(&path).unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported checkpoint format version 99"),
            "got: {}",
            err
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tampered_checksum_errors() {
        let dir = temp_dir("tamper");
        std::fs::create_dir_all(&dir).unwrap();
        let (_, s, g) = sample_graph();
        let path = write_call_graph_checkpoint(&dir, &s, &g).unwrap();

        // Flip a byte in the stored JSON (mutate an edge count). Reading
        // must fail the checksum check, not silently return a wrong graph.
        let raw = std::fs::read_to_string(&path).unwrap();
        let idx = raw.find("\"calls\":1").unwrap();
        // `"calls"` is 7 bytes; `:` is index 7; the `1` is index 8.
        let mut bytes = raw.into_bytes();
        bytes[idx + 8] = b'2'; // 1 -> 2, still valid JSON
        std::fs::write(&path, bytes).unwrap();
        let err = read_call_graph_checkpoint(&path).unwrap_err();
        assert!(
            err.to_string().contains("checksum mismatch"),
            "got: {}",
            err
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn replay_equivalence_false_after_growth() {
        let dir = temp_dir("growth");
        std::fs::create_dir_all(&dir).unwrap();
        let (log, s, g) = sample_graph();
        let path = write_call_graph_checkpoint(&dir, &s, &g).unwrap();
        let ckpt = read_call_graph_checkpoint(&path).unwrap();
        assert!(graph_matches_checkpoint(&log, &s, &ckpt));

        // Append a new root c; graph now has an extra node/edge.
        let c = InvocationId::now();
        let sc = sid("c");
        log.replay_record(&mk(&s, 99, Some(c), None, Some(sc)))
            .unwrap();
        assert!(
            !graph_matches_checkpoint(&log, &s, &ckpt),
            "old checkpoint must not match a grown log"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
