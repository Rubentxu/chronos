//! Async JDWP event loop for capturing trace events from a JVM.
//!
//! The event loop reads JDWP events from the JVM and converts them to
//! Chronos TraceEvents, which are sent to a channel for buffering.

use crate::error::JavaError;
use crate::event_parser::jdwp_event_to_trace_event;
use crate::protocol::JdwpClient;
use chronos_domain::TraceEvent;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Run the JDWP event loop.
///
/// This function reads JDWP events from the JVM and converts them to
/// TraceEvents, sending them through the provided channel.
///
/// The loop terminates when:
/// - The cancellation token is signaled
/// - An error occurs reading from JDWP
/// - The JVM subprocess exits
///
/// # Arguments
/// * `protocol` - Arc wrapper around the JDWP client (wrapped in tokio::sync::Mutex)
/// * `events_tx` - Channel sender for TraceEvents
/// * `cancel` - Cancellation token to signal stop
///
/// # Note
/// The protocol client is wrapped in Arc<tokio::sync::Mutex<...>> to allow shared
/// access from the event loop while the adapter stores it. The Mutex is an async
/// mutex that can be held across await points.
pub async fn run_jdwp_event_loop(
    protocol: Arc<tokio::sync::Mutex<JdwpClient>>,
    events_tx: mpsc::Sender<TraceEvent>,
    cancel: CancellationToken,
) -> Result<(), JavaError> {
    let start_time = Instant::now();
    let mut event_id: u64 = 0;

    loop {
        // Check for cancellation before each read
        if cancel.is_cancelled() {
            tracing::debug!("JDWP event loop: cancellation requested, exiting");
            break;
        }

        // Read next JDWP event with timeout to allow cancellation check
        let read_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut client = protocol.lock().await;
            client.read_event().await
        })
        .await;

        match read_result {
            Ok(Ok(jdwp_event)) => {
                // Convert JDWP event to TraceEvent
                let timestamp_ns = start_time.elapsed().as_nanos() as u64;
                event_id += 1;

                let trace_event =
                    jdwp_event_to_trace_event(jdwp_event.clone(), event_id, timestamp_ns);

                // Try to send, but if the receiver is dropped, exit the loop
                if events_tx.send(trace_event).await.is_err() {
                    tracing::debug!("JDWP event loop: receiver dropped, exiting");
                    break;
                }
            }
            Ok(Err(e)) => {
                tracing::error!("JDWP read error: {}", e);
                return Err(e);
            }
            Err(_) => {
                // Timeout - loop back to check cancellation
                continue;
            }
        }
    }

    Ok(())
}

/// Map a JdwpEvent to a TraceEvent with timing information.
///
/// This is a convenience function for testing without needing the full async loop.
pub fn jdwp_event_to_trace(
    event: crate::protocol::JdwpEvent,
    event_id: u64,
    timestamp_ns: u64,
) -> TraceEvent {
    jdwp_event_to_trace_event(event, event_id, timestamp_ns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::event_kind;

    #[test]
    fn test_jdwp_event_to_trace_conversion() {
        let jdwp_event = crate::protocol::JdwpEvent {
            kind: event_kind::METHOD_ENTRY,
            thread_id: 12345,
            class_signature: "Lcom/example/Foo;".to_string(),
            method_name: "bar".to_string(),
            line: Some(42),
        };

        let trace = jdwp_event_to_trace(jdwp_event, 1, 1000);

        assert_eq!(trace.event_id, 1);
        assert_eq!(trace.timestamp_ns, 1000);
        assert_eq!(trace.thread_id, 12345);
    }

    #[test]
    fn test_jdwp_event_to_trace_method_exit() {
        let jdwp_event = crate::protocol::JdwpEvent {
            kind: event_kind::METHOD_EXIT,
            thread_id: 99,
            class_signature: "Lmy/pkg/Util;".to_string(),
            method_name: "process".to_string(),
            line: Some(10),
        };

        let trace = jdwp_event_to_trace(jdwp_event, 2, 2000);

        assert_eq!(trace.event_type, chronos_domain::EventType::FunctionExit);
    }

    #[test]
    fn test_jdwp_event_to_trace_exception() {
        let jdwp_event = crate::protocol::JdwpEvent {
            kind: event_kind::EXCEPTION,
            thread_id: 777,
            class_signature: "Ljava/lang/NullPointerException;".to_string(),
            method_name: "getMessage".to_string(),
            line: Some(100),
        };

        let trace = jdwp_event_to_trace(jdwp_event, 3, 3000);

        assert_eq!(
            trace.event_type,
            chronos_domain::EventType::ExceptionThrown
        );
    }

    #[test]
    fn test_jdwp_event_to_trace_breakpoint() {
        let jdwp_event = crate::protocol::JdwpEvent {
            kind: event_kind::BREAKPOINT,
            thread_id: 888,
            class_signature: "Lcom/example/Main;".to_string(),
            method_name: "main".to_string(),
            line: Some(10),
        };

        let trace = jdwp_event_to_trace(jdwp_event, 4, 4000);

        assert_eq!(trace.event_type, chronos_domain::EventType::BreakpointHit);
    }
}
