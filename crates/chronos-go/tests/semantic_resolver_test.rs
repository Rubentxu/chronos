use chronos_domain::{
    EventData, EventType, GoEventKind, Language, VariableInfo, VariableScope,
    SourceLocation, TraceEvent,
};
use chronos_domain::semantic::{ResolveContext, SemanticEventKind, SemanticResolver};
use chronos_go::semantic_resolver::GoSemanticResolver;

fn make_go_call_event() -> TraceEvent {
    TraceEvent::new(
        1,
        1_000_000,
        100,
        EventType::FunctionEntry,
        SourceLocation {
            file: Some("main.go".into()),
            line: Some(10),
            column: None,
            function: Some("main".into()),
            address: 0,
        },
        EventData::GoFrame {
            goroutine_id: 42,
            function_name: "main.processOrder".into(),
            file: Some("main.go".into()),
            line: Some(25),
            locals: Some(vec![VariableInfo {
                name: "orderId".into(),
                value: "\"ORD-123\"".into(),
                type_name: "string".into(),
                address: 0,
                scope: VariableScope::Local,
            }]),
            event_kind: GoEventKind::Breakpoint,
        },
    )
}

#[test]
fn test_resolver_language() {
    assert_eq!(GoSemanticResolver::new().language(), Language::Go);
}

#[test]
fn test_resolver_name() {
    assert_eq!(GoSemanticResolver::new().name(), "go-delve");
}

#[test]
fn test_resolve_breakpoint() {
    let resolver = GoSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_go_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    assert!(matches!(se.kind, SemanticEventKind::FunctionCalled { .. }));
    assert_eq!(se.language, Language::Go);
}

#[test]
fn test_resolve_extracts_function_name() {
    let resolver = GoSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_go_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    match se.kind {
        SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "main.processOrder");
        }
        _ => panic!("expected FunctionCalled"),
    }
}

#[test]
fn test_resolve_non_go_returns_none() {
    let resolver = GoSemanticResolver::new();
    let event = TraceEvent::new(
        1,
        0,
        1,
        EventType::FunctionEntry,
        SourceLocation::default(),
        EventData::Empty,
    );
    assert!(resolver
        .resolve(&event, &ResolveContext { pid: 1234, binary_path: None })
        .is_none());
}

#[test]
fn test_description_contains_function_and_goroutine_id() {
    let resolver = GoSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_go_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    assert!(se.description.contains("main.processOrder"));
    assert!(se.description.contains("goid=42"));
}
