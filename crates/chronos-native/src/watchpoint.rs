//! Hardware watchpoint management using x86 debug registers (DR0-DR7).
//!
//! Provides zero-overhead memory access watchpoints via CPU debug registers.
//! On x86-64, DR0-DR3 hold watchpoint addresses, DR6 holds status, DR7 controls enable/size/condition.

use nix::sys::ptrace;
use nix::unistd::Pid;
use std::os::raw::{c_long, c_void};
use thiserror::Error;

/// x86-64 hardware watchpoint conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchpointCondition {
    Execute = 0b00,   // DR7 R/W = 00
    Write = 0b01,     // DR7 R/W = 01
    ReadWrite = 0b11,  // DR7 R/W = 11
}

/// Size of memory region to watch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchpointSize {
    Byte1 = 0b00,
    Byte2 = 0b01,
    Byte4 = 0b11,
    Byte8 = 0b10,  // Note: 0b10 = 8 bytes on x86-64
}

/// A configured hardware watchpoint
#[derive(Debug, Clone)]
pub struct HardwareWatchpoint {
    /// Debug register index (0-3)
    pub dr_index: u8,
    /// Watched memory address
    pub address: u64,
    /// Access condition
    pub condition: WatchpointCondition,
    /// Size of watched region
    pub size: WatchpointSize,
}

/// Manages the 4 x86 debug registers for a single process
pub struct HardwareWatchpointManager {
    pid: Pid,
    watchpoints: [Option<HardwareWatchpoint>; 4],
}

impl HardwareWatchpointManager {
    /// Create a new manager for the given process ID.
    pub fn new(pid: u32) -> Self {
        Self {
            pid: Pid::from_raw(pid as i32),
            watchpoints: [None, None, None, None],
        }
    }

    /// Set a hardware watchpoint. Returns the watchpoint or error if all 4 slots are used.
    pub fn set_watchpoint(
        &mut self,
        address: u64,
        condition: WatchpointCondition,
        size: WatchpointSize,
    ) -> Result<HardwareWatchpoint, WatchpointError> {
        let slot = self
            .watchpoints
            .iter()
            .position(|w| w.is_none())
            .ok_or(WatchpointError::MaxWatchpointsExceeded)?;

        // Write address to DRi
        self.write_dr(slot as u8, address)?;

        // Update DR7 to enable watchpoint
        let dr7 = self.read_dr7()?;
        let new_dr7 = Self::set_dr7_bits(dr7, slot as u8, condition, size);
        self.write_dr(7, new_dr7)?;

        let wp = HardwareWatchpoint {
            dr_index: slot as u8,
            address,
            condition,
            size,
        };
        self.watchpoints[slot] = Some(wp.clone());
        Ok(wp)
    }

    /// Clear a specific watchpoint by DR index.
    pub fn clear_watchpoint(&mut self, dr_index: u8) -> Result<(), WatchpointError> {
        if dr_index > 3 {
            return Err(WatchpointError::InvalidIndex(dr_index));
        }
        let dr7 = self.read_dr7()?;
        let new_dr7 = Self::clear_dr7_bits(dr7, dr_index);
        self.write_dr(7, new_dr7)?;
        self.watchpoints[dr_index as usize] = None;
        Ok(())
    }

    /// Clear all watchpoints.
    pub fn clear_all(&mut self) -> Result<(), WatchpointError> {
        self.write_dr(7, 0)?; // disable all
        self.watchpoints = [None, None, None, None];
        Ok(())
    }

    /// Get DR register offset for a given index (0-7).
    /// These are offsets in bytes within the user area (struct user on x86-64).
    /// From Linux kernel: offsetof(struct user, u_debugreg[i]) = 848 + 8*i
    pub fn dr_offset(index: u8) -> u64 {
        match index {
            0 => 848,
            1 => 856,
            2 => 864,
            3 => 872,
            6 => 888,
            7 => 896,
            _ => panic!("invalid DR index {}", index),
        }
    }

    /// DR7 bit layout for watchpoint i:
    ///   L{i} = bit 2*i (local enable)
    ///   R/W{i} = bits 16+4*i .. 17+4*i
    ///   LEN{i} = bits 18+4*i .. 19+4*i
    pub fn set_dr7_bits(dr7: u64, i: u8, cond: WatchpointCondition, size: WatchpointSize) -> u64 {
        let mut v = dr7;
        // Enable local bit
        v |= 1u64 << (2 * i);
        // Clear R/W and LEN bits for this watchpoint
        let base = 16 + 4 * i as u64;
        v &= !(0b1111u64 << base);
        // Set R/W
        let rw: u64 = match cond {
            WatchpointCondition::Execute => 0b00,
            WatchpointCondition::Write => 0b01,
            WatchpointCondition::ReadWrite => 0b11,
        };
        v |= rw << base;
        // Set LEN
        let len: u64 = size as u64;
        v |= len << (base + 2);
        v
    }

