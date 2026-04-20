//! Trace event types.

use crate::trace::SourceLocation;
use crate::value::VariableInfo;
use serde::{Deserialize, Serialize};

/// Unique identifier for events within a session.
pub type EventId = u64;

/// Timestamp in nanoseconds since session start.
pub type TimestampNs = u64;

/// Thread identifier.
pub type ThreadId = u64;

/// The type of a trace event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventType {
    // Syscalls
    SyscallEnter = 0,
    SyscallExit = 1,

    // Functions
    FunctionEntry = 2,
    FunctionExit = 3,

    // Variables
    VariableWrite = 4,

    // Memory
    MemoryWrite = 5,

    // Signals
    SignalDelivered = 6,

    // Breakpoints
    BreakpointHit = 7,

    // Threads
    ThreadCreate = 8,
    ThreadExit = 9,

    // Exceptions
    ExceptionThrown = 10,

    // Custom / Unknown
    Custom = 254,
    Unknown = 255,
}

impl EventType {
    /// Returns true if this is a syscall event.
    pub fn is_syscall(&self) -> bool {
        matches!(self, EventType::SyscallEnter | EventType::SyscallExit)
    }

    /// Returns true if this is a function event.
    pub fn is_function(&self) -> bool {
        matches!(self, EventType::FunctionEntry | EventType::FunctionExit)
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::SyscallEnter => write!(f, "syscall_enter"),
            EventType::SyscallExit => write!(f, "syscall_exit"),
            EventType::FunctionEntry => write!(f, "function_entry"),
            EventType::FunctionExit => write!(f, "function_exit"),
            EventType::VariableWrite => write!(f, "variable_write"),
            EventType::MemoryWrite => write!(f, "memory_write"),
            EventType::SignalDelivered => write!(f, "signal_delivered"),
            EventType::BreakpointHit => write!(f, "breakpoint_hit"),
            EventType::ThreadCreate => write!(f, "thread_create"),
            EventType::ThreadExit => write!(f, "thread_exit"),
            EventType::ExceptionThrown => write!(f, "exception_thrown"),
            EventType::Custom => write!(f, "custom"),
            EventType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Event-specific data carried by a [`TraceEvent`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventData {
    Empty,

    /// System call data.
    Syscall {
        name: String,
        number: u64,
        args: Vec<u64>,
        return_value: i64,
    },

    /// Function entry/exit data.
    Function {
        name: String,
        signature: Option<String>,
    },

    /// Variable write data.
    Variable(VariableInfo),

    /// Memory write data.
    Memory {
        address: u64,
        size: usize,
        data: Option<Vec<u8>>,
    },

    /// Signal data.
    Signal {
        signal_number: i32,
        signal_name: String,
    },

    /// Breakpoint hit data.
    Breakpoint {
        breakpoint_id: u64,
        address: u64,
    },

    /// Thread data.
    Thread {
        name: Option<String>,
        tid: u64,
    },

    /// Exception data.
    Exception {
        type_name: String,
        message: String,
    },

    /// Register state snapshot (x86_64).
    Registers(RegisterState),

    /// Custom event data.
    Custom {
        name: String,
        data_json: String,
    },
}

/// x86_64 CPU register state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl Default for RegisterState {
    fn default() -> Self {
        Self {
            rax: 0, rbx: 0, rcx: 0, rdx: 0,
            rsi: 0, rdi: 0, rbp: 0, rsp: 0,
            r8: 0, r9: 0, r10: 0, r11: 0,
            r12: 0, r13: 0, r14: 0, r15: 0,
            rip: 0, rflags: 0,
        }
    }
}

/// A single trace event — the fundamental unit of recorded execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceEvent {
    /// Monotonically increasing event identifier within a session.
    pub event_id: EventId,
    /// Timestamp in nanoseconds since session start.
    pub timestamp_ns: TimestampNs,
    /// Thread that produced this event.
    pub thread_id: ThreadId,
    /// Category of event.
    pub event_type: EventType,
    /// Source location (file, line, function, address).
    pub location: SourceLocation,
    /// Event-specific payload.
    pub data: EventData,
}

impl TraceEvent {
    /// Create a new trace event.
    pub fn new(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        event_type: EventType,
        location: SourceLocation,
        data: EventData,
    ) -> Self {
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type,
            location,
            data,
        }
    }

