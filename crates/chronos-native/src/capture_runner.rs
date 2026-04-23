//! Capture runner — orchestrates the full ptrace event loop.
//!
//! Connects PtraceTracer + SymbolResolver into a single cohesive capture
//! pipeline that runs in a background thread and feeds TraceEvents into a
//! channel for consumption.
//!
//! ## Function entry tracking
//!
//! The runner pre-loads ELF function symbols from the target binary and
//! emits `FunctionEntry` events when execution stops at known function entry
//! addresses (SIGTRAP stops that match symbol addresses).

use crate::dwarf::DwarfReader;
use crate::native_adapter::NativeAdapter;
use crate::ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
use crate::symbol_resolver::SymbolResolver;
use chronos_domain::{CaptureConfig, SourceLocation, TraceEvent};
use nix::sys::ptrace;
use nix::unistd::Pid;
use std::collections::HashMap;
use std::path::PathBuf;
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
// AttachMode
// ---------------------------------------------------------------------------

/// Describes how the traced process was started.
#[derive(Debug, Clone)]
pub enum AttachMode {
    /// Fork+exec a new process (existing behavior).
    Spawn { program: PathBuf, args: Vec<String> },
    /// Attach to an already-running process by PID.
    Attach(u32), // PID
}

// ---------------------------------------------------------------------------
// Function entry tracking
// ---------------------------------------------------------------------------

/// Tracks function entries by matching SIGTRAP stops to known symbol addresses.
///
/// When execution stops at a known function entry address (via SIGTRAP),
/// emits a `FunctionEntry` event. This is a simpler model than breakpoint
/// injection — we just observe natural SIGTRAP stops at function entries.
struct FunctionEntryTracker {
    /// Set of known function entry addresses.
    function_addresses: std::collections::HashSet<u64>,
    /// Address → function name (pre-resolved from ELF symbols).
    function_names: HashMap<u64, String>,
}

impl FunctionEntryTracker {
    /// Create a new tracker from the symbol resolver.
    ///
    /// Returns `None` if no symbols could be resolved.
    fn new(symbol_resolver: &SymbolResolver) -> Option<Self> {
        let mut function_addresses = std::collections::HashSet::new();
        let mut function_names = HashMap::new();

        for sym in symbol_resolver.symbols().values() {
            if sym.size > 0 {
                function_addresses.insert(sym.address);
                function_names.insert(sym.address, sym.name.clone());
            }
        }

        if function_addresses.is_empty() {
            warn!("No function symbols found — function entry tracking disabled");
            return None;
        }

        info!("Tracking {} function entry addresses", function_addresses.len());

        Some(Self {
            function_addresses,
            function_names,
        })
    }

