//! Debug target support for multiple languages.
//!
//! Provides trait and implementations for debugging programs in various languages.

#[cfg(feature = "java")]
pub mod java;
#[cfg(feature = "go")]
pub mod go;
#[cfg(feature = "python")]
pub mod python;
#[cfg(feature = "nodejs")]
pub mod nodejs;
#[cfg(feature = "cpp")]
pub mod c;
pub mod rust;

use crate::error::SandboxError;

/// Trait for debuggable language targets.
///
/// Implementors must provide language-specific debug protocol implementations
/// for attaching to and controlling debug targets.
pub trait DebugTarget: Send + Sync {
    /// Attaches to a running process by ID.
    fn attach(&self, pid: u32) -> Result<(), SandboxError>;

    /// Spawns a new process and attaches to it.
    fn spawn(&self, program: &str, args: &[&str]) -> Result<u32, SandboxError>;

    /// Checks if the debugger is currently attached.
    fn is_attached(&self) -> bool;

    /// Sets a breakpoint at the given address.
    fn set_breakpoint(&self, address: u64) -> Result<(), SandboxError>;

    /// Waits for a breakpoint to be hit.
    fn wait(&self) -> Result<BreakpointHit, SandboxError>;

    /// Continues execution after a breakpoint.
    fn resume(&self) -> Result<(), SandboxError>;

    /// Detaches from the target.
    fn detach(&self) -> Result<(), SandboxError>;
}

/// Information about a breakpoint hit event.
#[derive(Debug, Clone)]
pub struct BreakpointHit {
    /// Process ID.
    pub pid: u32,
    /// Thread ID.
    pub tid: u32,
    /// Address where the breakpoint was hit.
    pub address: u64,
}

/// Manages multiple debug targets.
pub struct TargetManager {
    targets: std::collections::HashMap<String, Box<dyn DebugTarget>>,
}

#[allow(clippy::derivable_impls)]
impl Default for TargetManager {
    fn default() -> Self {
        Self {
            targets: std::collections::HashMap::new(),
        }
    }
}

impl TargetManager {
    /// Creates a new TargetManager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a target for a language.
    pub fn register<T: DebugTarget + 'static>(&mut self, language: &str, target: T) {
        self.targets.insert(language.to_string(), Box::new(target));
    }

    /// Gets a target by language.
    pub fn get(&self, language: &str) -> Option<&dyn DebugTarget> {
        self.targets.get(language).map(|b| b.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_manager_placeholder() {
        let manager = TargetManager::new();
        assert!(manager.get("rust").is_none());
    }
}
