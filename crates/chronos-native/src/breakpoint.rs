//! Breakpoint management using INT3 injection.
//!
//! Provides `BreakpointManager` that can set/remove software breakpoints
//! by injecting the INT3 instruction (0xCC) at function entry points.
//!
//! # How it works
//!
//! 1. **Set**: Read the byte at the target address, replace it with 0xCC (INT3),
//!    save the original byte.
//! 2. **Hit**: When the tracee stops at a breakpoint, the RIP is past the INT3
//!    (at address+1). We restore the original byte, set RIP back by 1, single-step,
//!    then re-insert the INT3.
//! 3. **Remove**: Restore the original byte.

use nix::sys::ptrace;
use nix::unistd::Pid;
use std::collections::HashMap;
use tracing::{debug, info};

/// INT3 instruction opcode.
const INT3: u8 = 0xCC;

/// State of a software breakpoint.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Address where the breakpoint is set.
    pub address: u64,
    /// Original byte that was replaced with INT3.
    pub original_byte: u8,
    /// Whether the breakpoint is currently active (INT3 is in place).
    pub enabled: bool,
    /// Unique breakpoint ID.
    pub id: u64,
    /// Optional function name for this breakpoint.
    pub function_name: Option<String>,
}

/// Manages software breakpoints for a traced process.
pub struct BreakpointManager {
    /// Pid of the traced process.
    pid: Pid,
    /// Active breakpoints: address → Breakpoint.
    breakpoints: HashMap<u64, Breakpoint>,
    /// Next breakpoint ID.
    next_id: u64,
}

impl BreakpointManager {
    /// Create a new breakpoint manager for the given PID.
    pub fn new(pid: i32) -> Self {
        Self {
            pid: Pid::from_raw(pid),
            breakpoints: HashMap::new(),
            next_id: 0,
        }
    }

    /// Get the PID being managed.
    pub fn pid(&self) -> i32 {
        self.pid.as_raw()
    }

    /// Get all active breakpoints.
    pub fn breakpoints(&self) -> &HashMap<u64, Breakpoint> {
        &self.breakpoints
    }

