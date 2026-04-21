//! Convert Delve stack frames into Chronos trace events.

use crate::rpc::StackFrame;
use chronos_domain::{EventData, GoEventKind, SourceLocation, TraceEvent};

/// Convert a Delve stack frame into a Chronos TraceEvent.
pub fn stack_frame_to_trace_event(
    frame: &StackFrame,
    goroutine_id: u64,
    event_id: u64,
    timestamp_ns: u64,
    kind: GoEventKind,
) -> TraceEvent {
    let function_name = frame
        .function
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_default();

    let file = Some(frame.file.clone());
    let line = Some(frame.line as u32);

    let location = SourceLocation {
        file: file.clone(),
        line,
        function: Some(function_name.clone()),
        ..Default::default()
    };

    // Convert Delve variables to VariableInfo
    let locals = frame.locals.as_ref().map(|vars| {
        vars.iter()
            .map(|v| {
                chronos_domain::VariableInfo::new(
                    &v.name,
                    &v.value,
                    "unknown",
                    0,
                    chronos_domain::VariableScope::Local,
                )
            })
            .collect()
    });

    let data = EventData::GoFrame {
        goroutine_id,
        function_name,
        file,
        line,
        locals,
        event_kind: kind,
    };

    TraceEvent {
        event_id,
        timestamp_ns,
        thread_id: goroutine_id,
        event_type: chronos_domain::EventType::BreakpointHit,
        location,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::FunctionInfo;

    #[test]
    fn test_stack_frame_to_trace_event_basic() {
        let frame = StackFrame {
            function: Some(FunctionInfo {
                name: "main.main".to_string(),
            }),
            file: "/path/to/main.go".to_string(),
            line: 10,
            locals: None,
        };

        let event = stack_frame_to_trace_event(&frame, 1, 1, 1000, GoEventKind::Breakpoint);

        assert_eq!(event.event_id, 1);
        assert_eq!(event.timestamp_ns, 1000);
        assert_eq!(event.thread_id, 1);
        assert_eq!(event.event_type, chronos_domain::EventType::BreakpointHit);

        match &event.data {
            EventData::GoFrame {
                goroutine_id,
                function_name,
                event_kind,
                ..
            } => {
                assert_eq!(*goroutine_id, 1);
                assert_eq!(function_name, "main.main");
                assert_eq!(*event_kind, GoEventKind::Breakpoint);
            }
            _ => panic!("Expected GoFrame data"),
        }

        assert_eq!(event.location.file.as_deref(), Some("/path/to/main.go"));
        assert_eq!(event.location.line, Some(10));
        assert_eq!(event.location.function.as_deref(), Some("main.main"));
    }

    #[test]
    fn test_stack_frame_with_locals() {
        use crate::rpc::DelveVar;

        let frame = StackFrame {
            function: Some(FunctionInfo {
                name: "main.process".to_string(),
            }),
            file: "/path/to/main.go".to_string(),
            line: 25,
            locals: Some(vec![
                DelveVar {
                    name: "count".to_string(),
                    value: "42".to_string(),
                },
                DelveVar {
                    name: "name".to_string(),
                    value: "\"hello\"".to_string(),
                },
            ]),
        };

        let event = stack_frame_to_trace_event(&frame, 5, 10, 5000, GoEventKind::Step);

        match &event.data {
            EventData::GoFrame { locals, .. } => {
                assert!(locals.is_some());
                let locals = locals.as_ref().unwrap();
                assert_eq!(locals.len(), 2);
                assert_eq!(locals[0].name, "count");
                assert_eq!(locals[0].value, "42");
                assert_eq!(locals[1].name, "name");
                assert_eq!(locals[1].value, "\"hello\"");
            }
            _ => panic!("Expected GoFrame data"),
        }
    }

    #[test]
    fn test_stack_frame_without_function_name() {
        let frame = StackFrame {
            function: None,
            file: "/path/to/main.go".to_string(),
            line: 1,
            locals: None,
        };

        let event = stack_frame_to_trace_event(&frame, 2, 3, 2000, GoEventKind::GoroutineStop);

        match &event.data {
            EventData::GoFrame { function_name, .. } => {
                assert_eq!(function_name, "");
            }
            _ => panic!("Expected GoFrame data"),
        }
    }
}
