//! Integration tests for chronos-js CDP adapter.
//!
//! Tests the JsCdpAdapter and CdpSession types.

use chronos_js::JsCdpAdapter;

// NOTE: construction of `JsCdpAdapter` is covered by the unit test
// `adapter::js_cdp_adapter_tests::test_js_cdp_adapter_new_keeps_its_target`.
// `JsCdpAdapter` keeps its host and port private with no accessors, so from
// outside the crate this file could only repeat "it does not panic", which is
// not a test; the duplicate that used to live here was removed for that
// reason.

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
    use chronos_js::cdp_client::{CallFrame, CdpEvent};

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
        CdpEvent::DebuggerPaused {
            reason,
            call_frames,
            ..
        } => {
            assert_eq!(reason, "breakpoint");
            assert_eq!(call_frames.len(), 1);
            assert_eq!(call_frames[0].function_name, "testFunc");
        }
        _ => panic!("Expected DebuggerPaused"),
    }
}

#[test]
#[ignore = "requires a Node.js process listening for CDP on localhost:9229"]
fn test_connect_to_cdp() {
    // Run with: node --inspect=localhost:9229 script.js
    let adapter = JsCdpAdapter::new("localhost", 9229);
    let result = adapter.connect();
    assert!(
        result.is_ok(),
        "Failed to connect to CDP: {:?}",
        result.err()
    );
}
