//! Unit tests for event_mapper - CDP paused event to WASM TraceEvents conversion.
//!
//! Note: Some tests use WasmBreakpointManager::new_dummy() which has unsafe behavior
//! in its implementation. The tests here are designed to minimize exposure to that.
//! Full testing requires fixing the underlying dummy constructor.

use chronos_browser::event_mapper::{CdpDebuggerPaused, CdpCallFrame, CdpLocation};
use chronos_domain::trace::{WasmModuleInfo, WasmFunctionInfo};
use std::collections::HashMap;

fn make_test_module() -> HashMap<String, WasmModuleInfo> {
    let module = WasmModuleInfo {
        script_id: "wasm-123".to_string(),
        url: Some("add.wasm".to_string()),
        hash: "abc123".to_string(),
        build_id: None,
        functions: vec![
            WasmFunctionInfo {
                function_index: 0,
                name: Some("add".to_string()),
                body_start: 0,
                body_end: 20,
                breakpoint_id: None,
            },
            WasmFunctionInfo {
                function_index: 1,
                name: Some("multiply".to_string()),
                body_start: 20,
                body_end: 40,
                breakpoint_id: None,
            },
            WasmFunctionInfo {
                function_index: 2,
                name: Some("fibonacci".to_string()),
                body_start: 40,
                body_end: 100,
                breakpoint_id: None,
            },
        ],
    };
    let mut map = HashMap::new();
    map.insert("wasm-123".to_string(), module);
    map
}

// Note: The event_mapper function paused_to_wasm_events requires a WasmBreakpointManager
// which has unsafe implementation in new_dummy(). The tests here focus on
// data structure testing and serialization/deserialization.

#[test]
fn test_cdp_debugger_paused_deserialize() {
    // CDP events use camelCase in JSON, serde renames to snake_case struct fields.
    let json = r#"{
        "callFrames": [
            {
                "functionName": "add",
                "location": {
                    "scriptId": "123",
                    "lineNumber": 10,
                    "columnNumber": 0
                },
                "scopeChain": []
            }
        ],
        "reason": "breakpoint",
        "hitBreakpoints": ["bp-1"]
    }"#;

    let paused: CdpDebuggerPaused = serde_json::from_str(json).unwrap();
    assert_eq!(paused.reason, "breakpoint");
    assert_eq!(paused.call_frames.len(), 1);
    assert_eq!(paused.call_frames[0].function_name, "add");
    assert_eq!(paused.hit_breakpoints, vec!["bp-1"]);
}

#[test]
fn test_find_function_by_offset() {
    let modules = make_test_module();
    let module = modules.get("wasm-123").unwrap();

    // Find function by offset
    let add_func = module.functions.iter().find(|f| f.name.as_deref() == Some("add"));
    assert!(add_func.is_some());
    let add_func = add_func.unwrap();

    // Verify offsets
    assert!(add_func.body_start <= 5 && add_func.body_end > 5);
    assert_eq!(add_func.function_index, 0);
}

#[test]
fn test_wasm_module_functions_have_correct_offsets() {
    let modules = make_test_module();
    let module = modules.get("wasm-123").unwrap();

    // Verify function ranges don't overlap
    let mut functions: Vec<_> = module.functions.iter().collect();
    functions.sort_by_key(|f| f.body_start);

    for window in functions.windows(2) {
        let first = &window[0];
        let second = &window[1];
        assert!(
            first.body_end <= second.body_start,
            "Functions {:?} and {:?} have overlapping ranges",
            first.name,
            second.name
        );
    }
}

#[test]
fn test_wasm_module_info_serialization() {
    let module = WasmModuleInfo {
        script_id: "test-id".to_string(),
        url: Some("test.wasm".to_string()),
        hash: "def456".to_string(),
        build_id: Some("build-123".to_string()),
        functions: vec![
            WasmFunctionInfo {
                function_index: 0,
                name: Some("test".to_string()),
                body_start: 0,
                body_end: 50,
                breakpoint_id: Some("bp-1".to_string()),
            },
        ],
    };

    // Test round-trip through JSON
    let json = serde_json::to_string(&module).unwrap();
    let parsed: WasmModuleInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.script_id, "test-id");
    assert_eq!(parsed.url, Some("test.wasm".to_string()));
    assert_eq!(parsed.hash, "def456");
    assert_eq!(parsed.functions.len(), 1);
    assert_eq!(parsed.functions[0].name.as_deref(), Some("test"));
}

#[test]
fn test_wasm_function_info_no_breakpoint() {
    let func = WasmFunctionInfo {
        function_index: 5,
        name: Some("fibonacci".to_string()),
        body_start: 100,
        body_end: 200,
        breakpoint_id: None,
    };

    assert_eq!(func.function_index, 5);
    assert_eq!(func.name.as_deref(), Some("fibonacci"));
    assert!(func.breakpoint_id.is_none());
}

#[test]
fn test_cdp_location_creation() {
    let location = CdpLocation {
        script_id: "wasm-script".to_string(),
        line_number: 42,
        column_number: Some(10),
    };

    assert_eq!(location.script_id, "wasm-script");
    assert_eq!(location.line_number, 42);
    assert_eq!(location.column_number, Some(10));
}

#[test]
fn test_cdp_call_frame_creation() {
    let frame = CdpCallFrame {
        function_name: "multiply".to_string(),
        location: CdpLocation {
            script_id: "123".to_string(),
            line_number: 20,
            column_number: None,
        },
        scope_chain: vec![],
    };

    assert_eq!(frame.function_name, "multiply");
    assert_eq!(frame.location.script_id, "123");
    assert!(frame.scope_chain.is_empty());
}
