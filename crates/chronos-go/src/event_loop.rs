//! Poll-based Delve event loop for Go trace capture.
//!
//! This module implements the capture loop that drives Delve's continue/step
//! commands and harvests goroutine state changes as TraceEvents.
//!
//! # Goroutine IDs vs OS Threads
//!
//! IMPORTANT: Go goroutines are NOT the same as OS threads. Go uses an M:N
//! threading model where multiple goroutines are multiplexed onto fewer OS
//! threads. When we report thread_id in TraceEvent, we use the goroutine ID
//! (not the underlying thread ID) because:
//! - Goroutine IDs are stable across context switches
//! - They uniquely identify the Go execution unit of interest
//! - The debugger API exposes goroutine IDs, not raw thread IDs

use std::sync::Arc;
use std::time::Instant;

use chronos_domain::{EventData, GoEventKind, SourceLocation, StackFrame as ChronosStackFrame, ThreadInfo, ThreadState, TraceEvent, VariableInfo};
use tokio::sync::{mpsc, Mutex as TokioMutex};

use crate::error::GoError;
use crate::rpc::{DelveClient, GoroutineInfo, StackFrame};

/// Shared Delve RPC client wrapped in tokio Mutex for thread-safe access.
///
/// We use tokio's Mutex (not std::sync::Mutex) because:
/// - The event loop is async and needs to await while holding the lock
/// - tokio::sync::Mutex is Send + Sync, allowing use in async contexts
pub type DelveRpcClient = TokioMutex<DelveClient>;

/// Run the Delve event loop.
///
/// This function drives Delve's continue command and harvests goroutine state
/// changes as TraceEvents. The loop polls Delve for state transitions and emits
/// events on each stop.
///
/// # Polling Strategy
///
/// The loop works as follows:
/// 1. Issue `Command { Name: "continue" }` to resume execution
/// 2. Poll `RPCServer.State` until `currentThread` changes or `exited` is true
/// 3. On each halt (breakpoint, step, goroutine transition):
///    - Call `ListGoroutinesOut` to discover all goroutines
///    - For the current goroutine, call `StacktraceOut` for its stack
///    - Convert each frame to a TraceEvent
/// 4. On cancellation → send `Command { Name: "halt" }` then exit
///
/// # Arguments
/// * `rpc` - Arc wrapper around the Delve RPC client
/// * `events_tx` - Channel sender for TraceEvents
/// * `cancel` - Cancellation flag to signal stop
pub async fn run_delve_event_loop(
    rpc: Arc<DelveRpcClient>,
    events_tx: mpsc::Sender<TraceEvent>,
    cancel: Arc<AtomicCancel>,
) -> Result<(), GoError> {
    tracing::info!("Delve event loop starting");

    // Track next event ID
    let mut next_event_id: u64 = 1;

    // Track last known goroutine to detect switches
    let mut last_goroutine_id: Option<i64> = None;

    loop {
        // Check cancellation first
        if cancel.is_cancelled() {
            tracing::info!("Delve event loop received cancellation");
            // Halt Delve before exiting
            let _ = rpc.lock().await.command("halt").await;
            break;
        }

        // Issue continue command to resume execution
        let state = match rpc.lock().await.command("continue").await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Delve continue command failed: {}", e);
                break;
            }
        };

        // Check if process exited
        if state.exited == Some(true) {
            tracing::info!("Delve target process exited");
            break;
        }

        // Get current thread info
        let current_thread = match &state.currentThread {
            Some(t) => t,
            None => {
                // No thread info, wait a bit and continue
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                continue;
            }
        };

        let goroutine_id = current_thread.goroutineID;
        let timestamp_ns = Instant::now()
            .elapsed()
            .as_nanos() as u64;

        // Check if goroutine changed (scheduler switch)
        let event_kind = if last_goroutine_id != Some(goroutine_id) {
            last_goroutine_id = Some(goroutine_id);
            GoEventKind::GoroutineStop
        } else {
            // Check if we stopped at a breakpoint or stepped
            GoEventKind::Breakpoint
        };

        // Get full goroutine list for this stop
        let goroutines = match rpc.lock().await.list_goroutines().await {
            Ok(gs) => gs,
            Err(e) => {
                tracing::warn!("Failed to list goroutines: {}", e);
                Vec::new()
            }
        };

        // Emit events for each goroutine's current location
        for goroutine in &goroutines {
            let frame = &goroutine.currentLoc;

            // Determine event kind for this specific goroutine
            let kind = if goroutine.id == goroutine_id {
                event_kind.clone()
            } else {
                GoEventKind::Breakpoint // Other goroutines at breakpoint
            };

            // Get stacktrace for this goroutine if it's the current one
            let locals = if goroutine.id == goroutine_id {
                get_goroutine_locals(&rpc, goroutine.id, 20).await
            } else {
                None
            };

            // Convert to TraceEvent
            let trace_event = goroutine_to_trace_event(
                goroutine,
                frame,
                next_event_id,
                timestamp_ns,
                kind,
                locals,
            );

            next_event_id += 1;

            // Send event (non-blocking)
            if events_tx.send(trace_event).await.is_err() {
                tracing::warn!("Event channel closed, stopping loop");
                return Ok(());
            }
        }

        // Brief pause before next continue to avoid busy-looping
        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
    }

    tracing::info!("Delve event loop terminated");
    Ok(())
}

/// Simple atomic cancellation flag.
///
/// We use this instead of tokio_util::CancellationToken to avoid
/// the tokio-util dependency version conflict in the workspace.
#[derive(Debug, Clone)]
pub struct AtomicCancel(Arc<std::sync::atomic::AtomicBool>);

