use chronos_domain::{
    EventData, EventType, Language, PythonEventKind, SourceLocation, TraceEvent, VariableInfo,
    VariableScope,
};
use chronos_domain::semantic::{ResolveContext, SemanticEventKind, SemanticResolver};
use chronos_python::semantic_resolver::PythonSemanticResolver;

fn make_python_call_event() -> TraceEvent {
    TraceEvent::new(
        1,
        1_000_000,
        100,
        EventType::FunctionEntry,
        SourceLocation {
            file: Some("/app/main.py".into()),
            line: Some(10),
            column: None,
            function: Some("foo".into()),
            address: 0,
        },
        EventData::PythonFrame {
            qualified_name: "mymodule.foo".into(),
            file: "/app/main.py".into(),
            line: 10,
            is_generator: false,
            locals: Some(vec![
                VariableInfo::new("x", "42", "int", 0, VariableScope::Local),
                VariableInfo::new("name", "\"Alice\"", "str", 0, VariableScope::Local),
            ]),
            event_kind: PythonEventKind::Call,
        },
    )
}

#[test]
fn test_resolver_language() {
    assert_eq!(PythonSemanticResolver::new().language(), Language::Python);
}

#[test]
fn test_resolver_name() {
    assert_eq!(PythonSemanticResolver::new().name(), "python-sysprobe");
}

#[test]
fn test_resolve_function_call() {
    let resolver = PythonSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_python_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    assert!(matches!(se.kind, SemanticEventKind::FunctionCalled { .. }));
    assert_eq!(se.language, Language::Python);
}

#[test]
fn test_resolve_function_call_extracts_locals() {
    let resolver = PythonSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_python_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    match se.kind {
        SemanticEventKind::FunctionCalled {
            function,
            arguments,
            ..
        } => {
            assert_eq!(function, "mymodule.foo");
            assert!(arguments.iter().any(|(n, v)| n == "x" && v == "42"));
        }
        _ => panic!("expected FunctionCalled"),
    }
}

#[test]
fn test_resolve_non_python_returns_none() {
    let resolver = PythonSemanticResolver::new();
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
fn test_description_contains_function_and_locals() {
    let resolver = PythonSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_python_call_event(),
            &ResolveContext {
                pid: 1234,
                binary_path: None,
            },
        )
        .unwrap();
    assert!(se.description.contains("mymodule.foo"));
    assert!(se.description.contains("x=42"));
    assert!(se.description.contains("/app/main.py:10"));
}