    /// Create a function entry event.
    pub fn function_entry(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        name: impl Into<String>,
        address: u64,
    ) -> Self {
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(name.into()),
                address,
                ..Default::default()
            },
            data: EventData::Function {
                name: String::new(), // populated from location
                signature: None,
            },
        }
    }

    /// Create a function exit event.
    pub fn function_exit(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        name: impl Into<String>,
        address: u64,
    ) -> Self {
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionExit,
            location: SourceLocation {
                function: Some(name.into()),
                address,
                ..Default::default()
            },
            data: EventData::Empty,
        }
    }

    /// Create a syscall enter event.
    pub fn syscall_enter(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        name: impl Into<String>,
        number: u64,
        args: Vec<u64>,
        address: u64,
    ) -> Self {
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::SyscallEnter,
            location: SourceLocation::from_address(address),
            data: EventData::Syscall {
                name: name.into(),
                number,
                args,
                return_value: 0,
            },
        }
    }

    /// Create a signal event.
    pub fn signal(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        signal_number: i32,
        signal_name: impl Into<String>,
        address: u64,
    ) -> Self {
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::SignalDelivered,
            location: SourceLocation::from_address(address),
            data: EventData::Signal {
                signal_number,
                signal_name: signal_name.into(),
            },
        }
    }

    /// Get the function name from this event, if applicable.
    pub fn function_name(&self) -> Option<&str> {
        self.location.function.as_deref()
    }
}

// SourceLocation needs Default for the ..Default::default() pattern
impl Default for SourceLocation {
    fn default() -> Self {
        Self {
            file: None,
            line: None,
            column: None,
            function: None,
            address: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::FunctionEntry.to_string(), "function_entry");
        assert_eq!(EventType::SyscallEnter.to_string(), "syscall_enter");
        assert_eq!(EventType::SignalDelivered.to_string(), "signal_delivered");
    }

    #[test]
    fn test_event_type_classifiers() {
        assert!(EventType::SyscallEnter.is_syscall());
        assert!(EventType::SyscallExit.is_syscall());
        assert!(!EventType::FunctionEntry.is_syscall());

        assert!(EventType::FunctionEntry.is_function());
        assert!(EventType::FunctionExit.is_function());
        assert!(!EventType::SignalDelivered.is_function());
    }

    #[test]
    fn test_trace_event_new() {
        let event = TraceEvent::new(
            1,
            1000,
            42,
            EventType::FunctionEntry,
            SourceLocation::new("main.rs", 10, "main", 0x401000),
            EventData::Empty,
        );
        assert_eq!(event.event_id, 1);
        assert_eq!(event.timestamp_ns, 1000);
        assert_eq!(event.thread_id, 42);
        assert_eq!(event.event_type, EventType::FunctionEntry);
        assert_eq!(event.function_name(), Some("main"));
    }

    #[test]
    fn test_trace_event_function_entry() {
        let event = TraceEvent::function_entry(1, 500, 1, "add", 0x402000);
        assert_eq!(event.event_type, EventType::FunctionEntry);
        assert_eq!(event.function_name(), Some("add"));
        assert_eq!(event.location.address, 0x402000);
    }

    #[test]
    fn test_trace_event_signal() {
        let event = TraceEvent::signal(5, 9999, 1, 11, "SIGSEGV", 0xDEAD);
        assert_eq!(event.event_type, EventType::SignalDelivered);
        match &event.data {
            EventData::Signal { signal_number, signal_name } => {
                assert_eq!(*signal_number, 11);
                assert_eq!(signal_name, "SIGSEGV");
            }
            _ => panic!("Expected Signal data"),
        }
    }

    #[test]
    fn test_register_state_default() {
        let regs = RegisterState::default();
        assert_eq!(regs.rax, 0);
        assert_eq!(regs.rip, 0);
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = TraceEvent::function_entry(42, 12345, 1, "process_data", 0x5000);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TraceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }
}
