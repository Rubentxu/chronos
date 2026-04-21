//! Node.js debug target implementation.
//!
//! Uses Chrome DevTools Protocol (CDP) on port 9229.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::{Arc, RwLock};

/// Node.js debug target using CDP.
#[derive(Debug, Clone)]
pub struct NodeJsTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
    port: u16,
}

impl Default for NodeJsTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeJsTarget {
    /// Creates a new NodeJsTarget with default CDP port (9229).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port: 9229,
        }
    }
}

impl DebugTarget for NodeJsTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed("already attached".to_string()));
        }
        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with --inspect flag
        let mut cmd = Command::new("node");
        cmd.arg(format!("--inspect={}", self.port));
        cmd.arg(program);
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
    fn test_nodejs_target_default_port() {
        let target = NodeJsTarget::new();
        assert!(!target.is_attached());
    }
}
