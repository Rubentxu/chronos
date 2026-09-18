//! Shared helpers for the ports/ test suites.
//!
//! Behavioral tests for each port live under
//! `crates/chronos-domain/tests/{probe,session,telemetry}_ports.rs`.
//! This module gives them a common spot for fixtures and
//! shared assertion macros without depending on the (currently
//! closed) implementation details of any adapter.

use chronos_domain::session_id::SessionId;

/// Build a `SessionId` with the given string suffix.
pub fn session_id(name: &str) -> SessionId {
    SessionId::new(format!("sess-{name}"))
}

#[allow(dead_code)]
pub fn active_handle(id: &str) -> chronos_domain::ports::SessionHandle {
    chronos_domain::ports::SessionHandle {
        session_id: session_id(id),
        state: chronos_domain::ports::SessionState::Active,
    }
}