    /// Get the number of active breakpoints.
    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.len()
    }

    /// Set a breakpoint at the given address.
    ///
    /// Reads the byte at the address, saves it, and replaces it with INT3 (0xCC).
    pub fn set_breakpoint(&mut self, address: u64) -> Result<u64, String> {
        if self.breakpoints.contains_key(&address) {
            return Err(format!("Breakpoint already set at 0x{:x}", address));
        }

        // Read the byte at the address using PTRACE_PEEKDATA
        // ptrace reads in word-sized chunks (8 bytes on x86_64)
        let word = ptrace::read(self.pid, address as ptrace::AddressType)
            .map_err(|e| format!("PTRACE_PEEKDATA at 0x{:x} failed: {}", address, e))?;

        let original_byte = (word & 0xFF) as u8;

        // Replace the low byte with INT3
        let modified_word = (word & !0xFF) | (INT3 as i64);

        // Write back using PTRACE_POKEDATA
        ptrace::write(self.pid, address as ptrace::AddressType, modified_word)
            .map_err(|e| format!("PTRACE_POKEDATA at 0x{:x} failed: {}", address, e))?;

        let id = self.next_id;
        self.next_id += 1;

        let bp = Breakpoint {
            address,
            original_byte,
            enabled: true,
            id,
            function_name: None,
        };

        debug!(
            "Set breakpoint #{} at 0x{:x} (original byte: 0x{:02x})",
            id, address, original_byte
        );

        self.breakpoints.insert(address, bp);
        Ok(id)
    }

    /// Set a breakpoint at the given address with a function name.
    pub fn set_breakpoint_at_function(
        &mut self,
        address: u64,
        function_name: impl Into<String>,
    ) -> Result<u64, String> {
        let id = self.set_breakpoint(address)?;
        if let Some(bp) = self.breakpoints.get_mut(&address) {
            bp.function_name = Some(function_name.into());
        }
        Ok(id)
    }

    /// Remove a breakpoint by address.
    ///
    /// Restores the original byte if the breakpoint is enabled.
    pub fn remove_breakpoint(&mut self, address: u64) -> Result<(), String> {
        let bp = self.breakpoints.remove(&address)
            .ok_or_else(|| format!("No breakpoint at 0x{:x}", address))?;

        if bp.enabled {
            self.restore_byte(&bp)?;
        }

        info!("Removed breakpoint #{} at 0x{:x}", bp.id, address);
        Ok(())
    }

    /// Remove a breakpoint by its ID.
    pub fn remove_breakpoint_by_id(&mut self, id: u64) -> Result<(), String> {
        let address = self.breakpoints.values()
            .find(|bp| bp.id == id)
            .map(|bp| bp.address)
            .ok_or_else(|| format!("No breakpoint with ID {}", id))?;

        self.remove_breakpoint(address)
    }

    /// Disable a breakpoint without removing it (restores original byte).
    pub fn disable_breakpoint(&mut self, address: u64) -> Result<(), String> {
        let original_byte = {
            let bp = self.breakpoints.get(&address)
                .ok_or_else(|| format!("No breakpoint at 0x{:x}", address))?;

            if !bp.enabled {
                return Ok(());
            }

            bp.original_byte
        };

        // Restore the byte (separate borrow)
        let word = ptrace::read(self.pid, address as ptrace::AddressType)
            .map_err(|e| format!("PTRACE_PEEKDATA failed: {}", e))?;
        let restored_word = (word & !0xFF) | (original_byte as i64);
        ptrace::write(self.pid, address as ptrace::AddressType, restored_word)
            .map_err(|e| format!("PTRACE_POKEDATA failed: {}", e))?;

        // Update state
        if let Some(bp) = self.breakpoints.get_mut(&address) {
            bp.enabled = false;
            debug!("Disabled breakpoint #{} at 0x{:x}", bp.id, address);
        }

        Ok(())
    }

    /// Re-enable a previously disabled breakpoint.
    pub fn enable_breakpoint(&mut self, address: u64) -> Result<(), String> {
        let is_enabled = {
            let bp = self.breakpoints.get(&address)
                .ok_or_else(|| format!("No breakpoint at 0x{:x}", address))?;
            bp.enabled
        };

        if !is_enabled {
            // Inject INT3 (separate borrow)
            let word = ptrace::read(self.pid, address as ptrace::AddressType)
                .map_err(|e| format!("PTRACE_PEEKDATA failed: {}", e))?;
            let modified_word = (word & !0xFF) | (INT3 as i64);
            ptrace::write(self.pid, address as ptrace::AddressType, modified_word)
                .map_err(|e| format!("PTRACE_POKEDATA failed: {}", e))?;

            if let Some(bp) = self.breakpoints.get_mut(&address) {
                bp.enabled = true;
                debug!("Re-enabled breakpoint #{} at 0x{:x}", bp.id, address);
            }
        }

        Ok(())
    }

    /// Check if there's a breakpoint at the given address.
    pub fn is_breakpoint(&self, address: u64) -> bool {
        self.breakpoints.contains_key(&address)
    }

    /// Get breakpoint info for an address.
    pub fn get_breakpoint(&self, address: u64) -> Option<&Breakpoint> {
        self.breakpoints.get(&address)
    }

    /// Handle a breakpoint hit.
    ///
    /// When a breakpoint is hit:
    /// 1. The RIP is at address+1 (past the INT3).
    /// 2. We restore the original byte.
    /// 3. Set RIP back by 1.
    /// 4. Single-step over the original instruction.
    /// 5. Re-insert INT3.
    ///
    /// Returns the address of the breakpoint that was hit, or None if not ours.
    pub fn handle_breakpoint_hit(&mut self) -> Result<Option<u64>, String> {
        // Get current RIP
        let regs = ptrace::getregs(self.pid)
            .map_err(|e| format!("PTRACE_GETREGS failed: {}", e))?;

        let rip = regs.rip;
        let bp_address = rip - 1;

        // Check if RIP-1 has a breakpoint
        let has_bp = self.breakpoints.contains_key(&bp_address);
        if !has_bp {
            return Ok(None);
        }

        debug!("Breakpoint hit at 0x{:x} (RIP=0x{:x})", bp_address, rip);

        // Restore original byte
        {
            let bp = self.breakpoints.get(&bp_address).unwrap();
            self.restore_byte(bp)?;
        }

        // Set RIP back to the breakpoint address
        let mut new_regs = regs;
        new_regs.rip = bp_address;
        ptrace::setregs(self.pid, new_regs)
            .map_err(|e| format!("PTRACE_SETREGS failed: {}", e))?;

        // Single-step over the original instruction
        ptrace::step(self.pid, None)
            .map_err(|e| format!("PTRACE_SINGLESTEP failed: {}", e))?;

        // Wait for the step to complete
        nix::sys::wait::waitpid(self.pid, None)
            .map_err(|e| format!("waitpid after step failed: {}", e))?;

        // Re-insert INT3
        {
            let bp = self.breakpoints.get(&bp_address).unwrap();
            self.inject_int3(bp)?;
        }

        Ok(Some(bp_address))
    }

    /// Remove all breakpoints.
    pub fn remove_all(&mut self) -> Result<(), String> {
        let addresses: Vec<u64> = self.breakpoints.keys().copied().collect();
        for addr in addresses {
            self.remove_breakpoint(addr)?;
        }
        Ok(())
    }

    /// Restore the original byte at a breakpoint location.
    fn restore_byte(&self, bp: &Breakpoint) -> Result<(), String> {
        let word = ptrace::read(self.pid, bp.address as ptrace::AddressType)
            .map_err(|e| format!("PTRACE_PEEKDATA failed: {}", e))?;

        // Put original byte back in the low position
        let restored_word = (word & !0xFF) | (bp.original_byte as i64);

        ptrace::write(self.pid, bp.address as ptrace::AddressType, restored_word)
            .map_err(|e| format!("PTRACE_POKEDATA failed: {}", e))?;

        Ok(())
    }

    /// Inject INT3 at a breakpoint location.
    fn inject_int3(&self, bp: &Breakpoint) -> Result<(), String> {
        let word = ptrace::read(self.pid, bp.address as ptrace::AddressType)
            .map_err(|e| format!("PTRACE_PEEKDATA failed: {}", e))?;

        let modified_word = (word & !0xFF) | (INT3 as i64);

        ptrace::write(self.pid, bp.address as ptrace::AddressType, modified_word)
            .map_err(|e| format!("PTRACE_POKEDATA failed: {}", e))?;

        Ok(())
    }
}

