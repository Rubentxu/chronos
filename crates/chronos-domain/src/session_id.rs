//! Session identity.
//!
//! `SessionId` is the single canonical session-identity newtype.
//! It lives in domain because (a) the application ports need to refer
//! to sessions without depending on a concrete backend, and (b)
//! `chronos_log` already depends on `chronos_domain` for
//! `InvocationId` and `SymbolId`, so the direction is clean.
//!
//! REC-C3.3.1 (this cycle) promotes the wire/persistence derives
//! (`Serialize`, `Deserialize`, `Default`) onto the canonical type so
//! `chronos_log` can re-export it via `pub use` and the on-disk
//! segment encoder can read the inner string through the accessor
//! (`as_str`/`as_bytes`) instead of touching the field directly.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Canonical session identity.
///
/// Single newtype. All call sites — application ports, log adapters,
/// wire cursors, native backends — resolve to this definition.
///
/// Derives are the full set the previous
/// `chronos_log::record::SessionId` carried, plus the new
/// `Serialize` / `Deserialize` / `Default` so the canonical type can
/// stand in for the duplicate one. Field stays private; persistence
/// and on-disk encoding reach the inner string through
/// [`SessionId::as_str`] / [`SessionId::as_bytes`] / [`SessionId::into_inner`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SessionId(String);

impl SessionId {
    /// Wrap a `String` as a `SessionId`. No validation: the call site
    /// is responsible for choosing a unique string.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow the underlying bytes (UTF-8 view of the inner string).
    /// Added by REC-C3.3.1 so the on-disk segment encoder can write
    /// the session id without touching the private field.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Consume the `SessionId` and return the inner `String`.
    /// Useful for callers that need an owned handle to the identity
    /// (path construction, diagnostic formatting, JSON output).
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_string() {
        let id: SessionId = "sess-123".into();
        assert_eq!(id.as_str(), "sess-123");
        assert_eq!(id.to_string(), "sess-123");
        assert_eq!(id.as_bytes(), b"sess-123");
    }

    #[test]
    fn into_inner_returns_string() {
        let id = SessionId::new("hello");
        assert_eq!(id.into_inner(), "hello");
    }

    #[test]
    fn equality_holds_across_clones() {
        let a = SessionId::new("abc");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_round_trip_preserves_value() {
        let id = SessionId::new("serde-123");
        let json = serde_json::to_string(&id).expect("serialize");
        // newtype tuple struct serializes as a JSON string.
        assert_eq!(json, "\"serde-123\"");
        let back: SessionId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);
    }

    #[test]
    fn default_is_empty_string() {
        let id: SessionId = Default::default();
        assert_eq!(id.as_str(), "");
    }
}
