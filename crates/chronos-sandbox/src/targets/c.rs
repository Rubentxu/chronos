//! C/C++ debug target implementation.
//!
//! Uses ptrace for debugging C programs.
//! Delegates to chronos-native's PtraceTracer and BreakpointManager.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use chronos_native::breakpoint::BreakpointManager;
use chronos_native::ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
use std::sync::{Arc, Mutex, RwLock};

/// C/C++ debug target using ptrace.
#[derive(Clone)]
pub struct CTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
    tracer: Arc<Mutex<PtraceTracer>>,
    breakpoint_manager: Arc<Mutex<Option<BreakpointManager>>>,
}

impl std::fmt::Debug for CTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CTarget")
            .field("attached", &*self.attached.read().unwrap())
            .field("pid", &*self.pid.read().unwrap())
            .finish()
    }
}

impl Default for CTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CTarget {
    /// Creates a new CTarget.
    pub fn new() -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            tracer: Arc::new(Mutex::new(PtraceTracer::new(PtraceConfig::default()))),
            breakpoint_manager: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the current PID if attached.
    fn current_pid(&self) -> Option<u32> {
        *self.pid.read().unwrap()
    }
}

impl DebugTarget for CTarget {
    fn attach(&self, pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }

        #[cfg(target_os = "linux")]
        {
            let mut tracer = self.tracer.lock().unwrap();
            tracer
                .attach(pid as i32)
                .map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;

            // Create breakpoint manager for this PID
            let mut bp_mgr = self.breakpoint_manager.lock().unwrap();
            *bp_mgr = Some(BreakpointManager::new(pid as i32));

            *self.attached.write().unwrap() = true;
            *self.pid.write().unwrap() = Some(pid);
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SandboxError::PtraceBlocked(
                "ptrace only supported on Linux".to_string(),
            ))
        }
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        let child = Command::new(program)
            .args(args)
            .spawn()
            .map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;

        Ok(child.id())
    }

    fn is_attached(&self) -> bool {
        *self.attached.read().unwrap()
    }

    fn set_breakpoint(&self, address: u64) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }

        let mut bp_mgr = self.breakpoint_manager.lock().unwrap();
        if let Some(ref mut mgr) = *bp_mgr {
            mgr.set_breakpoint(address)
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("set_breakpoint failed: {}", e)))?;
        }
        Ok(())
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        let _ = self.current_pid()
            .ok_or_else(|| SandboxError::DebugTargetConnectFailed("not attached".to_string()))?;

        let mut tracer = self.tracer.lock().unwrap();
        let mut bp_mgr = self.breakpoint_manager.lock().unwrap();

        loop {
            let event = tracer.wait_event()
                .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("wait_event failed: {}", e)))?;

            match event {
                Some(PtraceEvent::Stopped { pid: evt_pid, signal, .. }) => {
                    // SIGTRAP (signal 5) indicates a breakpoint hit
                    if signal == 5 {
                        // Check if it's one of our breakpoints
                        if let Some(ref mut mgr) = *bp_mgr {
                            if let Ok(Some(bp_addr)) = mgr.handle_breakpoint_hit() {
                                return Ok(BreakpointHit {
                                    pid: evt_pid as u32,
                                    tid: evt_pid as u32,
                                    address: bp_addr,
                                });
                            }
                        }
                        // If not our breakpoint, return with address 0 (unknown)
                        // In a real implementation we'd read the PC from registers
                        return Ok(BreakpointHit {
                            pid: evt_pid as u32,
                            tid: evt_pid as u32,
                            address: 0,
                        });
                    }
                    // Not SIGTRAP, continue waiting
                    tracer.continue_execution(evt_pid)
                        .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("continue failed: {}", e)))?;
                }
                Some(PtraceEvent::Exited { pid: _evt_pid, exit_code }) => {
                    return Err(SandboxError::DebugTargetConnectFailed(
                        format!("process exited with code {}", exit_code),
                    ));
                }
                Some(PtraceEvent::Signaled { pid: _evt_pid, signal_name, .. }) => {
                    return Err(SandboxError::DebugTargetConnectFailed(
                        format!("process killed by {}", signal_name),
                    ));
                }
                Some(PtraceEvent::Syscall { pid: evt_pid, .. }) => {
                    // Syscall stop, continue
                    tracer.syscall_continue(evt_pid)
                        .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("syscall_continue failed: {}", e)))?;
                }
                Some(PtraceEvent::PtraceEvent { pid: evt_pid, .. }) => {
                    // Ptrace event (clone, exec, etc.), continue
                    tracer.continue_execution(evt_pid)
                        .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("continue failed: {}", e)))?;
                }
                Some(PtraceEvent::Registers { pid: evt_pid, .. }) => {
                    // Registers captured, continue
                    tracer.continue_execution(evt_pid)
                        .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("continue failed: {}", e)))?;
                }
                None => {
                    return Err(SandboxError::DebugTargetConnectFailed(
                        "no more traced processes".to_string(),
                    ));
                }
            }
        }
    }

    fn resume(&self) -> Result<(), SandboxError> {
        let pid = self.current_pid()
            .ok_or_else(|| SandboxError::DebugTargetConnectFailed("not attached".to_string()))?;

        let tracer = self.tracer.lock().unwrap();
        tracer.continue_execution(pid as i32)
            .map_err(|e| SandboxError::DebugTargetConnectFailed(format!("continue failed: {}", e)))
    }

    fn detach(&self) -> Result<(), SandboxError> {
        let pid = match self.current_pid() {
            Some(p) => p,
            None => return Ok(()), // Already detached
        };

        // Remove all breakpoints
        {
            let mut bp_mgr = self.breakpoint_manager.lock().unwrap();
            if let Some(ref mut mgr) = *bp_mgr {
                let _ = mgr.remove_all();
            }
        }

        // Detach from process
        let tracer = self.tracer.lock().unwrap();
        if let Err(e) = tracer.detach(pid as i32) {
            return Err(SandboxError::DebugTargetConnectFailed(format!("detach failed: {}", e)));
        }

        *self.attached.write().unwrap() = false;
        *self.pid.write().unwrap() = None;

        // Clear breakpoint manager
        let mut bp_mgr = self.breakpoint_manager.lock().unwrap();
        *bp_mgr = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_target_creation() {
        let target = CTarget::new();
        assert!(!target.is_attached());
    }

    #[test]
    fn test_c_target_attach_not_attached() {
        let target = CTarget::new();
        // Trying to set breakpoint without attach should fail
        let result = target.set_breakpoint(0x400000);
        assert!(result.is_err());
    }

    #[test]
    fn test_c_target_wait_not_attached() {
        let target = CTarget::new();
        let result = target.wait();
        assert!(result.is_err());
    }

    #[test]
    fn test_c_target_resume_not_attached() {
        let target = CTarget::new();
        let result = target.resume();
        assert!(result.is_err());
    }

    #[test]
    fn test_c_target_detach_when_not_attached() {
        let target = CTarget::new();
        // Detaching when not attached should be ok (no-op)
        let result = target.detach();
        assert!(result.is_ok());
        assert!(!target.is_attached());
    }

    #[test]
    #[ignore = "requires ptrace capability or real process"]
    fn test_c_target_ptrace_integration() {
        // This test requires a real process to attach to
        // and ptrace capability. Marked as ignored by default.
        let target = CTarget::new();
        // Spawn a process and attach to it
        let mut child = std::process::Command::new("sleep")
            .arg("10")
            .spawn()
            .expect("sleep should spawn");

        let pid = child.id();
        let result = target.attach(pid);
        // May fail with permission error in restricted environments
        if result.is_ok() {
            let detach_result = target.detach();
            assert!(detach_result.is_ok());
        }

        let _ = child.kill();
    }
}
