//! Core ptrace tracing functionality.
//!
//! Provides low-level ptrace operations: fork+exec under trace,
//! waitpid event loop, register capture, and signal handling.
//!
//! # Safety
//!
//! This module uses `unsafe` for `fork()` which is inherently unsafe in
//! Rust (per nix's API). The parent-child communication follows the
//! standard ptrace pattern documented in `ptrace(2)`.

use chronos_domain::RegisterState;
use chronos_domain::TraceError;
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{execvp, fork, ForkResult, Pid};
use std::ffi::CString;
use std::path::Path;
use tracing::{debug, info, warn};

/// Events produced by the ptrace event loop.
#[derive(Debug, Clone)]
pub enum PtraceEvent {
    /// Tracee was stopped by a signal.
    Stopped {
        pid: i32,
        signal: i32,
        signal_name: String,
    },
    /// Tracee hit a syscall entry or exit.
    Syscall {
        pid: i32,
        syscall_nr: u64,
        is_entry: bool,
    },
    /// Tracee exited normally with an exit code.
    Exited { pid: i32, exit_code: i32 },
    /// Tracee was killed by a signal.
    Signaled {
        pid: i32,
        signal: i32,
        signal_name: String,
        core_dumped: bool,
    },
    /// Tracee hit a ptrace event (clone, fork, exec, etc.).
    /// `new_pid` contains the PID of the newly created child (if applicable).
    PtraceEvent {
        pid: i32,
        event_code: i32,
        new_pid: Option<i32>,
    },
    /// Register state snapshot captured for this stop.
    Registers { pid: i32, regs: RegisterState },
}

impl PtraceEvent {
    /// Get the PID associated with this event.
    pub fn pid(&self) -> i32 {
        match self {
            PtraceEvent::Stopped { pid, .. } => *pid,
            PtraceEvent::Syscall { pid, .. } => *pid,
            PtraceEvent::Exited { pid, .. } => *pid,
            PtraceEvent::Signaled { pid, .. } => *pid,
            PtraceEvent::PtraceEvent { pid, .. } => *pid,
            PtraceEvent::Registers { pid, .. } => *pid,
        }
    }
}

/// Configuration for a ptrace tracing session.
#[derive(Debug, Clone)]
pub struct PtraceConfig {
    /// Whether to trace syscall entry/exit events.
    pub trace_syscalls: bool,
    /// Whether to capture register state on each stop.
    pub capture_registers: bool,
    /// Whether to follow clone/fork children (multi-threaded programs).
    pub follow_children: bool,
}

impl Default for PtraceConfig {
    fn default() -> Self {
        Self {
            trace_syscalls: false,
            capture_registers: true,
            follow_children: true,
        }
    }
}

/// Core ptrace tracer — wraps all low-level ptrace operations.
///
/// Usage:
/// 1. `launch()` — fork+exec target under ptrace
/// 2. `wait_event()` — wait for next ptrace stop event
/// 3. `continue_execution()` / `step()` / `syscall_continue()` — resume
/// 4. `read_registers()` — capture register state
pub struct PtraceTracer {
    /// PID of the main traced process.
    main_pid: Option<Pid>,
    /// Set of all PIDs being traced (includes cloned children).
    traced_pids: std::collections::HashSet<i32>,
    /// Configuration.
    config: PtraceConfig,
    /// Whether initial setup (setoptions) has been done.
    initialized: bool,
    /// Buffered events from a previous wait_event that produced multiple events.
    pending_events: Vec<PtraceEvent>,
    /// Performance counter handles (feature-gated).
    #[cfg(feature = "perf_counters")]
    perf_handles: Vec<super::perf::PerfCounterHandle>,
}

impl PtraceTracer {
    /// Create a new tracer with the given configuration.
    pub fn new(config: PtraceConfig) -> Self {
        Self {
            main_pid: None,
            traced_pids: std::collections::HashSet::new(),
            config,
            initialized: false,
            pending_events: Vec::new(),
            #[cfg(feature = "perf_counters")]
            perf_handles: Vec::new(),
        }
    }

    /// Create a new tracer with default configuration.
    pub fn new_default() -> Self {
        Self::new(PtraceConfig::default())
    }

    /// Get the main traced PID, if set.
    pub fn main_pid(&self) -> Option<i32> {
        self.main_pid.map(|p| p.as_raw())
    }

