//! Trace event types.

use crate::trace::SourceLocation;
use crate::value::VariableInfo;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

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

/// Kind of Python trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum PythonEventKind {
    Call,
    Return,
    Exception,
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

    /// Python function call/return/exception data.
    PythonFrame {
        /// "module.Class.method" or just "func_name"
        qualified_name: String,
        /// Absolute path to .py file
        file: String,
        /// Line number
        line: u32,
        /// Whether the frame is a generator (always false in MVP)
        is_generator: bool,
        /// Captured locals at call site
        locals: Option<Vec<VariableInfo>>,
        /// Kind of Python event
        event_kind: PythonEventKind,
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

    /// Create a Python call event.
    pub fn python_call(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        qualified_name: impl Into<String>,
        file: impl Into<String>,
        line: u32,
    ) -> Self {
        let qualified_name = qualified_name.into();
        let file = file.into();
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                file: Some(file.clone()),
                line: Some(line),
                function: Some(qualified_name.clone()),
                ..Default::default()
            },
            data: EventData::PythonFrame {
                qualified_name,
                file,
                line,
                is_generator: false,
                locals: None,
                event_kind: PythonEventKind::Call,
            },
        }
    }

    /// Create a Python call event with local variables.
    pub fn python_call_with_locals(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        qualified_name: impl Into<String>,
        file: impl Into<String>,
        line: u32,
        locals: Vec<VariableInfo>,
    ) -> Self {
        let qualified_name = qualified_name.into();
        let file = file.into();
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                file: Some(file.clone()),
                line: Some(line),
                function: Some(qualified_name.clone()),
                ..Default::default()
            },
            data: EventData::PythonFrame {
                qualified_name,
                file,
                line,
                is_generator: false,
                locals: Some(locals),
                event_kind: PythonEventKind::Call,
            },
        }
    }

    /// Create a Python return event.
    pub fn python_return(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        qualified_name: impl Into<String>,
        file: impl Into<String>,
        line: u32,
    ) -> Self {
        let qualified_name = qualified_name.into();
        let file = file.into();
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionExit,
            location: SourceLocation {
                file: Some(file.clone()),
                line: Some(line),
                function: Some(qualified_name.clone()),
                ..Default::default()
            },
            data: EventData::PythonFrame {
                qualified_name,
                file,
                line,
                is_generator: false,
                locals: None,
                event_kind: PythonEventKind::Return,
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
    use crate::VariableScope;

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

    #[test]
    fn test_python_frame_serialization_roundtrip() {
        // Test PythonEventKind variants
        assert_eq!(
            serde_json::to_string(&PythonEventKind::Call).unwrap(),
            "\"Call\""
        );
        assert_eq!(
            serde_json::to_string(&PythonEventKind::Return).unwrap(),
            "\"Return\""
        );
        assert_eq!(
            serde_json::to_string(&PythonEventKind::Exception).unwrap(),
            "\"Exception\""
        );

        // Test PythonFrame roundtrip through EventData
        let locals = vec![
            VariableInfo::new("x", "42", "int", 0x1000, VariableScope::Local),
            VariableInfo::new("name", "'hello'", "str", 0x2000, VariableScope::Local),
        ];
        let python_data = EventData::PythonFrame {
            qualified_name: "my_module.MyClass.my_method".to_string(),
            file: "/path/to/script.py".to_string(),
            line: 42,
            is_generator: false,
            locals: Some(locals),
            event_kind: PythonEventKind::Call,
        };
        let json = serde_json::to_string(&python_data).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(python_data, deserialized);

        // Test without locals
        let python_data_no_locals = EventData::PythonFrame {
            qualified_name: "simple_func".to_string(),
            file: "/path/to/script.py".to_string(),
            line: 10,
            is_generator: false,
            locals: None,
            event_kind: PythonEventKind::Return,
        };
        let json = serde_json::to_string(&python_data_no_locals).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(python_data_no_locals, deserialized);
    }

    #[test]
    fn test_python_frame_entry_exit() {
        // Test python_call constructor
        let call_event = TraceEvent::python_call(
            1,
            1000,
            42,
            "my_module.MyClass.my_method",
            "/path/to/script.py",
            42,
        );
        assert_eq!(call_event.event_id, 1);
        assert_eq!(call_event.timestamp_ns, 1000);
        assert_eq!(call_event.thread_id, 42);
        assert_eq!(call_event.location.file.as_deref(), Some("/path/to/script.py"));
        assert_eq!(call_event.location.line, Some(42));
        match &call_event.data {
            EventData::PythonFrame { qualified_name, file, line, is_generator, locals, event_kind } => {
                assert_eq!(qualified_name, "my_module.MyClass.my_method");
                assert_eq!(file, "/path/to/script.py");
                assert_eq!(*line, 42);
                assert!(!*is_generator);
                assert!(locals.is_none());
                assert_eq!(event_kind, &PythonEventKind::Call);
            }
            _ => panic!("Expected PythonFrame data"),
        }

        // Test python_return constructor
        let return_event = TraceEvent::python_return(
            2,
            2000,
            42,
            "my_module.MyClass.my_method",
            "/path/to/script.py",
            50,
        );
        assert_eq!(return_event.event_id, 2);
        assert_eq!(return_event.timestamp_ns, 2000);
        match &return_event.data {
            EventData::PythonFrame { event_kind, .. } => {
                assert_eq!(event_kind, &PythonEventKind::Return);
            }
            _ => panic!("Expected PythonFrame data"),
        }

        // Test python_call_with_locals
        let locals = vec![
            VariableInfo::new("x", "42", "int", 0x1000, VariableScope::Local),
        ];
        let call_with_locals = TraceEvent::python_call_with_locals(
            3,
            1500,
            42,
            "my_func",
            "/path/to/script.py",
            10,
            locals.clone(),
        );
        match &call_with_locals.data {
            EventData::PythonFrame { locals: event_locals, event_kind, .. } => {
                assert!(event_locals.is_some());
                assert_eq!(event_locals.as_ref().unwrap().len(), 1);
                assert_eq!(event_kind, &PythonEventKind::Call);
            }
            _ => panic!("Expected PythonFrame data"),
        }
    }
}