    /// Check if the given address matches a known function entry.
    ///
    /// Returns `Some((function_name, address))` if this is a known function entry,
    /// `None` otherwise.
    fn try_get_function_entry(&self, address: u64) -> Option<(String, u64)> {
        if self.function_addresses.contains(&address) {
            let name = self
                .function_names
                .get(&address)
                .cloned()
                .unwrap_or_else(|| format!("func_0x{:x}", address));
            Some((name, address))
        } else {
            None
        }
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
    #[allow(dead_code)]
    /// If set, attach to this PID instead of spawning a new process.
    attach_pid: Option<u32>,
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
            attach_pid: None,
        }
    }

    /// Create a runner that attaches to an existing process by PID.
    pub fn attach_to(pid: u32, config: CaptureConfig) -> Self {
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
            attach_pid: Some(pid),
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

        let mode = AttachMode::Spawn {
            program: binary_path,
            args,
        };

        run_capture_loop(
            &mode,
            &ptrace_config,
            &stop_flag,
            symbol_resolver.as_ref(),
            self.config.max_duration_ms,
        )
    }

    /// Start capture in a background thread (for long-running programs).
    pub fn start(&mut self) -> Result<i32, String> {
        let program = self.config.target.clone();
        let args = self.config.args.clone();
        let ptrace_config = self.ptrace_config.clone();
        let stop_flag = self.stop_flag.clone();
        let max_duration_ms = self.config.max_duration_ms;

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

        let mode = AttachMode::Spawn {
            program: binary_path,
            args,
        };

        let thread_handle = std::thread::Builder::new()
            .name("chronos-capture".into())
            .spawn(move || {
                run_capture_loop(
                    &mode,
                    &ptrace_config,
                    &stop_flag,
                    symbol_resolver.as_ref(),
                    max_duration_ms,
                )
            })
            .map_err(|e| format!("Failed to spawn capture thread: {}", e))?;

        self.thread_handle = Some(thread_handle);

        // Give the thread a moment to start and fork
        std::thread::sleep(std::time::Duration::from_millis(50));

        Ok(0) // PID is inside the thread; we return a placeholder
    }

    /// Run attach-based capture to completion (blocking).
    ///
    /// This is for attach mode where the process may not be an ELF binary
    /// we can symbol-resolve, so symbol_resolver is None.
    pub fn run_to_completion_attach(pid: u32, config: CaptureConfig) -> Result<CaptureResult, String> {
        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
        };

        let stop_flag = Arc::new(AtomicBool::new(false));
        let mode = AttachMode::Attach(pid);

        run_capture_loop(&mode, &ptrace_config, &stop_flag, None, config.max_duration_ms)
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
/// 1. Forks and execs the target (spawn mode) OR attaches to an existing PID (attach mode)
/// 2. Installs INT3 breakpoints at function entry points (if symbols available)
/// 3. Runs the ptrace event loop
/// 4. Converts PtraceEvents to TraceEvents (with symbol resolution)
/// 5. Emits `FunctionEntry`/`FunctionExit` events on breakpoint hits
/// 6. Detaches from (spawn mode kills) the target on cleanup
/// 7. Returns the collected events
///
/// If `max_duration_ms` is Some, the loop will exit with CaptureEndReason::Failed
/// after the specified duration.
fn run_capture_loop(
    mode: &AttachMode,
    ptrace_config: &PtraceConfig,
    stop_flag: &AtomicBool,
    symbol_resolver: Option<&SymbolResolver>,
    max_duration_ms: Option<u64>,
) -> Result<CaptureResult, String> {
    let mut tracer = PtraceTracer::new(ptrace_config.clone());
    let adapter = NativeAdapter::new();

    // Launch or attach based on mode
    let (pid, target_info, dwarf_data_opt): (i32, String, Option<std::borrow::Cow<'static, [u8]>>) = match mode {
        AttachMode::Spawn { program, args } => {
            let pid = tracer
                .launch(program, args)
                .map_err(|e| format!("Launch failed: {}", e))?;
            info!("Capture started: PID {} for {}", pid, program.display());

            // Try to load DWARF debug info (best-effort - binary may be stripped)
            let dwarf_data: Option<std::borrow::Cow<'static, [u8]>> =
                std::fs::read(program).map(std::borrow::Cow::Owned).ok();
            (pid, format!("{}", program.display()), dwarf_data)
        }
        AttachMode::Attach(pid) => {
            tracer
                .attach(*pid as i32)
                .map_err(|e| format!("Attach to PID {} failed: {}", pid, e))?;
            info!("Attached to running process: PID {}", pid);
            (*pid as i32, format!("PID {}", pid), None)
        }
    };

    // DWARF debug info is only available in spawn mode (for the binary we launched)
    let dwarf_reader: Option<DwarfReader<'_>> = if let Some(ref data) = dwarf_data_opt {
        match DwarfReader::new(data) {
            Ok(reader) => {
                debug!("DWARF debug info loaded successfully");
                Some(reader)
            }
            Err(e) => {
                debug!("No DWARF info available ({}): {}", target_info, e);
                None
            }
        }
    } else {
        debug!("No DWARF info available for {}", target_info);
        None
    };

    // Install function entry tracker (SIGTRAP at known function addresses)
    let mut entry_tracker: Option<FunctionEntryTracker> = None;
    if let Some(resolver) = symbol_resolver {
        entry_tracker = FunctionEntryTracker::new(resolver);
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
    let start_time = std::time::Instant::now();

    // Syscall state tracking (toggle between entry/exit)
    let mut syscall_is_entry: HashMap<i32, bool> = HashMap::new();

    // Event loop
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            info!("Stop flag set, ending capture");
            end_reason = CaptureEndReason::StoppedByUser;
            break;
        }

        // Check timeout
        if let Some(max_ms) = max_duration_ms {
            if start_time.elapsed().as_millis() as u64 > max_ms {
                info!("Capture duration limit reached ({}ms), ending capture", max_ms);
                end_reason = CaptureEndReason::Failed(format!("timeout after {}ms", max_ms));
                break;
            }
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

        // --- Function entry detection --------------------------------
        // Check if this is a SIGTRAP stop at a known function entry address.
        // We do this BEFORE the generic PtraceEvent → TraceEvent conversion
        // so we can emit function_entry with the resolved symbol name.
        let mut handled_as_function_entry = false;

        if let Some(ref tracker) = entry_tracker {
            if let PtraceEvent::Stopped {
                pid: evt_pid,
                signal,
                ..
            } = &ptrace_event
            {
                if *signal == 5 {
                    // SIGTRAP - get the instruction pointer to check if we're at a function entry
                    let ip = match ptrace::getregs(Pid::from_raw(*evt_pid)) {
                        Ok(regs) => regs.rip,
                        Err(e) => {
                            debug!("Failed to get registers for PID {}: {}", evt_pid, e);
                            0
                        }
                    };

                    if let Some((func_name, func_addr)) = tracker.try_get_function_entry(ip) {
                        debug!(
                            "Function ENTRY at 0x{:x}: {} (PID {})",
                            func_addr, func_name, evt_pid
                        );

                        let mut trace_event = TraceEvent::function_entry(
                            event_id,
                            timestamp_ns,
                            *evt_pid as u64,
                            &func_name,
                            func_addr,
                        );

                        // Enrich with DWARF source location if available (best-effort)
                        if let Some(ref reader) = dwarf_reader {
                            if let Some(dwarf_loc) = reader.source_location(func_addr) {
                                trace_event.location.file = dwarf_loc.file;
                                trace_event.location.line = dwarf_loc.line;
                                trace_event.location.column = dwarf_loc.column;
                            }
                        }

                        events.push(trace_event);
                        event_id += 1;
                        handled_as_function_entry = true;

                        // Just continue - no breakpoint injection needed
                        let continue_result = if ptrace_config.trace_syscalls {
                            tracer.syscall_continue(*evt_pid)
                        } else {
                            tracer.continue_execution(*evt_pid)
                        };
                        if let Err(e) = continue_result {
                            debug!("Failed to continue PID {} after function entry: {}", evt_pid, e);
                        }
                    }
                }
            }
        }

        if handled_as_function_entry {
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
                PtraceEvent::Exited { .. } | PtraceEvent::Signaled { .. }
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
            } else if let PtraceEvent::PtraceEvent { new_pid, .. } = &adjusted_event {
                // PtraceEvent (clone/fork/vfork/exec): continue the parent AND
                // the newly-created child so both can proceed.
                let res = tracer.continue_execution(event_pid);
                if let Some(child_pid) = new_pid {
                    if *child_pid > 0 {
                        // The new child stops with SIGSTOP shortly after creation.
                        // Continue it so it can run. If it hasn't stopped yet this
                        // is harmless — the next wait_event will catch the SIGSTOP.
                        debug!("Continuing new child PID {}", child_pid);
                        if let Err(e) = tracer.continue_execution(*child_pid) {
                            debug!("Could not continue new child PID {}: {}", child_pid, e);
                        }
                    }
                }
                res
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

    // Clean up: detach (attach mode) or kill (spawn mode)
    // Only kill if the process is still alive (i.e., capture was stopped
    // by user or failed). If it exited or was signaled, it's already dead.
    match mode {
        AttachMode::Attach(pid) => {
            if let Err(e) = tracer.detach(*pid as i32) {
                warn!("Failed to detach from PID {}: {}", pid, e);
            } else {
                info!("Detached from PID {}, process continues", pid);
            }
        }
        AttachMode::Spawn { .. } => {
            let already_dead = matches!(
                end_reason,
                CaptureEndReason::Exited(_) | CaptureEndReason::Signaled { .. }
            );
            if already_dead {
                debug!(
                    "Process already dead ({:?}), skipping kill",
                    end_reason
                );
            } else if let Err(e) = tracer.kill(pid) {
                warn!("Failed to kill PID {}: {}", pid, e);
            } else {
                info!("Killed spawned PID {}", pid);
            }
        }
    }

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

    // ========================================================================
    // Attach mode tests
    // ========================================================================

    #[test]
    fn test_attach_mode_enum_spawn() {
        let mode = AttachMode::Spawn {
            program: PathBuf::from("/bin/true"),
            args: vec!["arg1".to_string()],
        };
        match mode {
            AttachMode::Spawn { program, args } => {
                assert_eq!(program, PathBuf::from("/bin/true"));
                assert_eq!(args, vec!["arg1"]);
            }
            AttachMode::Attach(_) => panic!("Expected Spawn variant"),
        }
    }

    #[test]
    fn test_attach_mode_enum_attach() {
        let mode = AttachMode::Attach(12345);
        match mode {
            AttachMode::Attach(pid) => assert_eq!(pid, 12345),
            AttachMode::Spawn { .. } => panic!("Expected Attach variant"),
        }
    }

    #[test]
    fn test_capture_runner_attach_to_constructor() {
        let config = CaptureConfig::new("irrelevant");
        let runner = CaptureRunner::attach_to(54321, config);
        // Can't access private field directly, but verify it doesn't panic
        assert!(!runner.stop_flag.load(Ordering::SeqCst));
        assert!(runner.thread_handle.is_none());
    }

    /// Integration test: attach to a `sleep 10` process, verify attach succeeds.
    /// May fail with permission error in CI environments without CAP_SYS_PTRACE.
    #[test]
    fn test_attach_to_sleep_process() {
        use std::process::Command;

        // Spawn a long-running process
        let mut child = Command::new("sleep").arg("10").spawn().expect("sleep should spawn");
        let pid = child.id();

        // Give it a moment to start
        std::thread::sleep(std::time::Duration::from_millis(100));

        let config = CaptureConfig::new("sleep 10");
        let result = CaptureRunner::run_to_completion_attach(pid, config);

        // Clean up: kill the sleep process
        let _ = child.kill();
        let _ = child.wait();

        match result {
            Ok(capture) => {
                // Attach succeeded - we may or may not have events depending on timing
                info!("Attach succeeded, {} events collected", capture.total_events);
            }
            Err(e) if e.contains("Operation not permitted") || e.contains("EPERM") => {
                // Expected in restricted CI environments without CAP_SYS_PTRACE
                info!("Expected permission error in restricted environment: {}", e);
            }
            Err(e) if e.contains("No such process") || e.contains("ESRCH") => {
                // Process may have already exited
                info!("Process already exited: {}", e);
            }
            Err(e) => {
                panic!("Unexpected attach error: {}", e);
            }
        }
    }

    /// Test that attaching to an invalid PID returns an appropriate error.
    #[test]
    fn test_attach_invalid_pid() {
        let config = CaptureConfig::new("nonexistent");
        // Use a very high PID that is unlikely to exist
        let result = CaptureRunner::run_to_completion_attach(999999999, config);
        assert!(
            result.is_err(),
            "Expected error for invalid PID"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("No such process") || err.contains("ESRCH") || err.contains("Attach"),
            "Expected ESRCH or attach error, got: {}",
            err
        );
    }
}