    /// Clear DR7 bits for watchpoint i (disable and clear R/W+LEN).
    pub fn clear_dr7_bits(dr7: u64, i: u8) -> u64 {
        let mut v = dr7;
        v &= !(1u64 << (2 * i)); // disable local enable
        let base = 16 + 4 * i as u64;
        v &= !(0b1111u64 << base); // clear R/W + LEN
        v
    }

    /// Check DR6 to see which watchpoint triggered.
    /// Returns Some(dr_index) if a watchpoint trap is indicated, None otherwise.
    pub fn check_dr6(dr6: u64) -> Option<u8> {
        // DR6 bits B0-B3 indicate which watchpoint triggered
        if dr6 & 0x1 != 0 {
            Some(0)
        } else if dr6 & 0x2 != 0 {
            Some(1)
        } else if dr6 & 0x4 != 0 {
            Some(2)
        } else if dr6 & 0x8 != 0 {
            Some(3)
        } else {
            None
        }
    }

    fn write_dr(&self, index: u8, value: u64) -> Result<(), WatchpointError> {
        ptrace::write_user(self.pid, Self::dr_offset(index) as *mut c_void, value as c_long)
            .map_err(|e| WatchpointError::PtraceError(e.to_string()))
    }

    fn read_dr7(&self) -> Result<u64, WatchpointError> {
        ptrace::read_user(self.pid, Self::dr_offset(7) as *mut c_void)
            .map(|v| v as u64)
            .map_err(|e| WatchpointError::PtraceError(e.to_string()))
    }

    /// Get the current watchpoint configuration for inspection.
    pub fn watchpoints(&self) -> &[Option<HardwareWatchpoint>; 4] {
        &self.watchpoints
    }
}

#[derive(Debug, Error)]
pub enum WatchpointError {
    #[error("Maximum 4 hardware watchpoints exceeded")]
    MaxWatchpointsExceeded,
    #[error("Invalid DR index: {0}")]
    InvalidIndex(u8),
    #[error("ptrace error: {0}")]
    PtraceError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dr7_set_write_watchpoint_dr0() {
        // write watchpoint on DR0, 8 bytes: L0=1, R/W0=01, LEN0=10
        let dr7 = HardwareWatchpointManager::set_dr7_bits(
            0,
            0,
            WatchpointCondition::Write,
            WatchpointSize::Byte8,
        );
        assert_ne!(dr7, 0);
        assert!(dr7 & 0x1 != 0, "L0 enable bit should be set");
        assert_eq!((dr7 >> 16) & 0b11, 0b01, "R/W=write");
        assert_eq!((dr7 >> 18) & 0b11, 0b10, "LEN=8bytes");
    }

    #[test]
    fn test_dr7_set_readwrite_watchpoint_dr1() {
        // readwrite watchpoint on DR1, 4 bytes: L1=1, R/W1=11, LEN1=11
        let dr7 = HardwareWatchpointManager::set_dr7_bits(
            0,
            1,
            WatchpointCondition::ReadWrite,
            WatchpointSize::Byte4,
        );
        assert!(dr7 & 0x4 != 0, "L1 enable bit should be set");
        assert_eq!((dr7 >> 20) & 0b11, 0b11, "R/W=readwrite");
        assert_eq!((dr7 >> 22) & 0b11, 0b11, "LEN=4bytes");
    }

    #[test]
    fn test_dr7_set_execute_watchpoint_dr2() {
        // execute watchpoint on DR2, 1 byte: L2=1, R/W2=00, LEN2=00
        let dr7 = HardwareWatchpointManager::set_dr7_bits(
            0,
            2,
            WatchpointCondition::Execute,
            WatchpointSize::Byte1,
        );
        assert!(dr7 & 0x10 != 0, "L2 enable bit should be set");
        assert_eq!((dr7 >> 24) & 0b11, 0b00, "R/W=execute");
        assert_eq!((dr7 >> 26) & 0b11, 0b00, "LEN=1byte");
    }

    #[test]
    fn test_dr7_set_byte2_watchpoint_dr3() {
        // write watchpoint on DR3, 2 bytes: L3=1, R/W3=01, LEN3=01
        let dr7 = HardwareWatchpointManager::set_dr7_bits(
            0,
            3,
            WatchpointCondition::Write,
            WatchpointSize::Byte2,
        );
        assert!(dr7 & 0x40 != 0, "L3 enable bit should be set");
        assert_eq!((dr7 >> 28) & 0b11, 0b01, "R/W=write");
        assert_eq!((dr7 >> 30) & 0b11, 0b01, "LEN=2bytes");
    }

    #[test]
    fn test_dr7_clear_bits() {
        let dr7 = HardwareWatchpointManager::set_dr7_bits(
            0,
            0,
            WatchpointCondition::Write,
            WatchpointSize::Byte8,
        );
        let cleared = HardwareWatchpointManager::clear_dr7_bits(dr7, 0);
        assert_eq!(cleared & 0x1, 0, "L0 should be disabled");
        assert_eq!((cleared >> 16) & 0b1111, 0, "R/W+LEN should be cleared");
    }