impl AtomicCancel {
    pub fn new() -> Self {
        Self(Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for AtomicCancel {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a goroutine's state to a TraceEvent.
fn goroutine_to_trace_event(
    goroutine: &GoroutineInfo,
    frame: &StackFrame,
    event_id: u64,
    timestamp_ns: u64,
    kind: GoEventKind,
    locals: Option<Vec<VariableInfo>>,
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

    let data = EventData::GoFrame {
        goroutine_id: goroutine.id as u64,
        function_name,
        file,
        line,
        locals,
        event_kind: kind,
    };

    // NOTE: thread_id is the goroutine ID, not the OS thread ID
    // Go goroutines are M:N scheduled onto OS threads
    TraceEvent {
        event_id,
        timestamp_ns,
        thread_id: goroutine.id as u64,
        event_type: chronos_domain::EventType::BreakpointHit,
        location,
        data,
    }
}

/// Get local variables for a goroutine at a given stack depth.
async fn get_goroutine_locals(
    rpc: &Arc<DelveRpcClient>,
    goroutine_id: i64,
    depth: i32,
) -> Option<Vec<VariableInfo>> {
    let frames = rpc.lock().await.stacktrace(goroutine_id, depth).await.ok()?;

    // Get locals from the top frame (depth 0)
    let top_frame = frames.first()?;
    let locals = top_frame.locals.as_ref()?;

    Some(
        locals
            .iter()
            .map(|v| {
                VariableInfo::new(
                    &v.name,
                    &v.value,
                    "unknown", // type_name not available in basic Delve response
                    0,         // address not available
                    chronos_domain::VariableScope::Local,
                )
            })
            .collect(),
    )
}

/// Convert Delve stack frames to Chronos stack frames.
pub fn delve_stack_to_chronos_frames(
    frames: &[StackFrame],
    start_frame_id: u64,
) -> Vec<ChronosStackFrame> {
    frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let function_name = f
                .function
                .as_ref()
                .map(|fn_| fn_.name.clone())
                .unwrap_or_default();

            let variables = f.locals.as_ref().map(|vars| {
                vars.iter()
                    .map(|v| {
                        VariableInfo::new(
                            &v.name,
                            &v.value,
                            "unknown",
                            0,
                            chronos_domain::VariableScope::Local,
                        )
                    })
                    .collect()
            }).unwrap_or_default();

            ChronosStackFrame {
                frame_id: start_frame_id + i as u64,
                function_name,
                source_file: Some(f.file.clone()),
                line: Some(f.line as u32),
                variables,
            }
        })
        .collect()
}

/// Convert a list of Delve goroutines to ThreadInfo.
pub fn goroutines_to_thread_info(goroutines: &[GoroutineInfo]) -> Vec<ThreadInfo> {
    goroutines
        .iter()
        .map(|g| {
            // Use goroutine ID as thread_id
            // Go goroutines ≠ OS threads - we use goroutine ID for traceability
            ThreadInfo {
                thread_id: g.id as u64,
                name: format!("Goroutine {}", g.id),
                state: ThreadState::Running, // Delve doesn't provide goroutine state
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_cancel_basics() {
        let cancel = AtomicCancel::new();
        assert!(!cancel.is_cancelled());

        cancel.cancel();
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn test_atomic_cancel_clone() {
        let cancel = AtomicCancel::new();
        let cancel2 = cancel.clone();

        assert!(!cancel2.is_cancelled());
        cancel.cancel();
        assert!(cancel2.is_cancelled());
    }

    #[test]
    fn test_goroutines_to_thread_info() {
        let goroutines = vec![
            GoroutineInfo {
                id: 1,
                currentLoc: StackFrame {
                    function: Some(crate::rpc::FunctionInfo {
                        name: "main.main".to_string(),
                    }),
                    file: "/path/main.go".to_string(),
                    line: 10,
                    locals: None,
                },
            },
            GoroutineInfo {
                id: 2,
                currentLoc: StackFrame {
                    function: Some(crate::rpc::FunctionInfo {
                        name: "main.foo".to_string(),
                    }),
                    file: "/path/main.go".to_string(),
                    line: 25,
                    locals: None,
                },
            },
        ];

        let threads = goroutines_to_thread_info(&goroutines);

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].thread_id, 1);
        assert_eq!(threads[0].name, "Goroutine 1");
        assert_eq!(threads[1].thread_id, 2);
        assert_eq!(threads[1].name, "Goroutine 2");
    }

    #[test]
    fn test_delve_stack_to_chronos_frames() {
        let frames = vec![
            StackFrame {
                function: Some(crate::rpc::FunctionInfo {
                    name: "main.process".to_string(),
                }),
                file: "/path/main.go".to_string(),
                line: 15,
                locals: Some(vec![crate::rpc::DelveVar {
                    name: "x".to_string(),
                    value: "42".to_string(),
                }]),
            },
            StackFrame {
                function: Some(crate::rpc::FunctionInfo {
                    name: "main.main".to_string(),
                }),
                file: "/path/main.go".to_string(),
                line: 30,
                locals: None,
            },
        ];

        let chronos_frames = delve_stack_to_chronos_frames(&frames, 0);

        assert_eq!(chronos_frames.len(), 2);
        assert_eq!(chronos_frames[0].frame_id, 0);
        assert_eq!(chronos_frames[0].function_name, "main.process");
        assert_eq!(chronos_frames[0].line, Some(15));
        assert_eq!(chronos_frames[0].variables.len(), 1);

        assert_eq!(chronos_frames[1].frame_id, 1);
        assert_eq!(chronos_frames[1].function_name, "main.main");
        assert_eq!(chronos_frames[1].line, Some(30));
    }
}