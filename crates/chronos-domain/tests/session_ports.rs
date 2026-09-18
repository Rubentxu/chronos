//! Behavioral tests for the session port (REC-C3.1, B7).
//!
//! Coverage:
//! - Round-trip: insert → get → remove.
//! - Upsert semantics: returns the previous handle on replace.
//! - List + len match upsert/remove.
//! - Error paths: get on unknown id → `SessionNotFound`.

use chronos_domain::error::TraceError;
use chronos_domain::ports::{
    InMemorySessionRepository, SessionHandle, SessionRepository, SessionState,
};

mod common;
use common::{active_handle, session_id};

#[test]
fn round_trip_get_after_upsert() {
    let repo = InMemorySessionRepository::new();
    let handle = active_handle("alpha");
    repo.upsert(handle.clone()).unwrap();
    let out = repo.get(&session_id("alpha")).unwrap();
    assert_eq!(out, handle);
}

#[test]
fn upsert_returns_previous_handle_on_replace() {
    let repo = InMemorySessionRepository::new();
    let id = session_id("bravo");
    let first = SessionHandle {
        session_id: id.clone(),
        state: SessionState::Active,
    };
    let second = SessionHandle {
        session_id: id.clone(),
        state: SessionState::Paused,
    };
    let prev = repo.upsert(first).unwrap();
    assert!(prev.is_none(), "first insert should report no prior handle");

    let prev = repo.upsert(second.clone()).unwrap();
    let expected_first = SessionHandle {
        session_id: id.clone(),
        state: SessionState::Active,
    };
    assert_eq!(
        prev,
        Some(expected_first),
        "second insert should report prior handle"
    );

    let out = repo.get(&id).unwrap();
    assert_eq!(out, second);
}

#[test]
fn remove_unknown_returns_session_not_found() {
    let repo = InMemorySessionRepository::new();
    let err = repo.remove(&session_id("charlie")).unwrap_err();
    assert!(matches!(err, TraceError::SessionNotFound { .. }));
}

#[test]
fn get_unknown_returns_session_not_found() {
    let repo = InMemorySessionRepository::new();
    let err = repo.get(&session_id("delta")).unwrap_err();
    assert!(matches!(err, TraceError::SessionNotFound { .. }));
}

#[test]
fn list_and_len_track_upserts_and_removes() {
    let repo = InMemorySessionRepository::new();
    assert_eq!(repo.len(), 0);
    assert!(repo.list().is_empty());

    repo.upsert(active_handle("echo")).unwrap();
    repo.upsert(active_handle("foxtrot")).unwrap();
    assert_eq!(repo.len(), 2);
    // HashMap iteration order is not guaranteed. Sort by string for
    // the assertion.
    let mut listed = repo.list();
    listed.sort_by_key(|s| s.to_string());
    assert_eq!(listed, vec![session_id("echo"), session_id("foxtrot")]);

    let removed = repo.remove(&session_id("echo")).unwrap();
    assert_eq!(removed.state, SessionState::Active);
    assert_eq!(repo.len(), 1);
}

#[test]
fn repo_is_send_sync_via_arc() {
    let repo = InMemorySessionRepository::new();
    let arc: std::sync::Arc<dyn SessionRepository> = repo.into_arc();
    let _check: &(dyn SessionRepository + Send + Sync) = &*arc;
}
