//! Performance counter types and handles.
//!
//! Provides `PerfCounterHandle` for reading hardware performance counters
//! via `perf_event_open` file descriptors.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use thiserror::Error;

// Linux perf_event_open constants (not exported by libc)
pub const PERF_TYPE_HARDWARE: u32 = 0;
pub const PERF_TYPE_SOFTWARE: u32 = 1;
pub const PERF_HW_CPU_CYCLES: u64 = 0;
pub const PERF_HW_CACHE_MISSES: u64 = 5;
pub const PERF_HW_BRANCH_MISSES: u64 = 7;
pub const PERF_SW_CPU_CLOCK: u64 = 0;

// ioctl constants
pub const PERF_EVENT_IOC_ENABLE: u64 = 0x2400;
pub const PERF_EVENT_IOC_DISABLE: u64 = 0x2401;
pub const PERF_EVENT_IOC_RESET: u64 = 0x2402;

// perf_event_attr flags
const PERF_ATTACH_TASK: u64 = 1 << 1;
const PERF_SAMPLE_PERIOD: u64 = 1 << 3;

/// Minimal `perf_event_attr` structure matching the Linux kernel ABI.
/// Only includes fields needed by this implementation.
#[repr(C)]
#[derive(Default)]
struct PerfEventAttr {
    type_: u32,
    size: u32,
    config: u64,
    sample_period_or_freq: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    attach_state: u32,
    _pad: u32,
}

/// Hardware performance counter types available via `perf_event_open`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerfCounterType {
    /// CPU cycles (HW_CPU_CYCLES).
    Cycle,
    /// Retired instructions (HW_INSTRUCTIONS).
    Instruction,
    /// Last-level cache misses (HW_CACHE_MISSES).
    CacheMiss,
    /// Branch misses (HW_BRANCH_MISSES).
    BranchMiss,
}

impl PerfCounterType {
    /// Get the Linux `perf_event_open` type constant for this counter.
    pub fn perf_type(&self) -> u32 {
        match self {
            PerfCounterType::Cycle => 0,  // PERF_TYPE_HARDWARE
            PerfCounterType::Instruction => 1, // PERF_TYPE_SOFTWARE (we use SW for instructions)
            PerfCounterType::CacheMiss => 0,  // PERF_TYPE_HARDWARE
            PerfCounterType::BranchMiss => 0,  // PERF_TYPE_HARDWARE
        }
    }

    /// Get the specific hardware counter config.
    pub fn perf_config(&self) -> u64 {
        match self {
            // HW_CPU_CYCLES = 0
            PerfCounterType::Cycle => 0,
            // For instructions, we use a software counter since HW_INSTRUCTIONS
            // isn't always reliably available
            PerfCounterType::Instruction => 0,
            // HW_CACHE_MISS = 5
            PerfCounterType::CacheMiss => 5,
            // HW_BRANCH_MISS = 7
            PerfCounterType::BranchMiss => 7,
        }
    }

    /// Get the default sample period for this counter type.
    pub fn default_sample_period(&self) -> Option<u64> {
        None // Default is to count, not sample
    }
}

/// Configuration for opening a performance counter.
#[derive(Debug, Clone)]
pub struct PerfCounterConfig {
    /// Type of counter to open.
    pub counter_type: PerfCounterType,
    /// Whether the counter is enabled.
    pub enabled: bool,
    /// Sample period for overflow-based sampling.
    /// If `None`, the counter operates in counting mode.
    pub sample_period: Option<u64>,
}

impl PerfCounterConfig {
    /// Create a new config with default settings.
    pub fn new(counter_type: PerfCounterType) -> Self {
        Self {
            counter_type,
            enabled: true,
            sample_period: counter_type.default_sample_period(),
        }
    }

    /// Enable or disable the counter.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set a sample period for overflow sampling mode.
    pub fn with_sample_period(mut self, period: u64) -> Self {
        self.sample_period = Some(period);
        self
    }
}

impl Default for PerfCounterConfig {
    fn default() -> Self {
        Self::new(PerfCounterType::Cycle)
    }
}