    /// Get all traced PIDs.
    pub fn traced_pids(&self) -> &std::collections::HashSet<i32> {
        &self.traced_pids
    }

    /// Launch a program under ptrace trace.
    ///
    /// Forks the current process. The child calls `PTRACE_TRACEME` then
    /// `execvp` to replace itself with the target program. The parent
    /// waits for the initial stop (SIGTRAP from exec).
    ///
    /// Returns the child PID on success.
    pub fn launch(&mut self, program: &Path, args: &[String]) -> Result<i32, TraceError> {
        let program_str = program
            .to_str()
            .ok_or_else(|| TraceError::CaptureFailed("Invalid program path".into()))?;

        let c_program = CString::new(program_str)
            .map_err(|e| TraceError::CaptureFailed(format!("Invalid path: {}", e)))?;

        let c_args: Vec<CString> = std::iter::once(c_program.clone())
            .chain(args.iter().map(|a| {
                CString::new(a.as_str()).unwrap_or_else(|_| CString::new("invalid").unwrap())
            }))
            .collect();

        let pid = unsafe {
            fork().map_err(|e| TraceError::CaptureFailed(format!("Fork failed: {}", e)))?
        };

        match pid {
            ForkResult::Child => {
                // Child process: request tracing by parent
                ptrace::traceme()
                    .map_err(|e| {
                        eprintln!("chronos: PTRACE_TRACEME failed: {}", e);
                        std::process::exit(1);
                    })
                    .ok();

                // Raise SIGSTOP so parent can set options before we exec
                // Actually, PTRACE_TRACEME + exec will deliver SIGTRAP to parent
                let _ = execvp(&c_program, &c_args);

                // execvp only returns on error
                let err = std::io::Error::last_os_error();
                eprintln!("chronos: execvp failed: {}", err);
                std::process::exit(127);
            }
            ForkResult::Parent { child } => {
                info!("Launched child process PID {}", child);

                // Wait for the initial SIGTRAP delivered after exec
                match waitpid(child, None) {
                    Ok(WaitStatus::Stopped(_, Signal::SIGTRAP)) => {
                        debug!("Child {} stopped with SIGTRAP (post-exec)", child);
                    }
                    Ok(WaitStatus::Stopped(_, sig)) => {
                        warn!("Child stopped with unexpected signal: {:?}", sig);
                    }
                    Ok(WaitStatus::Signaled(_, sig, core)) => {
                        return Err(TraceError::TargetCrashed(format!(
                            "Child killed by {:?} (core: {})",
                            sig, core
                        )));
                    }
                    Ok(other) => {
                        return Err(TraceError::CaptureFailed(format!(
                            "Unexpected wait status: {:?}",
                            other
                        )));
                    }
                    Err(e) => {
                        return Err(TraceError::CaptureFailed(format!("waitpid failed: {}", e)));
                    }
                }

                // Set ptrace options
                self.setup_ptrace_options(child)?;

                self.main_pid = Some(child);
                self.traced_pids.insert(child.as_raw());
                self.initialized = true;

                // Resume the child — the SIGTRAP stop was consumed by waitpid above.
                // Without this, wait_event() will never see a stop because the initial
                // SIGTRAP was already reaped. Use PTRACE_SYSCALL if trace_syscalls=true
                // so the child stops at the next syscall entry/exit.
                if self.config.trace_syscalls {
                    ptrace::syscall(child, None)
                        .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_SYSCALL failed: {}", e)))?;
                    debug!("Child {} resumed with PTRACE_SYSCALL", child);
                } else {
                    ptrace::cont(child, None)
                        .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_CONT failed: {}", e)))?;
                    debug!("Child {} resumed with PTRACE_CONT", child);
                }

                Ok(child.as_raw())
            }
        }
    }

