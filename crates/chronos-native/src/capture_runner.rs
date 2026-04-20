//! Capture runner — orchestrates the full ptrace event loop.
//!
//! Connects PtraceTracer + SymbolResolver + BreakpointManager into a
//! single cohesive capture pipeline that runs in a background thread
//! and feeds TraceEvents into a channel for consumption.

use crate::native_adapter::NativeAdapter;
use crate::ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
use crate::symbol_resolver::SymbolResolver;
use chronos_domain::{CaptureConfig, SourceLocation, TraceEvent};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// State of a running capture.
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureState {
    /// Capture is being set up.
    Initializing,
    /// Actively collecting events.
    Running,
    /// Capture finished (process exited or stopped).
    Finished,
    /// Capture failed with an error.
    Failed(String),
}

/// Result of a completed capture session.
#[derive(Debug)]
pub struct CaptureResult {
    /// Collected trace events.
    pub events: Vec<TraceEvent>,
    /// Why the capture ended.
    pub end_reason: CaptureEndReason,
    /// Total events captured.
    pub total_events: u64,
}

/// Why the capture ended.
#[derive(Debug, Clone)]
pub enum CaptureEndReason {
    /// Process exited normally with exit code.
    Exited(i32),
    /// Process was killed by a signal.
    Signaled { signal: i32, signal_name: String },
    /// Capture was stopped by user.
    StoppedByUser,
    /// Capture failed with error.
    Failed(String),
}

/// Orchestrates a complete ptrace capture session.
///
/// Usage:
/// ```no_run
/// use chronos_native::capture_runner::CaptureRunner;
/// use chronos_domain::CaptureConfig;
///
/// let config = CaptureConfig::new("./my_program");
/// let mut runner = CaptureRunner::new(config);
///
/// // Start capture in background thread
/// let handle = runner.start().unwrap();
///
/// // ... wait or do other work ...
///
/// // Stop and collect results
/// let result = runner.stop_and_collect().unwrap();
/// println!("Captured {} events", result.total_events);
/// ```
pub struct CaptureRunner {
    config: CaptureConfig,
    ptrace_config: PtraceConfig,
    /// Shared stop flag.
    stop_flag: Arc<AtomicBool>,
    /// The join handle for the background thread.
    thread_handle: Option<std::thread::JoinHandle<Result<CaptureResult, String>>>,
}

impl CaptureRunner {
    /// Create a new capture runner with the given config.
    pub fn new(config: CaptureConfig) -> Self {
        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
        };

        Self {
            config,
            ptrace_config,
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Set custom ptrace configuration.
    pub fn with_ptrace_config(mut self, config: PtraceConfig) -> Self {
        self.ptrace_config = config;
        self
    }

    /// Start the capture, wait for the process to finish, and return results.
    ///
    /// This blocks until the traced process exits. For short-lived programs
    /// this is ideal. For long-running programs, use `start_background`.
    pub fn run_to_completion(&mut self) -> Result<CaptureResult, String> {
        let program = self.config.target.clone();
        let args = self.config.args.clone();
        let ptrace_config = self.ptrace_config.clone();
        let stop_flag = self.stop_flag.clone();

        let binary_path = PathBuf::from(&program);
        if !binary_path.exists() {
            return Err(format!("Target binary not found: {}", program));
        }

        // Pre-load symbols
        let symbol_resolver = {
            let mut resolver = SymbolResolver::new();
            match resolver.load_from_binary(&binary_path) {
                Ok(()) => {
                    Some(resolver)
                }
                Err(_e) => {
                    None
                }
            }
        };

        run_capture_loop(
            &binary_path,
            &args,
            &ptrace_config,
            &stop_flag,
            symbol_resolver.as_ref(),
        )
    }

    /// Start capture in a background thread (for long-running programs).
    pub fn start(&mut self) -> Result<i32, String> {
        let program = self.config.target.clone();
        let args = self.config.args.clone();
        let ptrace_config = self.ptrace_config.clone();
        let stop_flag = self.stop_flag.clone();

        let binary_path = PathBuf::from(&program);
        if !binary_path.exists() {
            return Err(format!("Target binary not found: {}", program));
        }

        // Pre-load symbols from the binary before forking
        let symbol_resolver = {
            let mut resolver = SymbolResolver::new();
            match resolver.load_from_binary(&binary_path) {
                Ok(()) => {
                    info!("Loaded {} symbols from {}", resolver.symbol_count(), program);
                    Some(resolver)
                }
                Err(e) => {
                    warn!("Could not load symbols from {}: {} (addresses won't resolve to names)", program, e);
                    None
                }
            }
        };

        let thread_handle = std::thread::Builder::new()
            .name("chronos-capture".into())
            .spawn(move || {
                run_capture_loop(
                    &binary_path,
                    &args,
                    &ptrace_config,
                    &stop_flag,
                    symbol_resolver.as_ref(),
                )
            })
            .map_err(|e| format!("Failed to spawn capture thread: {}", e))?;

        self.thread_handle = Some(thread_handle);

        // Give the thread a moment to start and fork
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(0) // PID is inside the thread; we return a placeholder
    }

    /// Stop the capture and collect all events.
    ///
    /// If the process already exited, this just collects the results.
    /// If it's still running, sets the stop flag and waits.
    pub fn stop_and_collect(&mut self) -> Result<CaptureResult, String> {
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.thread_handle.take() {
            match handle.join() {
                Ok(result) => result,
                Err(_) => Err("Capture thread panicked".into()),
            }
        } else {
            Err("No capture thread running".into())
        }
    }

}

/// Main capture loop — runs in the background thread.
///
/// This function:
/// 1. Forks and execs the target
/// 2. Runs the ptrace event loop
/// 3. Converts PtraceEvents to TraceEvents (with symbol resolution)
/// 4. Returns the collected events
fn run_capture_loop(
    program: &Path,
    args: &[String],
    ptrace_config: &PtraceConfig,
    stop_flag: &AtomicBool,
    symbol_resolver: Option<&SymbolResolver>,
) -> Result<CaptureResult, String> {
    let mut tracer = PtraceTracer::new(ptrace_config.clone());
    let adapter = NativeAdapter::new();

    // Launch the target
    let pid = tracer
        .launch(program, args)
        .map_err(|e| format!("Launch failed: {}", e))?;

    info!("Capture started: PID {} for {}", pid, program.display());

    // Continue after initial SIGTRAP (launch() leaves the process stopped)
    if ptrace_config.trace_syscalls {
        tracer.syscall_continue(pid)
            .map_err(|e| format!("Initial syscall_continue failed: {}", e))?;
    } else {
        tracer.continue_execution(pid)
            .map_err(|e| format!("Initial continue failed: {}", e))?;
    }

    let mut events: Vec<TraceEvent> = Vec::new();
    let mut event_id: u64 = 0;
    let mut end_reason = CaptureEndReason::StoppedByUser;

    // Syscall state tracking (toggle between entry/exit)
    let mut syscall_is_entry: HashMap<i32, bool> = HashMap::new();

    // Event loop
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            info!("Stop flag set, ending capture");
            end_reason = CaptureEndReason::StoppedByUser;
            break;
        }

