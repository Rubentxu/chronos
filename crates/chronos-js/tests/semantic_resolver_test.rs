use chronos_domain::{
    EventData, EventType, JsEventKind, Language, SourceLocation, TraceEvent, VariableInfo,
    VariableScope,
};
use chronos_domain::semantic::{ResolveContext, SemanticEventKind};
use chronos_js::semantic_resolver::JsSemanticResolver;
use chronos_domain::semantic::SemanticResolver;

fn make_js_call_event() -> TraceEvent {
    TraceEvent::new(
        1,
        1_000_000,
        100,
        EventType::FunctionEntry,
        SourceLocation {
            file: Some("app.js".into()),
            line: Some(10),
            column: None,
            function: Some("handleRequest".into()),
            address: 0,
        },
        EventData::JsFrame {
            function_name: "handleRequest".into(),
            script_url: "https://example.com/app.js".into(),
            line_number: 42,
            column_number: 5,
            locals: Some(vec![VariableInfo::new(
                "req",
                "Request { url: '/' }",
                "Request",
                0,
                VariableScope::Local,
            )]),
            scope_chain: vec!["Local".into(), "Script".into()],
            event_kind: JsEventKind::Breakpoint,
        },
    )
}

#[test]
fn test_resolver_language() {
    assert_eq!(JsSemanticResolver::new().language(), Language::JavaScript);
}

#[test]
fn test_resolver_name() {
    assert_eq!(JsSemanticResolver::new().name(), "js-v8");
}

#[test]
fn test_resolve_breakpoint() {
    let resolver = JsSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_js_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    assert!(matches!(se.kind, SemanticEventKind::FunctionCalled { .. }));
    assert_eq!(se.language, Language::JavaScript);
}

#[test]
fn test_resolve_extracts_function_name() {
    let resolver = JsSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_js_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    match se.kind {
        SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "handleRequest");
        }
        _ => panic!("expected FunctionCalled"),
    }
}

#[test]
fn test_resolve_non_js_returns_none() {
    let resolver = JsSemanticResolver::new();
    let event = TraceEvent::new(
        1,
        0,
        1,
        EventType::FunctionEntry,
        SourceLocation::default(),
        EventData::Empty,
    );
    assert!(resolver
        .resolve(
            &event,
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            }
        )
        .is_none());
}

#[test]
fn test_description_contains_function_and_url() {
    let resolver = JsSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_js_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    assert!(se.description.contains("handleRequest"));
    assert!(se.description.contains("app.js"));
}