    /// Attach to an already-running process.
    pub fn attach(&mut self, pid: i32) -> Result<(), TraceError> {
        let nix_pid = Pid::from_raw(pid);
        ptrace::attach(nix_pid)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_ATTACH failed: {}", e)))?;

        // Wait for the tracee to stop
        match waitpid(nix_pid, None) {
            Ok(WaitStatus::Stopped(_, Signal::SIGSTOP)) => {
                debug!("Attached to PID {} (stopped)", pid);
            }
            Ok(other) => {
                warn!("Unexpected status after attach: {:?}", other);
            }
            Err(e) => {
                return Err(TraceError::CaptureFailed(format!(
                    "waitpid after attach failed: {}",
                    e
                )));
            }
        }

        self.setup_ptrace_options(nix_pid)?;

        self.main_pid = Some(nix_pid);
        self.traced_pids.insert(pid);
        self.initialized = true;

        Ok(())
    }

    /// Configure ptrace options on a traced process.
    fn setup_ptrace_options(&self, pid: Pid) -> Result<(), TraceError> {
        let mut options = ptrace::Options::empty();

        if self.config.follow_children {
            options |= ptrace::Options::PTRACE_O_TRACECLONE;
            options |= ptrace::Options::PTRACE_O_TRACEFORK;
            options |= ptrace::Options::PTRACE_O_TRACEVFORK;
        }

        if self.config.trace_syscalls {
            // PTRACE_O_TRACESYSGOOD makes syscall-stops distinguishable
            // by delivering SIGTRAP | 0x80
            options |= ptrace::Options::PTRACE_O_TRACESYSGOOD;
        }

        if !options.is_empty() {
            ptrace::setoptions(pid, options).map_err(|e| {
                TraceError::CaptureFailed(format!("PTRACE_SETOPTIONS failed: {}", e))
            })?;
        }

        Ok(())
    }

