//! Unit tests for the 5 session-lifecycle MCP tools.
//!
//! These tests call `SessionsService::*` methods directly, bypassing the MCP
//! transport layer. They verify the service logic without requiring a running
//! chronos-mcp binary.

use std::collections::HashMap;
use std::collections::HashSet;

use chronos_domain::{Language, SourceLocation, TraceEvent};
use chronos_query::QueryEngine;
use chronos_services::error::ServiceError;
use chronos_services::sessions::{SessionsContext, SessionsService};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Build a minimal in-memory SessionStore.
fn make_store() -> chronos_store::SessionStore {
    chronos_store::SessionStore::in_memory().unwrap()
}

/// Build a HashMap with one engine containing 2 trace events.
fn make_engine_with_two_events(session_id: &str) -> (String, QueryEngine) {
    let events = vec![
        TraceEvent {
            event_id: 1,
            timestamp_ns: 1_000_000_000,
            thread_id: 1,
            event_type: chronos_domain::EventType::FunctionEntry,
            location: SourceLocation::default(),
            data: chronos_domain::EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        },
        TraceEvent {
            event_id: 2,
            timestamp_ns: 2_001_000_000,
            thread_id: 1,
            event_type: chronos_domain::EventType::FunctionExit,
            location: SourceLocation::default(),
            data: chronos_domain::EventData::Function {
                name: "main".to_string(),
                signature: None,
                symbol_id: None,
                invocation_id: None,
                parent_invocation_id: None,
            },
        },
    ];
    let engine = QueryEngine::new(events);
    (session_id.to_string(), engine)
}

/// Build a Mutex-wrapped HashMap with one engine.
fn make_engines(session_id: &str) -> Mutex<HashMap<String, QueryEngine>> {
    let (id, engine) = make_engine_with_two_events(session_id);
    let mut map = HashMap::new();
    map.insert(id, engine);
    Mutex::new(map)
}

/// Empty languages map.
fn make_languages() -> Mutex<HashMap<String, Language>> {
    Mutex::new(HashMap::new())
}

/// Empty connected_sessions set.
fn make_connected() -> std::sync::Mutex<HashSet<String>> {
    std::sync::Mutex::new(HashSet::new())
}

/// Build a SessionsContext from its parts.
fn make_context<'a>(
    engines: &'a Mutex<HashMap<String, QueryEngine>>,
    languages: &'a Mutex<HashMap<String, Language>>,
    connected: &'a std::sync::Mutex<HashSet<String>>,
    store: &'a chronos_store::SessionStore,
) -> SessionsContext<'a> {
    SessionsContext {
        engines,
        session_languages: languages,
        connected_sessions: connected,
        store,
    }
}

// ---------------------------------------------------------------------------
// save_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn save_session_ok() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    let result =
        SessionsService::save_session("s1", Language::Python, "/usr/bin/python3".to_string(), &ctx)
            .await
            .unwrap();

    assert_eq!(result.event_count, 2);
    assert_eq!(result.hash_count, 2);
    assert_eq!(result.language, "python");
    assert_eq!(result.target, "/usr/bin/python3");
    assert_eq!(result.duration_ms, 1001);
}

#[tokio::test]
async fn save_session_not_in_memory() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    let result = SessionsService::save_session("s2", Language::C, "main".to_string(), &ctx).await;

    assert!(matches!(
        result,
        Err(ServiceError::SessionNotInMemory(ref s)) if s == "s2"
    ));
}

// ---------------------------------------------------------------------------
// load_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn load_session_ok() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    // Save first
    SessionsService::save_session("s1", Language::Go, "./server".to_string(), &ctx)
        .await
        .unwrap();

    // Drop from memory
    engines.lock().await.remove("s1");

    // Load back
    let result = SessionsService::load_session("s1", &ctx).await.unwrap();

    assert_eq!(result.language, "go");
    assert_eq!(result.target, "./server");
    assert_eq!(result.event_count, 2);
    assert_eq!(result.duration_ms, 1001);
    assert_eq!(result.created_at, 2001);
}

#[tokio::test]
async fn load_session_not_found() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    let result = SessionsService::load_session("no-such-session", &ctx).await;

    assert!(matches!(
        result,
        Err(ServiceError::LoadFailed(ref e)) if e.contains("not found")
    ));
}

// ---------------------------------------------------------------------------
// list_sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_sessions_empty() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    let result = SessionsService::list_sessions(&ctx).await.unwrap();

    assert!(result.sessions.is_empty());
}

#[tokio::test]
async fn list_sessions_one() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    SessionsService::save_session("s1", Language::Python, "script.py".to_string(), &ctx)
        .await
        .unwrap();

    let result = SessionsService::list_sessions(&ctx).await.unwrap();

    assert_eq!(result.sessions.len(), 1);
    let s = &result.sessions[0];
    assert_eq!(s.session_id, "s1");
    assert_eq!(s.language, "python");
    assert_eq!(s.target, "script.py");
}

// ---------------------------------------------------------------------------
// delete_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_session_ok() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    SessionsService::save_session("s1", Language::C, "main".to_string(), &ctx)
        .await
        .unwrap();

    let result = SessionsService::delete_session("s1", &ctx).await.unwrap();

    assert_eq!(result.session_id, "s1");
}

#[tokio::test]
async fn delete_session_not_found() {
    let store = make_store();
    let engines = make_engines("s1");
    let languages = make_languages();
    let connected = make_connected();
    let ctx = make_context(&engines, &languages, &connected, &store);

    let result = SessionsService::delete_session("no-such", &ctx).await;

    assert!(matches!(
        result,
        Err(ServiceError::DeleteFailed(ref e)) if e.contains("not found")
    ));
}