impl Drop for BreakpointManager {
    fn drop(&mut self) {
        // Try to clean up breakpoints when dropped
        for bp in self.breakpoints.values() {
            if bp.enabled {
                // Best-effort restore
                let _ = self.restore_byte(bp);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_manager_new() {
        let mgr = BreakpointManager::new(1234);
        assert_eq!(mgr.pid(), 1234);
        assert_eq!(mgr.breakpoint_count(), 0);
    }

    #[test]
    fn test_breakpoint_state() {
        let bp = Breakpoint {
            address: 0x1000,
            original_byte: 0x55,
            enabled: true,
            id: 0,
            function_name: Some("main".into()),
        };

        assert_eq!(bp.address, 0x1000);
        assert_eq!(bp.original_byte, 0x55);
        assert!(bp.enabled);
        assert_eq!(bp.function_name.as_deref(), Some("main"));
    }

    #[test]
    fn test_int3_opcode() {
        assert_eq!(INT3, 0xCC);
    }

    #[test]
    fn test_breakpoint_manager_no_breakpoints_initially() {
        let mgr = BreakpointManager::new(9999);
        assert!(!mgr.is_breakpoint(0x1000));
        assert!(mgr.get_breakpoint(0x1000).is_none());
        assert_eq!(mgr.breakpoint_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_breakpoint() {
        let mut mgr = BreakpointManager::new(9999);
        let result = mgr.remove_breakpoint(0x1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No breakpoint"));
    }

    #[test]
    fn test_remove_by_nonexistent_id() {
        let mut mgr = BreakpointManager::new(9999);
        let result = mgr.remove_breakpoint_by_id(42);
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_nonexistent() {
        let mut mgr = BreakpointManager::new(9999);
        let result = mgr.disable_breakpoint(0x1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_enable_nonexistent() {
        let mut mgr = BreakpointManager::new(9999);
        let result = mgr.enable_breakpoint(0x1000);
        assert!(result.is_err());
    }

    // Note: set_breakpoint, handle_breakpoint_hit, etc. require a real
    // traced process and are tested via integration tests (T12).
    // They use PTRACE_PEEKDATA/PTRACE_POKEDATA which only work on a traced child.
}