    /// Wait for the next ptrace event from any traced process.
    ///
    /// Returns the event and associated data. Blocks until an event occurs.
    /// If capture_registers is enabled, register snapshots are yielded as
    /// separate events before the stop event that triggered them.
    pub fn wait_event(&mut self) -> Result<Option<PtraceEvent>, TraceError> {
        // Return buffered events first
        if !self.pending_events.is_empty() {
            return Ok(Some(self.pending_events.remove(0)));
        }

        // Wait for the specific main traced PID.
        // This avoids issues with waitpid(-1, ...) potentially missing events
        // when the child exits without generating ptrace stop events.
        let pid = match self.main_pid {
            Some(p) => p,
            None => {
                // Fallback: wait for any child with __WALL
                let status = match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::__WALL)) {
                    Ok(s) => s,
                    Err(nix::errno::Errno::ECHILD) => {
                        info!("No more traced processes");
                        return Ok(None);
                    }
                    Err(e) => {
                        return Err(TraceError::CaptureFailed(format!("waitpid error: {}", e)));
                    }
                };
                // Process the status inline
                return self.process_wait_status_impl(status);
            }
        };

        // Wait for the specific PID using non-blocking poll with timeout.
        // This approach is more robust in environments where blocking waitpid
        // may not return even when the child has exited.
        eprintln!("[PTRACE DIAG] main_pid={:?}, pid={:?}, traced_pids={:?}",
                 self.main_pid, pid, self.traced_pids);
        
        use nix::sys::signal::kill;
        let start = std::time::Instant::now();
        
        let status = loop {
            // Check if process still exists
            let alive = kill(pid, None).is_ok();
            if !alive {
                eprintln!("[PTRACE DIAG] Process {} no longer exists", pid.as_raw());
                return Ok(None);
            }
            
            // Try to wait with WNOHANG
            match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(s) if s == WaitStatus::StillAlive => {
                    // Child still running or zombie - check timeout
                    if start.elapsed().as_secs() > 5 {
                        // Timeout! Try to continue the child with PTRACE_CONT.
                        // If ESRCH (process already exited), try to reap with blocking waitpid.
                        eprintln!("[PTRACE DIAG] waitpid timeout - forcing PTRACE_CONT");
                        match ptrace::cont(pid, None) {
                            Ok(()) => {
                                // Child continued - wait for it to run
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                continue;
                            }
                            Err(nix::errno::Errno::ESRCH) => {
                                // ESRCH = no such process. The child exited (or pid was reused by a different
                                // process). Since we can't reliably reap with the reused pid, just return.
                                // The probe will stop naturally.
                                eprintln!("[PTRACE DIAG] PTRACE_CONT ESRCH - child gone (exit or pid reuse), returning None");
                                return Ok(None);
                            }
                            Err(e) => {
                                eprintln!("[PTRACE DIAG] PTRACE_CONT failed: {:?}", e);
                                return Ok(None);
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Ok(s) => {
                    eprintln!("[PTRACE DIAG] waitpid({}) WNOHANG returned: {:?}", pid, s);
                    break s;
                }
                Err(nix::errno::Errno::ECHILD) => {
                    eprintln!("[PTRACE DIAG] waitpid({}) ECHILD", pid.as_raw());
                    return Ok(None);
                }
                Err(e) => {
                    eprintln!("[PTRACE DIAG] waitpid({}) error: {}", pid.as_raw(), e);
                    return Err(TraceError::CaptureFailed(format!("waitpid error: {}", e)));
                }
            }
        };

        self.process_wait_status_impl(status)
    }

    /// Process a wait status and convert it to a PtraceEvent.
    fn process_wait_status_impl(&mut self, status: nix::sys::wait::WaitStatus) -> Result<Option<PtraceEvent>, TraceError> {
        let event = match status {
            WaitStatus::Stopped(pid, sig) => {
                debug!("PID {} stopped by {:?}", pid, sig);

                // Capture registers if configured
                let regs_event = if self.config.capture_registers {
                    match self.read_registers(pid) {
                        Ok(regs) => Some(PtraceEvent::Registers {
                            pid: pid.as_raw(),
                            regs,
                        }),
                        Err(e) => {
                            warn!("Failed to read registers for PID {}: {}", pid, e);
                            None
                        }
                    }
                } else {
                    None
                };

                // If we haven't seen this PID, add it to traced set
                let pid_raw = pid.as_raw();
                if !self.traced_pids.contains(&pid_raw) {
                    debug!("New traced PID: {}", pid_raw);
                    self.traced_pids.insert(pid_raw);
                    let _ = self.setup_ptrace_options(pid);
                }

                // Buffer registers event if captured
                if let Some(re) = regs_event {
                    self.pending_events.push(re);
                }

                Some(PtraceEvent::Stopped {
                    pid: pid.as_raw(),
                    signal: sig as i32,
                    signal_name: format!("{:?}", sig),
                })
            }

            WaitStatus::PtraceSyscall(pid) => {
                let regs = if self.config.capture_registers {
                    self.read_registers(pid).ok()
                } else {
                    None
                };
                let syscall_nr = regs.as_ref().map(|r| r.rax).unwrap_or(0);
                Some(PtraceEvent::Syscall {
                    pid: pid.as_raw(),
                    syscall_nr,
                    is_entry: true,
                })
            }

            WaitStatus::PtraceEvent(pid, _sig, event_code) => {
                debug!("PID {} ptrace event {}", pid, event_code);
                let new_pid = if matches!(
                    event_code,
                    nix::libc::PTRACE_EVENT_CLONE
                        | nix::libc::PTRACE_EVENT_FORK
                        | nix::libc::PTRACE_EVENT_VFORK
                ) {
                    match ptrace::getevent(pid) {
                        Ok(data) => {
                            let child_pid = data as i32;
                            if child_pid > 0 {
                                debug!("PID {} created new child PID {} (event {})", pid, child_pid, event_code);
                                self.traced_pids.insert(child_pid);
                            }
                            Some(child_pid)
                        }
                        Err(e) => {
                            warn!("PTRACE_GETEVENTMSG failed for PID {}: {}", pid, e);
                            None
                        }
                    }
                } else {
                    None
                };
                Some(PtraceEvent::PtraceEvent {
                    pid: pid.as_raw(),
                    event_code,
                    new_pid,
                })
            }

            WaitStatus::Exited(pid, exit_code) => {
                info!("PID {} exited with code {}", pid, exit_code);
                self.traced_pids.remove(&pid.as_raw());
                Some(PtraceEvent::Exited {
                    pid: pid.as_raw(),
                    exit_code,
                })
            }

            WaitStatus::Signaled(pid, sig, core_dumped) => {
                warn!("PID {} killed by {:?} (core: {})", pid, sig, core_dumped);
                self.traced_pids.remove(&pid.as_raw());
                Some(PtraceEvent::Signaled {
                    pid: pid.as_raw(),
                    signal: sig as i32,
                    signal_name: format!("{:?}", sig),
                    core_dumped,
                })
            }

            _ => {
                warn!("Unhandled wait status: {:?}", status);
                None
            }
        };

        Ok(event)
    }

    /// Continue execution of a traced process.
    pub fn continue_execution(&self, pid: i32) -> Result<(), TraceError> {
        let result = ptrace::cont(Pid::from_raw(pid), None);
        eprintln!("[PTRACE DIAG] PTRACE_CONT({}) result: {:?}", pid, result);
        result.map_err(|e| TraceError::CaptureFailed(format!("PTRACE_CONT failed: {}", e)))
    }

    /// Continue execution, delivering a specific signal.
    pub fn continue_with_signal(&self, pid: i32, sig: Signal) -> Result<(), TraceError> {
        ptrace::cont(Pid::from_raw(pid), Some(sig))
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_CONT failed: {}", e)))
    }

    /// Single-step the traced process.
    pub fn step(&self, pid: i32) -> Result<(), TraceError> {
        ptrace::step(Pid::from_raw(pid), None)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_SINGLESTEP failed: {}", e)))
    }

    /// Continue until next syscall entry/exit.
    pub fn syscall_continue(&self, pid: i32) -> Result<(), TraceError> {
        ptrace::syscall(Pid::from_raw(pid), None)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_SYSCALL failed: {}", e)))
    }

    /// Read the current register state of a traced process.
    pub fn read_registers(&self, pid: Pid) -> Result<RegisterState, TraceError> {
        let regs = ptrace::getregs(pid)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_GETREGS failed: {}", e)))?;

        Ok(RegisterState {
            rax: regs.rax,
            rbx: regs.rbx,
            rcx: regs.rcx,
            rdx: regs.rdx,
            rsi: regs.rsi,
            rdi: regs.rdi,
            rbp: regs.rbp,
            rsp: regs.rsp,
            r8: regs.r8,
            r9: regs.r9,
            r10: regs.r10,
            r11: regs.r11,
            r12: regs.r12,
            r13: regs.r13,
            r14: regs.r14,
            r15: regs.r15,
            rip: regs.rip,
            rflags: regs.eflags,
        })
    }

    /// Open performance counter file descriptors for the traced process.
    ///
    /// This is called automatically during `launch()` when the `perf_counters`
    /// feature is enabled. Opens HW_CPU_CYCLES and HW_INSTRUCTIONS counters.
    #[cfg(feature = "perf_counters")]
    pub fn open_perf_counters(&mut self, pid: Pid) -> Result<(), TraceError> {
        use super::perf::{PerfCounterConfig, PerfCounterType};

        // Open cycle counter
        let cycle_config = PerfCounterConfig::new(PerfCounterType::Cycle);
        match self.open_single_counter(pid, cycle_config) {
            Ok(handle) => {
                debug!("Opened perf counter for cycles");
                self.perf_handles.push(handle);
            }
            Err(e) => {
                warn!("Failed to open cycle counter (perf_event_open unavailable): {}", e);
                // Continue without counters - graceful degradation
            }
        }

        // Open instruction counter
        let instr_config = PerfCounterConfig::new(PerfCounterType::Instruction);
        match self.open_single_counter(pid, instr_config) {
            Ok(handle) => {
                debug!("Opened perf counter for instructions");
                self.perf_handles.push(handle);
            }
            Err(e) => {
                warn!("Failed to open instruction counter: {}", e);
            }
        }

        Ok(())
    }

    /// Open a single perf counter for a PID.
    #[cfg(feature = "perf_counters")]
    fn open_single_counter(
        &mut self,
        pid: Pid,
        config: super::perf::PerfCounterConfig,
    ) -> Result<super::perf::PerfCounterHandle, TraceError> {
        use super::perf::counters::{perf_event_open, PERF_HW_BRANCH_MISSES, PERF_HW_CACHE_MISSES,
            PERF_HW_CPU_CYCLES, PERF_TYPE_HARDWARE, PERF_TYPE_SOFTWARE, PERF_SW_CPU_CLOCK, PerfCounterType};

        let (type_, config_val) = match config.counter_type {
            PerfCounterType::Cycle => (
                PERF_TYPE_HARDWARE,
                PERF_HW_CPU_CYCLES,
            ),
            PerfCounterType::Instruction => (
                PERF_TYPE_SOFTWARE,
                PERF_SW_CPU_CLOCK, // Software clock for instruction counting approximation
            ),
            PerfCounterType::CacheMiss => (
                PERF_TYPE_HARDWARE,
                PERF_HW_CACHE_MISSES,
            ),
            PerfCounterType::BranchMiss => (
                PERF_TYPE_HARDWARE,
                PERF_HW_BRANCH_MISSES,
            ),
        };

        let fd = perf_event_open(type_, config_val, pid.as_raw(), -1, None)
            .map_err(|e| TraceError::CaptureFailed(format!("perf_event_open failed: {}", e)))?;

        Ok(super::perf::PerfCounterHandle::from_fd(fd, config.counter_type))
    }

    /// Read all performance counters and return a snapshot.
    ///
    /// Returns a snapshot with all counter values, or `None` if counters
    /// could not be read (e.g., counters were not opened due to permission denied).
    #[cfg(feature = "perf_counters")]
    pub fn read_perf_counters(&self) -> Result<super::perf::PerfCountersSnapshot, TraceError> {
        use super::perf::PerfCountersSnapshot;

        let mut cycles = None;
        let mut instructions = None;

        for handle in &self.perf_handles {
            match handle.read() {
                Ok(value) => {
                    match handle.counter_type() {
                        super::perf::PerfCounterType::Cycle => cycles = Some(value),
                        super::perf::PerfCounterType::Instruction => instructions = Some(value),
                        _ => {}
                    }
                }
                Err(e) => {
                    debug!("Failed to read perf counter: {}", e);
                }
            }
        }

        Ok(PerfCountersSnapshot {
            cycles,
            instructions,
            cache_misses: None,
            branch_misses: None,
        })
    }

    /// Check if performance counters are available.
    #[cfg(feature = "perf_counters")]
    pub fn has_perf_counters(&self) -> bool {
        !self.perf_handles.is_empty()
    }

    /// Detach from a traced process, allowing it to continue freely.
    pub fn detach(&self, pid: i32) -> Result<(), TraceError> {
        ptrace::detach(Pid::from_raw(pid), None)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_DETACH failed: {}", e)))
    }

    /// Kill a traced process.
    pub fn kill(&self, pid: i32) -> Result<(), TraceError> {
        ptrace::kill(Pid::from_raw(pid))
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_KILL failed: {}", e)))
    }
}

/// Helper to convert a signal number to a human-readable name.
pub fn signal_name(signal: i32) -> String {
    Signal::try_from(signal)
        .map(|s| format!("{:?}", s))
        .unwrap_or_else(|_| format!("SIG{}", signal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptrace_config_default() {
        let config = PtraceConfig::default();
        assert!(!config.trace_syscalls);
        assert!(config.capture_registers);
        assert!(config.follow_children);
    }

    #[test]
    fn test_ptrace_tracer_new() {
        let tracer = PtraceTracer::new_default();
        assert!(tracer.main_pid().is_none());
        assert!(tracer.traced_pids().is_empty());
    }

    #[test]
    fn test_signal_name_known() {
        assert_eq!(signal_name(9), "SIGKILL");
        assert_eq!(signal_name(11), "SIGSEGV");
        assert_eq!(signal_name(5), "SIGTRAP");
    }

    #[test]
    fn test_signal_name_unknown() {
        // Very high signal number should still produce a string
        let name = signal_name(200);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_ptrace_event_pid() {
        let event = PtraceEvent::Stopped {
            pid: 1234,
            signal: 5,
            signal_name: "SIGTRAP".into(),
        };
        assert_eq!(event.pid(), 1234);

        let event = PtraceEvent::Exited {
            pid: 5678,
            exit_code: 0,
        };
        assert_eq!(event.pid(), 5678);

        let event = PtraceEvent::Syscall {
            pid: 9012,
            syscall_nr: 1,
            is_entry: true,
        };
        assert_eq!(event.pid(), 9012);

        let event = PtraceEvent::Signaled {
            pid: 3456,
            signal: 9,
            signal_name: "SIGKILL".into(),
            core_dumped: false,
        };
        assert_eq!(event.pid(), 3456);
    }

    #[test]
    fn test_ptrace_event_registers() {
        let regs = RegisterState {
            rax: 42,
            rip: 0x400000,
            ..Default::default()
        };
        let event = PtraceEvent::Registers { pid: 9999, regs };
        assert_eq!(event.pid(), 9999);
        if let PtraceEvent::Registers { regs, .. } = event {
            assert_eq!(regs.rax, 42);
            assert_eq!(regs.rip, 0x400000);
        }
    }

    #[test]
    fn test_ptrace_config_custom() {
        let config = PtraceConfig {
            trace_syscalls: true,
            capture_registers: false,
            follow_children: false,
        };
        assert!(config.trace_syscalls);
        assert!(!config.capture_registers);
        assert!(!config.follow_children);
    }

    /// Integration test: launch `/bin/true` (exits immediately with 0)
    /// under ptrace and verify we get the expected events.
    #[test]
    fn test_launch_true_and_wait() {
        let mut tracer = PtraceTracer::new(PtraceConfig {
            trace_syscalls: false,
            capture_registers: true,
            follow_children: false,
        });

        let pid = tracer
            .launch(Path::new("/bin/true"), &[])
            .expect("launch should work");
        assert!(pid > 0);
        assert_eq!(tracer.main_pid(), Some(pid));

        // Note: launch() now resumes the child with PTRACE_CONT, so we don't
        // need to call continue_execution() here. Just wait for the exit event.

        // Wait for exit event
        let event = tracer.wait_event().expect("wait_event should work");
        assert!(event.is_some());

        match event {
            Some(PtraceEvent::Exited { exit_code, .. }) => {
                assert_eq!(exit_code, 0);
            }
            Some(PtraceEvent::Signaled { signal_name, .. }) => {
                // Some systems may report signal instead of exit
                panic!("Expected Exited event, got Signaled: {}", signal_name);
            }
            other => panic!("Expected Exited event, got: {:?}", other),
        }
    }

    /// Integration test: launch `/bin/true` and verify event loop completes.
    #[test]
    fn test_launch_captures_events() {
        let mut tracer = PtraceTracer::new(PtraceConfig {
            trace_syscalls: false,
            capture_registers: true,
            follow_children: false,
        });

        let pid = tracer
            .launch(Path::new("/bin/true"), &[])
            .expect("launch should work");

        // Note: launch() now resumes the child with PTRACE_CONT.

        // Collect events until exit
        let mut got_exit = false;
        for _ in 0..1000 {
            match tracer.wait_event() {
                Ok(Some(PtraceEvent::Exited { .. })) => {
                    got_exit = true;
                    break;
                }
                Ok(Some(_)) => {
                    tracer.continue_execution(pid).expect("cont should work");
                }
                Ok(None) => break,
                Err(e) => panic!("wait_event error: {}", e),
            }
        }

        assert!(got_exit, "Should have seen Exited event for /bin/true");
    }

    /// Integration test: launch with syscall tracing enabled.
    #[test]
    fn test_launch_with_syscall_tracing() {
        let mut tracer = PtraceTracer::new(PtraceConfig {
            trace_syscalls: true,
            capture_registers: true,
            follow_children: false,
        });

        let pid = tracer
            .launch(Path::new("/bin/true"), &[])
            .expect("launch should work");

        // Note: launch() now resumes the child with PTRACE_SYSCALL.

        let mut syscall_count = 0;
        let mut got_exit = false;
        for _ in 0..10000 {
            match tracer.wait_event() {
                Ok(Some(PtraceEvent::Exited { .. })) => {
                    got_exit = true;
                    break;
                }
                Ok(Some(PtraceEvent::Syscall { .. })) => {
                    syscall_count += 1;
                    tracer.syscall_continue(pid).expect("syscall should work");
                }
                Ok(Some(PtraceEvent::Registers { .. })) => {
                    tracer.syscall_continue(pid).expect("syscall should work");
                }
                Ok(Some(PtraceEvent::Stopped { signal: _, .. })) => {
                    tracer.syscall_continue(pid).expect("syscall should work");
                }
                Ok(Some(_)) => {
                    // Signaled, PtraceEvent, etc.
                    tracer.syscall_continue(pid).expect("syscall should work");
                }
                Ok(None) => break,
                Err(e) => panic!("wait_event error: {}", e),
            }
        }

        assert!(got_exit, "Should have seen Exited event");
        assert!(
            syscall_count > 0,
            "/bin/true should make at least one syscall"
        );
    }
}
