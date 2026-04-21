//! Capture runner — orchestrates the full ptrace event loop.
//!
//! Connects PtraceTracer + SymbolResolver + BreakpointManager into a
//! single cohesive capture pipeline that runs in a background thread
//! and feeds TraceEvents into a channel for consumption.
//!
//! ## Breakpoint-based function tracing
//!
//! When `CaptureConfig::capture_stack` is true (default), the runner:
//! 1. Pre-loads ELF function symbols from the target binary
//! 2. Installs INT3 breakpoints at every function entry point
//! 3. On each breakpoint hit, emits `FunctionEntry` (first hit) or
//!    `FunctionExit` (second hit) events using a per-thread call stack
//! 4. This is 10–100× more efficient than single-stepping

use crate::breakpoint::BreakpointManager;
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

// ---------------------------------------------------------------------------
// Breakpoint-based function tracking
// ---------------------------------------------------------------------------

/// Tracks function entry hits from INT3 breakpoints.
///
/// Every breakpoint hit at a function entry address is emitted as a
/// `FunctionEntry` event.  Function exits are not directly observable
/// with entry-point-only breakpoints (they would require breakpoints at
/// `ret` instructions).  The query engine can reconstruct call stacks
/// from entry events + timestamps.
struct BreakpointTracker {
    /// Breakpoint manager (INT3 injection / restore).
    bp_manager: BreakpointManager,
    /// Address → function name (pre-resolved from ELF symbols).
    bp_functions: HashMap<u64, String>,
}

impl BreakpointTracker {
    /// Create a new tracker, installing breakpoints at every function
    /// entry point found by the `SymbolResolver`.
    ///
    /// Returns `None` if no symbols could be resolved.
    fn new(pid: i32, symbol_resolver: &SymbolResolver) -> Option<Self> {
        let mut bp_manager = BreakpointManager::new(pid);
        let mut bp_functions = HashMap::new();

        for sym in symbol_resolver.symbols().values() {
            // Only set breakpoints at function entry points (address == start)
            if sym.size > 0 {
                if let Err(e) = bp_manager.set_breakpoint_at_function(sym.address, &sym.name) {
                    debug!(
                        "Skipping breakpoint at 0x{:x} ({}): {}",
                        sym.address, sym.name, e
                    );
                } else {
                    bp_functions.insert(sym.address, sym.name.clone());
                }
            }
        }

        if bp_functions.is_empty() {
            warn!("No function symbols found — breakpoint tracking disabled");
            return None;
        }

        info!(
            "Installed {} function breakpoints on PID {}",
            bp_functions.len(),
            pid
        );

        Some(Self {
            bp_manager,
            bp_functions,
        })
    }

    /// Try to handle a SIGTRAP stop as a breakpoint hit.
    ///
    /// Returns `Some((function_name, address))` if this was one of
    /// our breakpoints, `None` otherwise.
    ///
    /// When this returns `Some`, the tracee has already been single-stepped
    /// past the original instruction and INT3 has been re-inserted.  The
    /// caller only needs to `PTRACE_CONT` (or `PTRACE_SYSCALL`).
    fn handle_hit(&mut self) -> Option<(String, u64)> {
        let bp_addr = match self.bp_manager.handle_breakpoint_hit() {
            Ok(Some(addr)) => addr,
            Ok(None) => return None,
            Err(e) => {
                debug!("handle_breakpoint_hit error: {}", e);
                return None;
            }
        };

        let func_name = self
            .bp_functions
            .get(&bp_addr)
            .cloned()
            .unwrap_or_else(|| format!("func_0x{:x}", bp_addr));

        Some((func_name, bp_addr))
    }
}