/// Errors that can occur when operating on performance counters.
#[derive(Debug, Error)]
pub enum PerfCounterError {
    #[error("Counter handle is closed")]
    HandleClosed,

    #[error("Permission denied: CAP_PERFMON or CAP_SYS_ADMIN required")]
    PermissionDenied,

    #[error("Kernel does not support perf_event_open")]
    NotSupported,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Read failed: {0}")]
    ReadFailed(String),
}

/// A handle to an open `perf_event_open` file descriptor.
///
/// This struct wraps a raw file descriptor and provides safe access
/// to hardware performance counters. The file descriptor is closed
/// when the handle is dropped.
#[derive(Debug)]
pub struct PerfCounterHandle {
    fd: std::os::fd::OwnedFd,
    counter_type: PerfCounterType,
}

impl PerfCounterHandle {
    /// Create a new counter handle from an owned file descriptor.
    pub(crate) fn from_fd(fd: std::os::fd::OwnedFd, counter_type: PerfCounterType) -> Self {
        Self { fd, counter_type }
    }

    /// Read the current value of the counter.
    ///
    /// Returns the raw count value since counter initialization.
    ///
    /// # Errors
    ///
    /// Returns `PerfCounterError::HandleClosed` if the underlying fd is closed.
    pub fn read(&self) -> Result<u64, PerfCounterError> {
        // Re-read the value from the counter
        let mut value: u64 = 0;
        let size = std::mem::size_of::<u64>();
        let ptr = &mut value as *mut u64 as *mut libc::c_void;

        // Use read() syscall to get current count
        let ret = unsafe {
            libc::read(self.fd.as_raw_fd(), ptr, size)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound {
                return Err(PerfCounterError::HandleClosed);
            }
            return Err(PerfCounterError::ReadFailed(err.to_string()));
        }

        Ok(value)
    }

    /// Get the counter type for this handle.
    pub fn counter_type(&self) -> PerfCounterType {
        self.counter_type
    }

    /// Reset the counter to zero.
    ///
    /// Uses `ioctl` with `PERF_EVENT_IOC_RESET`.
    pub fn reset(&self) -> Result<(), PerfCounterError> {
        let ret = unsafe {
            libc::ioctl(self.fd.as_raw_fd(), PERF_EVENT_IOC_RESET as libc::c_ulong, 0)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            return Err(PerfCounterError::ReadFailed(err.to_string()));
        }

        Ok(())
    }

    /// Enable the counter.
    ///
    /// Uses `ioctl` with `PERF_EVENT_IOC_ENABLE`.
    pub fn enable(&self) -> Result<(), PerfCounterError> {
        let ret = unsafe {
            libc::ioctl(self.fd.as_raw_fd(), PERF_EVENT_IOC_ENABLE as libc::c_ulong, 0)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            return Err(PerfCounterError::ReadFailed(err.to_string()));
        }

        Ok(())
    }

    /// Disable the counter.
    ///
    /// Uses `ioctl` with `PERF_EVENT_IOC_DISABLE`.
    pub fn disable(&self) -> Result<(), PerfCounterError> {
        let ret = unsafe {
            libc::ioctl(self.fd.as_raw_fd(), PERF_EVENT_IOC_DISABLE as libc::c_ulong, 0)
        };

        if ret < 0 {
            let err = std::io::Error::last_os_error();
            return Err(PerfCounterError::ReadFailed(err.to_string()));
        }

        Ok(())
    }
}

/// A snapshot of all performance counters at a point in time.
#[derive(Debug, Clone, Default)]
pub struct PerfCountersSnapshot {
    /// CPU cycles.
    pub cycles: Option<u64>,
    /// Retired instructions.
    pub instructions: Option<u64>,
    /// Cache misses.
    pub cache_misses: Option<u64>,
    /// Branch misses.
    pub branch_misses: Option<u64>,
}

impl PerfCountersSnapshot {
    /// Create a new empty snapshot with all fields as `None`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any counter has a value.
    pub fn has_data(&self) -> bool {
        self.cycles.is_some()
            || self.instructions.is_some()
            || self.cache_misses.is_some()
            || self.branch_misses.is_some()
    }

