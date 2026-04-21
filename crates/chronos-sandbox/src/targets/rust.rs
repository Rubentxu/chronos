//! Rust debug target implementation.
//!
//! Uses `nix::ptrace` for debugging Rust programs.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Rust debug target using ptrace.
#[derive(Debug, Clone)]
pub struct RustTarget {
    attached: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
}

impl Default for RustTarget {
    fn default() -> Self {
        Self {
            attached: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl RustTarget {
    /// Creates a new RustTarget.
    pub fn new() -> Self {
        Self::default()
    }
}

impl DebugTarget for RustTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if self.attached.load(Ordering::SeqCst) {
            return Err(SandboxError::DebugTargetConnectFailed(
                "already attached".to_string(),
            ));
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;

            // Use `ptrace` via `gdb` or direct syscall
            let output = Command::new("gdb")
                .args(["-q", "-ex", &format!("attach {}", _pid), "-ex", "detach"])
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    self.attached.store(true, Ordering::SeqCst);
                    self.pid.store(_pid, Ordering::SeqCst);
                    Ok(())
                }
                Ok(out) => Err(SandboxError::DebugTargetConnectFailed(
                    String::from_utf8_lossy(&out.stderr).to_string(),
                )),
                Err(e) => Err(SandboxError::PtraceBlocked(e.to_string())),
            }
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

        let mut cmd = Command::new(program);
        cmd.args(args);

        let child = cmd.spawn().map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;

        let pid = child.id();
        self.pid.store(pid, Ordering::SeqCst);
        self.attached.store(true, Ordering::SeqCst);
        Ok(pid)
    }

    fn is_attached(&self) -> bool {
        self.attached.load(Ordering::SeqCst)
    }

    fn set_breakpoint(&self, _address: u64) -> Result<(), SandboxError> {
        if !self.attached.load(Ordering::SeqCst) {
            return Err(SandboxError::DebugTargetConnectFailed(
                "not attached".to_string(),
            ));
        }
        // ptrace breakpoint implementation would go here
        Ok(())
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        if !self.attached.load(Ordering::SeqCst) {
            return Err(SandboxError::DebugTargetConnectFailed(
                "not attached".to_string(),
            ));
        }
        // Wait for breakpoint hit implementation would go here
        Err(SandboxError::DebugTargetConnectFailed(
            "not implemented".to_string(),
        ))
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !self.attached.load(Ordering::SeqCst) {
            return Err(SandboxError::DebugTargetConnectFailed(
                "not attached".to_string(),
            ));
        }
        Ok(())
    }

    fn detach(&self) -> Result<(), SandboxError> {
        self.attached.store(false, Ordering::SeqCst);
        self.pid.store(0, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_target_creation() {
        let target = RustTarget::new();
        assert!(!target.is_attached());
    }
}
