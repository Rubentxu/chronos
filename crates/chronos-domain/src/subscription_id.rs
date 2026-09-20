//! Subscription identity.
//!
//! `SubscriptionId` is the canonical identity newtype for observe
//! subscriptions (tripwire and uprobe kinds alike). It follows the
//! `SessionId` pattern (REC-C4 / CONN-002): a private `String` wrapped
//! in a newtype, with wire derives so the MCP JSON boundary converts
//! for free and accessor methods (`as_str`/`into_inner`) so no layer
//! touches the inner field.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical subscription identity (`tripwire-<n>` or
/// `uprobe-<session>-<n>`).
///
/// No validation: ids are allocated by the observe dispatcher or echo
/// a previously-issued id, so the string is opaque to this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SubscriptionId(String);

impl SubscriptionId {
    /// Wrap a `String` as a `SubscriptionId`. No validation.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the `SubscriptionId` and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SubscriptionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SubscriptionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SubscriptionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_string() {
        let id: SubscriptionId = "tripwire-7".into();
        assert_eq!(id.as_str(), "tripwire-7");
        assert_eq!(id.to_string(), "tripwire-7");
    }

    #[test]
    fn into_inner_returns_string() {
        let id = SubscriptionId::new("uprobe-sess-1-2");
        assert_eq!(id.into_inner(), "uprobe-sess-1-2");
    }

    #[test]
    fn equality_holds_across_clones() {
        let a = SubscriptionId::new("abc");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_round_trip_preserves_value() {
        let id = SubscriptionId::new("tripwire-42");
        let json = serde_json::to_string(&id).expect("serialize");
        // Newtype tuple struct serializes as a JSON string.
        assert_eq!(json, "\"tripwire-42\"");
        let back: SubscriptionId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);
    }
}