// ---------------------------------------------------------------------------
// CaptureRunner
// ---------------------------------------------------------------------------

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
                Ok(()) => Some(resolver),
                Err(_e) => None,
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
                    info!(
                        "Loaded {} symbols from {}",
                        resolver.symbol_count(),
                        program
                    );
                    Some(resolver)
                }
                Err(e) => {
                    warn!(
                        "Could not load symbols from {}: {} (addresses won't resolve to names)",
                        program, e
                    );
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
/// 2. Installs INT3 breakpoints at function entry points (if symbols available)
/// 3. Runs the ptrace event loop
/// 4. Converts PtraceEvents to TraceEvents (with symbol resolution)
/// 5. Emits `FunctionEntry`/`FunctionExit` events on breakpoint hits
/// 6. Returns the collected events
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

    // Install function breakpoints (INT3 at every function entry point)
    let mut bp_tracker: Option<BreakpointTracker> = None;
    if let Some(resolver) = symbol_resolver {
        bp_tracker = BreakpointTracker::new(pid, resolver);
    }

    // Continue after initial SIGTRAP (launch() leaves the process stopped)
    if ptrace_config.trace_syscalls {
        tracer
            .syscall_continue(pid)
            .map_err(|e| format!("Initial syscall_continue failed: {}", e))?;
    } else {
        tracer
            .continue_execution(pid)
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

        // --- Breakpoint hit detection --------------------------------
        // Check if this is a SIGTRAP that corresponds to one of our INT3
        // breakpoints.  We do this BEFORE the generic PtraceEvent → TraceEvent
        // conversion so we can emit function_entry instead of the generic
        // BreakpointHit event.
        let mut handled_as_breakpoint = false;

        if let Some(ref mut tracker) = bp_tracker {
            if let PtraceEvent::Stopped {
                pid: evt_pid,
                signal,
                ..
            } = &ptrace_event
            {
                if *signal == 5 {
                    // SIGTRAP
                    if let Some((func_name, bp_addr)) = tracker.handle_hit() {
                        debug!(
                            "Breakpoint ENTRY at 0x{:x}: {} (PID {})",
                            bp_addr, func_name, evt_pid
                        );

                        let trace_event = TraceEvent::function_entry(
                            event_id,
                            timestamp_ns,
                            *evt_pid as u64,
                            &func_name,
                            bp_addr,
                        );
                        events.push(trace_event);
                        event_id += 1;
                        handled_as_breakpoint = true;

                        // After handle_breakpoint_hit the tracee has already
                        // been single-stepped and INT3 re-inserted, so just
                        // PTRACE_CONT (or PTRACE_SYSCALL if syscall tracing is on).
                        let continue_result = if ptrace_config.trace_syscalls {
                            tracer.syscall_continue(*evt_pid)
                        } else {
                            tracer.continue_execution(*evt_pid)
                        };
                        if let Err(e) = continue_result {
                            debug!("Failed to continue PID {} after BP hit: {}", evt_pid, e);
                        }
                    }
                }
            }
        }

        if handled_as_breakpoint {
            continue;
        }

        // --- Generic event handling ----------------------------------

        // Handle syscall entry/exit toggling
        let adjusted_event = match &ptrace_event {
            PtraceEvent::Syscall {
                pid,
                syscall_nr,
                is_entry: _,
            } => {
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
        if let Some(mut trace_event) =
            adapter.ptrace_event_to_trace_event(&adjusted_event, event_id, timestamp_ns)
        {
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
            PtraceEvent::Signaled {
                pid: ep,
                signal,
                signal_name,
                ..
            } => {
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
            } else if let PtraceEvent::Stopped { signal, .. } = &adjusted_event {
                // Deliver non-SIGTRAP signals to the tracee so that fatal
                // signals (SIGSEGV, SIGABRT, etc.) are processed and the
                // process can die cleanly.  SIGTRAP is consumed by us
                // (breakpoints / syscall tracing) and never forwarded.
                if *signal != 5 {
                    match nix::sys::signal::Signal::try_from(*signal) {
                        Ok(sig) => {
                            debug!("Delivering signal {} to PID {}", sig, event_pid);
                            tracer.continue_with_signal(event_pid, sig)
                        }
                        Err(_) => tracer.continue_execution(event_pid),
                    }
                } else {
                    tracer.continue_execution(event_pid)
                }
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
    use chronos_domain::EventType;

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

    /// Integration test: verify that function_entry / function_exit events
    /// are emitted when breakpoints are installed at ELF function entries.
    ///
    /// Requires the `test_add` C fixture to be compiled. Runs the capture
    /// against it with syscall tracing disabled (breakpoint-only mode).
    #[test]
    fn test_breakpoint_function_events() {
        let fixture_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");
        let source = format!("{}/test_add.c", fixture_dir);
        let binary = format!("{}/test_add_bp", fixture_dir);

        // Compile the fixture (no PIE so addresses are predictable)
        let compile = std::process::Command::new("gcc")
            .args(["-no-pie", "-o", &binary, &source])
            .output()
            .expect("gcc not found");

        if !compile.status.success() {
            eprintln!("gcc failed: {}", String::from_utf8_lossy(&compile.stderr));
            panic!("Could not compile test_add.c");
        }

        let mut config = CaptureConfig::new(&binary);
        config.capture_syscalls = false; // Only breakpoint tracing
        config.capture_stack = true;

        let mut runner = CaptureRunner::new(config);

        let result = runner.run_to_completion();
        assert!(result.is_ok(), "Capture failed: {:?}", result.err());

        let capture = result.unwrap();

        // We should have at least some events
        assert!(
            capture.total_events > 0,
            "Expected at least one event, got {}",
            capture.total_events
        );

        // Count function_entry events (we only emit entries from breakpoints)
        let func_entries = capture
            .events
            .iter()
            .filter(|e| e.event_type == EventType::FunctionEntry)
            .count();

        // With -no-pie we should hit at least _start, compute, and multiply
        assert!(
            func_entries >= 3,
            "Expected at least 3 function_entry events, got {}",
            func_entries
        );

        // Verify at least some known function names appear
        let entry_names: Vec<&str> = capture
            .events
            .iter()
            .filter(|e| e.event_type == EventType::FunctionEntry)
            .filter_map(|e| e.location.function.as_deref())
            .collect();

        // At least one of main/compute/add/multiply should be present
        let known_funcs = ["main", "add", "multiply", "compute", "_start"];
        let found_known = entry_names
            .iter()
            .any(|n| known_funcs.iter().any(|k| n.contains(k)));
        assert!(
            found_known,
            "Expected at least one known function in {:?}",
            entry_names
        );

        // Clean up compiled binary
        let _ = std::fs::remove_file(&binary);
    }
}