    #[test]
    fn test_dr7_clear_bits_preserves_other_watchpoints() {
        // Set DR0 and DR1, then clear only DR0
        let dr7 = HardwareWatchpointManager::set_dr7_bits(0, 0, WatchpointCondition::Write, WatchpointSize::Byte8);
        let dr7 = HardwareWatchpointManager::set_dr7_bits(dr7, 1, WatchpointCondition::ReadWrite, WatchpointSize::Byte4);
        
        // Clear DR0 only
        let cleared = HardwareWatchpointManager::clear_dr7_bits(dr7, 0);
        
        // DR0 should be cleared
        assert_eq!(cleared & 0x1, 0, "L0 should be disabled");
        assert_eq!((cleared >> 16) & 0b1111, 0, "DR0 R/W+LEN should be cleared");
        
        // DR1 should still be set
        assert!(cleared & 0x4 != 0, "L1 should still be enabled");
        assert_eq!((cleared >> 20) & 0b11, 0b11, "DR1 R/W should still be readwrite");
        assert_eq!((cleared >> 22) & 0b11, 0b11, "DR1 LEN should still be 4bytes");
    }

    #[test]
    fn test_dr7_multiple_watchpoints() {
        // Set all 4 watchpoints with different conditions
        let dr7_0 = HardwareWatchpointManager::set_dr7_bits(0, 0, WatchpointCondition::Write, WatchpointSize::Byte8);
        let dr7_1 = HardwareWatchpointManager::set_dr7_bits(dr7_0, 1, WatchpointCondition::ReadWrite, WatchpointSize::Byte4);
        let dr7_2 = HardwareWatchpointManager::set_dr7_bits(dr7_1, 2, WatchpointCondition::Execute, WatchpointSize::Byte1);
        let dr7_3 = HardwareWatchpointManager::set_dr7_bits(dr7_2, 3, WatchpointCondition::Write, WatchpointSize::Byte2);

        // All enable bits should be set
        assert!(dr7_3 & 0x1 != 0, "L0 should be set");
        assert!(dr7_3 & 0x4 != 0, "L1 should be set");
        assert!(dr7_3 & 0x10 != 0, "L2 should be set");
        assert!(dr7_3 & 0x40 != 0, "L3 should be set");
    }

    #[test]
    fn test_dr6_check() {
        // B0 set
        assert_eq!(HardwareWatchpointManager::check_dr6(0x1), Some(0));
        // B1 set
        assert_eq!(HardwareWatchpointManager::check_dr6(0x2), Some(1));
        // B2 set
        assert_eq!(HardwareWatchpointManager::check_dr6(0x4), Some(2));
        // B3 set
        assert_eq!(HardwareWatchpointManager::check_dr6(0x8), Some(3));
        // Multiple bits set - returns first match
        assert_eq!(HardwareWatchpointManager::check_dr6(0xF), Some(0));
        // No bits set
        assert_eq!(HardwareWatchpointManager::check_dr6(0x0), None);
        // Only BD bit set (debug register access detected)
        assert_eq!(HardwareWatchpointManager::check_dr6(0x20), None);
    }

    #[test]
    fn test_watchpoint_condition_values() {
        assert_eq!(WatchpointCondition::Execute as u64, 0b00);
        assert_eq!(WatchpointCondition::Write as u64, 0b01);
        assert_eq!(WatchpointCondition::ReadWrite as u64, 0b11);
    }

    #[test]
    fn test_watchpoint_size_values() {
        assert_eq!(WatchpointSize::Byte1 as u64, 0b00);
        assert_eq!(WatchpointSize::Byte2 as u64, 0b01);
        assert_eq!(WatchpointSize::Byte4 as u64, 0b11);
        assert_eq!(WatchpointSize::Byte8 as u64, 0b10);
    }

    #[test]
    fn test_dr_offset_values() {
        assert_eq!(HardwareWatchpointManager::dr_offset(0), 848);
        assert_eq!(HardwareWatchpointManager::dr_offset(1), 856);
        assert_eq!(HardwareWatchpointManager::dr_offset(2), 864);
        assert_eq!(HardwareWatchpointManager::dr_offset(3), 872);
        assert_eq!(HardwareWatchpointManager::dr_offset(6), 888);
        assert_eq!(HardwareWatchpointManager::dr_offset(7), 896);
    }

    #[test]
    #[should_panic(expected = "invalid DR index 5")]
    fn test_dr_offset_panics_on_invalid_index() {
        let _ = HardwareWatchpointManager::dr_offset(5);
    }

    #[test]
    fn test_watchpoint_error_messages() {
        let err = WatchpointError::MaxWatchpointsExceeded;
        assert_eq!(err.to_string(), "Maximum 4 hardware watchpoints exceeded");

        let err = WatchpointError::InvalidIndex(2);
        assert_eq!(err.to_string(), "Invalid DR index: 2");

        let err = WatchpointError::PtraceError("Permission denied".to_string());
        assert_eq!(err.to_string(), "ptrace error: Permission denied");
    }
}