        let ptrace_event = match tracer.wait_event() {
            Ok(Some(event)) => event,
            Ok(None) => {
                eprintln!("[chronos] No more traced processes (ECHILD)");
                if matches!(end_reason, CaptureEndReason::StoppedByUser) {
                    end_reason = CaptureEndReason::Exited(0);
                }
                break;
            }
            Err(e) => {
                end_reason = CaptureEndReason::Failed(format!("wait_event: {}", e));
                break;
            }
        };

        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Handle syscall entry/exit toggling
        let adjusted_event = match &ptrace_event {
            PtraceEvent::Syscall { pid, syscall_nr, is_entry: _ } => {
                let is_entry = syscall_is_entry.entry(*pid).or_insert(true);
                let current = *is_entry;
                *is_entry = !current;
                PtraceEvent::Syscall {
                    pid: *pid,
                    syscall_nr: *syscall_nr,
                    is_entry: current,
                }
            }
            other => other.clone(),
        };

        // Convert to TraceEvent
        if let Some(mut trace_event) = adapter.ptrace_event_to_trace_event(
            &adjusted_event,
            event_id,
            timestamp_ns,
        ) {
            // Resolve symbol for the address
            if let Some(resolver) = symbol_resolver {
                let addr = trace_event.location.address;
                if addr > 0 {
                    if let Some(sym) = resolver.resolve(addr) {
                        trace_event.location = SourceLocation::new(
                            sym.file.as_deref().unwrap_or(""),
                            sym.line.unwrap_or(0),
                            &sym.name,
                            addr,
                        );
                    }
                }
            }
            events.push(trace_event);
            event_id += 1;
        }

        // Handle end-of-process events
        match &adjusted_event {
            PtraceEvent::Exited { pid: ep, exit_code } => {
                info!("PID {} exited with code {}", ep, exit_code);
                end_reason = CaptureEndReason::Exited(*exit_code);
            }
            PtraceEvent::Signaled { pid: ep, signal, signal_name, .. } => {
                info!("PID {} killed by {} ({})", ep, signal_name, signal);
                end_reason = CaptureEndReason::Signaled {
                    signal: *signal,
                    signal_name: signal_name.clone(),
                };
            }
            _ => {}
        }

        // Continue the traced process after handling the event.
        let event_pid = adjusted_event.pid();

        let should_continue = event_pid > 0
            && !matches!(
                adjusted_event,
                PtraceEvent::Exited { .. }
                    | PtraceEvent::Signaled { .. }
                    | PtraceEvent::PtraceEvent { .. }
            );

        if should_continue {
            let continue_result = if ptrace_config.trace_syscalls
                && matches!(adjusted_event, PtraceEvent::Syscall { .. })
            {
                tracer.syscall_continue(event_pid)
            } else {
                tracer.continue_execution(event_pid)
            };

            if let Err(e) = continue_result {
                debug!("Failed to continue PID {}: {}", event_pid, e);
            }
        }
    }

    let total = events.len() as u64;
    info!("Capture ended: {} events collected", total);

    Ok(CaptureResult {
        events,
        end_reason,
        total_events: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_runner_new() {
        let config = CaptureConfig::new("./test");
        let runner = CaptureRunner::new(config);
        assert!(!runner.stop_flag.load(Ordering::SeqCst));
        assert!(runner.thread_handle.is_none());
    }

    #[test]
    fn test_capture_runner_nonexistent_binary() {
        let config = CaptureConfig::new("/nonexistent/binary");
        let mut runner = CaptureRunner::new(config);
        let result = runner.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_capture_result_default() {
        let result = CaptureResult {
            events: vec![],
            end_reason: CaptureEndReason::Exited(0),
            total_events: 0,
        };
        assert_eq!(result.total_events, 0);
        assert!(result.events.is_empty());
    }
}
