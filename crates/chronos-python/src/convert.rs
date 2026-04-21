//! Convert DAP events to TraceEvents.

use crate::client::DapEvent;
use chronos_domain::{EventData, EventType, PythonEventKind, TraceEvent};
use serde::Deserialize;

/// DAP stopped event body.
#[derive(Debug, Deserialize)]
pub struct StoppedBody {
    pub reason: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub thread_id: Option<u64>,
    #[serde(default)]
    pub preserve_focus_hint: Option<bool>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub all_threads_stopped: Option<bool>,
    #[serde(default)]
    pub hit_breakpoint_ids: Option<Vec<String>>,
}

/// DAP output event body.
#[derive(Debug, Deserialize)]
pub struct OutputBody {
    pub output: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub timestamp: Option<u64>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// DAP thread event body.
#[derive(Debug, Deserialize)]
pub struct ThreadBody {
    pub reason: String,
    pub thread_id: u64,
}

/// Convert a DAP event to a TraceEvent, if applicable.
/// Returns None for events that should not produce TraceEvents.
pub fn dap_event_to_trace(event: &DapEvent, _session_id: &str) -> Option<TraceEvent> {
    match event.event.as_str() {
        "stopped" => convert_stopped(event),
        "output" => convert_output(event),
        "thread" => convert_thread(event),
        // Other events (breakpoint, step, etc.) are handled via "stopped"
        _ => None,
    }
}

fn convert_stopped(event: &DapEvent) -> Option<TraceEvent> {
    let body: StoppedBody = serde_json::from_value(event.body.clone()).ok()?;

    let event_kind = match body.reason.as_str() {
        "step" => PythonEventKind::Call, // Step in DAP is like hitting a step breakpoint
        "breakpoint" => PythonEventKind::Call,
        "exception" => PythonEventKind::Exception,
        "pause" => PythonEventKind::Call,
        _ => PythonEventKind::Call,
    };

    // Determine event type from event_kind before moving
    let event_type = match event_kind {
        PythonEventKind::Exception => EventType::ExceptionThrown,
        _ => EventType::FunctionEntry,
    };

    // For MVP, we create a minimal PythonFrame event
    // A full implementation would extract actual frame info from the stopped event
    let data = EventData::PythonFrame {
        qualified_name: format!("<stopped: {}>", body.reason),
        file: "<dap>".to_string(),
        line: 0,
        is_generator: false,
        locals: None,
        event_kind,
    };

    Some(TraceEvent {
        event_id: 0, // Will be assigned by session
        timestamp_ns: 0,
        thread_id: body.thread_id.unwrap_or(1),
        event_type,
        location: chronos_domain::trace::SourceLocation::default(),
        data,
    })
}

fn convert_output(event: &DapEvent) -> Option<TraceEvent> {
    let body: OutputBody = serde_json::from_value(event.body.clone()).ok()?;

    let category = body.category.unwrap_or_else(|| "console".to_string());

    // Map DAP output category to our console output type
    let (text, output_category) = match category.as_str() {
        "stdout" => (body.output, "stdout".to_string()),
        "stderr" => (body.output, "stderr".to_string()),
        _ => (body.output, "console".to_string()),
    };

    let data = EventData::PythonConsoleOutput {
        text,
        category: output_category,
    };

    Some(TraceEvent {
        event_id: 0,
        timestamp_ns: body.timestamp.unwrap_or(0),
        thread_id: 1,
        event_type: EventType::Custom,
        location: chronos_domain::trace::SourceLocation::default(),
        data,
    })
}

fn convert_thread(event: &DapEvent) -> Option<TraceEvent> {
    let _body: ThreadBody = serde_json::from_value(event.body.clone()).ok()?;
    // Thread events are informational for now
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_stopped_breakpoint() {
        let event = DapEvent {
            event: "stopped".to_string(),
            body: json!({
                "reason": "breakpoint",
                "threadId": 1,
                "allThreadsStopped": true
            }),
        };

        let trace = dap_event_to_trace(&event, "session-1");
        assert!(trace.is_some());
        let trace = trace.unwrap();
        assert_eq!(trace.thread_id, 1);
        match &trace.data {
            EventData::PythonFrame { event_kind, .. } => {
                assert_eq!(*event_kind, PythonEventKind::Call);
            }
            _ => panic!("Expected PythonFrame"),
        }
    }

    #[test]
    fn test_convert_stopped_exception() {
        let event = DapEvent {
            event: "stopped".to_string(),
            body: json!({
                "reason": "exception",
                "threadId": 2,
                "text": "ValueError: bad value"
            }),
        };

        let trace = dap_event_to_trace(&event, "session-1");
        assert!(trace.is_some());
        let trace = trace.unwrap();
        match &trace.data {
            EventData::PythonFrame { event_kind, .. } => {
                assert_eq!(*event_kind, PythonEventKind::Exception);
            }
            _ => panic!("Expected PythonFrame"),
        }
    }

    #[test]
    fn test_convert_output_stdout() {
        let event = DapEvent {
            event: "output".to_string(),
            body: json!({
                "output": "Hello, World!\n",
                "category": "stdout",
                "timestamp": 12345
            }),
        };

        let trace = dap_event_to_trace(&event, "session-1");
        assert!(trace.is_some());
        let trace = trace.unwrap();
        match &trace.data {
            EventData::PythonConsoleOutput { text, category } => {
                assert_eq!(text, "Hello, World!\n");
                assert_eq!(category, "stdout");
            }
            _ => panic!("Expected PythonConsoleOutput"),
        }
    }

    #[test]
    fn test_convert_output_stderr() {
        let event = DapEvent {
            event: "output".to_string(),
            body: json!({
                "output": "Error: something went wrong\n",
                "category": "stderr"
            }),
        };

        let trace = dap_event_to_trace(&event, "session-1");
        assert!(trace.is_some());
        let trace = trace.unwrap();
        match &trace.data {
            EventData::PythonConsoleOutput { category, .. } => {
                assert_eq!(category, "stderr");
            }
            _ => panic!("Expected PythonConsoleOutput"),
        }
    }

    #[test]
    fn test_convert_unknown_event() {
        let event = DapEvent {
            event: "invalid_event".to_string(),
            body: json!({}),
        };

        let trace = dap_event_to_trace(&event, "session-1");
        assert!(trace.is_none());
    }
}
