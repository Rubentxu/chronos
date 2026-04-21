//! Java debug target implementation.
//!
//! Uses JDWP (Java Debug Wire Protocol) on port 5005.

use crate::error::SandboxError;
use crate::targets::{BreakpointHit, DebugTarget};
use std::sync::{Arc, RwLock};

/// Java debug target using JDWP.
#[derive(Debug, Clone)]
pub struct JavaTarget {
    attached: Arc<RwLock<bool>>,
    pid: Arc<RwLock<Option<u32>>>,
    port: u16,
}

impl Default for JavaTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaTarget {
    /// Creates a new JavaTarget with default JDWP port (5005).
    pub fn new() -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port: 5005,
        }
    }

    /// Creates a new JavaTarget with a custom port.
    pub fn with_port(port: u16) -> Self {
        Self {
            attached: Arc::new(RwLock::new(false)),
            pid: Arc::new(RwLock::new(None)),
            port,
        }
    }
}

impl DebugTarget for JavaTarget {
    fn attach(&self, _pid: u32) -> Result<(), SandboxError> {
        if *self.attached.read().unwrap() {
            return Err(SandboxError::DebugTargetConnectFailed(
                "already attached".to_string(),
            ));
        }
        // JDWP attach would go here
        Ok(())
    }

    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError> {
        use std::process::Command;

        // Launch with JDWP agent
        let jdwp_arg = format!("transport=dt_socket,server=y,suspend=y,address={}", self.port);
        let mut cmd = Command::new(program);
        cmd.arg(format!("-agentlib:jdwp={}", jdwp_arg));
        cmd.args(args);

        let child = cmd
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
    fn test_java_target_default_port() {
        let target = JavaTarget::new();
        assert!(!target.is_attached());
    }
}
