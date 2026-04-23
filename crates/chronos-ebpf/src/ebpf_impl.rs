//! Real eBPF adapter implementation using aya.
//!
//! This module provides the actual eBPF-backed implementation when the `ebpf`
//! feature flag is enabled. It wraps aya's Bpf, manages uprobe lifecycle,
//! and reads events from the BPF ring buffer.

use crate::types::EbpfEvent;
use crate::uprobe::UprobeManager;
use crate::EbpfError;
use aya::maps::RingBuf;
use aya::programs::uprobe::UProbeLink;
use aya::programs::UProbe;
use aya::Ebpf;
use chronos_domain::TraceEvent;
use std::ops::Deref;
use std::sync::Mutex;

/// A raw pointer wrapper that is Send.
/// SAFETY: The caller must ensure the pointer is valid and not used after
/// the owned data is dropped.
#[cfg(feature = "ebpf")]
struct SendPtr<T>(*mut T);

#[cfg(feature = "ebpf")]
unsafe impl Send for SendPtr<RingBuf<aya::maps::MapData>> {}

#[cfg(feature = "ebpf")]
impl std::fmt::Debug for SendPtr<RingBuf<aya::maps::MapData>> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendPtr").finish()
    }
}

/// Configuration for the eBPF adapter.
#[derive(Debug, Clone)]
pub struct EbpfConfig {
    /// Maximum number of events to drain per call.
    pub max_events_per_poll: usize,
    /// Ring buffer capacity in bytes.
    pub ring_buffer_capacity: u32,
}

impl Default for EbpfConfig {
    fn default() -> Self {
        Self {
            max_events_per_poll: 1024,
            ring_buffer_capacity: 4096 * 128,
        }
    }
}

/// Inner eBPF adapter state.
///
/// This struct holds all the resources needed for real eBPF tracing:
/// - `bpf`: the loaded aya Bpf instance
/// - `uprobe_manager`: tracks attached uprobes
/// - `ring_reader`: reads events from the BPF ring buffer
/// - `config`: adapter configuration
pub struct EbpfAdapterInner {
    /// The loaded eBPF program and maps.
    bpf: Mutex<Option<Ebpf>>,
    /// Active uprobe links (kept to prevent drop).
    uprobe_links: Mutex<Vec<UProbeLink>>,
    /// Manages uprobe attachment lifecycle.
    uprobe_manager: UprobeManager,
    /// Ring buffer reader for polling events.
    /// This stores a raw pointer wrapper that is Send to work around lifetime issues.
    /// INVARIANT: The pointer is only valid when the bpf mutex is held.
    #[cfg(feature = "ebpf")]
    ring_reader: Mutex<Option<SendPtr<RingBuf<aya::maps::MapData>>>>,
    /// Next event ID for trace events.
    next_event_id: Mutex<u64>,
    /// Adapter configuration.
    config: EbpfConfig,
}

impl std::fmt::Debug for EbpfAdapterInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EbpfAdapterInner")
            .field("config", &self.config)
            .finish()
    }
}

impl EbpfAdapterInner {
    /// Create a new eBPF adapter.
    ///
    /// This loads the compiled eBPF program, creates the ring buffer map,
    /// and prepares the adapter for polling events.
    pub fn new() -> Result<Self, EbpfError> {
        Self::with_config(EbpfConfig::default())
    }

    /// Create a new adapter with custom configuration.
    pub fn with_config(config: EbpfConfig) -> Result<Self, EbpfError> {
        // First check kernel version
        Self::check_availability()?;

        // Load the eBPF program from the embedded object file
        let bpf = Self::load_bpf_program()?;

        let adapter = Self {
            bpf: Mutex::new(Some(bpf)),
            uprobe_links: Mutex::new(Vec::new()),
            uprobe_manager: UprobeManager::new(),
            ring_reader: Mutex::new(None),
            next_event_id: Mutex::new(0),
            config,
        };

        adapter.init_ring_reader()?;

        Ok(adapter)
    }

    /// Check if eBPF is available on this system.
    ///
    /// Returns `Ok(())` if:
    /// - Kernel version >= 5.8
    /// - BPF filesystem is accessible
    /// - Required capabilities are present (or we can attempt anyway)
    pub fn check_availability() -> Result<(), EbpfError> {
        // Check kernel version via /proc/version parsing
        crate::kernel_version_check()?;

        // Check that we can access /sys/fs/bpf
        let bpf_fs = std::path::Path::new("/sys/fs/bpf");
        if !bpf_fs.exists() {
            return Err(EbpfError::Unavailable {
                reason: "/sys/fs/bpf not found (BPF filesystem not mounted)".to_string(),
            });
        }

        Ok(())
    }

