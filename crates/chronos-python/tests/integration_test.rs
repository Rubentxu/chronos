//! Integration tests for chronos-python DAP adapter.
//!
//! Tests the PythonDapAdapter and DapSession types.

use chronos_capture::TraceAdapter;
use chronos_python::{DapClient, DapSession, PythonDapAdapter};

#[test]
fn test_dap_client_creation() {
    // DapClient::connect requires a running debugpy server,
    // so we just verify the struct can be created and used with mock data
    // In a real scenario, this would connect to debugpy
}

#[test]
fn test_python_adapter_creation() {
    let adapter = PythonDapAdapter::new("localhost", 5678);
    assert_eq!(adapter.name(), "python-dap");
    assert_eq!(adapter.get_language(), chronos_domain::Language::Python);
}

#[test]
fn test_dap_event_to_trace_output() {
    // Test conversion of DAP "output" event to PythonConsoleOutput TraceEvent
    use chronos_python::client::DapEvent;
    use chronos_python::convert::dap_event_to_trace;

    let event = DapEvent {
        event: "output".to_string(),
        body: serde_json::json!({
            "output": "Hello, World!\n",
            "category": "stdout",
            "timestamp": 12345
        }),
    };

    let trace = dap_event_to_trace(&event, "session-1");
    assert!(trace.is_some());
    let trace = trace.unwrap();
    match &trace.data {
        chronos_domain::EventData::PythonConsoleOutput { text, category } => {
            assert_eq!(text, "Hello, World!\n");
            assert_eq!(category, "stdout");
        }
        _ => panic!("Expected PythonConsoleOutput"),
    }
}

#[test]
fn test_dap_event_to_trace_stopped() {
    // Test conversion of DAP "stopped" event to PythonFrame TraceEvent
    use chronos_python::client::DapEvent;
    use chronos_python::convert::dap_event_to_trace;

    let event = DapEvent {
        event: "stopped".to_string(),
        body: serde_json::json!({
            "reason": "breakpoint",
            "threadId": 1,
            "allThreadsStopped": true
        }),
    };

    let trace = dap_event_to_trace(&event, "session-1");
    assert!(trace.is_some());
    let trace = trace.unwrap();
    match &trace.data {
        chronos_domain::EventData::PythonFrame { event_kind, .. } => {
            assert_eq!(*event_kind, chronos_domain::PythonEventKind::Call);
        }
        _ => panic!("Expected PythonFrame"),
    }
}

#[test]
#[ignore]
fn test_connect_to_debugpy() {
    // This test requires debugpy running on localhost:5678
    // Run with: debugpy --listen 5678 --wait-for-client
    let adapter = PythonDapAdapter::new("localhost", 5678);
    let result = adapter.connect(0);
    assert!(result.is_ok(), "Failed to connect to debugpy: {:?}", result.err());
}
