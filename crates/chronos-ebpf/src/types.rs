//! Wire format types for eBPF ↔ userspace communication.
//!
//! `EbpfEvent` is the fixed-size struct written into the BPF ring buffer
//! by uprobe programs. It must be `repr(C)` and `Copy` to match the
//! kernel-side layout exactly.

use serde::{Deserialize, Serialize};

/// Maximum length of a function name stored in an eBPF event.
pub const MAX_FUNC_NAME_LEN: usize = 64;

/// Event kind discriminant — mirrors `EventType` but uses a simple `u8`
/// for ABI compatibility with the BPF ring buffer.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EbpfEventKind {
    FunctionEntry = 0,
    FunctionExit = 1,
    VariableWrite = 2,
    MemoryWrite = 3,
}

impl Default for EbpfEventKind {
    fn default() -> Self {
        Self::FunctionEntry
    }
}

/// Fixed-size event struct written by BPF uprobe programs into the ring buffer.
///
/// Layout is `#[repr(C)]` so the kernel-side (BPF C or aya-bpf Rust) struct
/// can be read directly in userspace without re-serialization.
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EbpfEvent {
    /// Monotonic timestamp in nanoseconds (from `bpf_ktime_get_ns()`).
    pub timestamp_ns: u64,
    /// Thread / task group ID (from `bpf_get_current_pid_tgid()`).
    pub thread_id: u64,
    /// Instruction pointer (uprobe attach address).
    pub address: u64,
    /// Written value (for VariableWrite/MemoryWrite; 0 otherwise).
    pub value: u64,
    /// Event discriminant.
    pub kind: EbpfEventKind,
    /// Padding to maintain alignment.
    _pad: [u8; 7],
    /// Null-terminated function name (truncated to MAX_FUNC_NAME_LEN).
    #[serde(with = "serde_bytes_array")]
    pub function_name: [u8; MAX_FUNC_NAME_LEN],
}

impl Default for EbpfEvent {
    fn default() -> Self {
        Self {
            timestamp_ns: 0,
            thread_id: 0,
            address: 0,
            value: 0,
            kind: EbpfEventKind::default(),
            _pad: [0; 7],
            function_name: [0; MAX_FUNC_NAME_LEN],
        }
    }
}

/// Serde helper for fixed-size byte arrays.
mod serde_bytes_array {
    use serde::{Deserializer, Serializer};

    pub fn serialize<S, const N: usize>(arr: &[u8; N], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_bytes(arr)
    }

    pub fn deserialize<'de, D, const N: usize>(d: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let bytes: Vec<u8> = serde::Deserialize::deserialize(d)?;
        bytes
            .try_into()
            .map_err(|_| D::Error::custom(format!("expected {} bytes", N)))
    }
}

impl EbpfEvent {
    /// Create a `FunctionEntry` event (used in tests and userspace stubs).
    pub fn function_entry(timestamp_ns: u64, thread_id: u64, address: u64, name: &str) -> Self {
        let mut ev = Self {
            timestamp_ns,
            thread_id,
            address,
            value: 0,
            kind: EbpfEventKind::FunctionEntry,
            ..Default::default()
        };
        ev.set_function_name(name);
        ev
    }

    /// Create a `FunctionExit` event.
    pub fn function_exit(timestamp_ns: u64, thread_id: u64, address: u64) -> Self {
        Self {
            timestamp_ns,
            thread_id,
            address,
            value: 0,
            kind: EbpfEventKind::FunctionExit,
            ..Default::default()
        }
    }

    /// Create a `VariableWrite` event with a numeric value.
    pub fn variable_write(
        timestamp_ns: u64,
        thread_id: u64,
        address: u64,
        value: u64,
        name: &str,
    ) -> Self {
        let mut ev = Self {
            timestamp_ns,
            thread_id,
            address,
            value,
            kind: EbpfEventKind::VariableWrite,
            ..Default::default()
        };
        ev.set_function_name(name);
        ev
    }

