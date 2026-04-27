//! Unit tests for WasmSemanticResolver - WASM TraceEvents to SemanticEvents.

use chronos_browser::wasm_resolver::WasmSemanticResolver;
use chronos_domain::semantic::{ResolveContext, SemanticResolver};
use chronos_domain::trace::{Language, TraceEvent, WasmEventKind};

fn make_wasm_frame(
    function_index: u32,
    function_name: Option<String>,
    body_offset: u32,
    event_kind: WasmEventKind,
) -> TraceEvent {
    let event_id = 1;
    let timestamp_ns = 1000;
    let thread_id = 1;

    TraceEvent::wasm_frame(
        event_id,
        timestamp_ns,
        thread_id,
        function_index,
        function_name,
        body_offset,
        Some("test.wasm".to_string()),
        event_kind,
    )
}

#[test]
fn test_wasm_semantic_resolver_entry() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Entry);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "add");
        }
        _ => panic!("Expected FunctionCalled"),
    }
}

#[test]
fn test_wasm_semantic_resolver_return() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Return);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::FunctionReturned { function, .. } => {
            assert_eq!(function, "add");
        }
        _ => panic!("Expected FunctionReturned"),
    }
}

#[test]
fn test_wasm_semantic_resolver_exception() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Exception);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::Exception { .. } => {}
        _ => panic!("Expected Exception"),
    }
}

#[test]
fn test_wasm_semantic_resolver_non_wasm_event() {
    let resolver = WasmSemanticResolver::new();
    let event = TraceEvent::function_entry(1, 1000, 1, "main", 0x1000);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    // Non-WasmFrame events should return None
    assert!(semantic.is_none());
}

#[test]
fn test_wasm_semantic_resolver_breakpoint() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Breakpoint);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "add");
        }
        _ => panic!("Expected FunctionCalled for Breakpoint kind"),
    }
}

#[test]
fn test_wasm_semantic_resolver_step() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 10, WasmEventKind::Step);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::Generic { .. } => {}
        _ => panic!("Expected Generic for Step kind"),
    }
}

#[test]
fn test_wasm_semantic_resolver_other() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Other("debug".to_string()));

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::Generic { .. } => {}
        _ => panic!("Expected Generic for Other kind"),
    }
}

#[test]
fn test_wasm_semantic_resolver_no_function_name() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(5, None, 100, WasmEventKind::Entry);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    let semantic = semantic.unwrap();
    match semantic.kind {
        chronos_domain::semantic::SemanticEventKind::FunctionCalled { function, .. } => {
            assert_eq!(function, "wasm_function_5");
        }
        _ => panic!("Expected FunctionCalled"),
    }
}

#[test]
fn test_wasm_semantic_resolver_language() {
    let resolver = WasmSemanticResolver::new();
    assert_eq!(resolver.language(), Language::WebAssembly);
}

#[test]
fn test_wasm_semantic_resolver_name() {
    let resolver = WasmSemanticResolver::new();
    assert_eq!(resolver.name(), "wasm-cdp-resolver");
}

#[test]
fn test_wasm_semantic_resolver_timestamp_preserved() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Entry);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    assert_eq!(semantic.unwrap().timestamp_ns, 1000);
}

#[test]
fn test_wasm_semantic_resolver_thread_id_preserved() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Entry);

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    assert_eq!(semantic.unwrap().thread_id, 1);
}

#[test]
fn test_wasm_semantic_resolver_source_event_id_preserved() {
    let resolver = WasmSemanticResolver::new();
    let event = make_wasm_frame(0, Some("add".to_string()), 0, WasmEventKind::Entry);
    // event_id is set to 1 in make_wasm_frame

    let semantic = resolver.resolve(&event, &ResolveContext {
        pid: 1,
        binary_path: None,
    });

    assert!(semantic.is_some());
    assert_eq!(semantic.unwrap().source_event_id, 1);
}
