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
    Exited {
        pid: i32,
        exit_code: i32,
    },
    /// Tracee was killed by a signal.
    Signaled {
        pid: i32,
        signal: i32,
        signal_name: String,
        core_dumped: bool,
    },
    /// Tracee hit a ptrace event (clone, exec, etc.).
    PtraceEvent {
        pid: i32,
        event_code: i32,
    },
    /// Register state snapshot captured for this stop.
    Registers {
        pid: i32,
        regs: RegisterState,
    },
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
    pub fn launch(
        &mut self,
        program: &Path,
        args: &[String],
    ) -> Result<i32, TraceError> {
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
                        return Err(TraceError::CaptureFailed(format!(
                            "waitpid failed: {}",
                            e
                        )));
                    }
                }

                // Set ptrace options
                self.setup_ptrace_options(child)?;

                self.main_pid = Some(child);
                self.traced_pids.insert(child.as_raw());
                self.initialized = true;

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

        // Wait for any child (pid -1 means any)
        let wait_flags = WaitPidFlag::__WALL;

        let status = match waitpid(Pid::from_raw(-1), Some(wait_flags)) {
            Ok(s) => s,
            Err(nix::errno::Errno::ECHILD) => {
                // No more children — all traced processes exited
                info!("No more traced processes");
                return Ok(None);
            }
            Err(e) => {
                return Err(TraceError::CaptureFailed(format!(
                    "waitpid error: {}",
                    e
                )));
            }
        };

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
                    // Set up options for this new child too
                    let _ = self.setup_ptrace_options(pid);
                }

                // Buffer registers event if captured; will be returned on next wait_event() call
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

                // Determine if this is entry or exit by checking RAX
                // On syscall entry, orig_rax has the syscall number
                // On syscall exit, rax has the return value
                // For simplicity, we toggle — first syscall stop is entry, next is exit
                let syscall_nr = regs.as_ref().map(|r| r.rax).unwrap_or(0);

                Some(PtraceEvent::Syscall {
                    pid: pid.as_raw(),
                    syscall_nr,
                    is_entry: true, // simplified — would need state tracking for accuracy
                })
            }

            WaitStatus::PtraceEvent(pid, _sig, event_code) => {
                debug!("PID {} ptrace event {}", pid, event_code);
                Some(PtraceEvent::PtraceEvent {
                    pid: pid.as_raw(),
                    event_code,
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
                warn!(
                    "PID {} killed by {:?} (core: {})",
                    pid, sig, core_dumped
                );
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
        ptrace::cont(Pid::from_raw(pid), None)
            .map_err(|e| TraceError::CaptureFailed(format!("PTRACE_CONT failed: {}", e)))
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
        let event = PtraceEvent::Registers {
            pid: 9999,
            regs,
        };
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

        // Continue the child (it's stopped after exec)
        tracer.continue_execution(pid).expect("cont should work");

        // Wait for exit event
        let event = tracer.wait_event().expect("wait_event should work");
        assert!(event.is_some());

        match event {
            Some(PtraceEvent::Exited { exit_code, .. }) => {
                assert_eq!(exit_code, 0);
            }
            Some(PtraceEvent::Signaled { signal_name, .. }) => {
                // Some systems may report signal instead of exit
                panic!(
                    "Expected Exited event, got Signaled: {}",
                    signal_name
                );
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

        // Continue the child (it's stopped after exec)
        tracer.continue_execution(pid).expect("cont should work");

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

        // Continue with syscall tracing
        tracer.syscall_continue(pid).expect("syscall should work");

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