    /// Write `name` into `function_name`, truncating if necessary.
    pub fn set_function_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(MAX_FUNC_NAME_LEN - 1);
        self.function_name[..len].copy_from_slice(&bytes[..len]);
        self.function_name[len] = 0; // null-terminate
    }

    /// Read `function_name` as a UTF-8 string (stops at first null byte).
    pub fn get_function_name(&self) -> &str {
        let end = self
            .function_name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(MAX_FUNC_NAME_LEN);
        std::str::from_utf8(&self.function_name[..end]).unwrap_or("<invalid utf8>")
    }

    /// Convert to a `chronos_domain::TraceEvent`.
    pub fn to_trace_event(&self, event_id: u64) -> chronos_domain::TraceEvent {
        use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};

        let (event_type, data) = match self.kind {
            EbpfEventKind::FunctionEntry => (
                EventType::FunctionEntry,
                EventData::Function {
                    name: self.get_function_name().to_string(),
                    signature: None,
                },
            ),
            EbpfEventKind::FunctionExit => (EventType::FunctionExit, EventData::Empty),
            EbpfEventKind::VariableWrite => (EventType::VariableWrite, EventData::Empty),
            EbpfEventKind::MemoryWrite => (EventType::MemoryWrite, EventData::Empty),
        };

        let location = SourceLocation::new(
            "",
            0,
            self.get_function_name(),
            self.address,
        );

        TraceEvent::new(event_id, self.timestamp_ns, self.thread_id, event_type, location, data)
    }
}

/// Descriptor for a BPF map (mirrors `struct bpf_map_def` from kernel headers).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BpfMapDef {
    /// Map type (e.g., `BPF_MAP_TYPE_RINGBUF = 27`).
    pub type_: u32,
    /// Key size in bytes.
    pub key_size: u32,
    /// Value size in bytes.
    pub value_size: u32,
    /// Maximum number of entries.
    pub max_entries: u32,
    /// Map flags.
    pub map_flags: u32,
}

/// BPF map type constants.
pub mod bpf_map_type {
    pub const BPF_MAP_TYPE_RINGBUF: u32 = 27;
    pub const BPF_MAP_TYPE_HASH: u32 = 1;
    pub const BPF_MAP_TYPE_PERF_EVENT_ARRAY: u32 = 4;
}

impl BpfMapDef {
    /// Create a ring buffer map definition.
    pub fn ring_buffer(max_entries: u32) -> Self {
        Self {
            type_: bpf_map_type::BPF_MAP_TYPE_RINGBUF,
            key_size: 0,
            value_size: 0,
            max_entries,
            map_flags: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebpf_event_function_entry() {
        let ev = EbpfEvent::function_entry(1000, 42, 0xDEAD, "my_function");
        assert_eq!(ev.timestamp_ns, 1000);
        assert_eq!(ev.thread_id, 42);
        assert_eq!(ev.address, 0xDEAD);
        assert_eq!(ev.kind, EbpfEventKind::FunctionEntry);
        assert_eq!(ev.get_function_name(), "my_function");
    }

    #[test]
    fn test_ebpf_event_function_name_truncation() {
        // Name longer than MAX_FUNC_NAME_LEN - 1 should be truncated.
        let long_name = "a".repeat(200);
        let mut ev = EbpfEvent::default();
        ev.set_function_name(&long_name);
        let result = ev.get_function_name();
        assert_eq!(result.len(), MAX_FUNC_NAME_LEN - 1);
        assert!(result.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_ebpf_event_serialize() {
        let ev = EbpfEvent::function_entry(999, 1, 0x1000, "foo");
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("timestamp_ns"));
        assert!(json.contains("999"));
    }

    #[test]
    fn test_ebpf_event_deserialize() {
        let ev = EbpfEvent::variable_write(500, 2, 0x2000, 42, "bar");
        let json = serde_json::to_string(&ev).unwrap();
        let ev2: EbpfEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev2.timestamp_ns, 500);
        assert_eq!(ev2.kind, EbpfEventKind::VariableWrite);
        assert_eq!(ev2.value, 42);
    }

    #[test]
    fn test_ebpf_event_to_trace_event() {
        let ev = EbpfEvent::function_entry(1234, 7, 0x5000, "compute");
        let te = ev.to_trace_event(10);

        use chronos_domain::EventType;
        assert_eq!(te.event_id, 10);
        assert_eq!(te.timestamp_ns, 1234);
        assert_eq!(te.thread_id, 7);
        assert_eq!(te.event_type, EventType::FunctionEntry);
        assert_eq!(te.location.address, 0x5000);
    }

    #[test]
    fn test_bpf_map_def_ring_buffer() {
        let def = BpfMapDef::ring_buffer(4096);
        assert_eq!(def.type_, bpf_map_type::BPF_MAP_TYPE_RINGBUF);
        assert_eq!(def.max_entries, 4096);
        assert_eq!(def.key_size, 0);
        assert_eq!(def.value_size, 0);
    }

    #[test]
    fn test_ebpf_event_size_is_stable() {
        // Size must be a multiple of 8 for BPF alignment.
        assert_eq!(std::mem::size_of::<EbpfEvent>() % 8, 0);
    }
}