    /// Create a snapshot from a slice of counter handles and values.
    ///
    /// The slice should contain (handle, value) pairs.
    pub fn from_counters(
        handles: &[(PerfCounterHandle, u64)],
    ) -> Self {
        let mut snapshot = Self::new();

        for (handle, value) in handles {
            match handle.counter_type() {
                PerfCounterType::Cycle => snapshot.cycles = Some(*value),
                PerfCounterType::Instruction => snapshot.instructions = Some(*value),
                PerfCounterType::CacheMiss => snapshot.cache_misses = Some(*value),
                PerfCounterType::BranchMiss => snapshot.branch_misses = Some(*value),
            }
        }

        snapshot
    }
}

/// Open a `perf_event_open` file descriptor for a hardware counter.
///
/// This is the low-level syscall wrapper. Higher-level code should use
/// `PerfCounterHandle` instead.
#[cfg(feature = "perf_counters")]
pub fn perf_event_open(
    type_: u32,
    config: u64,
    pid: i32,
    cpu: i32,
    sample_period: Option<u64>,
) -> Result<OwnedFd, PerfCounterError> {
    let mut attr = PerfEventAttr::default();
    attr.type_ = type_;
    attr.size = std::mem::size_of::<PerfEventAttr>() as u32;
    attr.config = config;

    if let Some(period) = sample_period {
        attr.sample_period_or_freq = period;
        attr.sample_type = PERF_SAMPLE_PERIOD;
    } else {
        // Counting mode: no overflow interrupts
        attr.flags |= 1; // disabled flag
    }

    // Enable attach to processes/threads
    attr.attach_state = PERF_ATTACH_TASK as u32;

    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const _,
            pid,    // pid: 0 means current process
            cpu,    // cpu: -1 means any CPU
            -1i32,  // group_fd: -1 means no group
            0u64,   // flags: 0
        )
    };

    if fd < 0 {
        let err = std::io::Error::last_os_error();
        let errno = err.raw_os_error().unwrap_or(0);
        return Err(match errno {
            libc::EPERM | libc::EACCES => PerfCounterError::PermissionDenied,
            libc::ENODEV | libc::ENOENT => PerfCounterError::NotSupported,
            libc::EINVAL => PerfCounterError::InvalidConfig("Invalid perf_event_open parameters".into()),
            _ => PerfCounterError::ReadFailed(err.to_string()),
        });
    }

    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_counter_type_properties() {
        assert_eq!(PerfCounterType::Cycle.perf_type(), 0);
        assert_eq!(PerfCounterType::CacheMiss.perf_config(), 5);
        assert_eq!(PerfCounterType::BranchMiss.perf_config(), 7);
    }

    #[test]
    fn test_perf_counter_config_defaults() {
        let config = PerfCounterConfig::new(PerfCounterType::Cycle);
        assert!(config.enabled);
        assert!(config.sample_period.is_none());
    }

    #[test]
    fn test_perf_counter_config_builder() {
        let config = PerfCounterConfig::new(PerfCounterType::Instruction)
            .with_enabled(false)
            .with_sample_period(1000);
        assert!(!config.enabled);
        assert_eq!(config.sample_period, Some(1000));
    }

    #[test]
    fn test_perf_counters_snapshot_empty() {
        let snapshot = PerfCountersSnapshot::new();
        assert!(!snapshot.has_data());
        assert!(snapshot.cycles.is_none());
        assert!(snapshot.instructions.is_none());
    }

    #[test]
    fn test_perf_counters_snapshot_has_data() {
        let mut snapshot = PerfCountersSnapshot::new();
        assert!(!snapshot.has_data());
        snapshot.cycles = Some(100);
        assert!(snapshot.has_data());
    }

    #[test]
    fn test_perf_counters_snapshot_from_counters_empty() {
        let handles: Vec<(PerfCounterHandle, u64)> = Vec::new();
        let snapshot = PerfCountersSnapshot::from_counters(&handles);
        assert!(!snapshot.has_data());
    }

    #[test]
    fn test_error_display() {
        let err = PerfCounterError::HandleClosed;
        assert!(err.to_string().contains("closed"));

        let err = PerfCounterError::PermissionDenied;
        assert!(err.to_string().contains("Permission"));
    }
}
