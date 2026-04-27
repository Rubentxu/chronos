//! WebAssembly semantic resolver.
//!
//! Translates WASM TraceEvents into high-level SemanticEvents following the
//! pattern established in chronos-js/src/semantic_resolver.rs.

use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticEventKind, SemanticResolver};
use chronos_domain::trace::{EventData, EventType, Language, TraceEvent, WasmEventKind};

/// SemanticResolver for WebAssembly that enriches WASM frame events.
#[derive(Debug)]
pub struct WasmSemanticResolver;

impl WasmSemanticResolver {
    /// Create a new WASM semantic resolver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmSemanticResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticResolver for WasmSemanticResolver {
    fn language(&self) -> Language {
        Language::WebAssembly
    }

    fn name(&self) -> &str {
        "wasm-cdp-resolver"
    }

    fn resolve(&self, event: &TraceEvent, _ctx: &ResolveContext) -> Option<SemanticEvent> {
        let frame = match &event.data {
            EventData::WasmFrame {
                function_index,
                function_name,
                body_offset,
                module_url,
                locals,
                event_kind,
            } => (function_index, function_name, body_offset, module_url, locals, event_kind),
            _ => return None,
        };

        let (function_index, function_name, _body_offset, module_url, locals, event_kind) = frame;

        let func_name = function_name
            .clone()
            .unwrap_or_else(|| format!("wasm_function_{}", function_index));

        let description = match event.event_type {
            EventType::FunctionEntry | EventType::BreakpointHit => {
                let args: Vec<String> = locals
                    .as_ref()
                    .map(|l| {
                        l.iter()
                            .filter(|v| !v.name.starts_with("__"))
                            .map(|v| format!("{}={}", v.name, v.value))
                            .collect()
                    })
                    .unwrap_or_default();

                if args.is_empty() {
                    if let Some(url) = module_url {
                        format!("{}() in {}", func_name, url)
                    } else {
                        format!("{}()", func_name)
                    }
                } else {
                    if let Some(url) = module_url {
                        format!("{}({}) in {}", func_name, args.join(", "), url)
                    } else {
                        format!("{}({})", func_name, args.join(", "))
                    }
                }
            }
            _ => return None,
        };

        let kind = match event_kind {
            WasmEventKind::Entry | WasmEventKind::Breakpoint => {
                let args: Vec<(String, String)> = locals
                    .as_ref()
                    .map(|l| {
                        l.iter()
                            .filter(|v| !v.name.starts_with("__"))
                            .take(5)
                            .map(|v| (v.name.clone(), v.value.clone()))
                            .collect()
                    })
                    .unwrap_or_default();

                SemanticEventKind::FunctionCalled {
                    function: func_name,
                    module: module_url.clone(),
                    arguments: args,
                }
            }
            WasmEventKind::Return => SemanticEventKind::FunctionReturned {
                function: func_name,
                return_value: None,
            },
            WasmEventKind::Exception => SemanticEventKind::Exception {
                type_name: "WebAssemblyException".to_string(),
                message: format!(
                    "{} in {}",
                    func_name,
                    module_url.as_deref().unwrap_or("unknown")
                ),
                stack_trace: vec![format!(
                    "{} at offset {}",
                    func_name,
                    _body_offset
                )],
            },
            WasmEventKind::Other(tag) => SemanticEventKind::Generic {
                summary: format!(
                    "WASM {} at {} in {}",
                    tag,
                    _body_offset,
                    module_url.as_deref().unwrap_or("unknown")
                ),
            },
            WasmEventKind::Step => SemanticEventKind::Generic {
                summary: format!(
                    "WASM step in {} at offset {}",
                    func_name,
                    _body_offset
                ),
            },
        };

        Some(SemanticEvent {
            source_event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
            language: Language::WebAssembly,
            kind,
            description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            SemanticEventKind::FunctionCalled { function, .. } => {
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
            SemanticEventKind::FunctionReturned { function, .. } => {
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
            SemanticEventKind::Exception { .. } => {}
            _ => panic!("Expected Exception"),
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
    fn test_wasm_semantic_resolver_no_function_name() {
        let resolver = WasmSemanticResolver::new();
        let event = make_wasm_frame(5, None, 100, WasmEventKind::Breakpoint);

        let semantic = resolver.resolve(&event, &ResolveContext {
            pid: 1,
            binary_path: None,
        });

        assert!(semantic.is_some());
        let semantic = semantic.unwrap();
        match semantic.kind {
            SemanticEventKind::FunctionCalled { function, .. } => {
                assert_eq!(function, "wasm_function_5");
            }
            _ => panic!("Expected FunctionCalled"),
        }
    }

    #[test]
    fn test_wasm_semantic_resolver_non_wasm_event() {
        use chronos_domain::EventData;

        let resolver = WasmSemanticResolver::new();
        let event = TraceEvent::function_entry(1, 1000, 1, "main", 0x1000);

        let semantic = resolver.resolve(&event, &ResolveContext {
            pid: 1,
            binary_path: None,
        });

        assert!(semantic.is_none());
    }
}