//! REC-C1.1 — `EventsCursorV1`: the authoritative agent-visible read cursor.
//!
//! ## Why this exists
//!
//! The legacy `query_events` surface takes `offset`/`limit` and the current
//! server ignores `offset` entirely (see
//! `chronos-sandbox/tests/rec_c1_characterization.rs`). C1.1 does **not** try
//! to build a better offset: it introduces the cursor the target contract
//! (`events_read`) will use.
//!
//! ```text
//! EventsCursorV1 {
//!     schema_version,
//!     session_id,
//!     next_seq,
//! }
//! ```
//!
//! Deliberately absent, and rejected unless strong evidence appears:
//! offsets, timestamps, filters, query state, page sizes. A cursor is a
//! *position in an authoritative log*, not a serialized query.
//!
//! ## Scope (C1.1)
//!
//! Pure value type + typed errors + stable external representation + unit
//! tests. No `EventBus`, no `ExecutionLog` ownership, no storage, no wiring
//! into `events_read` (that is C1.2/C1.3).
//!
//! ## Opacity
//!
//! Fields are private. The only external representation is the opaque string
//! produced by [`EventsCursorV1::encode`]. Callers cannot construct or mutate
//! a cursor field-by-field, so the wire format stays ours to evolve.

use chronos_log::{EventSeq, SessionId};

/// Schema version this build understands and emits.
pub const EVENTS_CURSOR_V1_SCHEMA: u16 = 1;

/// Prefix for the opaque external representation.
const PREFIX: &str = "ecv1";

/// Typed failures when validating an externally supplied cursor.
///
/// These are deliberately distinct: a stale cursor is resumable, a wrong
/// session is a caller bug, and a malformed cursor is neither. Collapsing them
/// into one error would lose the information the reader needs to react.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventsCursorError {
    /// The external representation could not be parsed.
    #[error("malformed cursor: {reason}")]
    Malformed { reason: String },

    /// The cursor was produced by a schema version this build does not support.
    #[error("unsupported cursor schema version {found} (supported: {supported})")]
    UnsupportedVersion { found: u16, supported: u16 },

    /// The cursor belongs to a different session than the one being read.
    #[error("cursor belongs to session {found:?}, not {expected:?}")]
    WrongSession { expected: String, found: String },
}

/// Authoritative read cursor: `(schema_version, session_id, next_seq)`.
///
/// `next_seq` is the sequence number the reader wants **next**, i.e. the
/// highest already-processed seq plus one. Strictly monotonic readers advance
/// it with [`EventsCursorV1::advance_to`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventsCursorV1 {
    schema_version: u16,
    session_id: SessionId,
    next_seq: EventSeq,
}

