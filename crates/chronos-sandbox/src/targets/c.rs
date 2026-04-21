//! C/C++ debug target implementation.
//!
//! Uses ptrace for debugging C programs.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::{Arc, RwLock};

/// C/C++ debug target using ptrace.
#[derive(Debug, Clone)]
pub struct CTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
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
        }
    }
}

impl DebugTarget for CTarget {
    fn attach(&self, pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;

            // Use gdb for attaching
            let output = Command::new("gdb")
                .args(["-q", "-ex", &format!("attach {}", pid), "-ex", "detach"])
                .output();

            match output {
                Ok(out) if out.status.success() => Ok(()),
                Ok(out) => Err(SandboxError::DebugTargetConnectFailed(
                    String::from_utf8_lossy(&out.stderr).to_string(),
                )),
                Err(e) => Err(SandboxError::PtraceBlocked(e.to_string())),
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SandboxError::PtraceBlocked("ptrace only supported on Linux".to_string()))
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

    fn set_breakpoint(&self, _address: u64) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }
        Ok(())
    }

    fn wait(&self) -> Result<BreakpointHit, SandboxError> {
        Err(SandboxError::DebugTargetConnectFailed("not implemented".to_string()))
    }

    fn resume(&self) -> Result<(), SandboxError> {
        if !*self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("not attached".to_string()));
        }
        Ok(())
    }

    fn detach(&self) -> Result<(), SandboxError> {
        *self.attached.write().unwrap() = false;
        *self.pid.write().unwrap() = None;
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
}