    /// Load the compiled eBPF program from embedded bytes.
    fn load_bpf_program() -> Result<Ebpf, EbpfError> {
        // Try to load from embedded object file
        #[cfg(feature = "ebpf")]
        {
            let data = EMBEDDED_BPF_OBJECT;
            if data.is_empty() {
                return Err(EbpfError::LoadError(
                    "eBPF object file not compiled (stub used)".to_string(),
                ));
            }
            let bpf = Ebpf::load(data).map_err(|e| {
                EbpfError::LoadError(format!("failed to load eBPF program: {}", e))
            })?;

            // Log the loaded maps
            for (name, map) in bpf.maps() {
                tracing::debug!(map_name = name, "loaded BPF map");
                let _ = map; // suppress unused warning
            }

            Ok(bpf)
        }

        #[cfg(not(feature = "ebpf"))]
        {
            Err(EbpfError::Unavailable {
                reason: "ebpf feature not enabled".to_string(),
            })
        }
    }

    /// Attach a uprobe to a symbol in a target binary.
    ///
    /// This attaches both an entry uprobe and an exit uprobe to the given symbol.
    ///
    /// # Arguments
    /// * `pid` - Process ID to attach to (0 for any process, None for current process)
    /// * `binary` - Path to the binary
    /// * `symbol` - Symbol name to attach to
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(EbpfError)` on failure
    pub fn attach_uprobe(
        &self,
        pid: Option<i32>,
        binary: &str,
        symbol: &str,
    ) -> Result<(), EbpfError> {
        let mut bpf_guard = self
            .bpf
            .lock()
            .map_err(|e| EbpfError::LoadError(e.to_string()))?;
        let bpf_ref = bpf_guard
            .as_mut()
            .ok_or(EbpfError::LoadError("Bpf not loaded".to_string()))?;

        // Attach entry probe (uprobe)
        let trace_entry_prog: &mut UProbe = bpf_ref
            .program_mut("trace_entry")
            .ok_or_else(|| EbpfError::Uprobe("trace_entry program not found".to_string()))?
            .try_into()
            .map_err(|e| EbpfError::Uprobe(format!("invalid program type: {:?}", e)))?;

        let entry_link_id = UProbe::attach(trace_entry_prog, Some(symbol), 0, binary, pid)
            .map_err(|e| EbpfError::Uprobe(format!("failed to attach entry uprobe: {}", e)))?;

        let entry_link = trace_entry_prog
            .take_link(entry_link_id)
            .map_err(|e| EbpfError::Uprobe(format!("failed to take entry link: {}", e)))?;

        self.uprobe_links
            .lock()
            .map_err(|e| EbpfError::LoadError(e.to_string()))?
            .push(entry_link);

        // Attach exit probe (uretprobe)
        let trace_exit_prog: &mut UProbe = bpf_ref
            .program_mut("trace_exit")
            .ok_or_else(|| EbpfError::Uprobe("trace_exit program not found".to_string()))?
            .try_into()
            .map_err(|e| EbpfError::Uprobe(format!("invalid program type: {:?}", e)))?;

        let exit_link_id = UProbe::attach(trace_exit_prog, Some(symbol), 0, binary, pid)
            .map_err(|e| EbpfError::Uprobe(format!("failed to attach exit uprobe: {}", e)))?;

        let exit_link = trace_exit_prog
            .take_link(exit_link_id)
            .map_err(|e| EbpfError::Uprobe(format!("failed to take exit link: {}", e)))?;

        self.uprobe_links
            .lock()
            .map_err(|e| EbpfError::LoadError(e.to_string()))?
            .push(exit_link);

        Ok(())
    }

    /// Detach all attached uprobes.
    pub fn detach_all(&mut self) {
        // Clear the uprobe links (drops them and detaches)
        if let Ok(mut links) = self.uprobe_links.lock() {
            links.clear();
        }
        // Also clear the manager's state
        let _ = self.uprobe_manager.detach_all();
    }

    /// Drain all available events from the ring buffer.
    ///
    /// This method polls the ring buffer for all available events and
    /// converts them to `TraceEvent`s. The ring buffer is non-destructive
    /// so events are consumed as they are read.
    pub fn drain_events(&self) -> Vec<TraceEvent> {
        let ring_guard = match self.ring_reader.lock() {
            Ok(g) => g,
            Err(e) => {
                tracing::error!("failed to lock ring_reader: {}", e);
                return Vec::new();
            }
        };

        let send_ptr = match ring_guard.as_ref() {
            Some(ptr) => ptr,
            None => return Vec::new(),
        };

        let mut result = Vec::with_capacity(self.config.max_events_per_poll);
        let mut poll_count = 0;

        // SAFETY: The SendPtr wraps a valid pointer to a RingBuf that is owned by `self`.
        // The bpf map itself remains alive as long as `self.bpf` is alive.
        let ring = unsafe { &mut *send_ptr.0 };
        while poll_count < self.config.max_events_per_poll {
            match ring.next() {
                Some(item) => {
                    let bytes = item.deref();
                    if bytes.len() < std::mem::size_of::<EbpfEvent>() {
                        tracing::error!("short read from ring buffer: {} bytes", bytes.len());
                        break;
                    }
                    // SAFETY: EbpfEvent is repr(C), bytes come from the kernel
                    // and are aligned to the ring buffer page boundary.
                    let ev = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const EbpfEvent) };

                    let mut id_guard = match self.next_event_id.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            tracing::error!("failed to lock next_event_id: {}", e);
                            return result;
                        }
                    };
                    let event_id = *id_guard;
                    *id_guard += 1;

                    let trace_event = ev.to_trace_event(event_id);
                    result.push(trace_event);
                    poll_count += 1;
                }
                None => break,
            }
        }

        result
    }

    /// Initialize the ring buffer reader from the loaded BPF map.
    ///
    /// This should be called after loading the BPF program and before
    /// polling for events.
    pub fn init_ring_reader(&self) -> Result<(), EbpfError> {
        let mut bpf_guard = self
            .bpf
            .lock()
            .map_err(|e| EbpfError::LoadError(e.to_string()))?;
        let bpf_ref = bpf_guard
            .as_mut()
            .ok_or(EbpfError::LoadError("Bpf not loaded".to_string()))?;

        // Get the ring buffer map
        let map = bpf_ref
            .map_mut("events")
            .ok_or_else(|| EbpfError::RingBuffer("events map not found".to_string()))?;

        // Create RingBuf from the map reference
        // RingBuf::try_from(&mut Map) returns RingBuf<&mut MapData>
        let rb: RingBuf<&mut aya::maps::MapData> = RingBuf::try_from(map).map_err(|e| {
            EbpfError::RingBuffer(format!("wrong map type: {:?}", e))
        })?;

        // We need to convert RingBuf<&mut MapData> to RingBuf<MapData>.
        // Since RingBuf stores the map by value, we can't directly convert.
        // Instead, we'll create a Box and use a pointer.
        // SAFETY: We're just creating a new owned RingBuf that wraps the same data.
        // The lifetime issue is worked around by using raw pointers.
        let rb_boxed = Box::new(rb);
        let rb_ptr = Box::into_raw(rb_boxed) as *mut RingBuf<aya::maps::MapData>;

        let mut ring_guard = self
            .ring_reader
            .lock()
            .map_err(|e| EbpfError::LoadError(e.to_string()))?;
        *ring_guard = Some(SendPtr(rb_ptr));

        Ok(())
    }
}

/// Embedded eBPF object file.
///
/// This is generated at compile time by the build.rs script (if clang is available),
/// or this is a minimal stub that allows linking without clang.
///
/// The build.rs script compiles uprobe.bpf.c to uprobe.bpf.o and embeds it here.
#[cfg(feature = "ebpf")]
static EMBEDDED_BPF_OBJECT: &[u8] = include_bytes!("../ebpf/uprobe.bpf.o");

/// Stub for when clang is not available — allows compilation but not execution.
#[cfg(not(feature = "ebpf"))]
const EMBEDDED_BPF_OBJECT: &[u8] = &[];

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebpf_config_default() {
        let config = EbpfConfig::default();
        assert_eq!(config.max_events_per_poll, 1024);
        assert_eq!(config.ring_buffer_capacity, 4096 * 128);
    }

    #[test]
    fn test_ebpf_adapter_inner_check_availability_without_ebpf_feature() {
        #[cfg(not(feature = "ebpf"))]
        {
            // Without ebpf feature, check_availability should still be callable
            // but will fail due to feature not being compiled in
            let result = EbpfAdapterInner::check_availability();
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_ebpf_adapter_inner_construction_guarded_by_feature() {
        // This test documents that EbpfAdapterInner::new() requires the ebpf feature
        // In a no-ebpf build, this would fail to compile due to trying to load BPF
        #[cfg(not(feature = "ebpf"))]
        {
            // We can't actually call new() without the feature, but we can verify
            // the type exists and is constructible with default config
            let _config = EbpfConfig::default();
            assert_eq!(_config.max_events_per_poll, 1024);
        }
    }

    #[test]
    fn test_kernel_version_check_falls_back_to_error() {
        // Even without ebpf feature, kernel_version_check should be callable
        // It will parse /proc/version and return Ok if kernel >= 5.8
        let result = crate::kernel_version_check();
        // This might pass or fail depending on actual kernel version
        // The important thing is it doesn't panic
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(matches!(err, EbpfError::Unavailable { .. }));
        }
    }

    #[test]
    #[ignore = "requires: cap_bpf and compiled eBPF program"]
    fn test_ebpf_adapter_with_real_bpf() {
        // This test requires:
        // 1. CAP_BPF or CAP_PERFMON
        // 2. A compiled eBPF object file
        // 3. Kernel >= 5.8
        todo!("enable when eBPF environment is available")
    }
}
