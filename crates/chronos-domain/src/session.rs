//! Session-level domain types.

use serde::{Deserialize, Serialize};

/// Metadata for a saved session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub session_id: String,
    /// Unix timestamp ms when the session was created.
    pub created_at: u64,
    /// Language/runtime: "python", "java", "go", "native".
    pub language: String,
    /// Target program path or name.
    pub target: String,
    /// Total number of events stored.
    pub event_count: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// True after a v2 `session_stop{seal_tail=true}` call (m7-04).
    /// Default `false`; old metadata files load with `false`.
    #[serde(default)]
    pub tail_sealed: bool,
    /// Wall-clock timestamp (ms) when the session was sealed.
    /// Only set when `tail_sealed=true`. Default `None`.
    #[serde(default)]
    pub sealed_at: Option<u64>,
}
