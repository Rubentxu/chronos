use chronos_domain::{
    EventData, EventType, JavaEventKind, Language, SourceLocation, TraceEvent, VariableInfo,
    VariableScope,
};
use chronos_domain::semantic::{ResolveContext, SemanticEventKind, SemanticResolver};
use chronos_java::semantic_resolver::JavaSemanticResolver;

fn make_java_call_event() -> TraceEvent {
    TraceEvent::new(
        1,
        1_000_000,
        100,
        EventType::FunctionEntry,
        SourceLocation {
            file: Some("Main.java".into()),
            line: Some(10),
            column: None,
            function: Some("main".into()),
            address: 0,
        },
        EventData::JavaFrame {
            class_name: "com.example.MyService".into(),
            method_name: "processOrder".into(),
            signature: Some("(Ljava/lang/String;)V".into()),
            file: Some("MyService.java".into()),
            line: Some(42),
            locals: Some(vec![VariableInfo::new(
                "orderId",
                "\"ORD-123\"",
                "String",
                0,
                VariableScope::Local,
            )]),
            event_kind: JavaEventKind::MethodEntry,
        },
    )
}

#[test]
fn test_resolver_language() {
    assert_eq!(JavaSemanticResolver::new().language(), Language::Java);
}

#[test]
fn test_resolver_name() {
    assert_eq!(JavaSemanticResolver::new().name(), "java-jdwp");
}

#[test]
fn test_resolve_method_entry() {
    let resolver = JavaSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_java_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    assert!(matches!(se.kind, SemanticEventKind::FunctionCalled { .. }));
    assert_eq!(se.language, Language::Java);
}

#[test]
fn test_resolve_method_entry_extracts_qualified_name() {
    let resolver = JavaSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_java_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    match se.kind {
        SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "com.example.MyService.processOrder");
        }
        _ => panic!("expected FunctionCalled"),
    }
}

#[test]
fn test_resolve_non_java_returns_none() {
    let resolver = JavaSemanticResolver::new();
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
fn test_description_contains_class_and_method() {
    let resolver = JavaSemanticResolver::new();
    let se = resolver
        .resolve(
            &make_java_call_event(),
            &ResolveContext { pid: 1234, binary_path: None },
        )
        .unwrap();
    assert!(se.description.contains("com.example.MyService"));
    assert!(se.description.contains("processOrder"));
}
