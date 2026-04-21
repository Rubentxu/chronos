//! Convert JDWP debugger events into Chronos trace events.

use crate::protocol::JdwpEvent;
use chronos_domain::{EventData, EventType, JavaEventKind, SourceLocation, TraceEvent};

/// Convert a JDWP event into a Chronos TraceEvent.
pub fn jdwp_event_to_trace_event(ev: JdwpEvent, event_id: u64, timestamp_ns: u64) -> TraceEvent {
    use crate::protocol::event_kind;

    let (event_type, java_event_kind) = match ev.kind {
        event_kind::METHOD_ENTRY => (EventType::FunctionEntry, JavaEventKind::MethodEntry),
        event_kind::METHOD_EXIT => (EventType::FunctionExit, JavaEventKind::MethodExit),
        event_kind::EXCEPTION => (EventType::ExceptionThrown, JavaEventKind::Exception),
        event_kind::BREAKPOINT => (EventType::BreakpointHit, JavaEventKind::MethodEntry),
        event_kind::STEP => (EventType::BreakpointHit, JavaEventKind::MethodEntry),
        _ => (EventType::Custom, JavaEventKind::MethodEntry),
    };

    // Convert class signature like "Lcom/example/Foo;" to "com.example.Foo"
    let class_name = normalize_class_signature(&ev.class_signature);
    let method_name = ev.method_name.clone();
    let qualified_name = format!("{}.{}", class_name, method_name);

    let location = SourceLocation {
        file: None,
        line: ev.line,
        function: Some(qualified_name.clone()),
        ..Default::default()
    };

    let data = EventData::JavaFrame {
        class_name,
        method_name,
        signature: None,
        file: None,
        line: ev.line,
        locals: None,
        event_kind: java_event_kind,
    };

    TraceEvent {
        event_id,
        timestamp_ns,
        thread_id: ev.thread_id,
        event_type,
        location,
        data,
    }
}

/// Normalize a JVM class signature to a Java-style dotted name.
///
/// Input: "Lcom/example/Foo;" → Output: "com.example.Foo"
/// Input: "[I" (int array) → Output: "int[]"
/// Input: "java.lang.String" → Output: "java.lang.String"
fn normalize_class_signature(sig: &str) -> String {
    // Remove leading 'L' and trailing ';'
    let sig = sig.trim();
    let inner = if sig.starts_with('L') && sig.ends_with(';') {
        &sig[1..sig.len() - 1]
    } else {
        sig
    };

    // Replace '/' with '.'
    inner.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::event_kind;

    #[test]
    fn test_jdwp_method_entry_to_trace_event() {
        let jdwp_event = JdwpEvent {
            kind: event_kind::METHOD_ENTRY,
            thread_id: 12345,
            class_signature: "Lcom/example/Foo;".to_string(),
            method_name: "bar".to_string(),
            line: Some(42),
        };

        let trace = jdwp_event_to_trace_event(jdwp_event, 1, 1000);

        assert_eq!(trace.event_id, 1);
        assert_eq!(trace.timestamp_ns, 1000);
        assert_eq!(trace.thread_id, 12345);
        assert_eq!(trace.event_type, EventType::FunctionEntry);

        match &trace.data {
            EventData::JavaFrame {
                class_name,
                method_name,
                event_kind,
                ..
            } => {
                assert_eq!(class_name, "com.example.Foo");
                assert_eq!(method_name, "bar");
                assert_eq!(*event_kind, JavaEventKind::MethodEntry);
            }
            _ => panic!("Expected JavaFrame data"),
        }
    }

    #[test]
    fn test_jdwp_method_exit_to_trace_event() {
        let jdwp_event = JdwpEvent {
            kind: event_kind::METHOD_EXIT,
            thread_id: 99,
            class_signature: "Lmy/pkg/Util;".to_string(),
            method_name: "process".to_string(),
            line: Some(10),
        };

        let trace = jdwp_event_to_trace_event(jdwp_event, 2, 2000);

        assert_eq!(trace.event_type, EventType::FunctionExit);
        match &trace.data {
            EventData::JavaFrame { event_kind, .. } => {
                assert_eq!(*event_kind, JavaEventKind::MethodExit);
            }
            _ => panic!("Expected JavaFrame data"),
        }
    }

    #[test]
    fn test_normalize_class_signature() {
        assert_eq!(
            normalize_class_signature("Lcom/example/Foo;"),
            "com.example.Foo"
        );
        assert_eq!(
            normalize_class_signature("Ljava/lang/String;"),
            "java.lang.String"
        );
        assert_eq!(
            normalize_class_signature("java.lang.Object"),
            "java.lang.Object"
        );
    }
}
