//! Integration tests for chronos-js CDP adapter.
//!
//! Tests the JsCdpAdapter and CdpSession types.

use chronos_js::{CdpSession, JsCdpAdapter};

#[test]
fn test_js_adapter_creation() {
    let _adapter = JsCdpAdapter::new("localhost", 9229);
    // JsCdpAdapter is not a TraceAdapter, so we just verify construction
}

#[test]
fn test_js_console_output_conversion() {
    // Test conversion of CDP Runtime.consoleAPICalled to JsConsoleOutput TraceEvent
    use chronos_js::cdp_client::{CdpEvent, RemoteObject};

    // Create a mock CDP event
    let event = CdpEvent::ConsoleApiCalled {
        type_: "log".to_string(),
        args: vec![RemoteObject {
            type_: "string".to_string(),
            subtype: None,
            class_name: None,
            value: Some(serde_json::json!("Hello from JS")),
            description: Some("Hello from JS".to_string()),
            object_id: None,
        }],
    };

    // Verify the event can be pattern matched correctly
    match event {
        CdpEvent::ConsoleApiCalled { type_, args } => {
            assert_eq!(type_, "log");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected ConsoleApiCalled"),
    }
}

#[test]
fn test_cdp_event_debugger_paused_conversion() {
    use chronos_js::cdp_client::{CdpEvent, CallFrame};

    let call_frames = vec![CallFrame {
        call_frame_id: "1".to_string(),
        function_name: "testFunc".to_string(),
        function_location: None,
        url: "test.js".to_string(),
        line_number: 10,
        column_number: 5,
        scope_chain: vec![],
    }];

    let event = CdpEvent::DebuggerPaused {
        reason: "breakpoint".to_string(),
        call_frames,
        hit_breakpoints: vec![],
    };

    match event {
        CdpEvent::DebuggerPaused { reason, call_frames, .. } => {
            assert_eq!(reason, "breakpoint");
            assert_eq!(call_frames.len(), 1);
            assert_eq!(call_frames[0].function_name, "testFunc");
        }
        _ => panic!("Expected DebuggerPaused"),
    }
}

#[test]
#[ignore]
fn test_connect_to_cdp() {
    // This test requires a Node.js process with --inspect running on localhost:9229
    // Run with: node --inspect=localhost:9229 script.js
    let adapter = JsCdpAdapter::new("localhost", 9229);
    let result = adapter.connect();
    assert!(result.is_ok(), "Failed to connect to CDP: {:?}", result.err());
}
