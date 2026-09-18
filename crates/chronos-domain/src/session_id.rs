//! Session identity.
//!
//! `SessionId` is a thin newtype over `String`. It exists in domain so
//! the probe ports (`ProbeRegistry::attach/detach/list_active`,
//! `ProbeFactory::create`) can refer to session identity without
//! depending on `chronos_log::SessionId` (which would create a Cargo
//! cyclic dependency: `chronos_log` depends on `chronos_domain` for
//! `InvocationId` and `SymbolId`).
//!
//! REC-C3.3 will introduce a domain-side alias to `chronos_log::SessionId`
//! once services are inverted through the ports declared in this cycle.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    }

    #[test]
    fn equality_holds_across_clones() {
        let a = SessionId::new("abc");
        let b = a.clone();
        assert_eq!(a, b);
    }
}
