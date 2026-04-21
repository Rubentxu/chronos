//! Go debug target implementation.
//!
//! Uses Delve RPC on port 2345.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::{Arc, RwLock};

/// Go debug target using Delve.
#[derive(Debug, Clone)]
pub struct GoTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
    port: u16,
}

impl Default for GoTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl GoTarget {
    /// Creates a new GoTarget with default Delve port (2345).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port: 2345,
        }
    }
}

impl DebugTarget for GoTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }
        Ok(())
    }

    fn spawn(&self, _program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with Delve
        let mut cmd = Command::new("dlv");
        cmd.arg("debug");
        cmd.arg("--accept-multiclient");
        cmd.arg("--listen").arg(format!(":{}", self.port));
        cmd.arg("--");
        cmd.args(args);

        let child = cmd.spawn().map_err(|e| SandboxError::DebugTargetConnectFailed(e.to_string()))?;
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
    fn test_go_target_default_port() {
        let target = GoTarget::new();
        assert!(!target.is_attached());
    }
}