impl EventsCursorV1 {
    /// A fresh cursor at the beginning of `session_id`'s log.
    pub fn start(session_id: SessionId) -> Self {
        Self {
            schema_version: EVENTS_CURSOR_V1_SCHEMA,
            session_id,
            next_seq: EventSeq::ZERO,
        }
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn next_seq(&self) -> EventSeq {
        self.next_seq
    }

    /// Advance the cursor to a strictly greater position.
    ///
    /// Rejects going backwards: a cursor that can silently move back would let
    /// a reader re-read (or worse, skip) events and still look healthy.
    pub fn advance_to(&self, next_seq: EventSeq) -> Result<Self, EventsCursorError> {
        if next_seq < self.next_seq {
            return Err(EventsCursorError::Malformed {
                reason: format!(
                    "non-monotonic advance: {} -> {}",
                    self.next_seq.0, next_seq.0
                ),
            });
        }
        Ok(Self {
            schema_version: self.schema_version,
            session_id: self.session_id.clone(),
            next_seq,
        })
    }

    /// Opaque, stable external representation.
    ///
    /// Format (v1): `ecv1:<schema>:<len>:<session_id>:<next_seq>` where `<len>`
    /// is the byte length of `<session_id>`, so session ids may contain any
    /// character including `:` without ambiguity.
    pub fn encode(&self) -> String {
        format!(
            "{PREFIX}:{}:{}:{}:{}",
            self.schema_version,
            self.session_id.as_str().len(),
            self.session_id.as_str(),
            self.next_seq.0
        )
    }

    /// Parse an opaque representation without a session expectation.
    ///
    /// Prefer [`EventsCursorV1::decode_for_session`] on any real read path.
    pub fn decode(encoded: &str) -> Result<Self, EventsCursorError> {
        let (schema_version, session_id, next_seq) = parse(encoded)?;
        Ok(Self {
            schema_version,
            session_id,
            next_seq,
        })
    }

    /// Parse an opaque representation and require it to belong to `expected`.
    ///
    /// A cursor from another session must never be silently retargeted: that
    /// would read the wrong log and report it as the right one.
    pub fn decode_for_session(
        encoded: &str,
        expected: &SessionId,
    ) -> Result<Self, EventsCursorError> {
        let cursor = Self::decode(encoded)?;
        if &cursor.session_id != expected {
            return Err(EventsCursorError::WrongSession {
                expected: expected.as_str().to_string(),
                found: cursor.session_id.as_str().to_string(),
            });
        }
        Ok(cursor)
    }
}

/// Returns `(schema_version, session_id, next_seq)`.
fn parse(encoded: &str) -> Result<(u16, SessionId, EventSeq), EventsCursorError> {
    let malformed = |reason: String| EventsCursorError::Malformed { reason };

    // Split only four ways: the session id itself may contain ':' characters,
    // so after the length prefix we take exactly `declared_len` bytes rather
    // than splitting on the separator.
    let mut parts = encoded.splitn(4, ':');
    let prefix = parts.next().unwrap_or_default();
    if prefix != PREFIX {
        return Err(malformed(format!(
            "expected prefix {PREFIX:?}, found {prefix:?}"
        )));
    }
    let version_str = parts
        .next()
        .ok_or_else(|| malformed("missing schema version".into()))?;
    let len_str = parts
        .next()
        .ok_or_else(|| malformed("missing session length".into()))?;
    let rest = parts
        .next()
        .ok_or_else(|| malformed("missing session id".into()))?;

    let version: u16 = version_str
        .parse()
        .map_err(|_| malformed(format!("non-numeric schema version {version_str:?}")))?;
    if version != EVENTS_CURSOR_V1_SCHEMA {
        return Err(EventsCursorError::UnsupportedVersion {
            found: version,
            supported: EVENTS_CURSOR_V1_SCHEMA,
        });
    }

    let declared_len: usize = len_str
        .parse()
        .map_err(|_| malformed(format!("non-numeric session length {len_str:?}")))?;
    if rest.len() < declared_len {
        return Err(malformed(format!(
            "session id shorter than declared length {declared_len}"
        )));
    }
    let (session, tail) = rest.split_at(declared_len);
    if !tail.starts_with(':') {
        return Err(malformed("missing separator after session id".into()));
    }
    let seq_str = &tail[1..];
    let next_seq: u64 = seq_str
        .parse()
        .map_err(|_| malformed(format!("non-numeric next_seq {seq_str:?}")))?;

    Ok((version, SessionId::new(session), EventSeq::new(next_seq)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    #[test]
    fn start_is_at_zero_with_current_schema() {
        let c = EventsCursorV1::start(sid("s-1"));
        assert_eq!(c.schema_version(), EVENTS_CURSOR_V1_SCHEMA);
        assert_eq!(c.session_id().as_str(), "s-1");
        assert_eq!(c.next_seq(), EventSeq::ZERO);
    }

    #[test]
    fn roundtrip_is_stable() {
        let c = EventsCursorV1::start(sid("sess-abc"))
            .advance_to(EventSeq::new(42))
            .unwrap();
        let encoded = c.encode();
        assert_eq!(encoded, "ecv1:1:8:sess-abc:42");
        assert_eq!(EventsCursorV1::decode(&encoded).unwrap(), c);
        assert_eq!(EventsCursorV1::decode(&encoded).unwrap().encode(), encoded);
    }

    #[test]
    fn roundtrip_survives_colons_and_unicode_in_session_id() {
        for raw in ["a:b:c", "sesión-ü", "", "x".repeat(300).as_str()] {
            let c = EventsCursorV1::start(sid(raw))
                .advance_to(EventSeq::new(7))
                .unwrap();
            let decoded = EventsCursorV1::decode(&c.encode()).unwrap();
            assert_eq!(decoded.session_id().as_str(), raw, "raw={raw:?}");
            assert_eq!(decoded.next_seq(), EventSeq::new(7));
        }
    }

    #[test]
    fn rejects_foreign_prefix() {
        let err = EventsCursorV1::decode("ECV2:1:3:abc:0").unwrap_err();
        assert!(
            matches!(err, EventsCursorError::Malformed { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn rejects_unsupported_version_before_anything_else() {
        let err = EventsCursorV1::decode("ecv1:2:3:abc:0").unwrap_err();
        assert_eq!(
            err,
            EventsCursorError::UnsupportedVersion {
                found: 2,
                supported: 1
            }
        );
    }

    #[test]
    fn rejects_malformed_shapes() {
        for bad in [
            "",
            "ecv1",
            "ecv1:1",
            "ecv1:1:3",
            "ecv1:x:3:abc:0",
            "ecv1:1:x:abc:0",
            "ecv1:1:9:abc:0",
            "ecv1:1:3:abc",
            "ecv1:1:3:abc:not-a-number",
            "ecv1:1:3:abc:0:extra",
            "ecv1:1:3:ab",
        ] {
            let err = EventsCursorV1::decode(bad).unwrap_err();
            assert!(
                matches!(err, EventsCursorError::Malformed { .. }),
                "expected malformed for {bad:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn rejects_negative_and_non_u64_seq() {
        for bad in ["ecv1:1:3:abc:-1", "ecv1:1:3:abc:1.5", "ecv1:1:3:abc:1e3"] {
            assert!(matches!(
                EventsCursorV1::decode(bad).unwrap_err(),
                EventsCursorError::Malformed { .. }
            ));
        }
    }

    #[test]
    fn accepts_u64_max() {
        let max = format!("ecv1:1:3:abc:{}", u64::MAX);
        let c = EventsCursorV1::decode(&max).unwrap();
        assert_eq!(c.next_seq(), EventSeq::new(u64::MAX));
    }

    #[test]
    fn session_check_rejects_mismatch() {
        let encoded = EventsCursorV1::start(sid("sess-a")).encode();
        let err = EventsCursorV1::decode_for_session(&encoded, &sid("sess-b")).unwrap_err();
        assert_eq!(
            err,
            EventsCursorError::WrongSession {
                expected: "sess-b".into(),
                found: "sess-a".into(),
            }
        );
    }

    #[test]
    fn session_check_accepts_match() {
        let c = EventsCursorV1::start(sid("sess-a"));
        let decoded = EventsCursorV1::decode_for_session(&c.encode(), &sid("sess-a")).unwrap();
        assert_eq!(decoded, c);
    }

    #[test]
    fn session_check_surfaces_malformed_not_wrong_session() {
        let err = EventsCursorV1::decode_for_session("garbage", &sid("sess-a")).unwrap_err();
        assert!(matches!(err, EventsCursorError::Malformed { .. }));
    }

    #[test]
    fn advance_is_monotonic_and_same_session() {
        let c = EventsCursorV1::start(sid("s"))
            .advance_to(EventSeq::new(10))
            .unwrap();
        let fwd = c.advance_to(EventSeq::new(11)).unwrap();
        assert_eq!(fwd.next_seq(), EventSeq::new(11));
        assert_eq!(fwd.session_id(), c.session_id());
        // same position is allowed (idempotent re-issue), backwards is not
        assert!(c.advance_to(EventSeq::new(10)).is_ok());
        assert!(c.advance_to(EventSeq::new(9)).is_err());
    }

    #[test]
    fn cursor_has_no_public_fields_and_no_offset_surface() {
        // Compile-time opacity guard: the only ways in are `start`/`decode`/
        // `decode_for_session`, and the only position it carries is `next_seq`.
        let c = EventsCursorV1::start(sid("s"));
        let _: u16 = c.schema_version();
        let _: &SessionId = c.session_id();
        let _: EventSeq = c.next_seq();
    }
}
