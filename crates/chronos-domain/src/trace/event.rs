//! Trace event types.

use crate::trace::SourceLocation;
use crate::value::VariableInfo;
use schemars::JsonSchema;
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

/// Kind of Python trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum PythonEventKind {
    Call,
    Return,
    Exception,
}

/// Kind of Java trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum JavaEventKind {
    MethodEntry,
    MethodExit,
    Exception,
}

/// Kind of Go trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum GoEventKind {
    Breakpoint,
    Step,
    GoroutineStop,
    Exception,
}

/// Kind of JavaScript trace event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum JsEventKind {
    Breakpoint,
    Step,
    Exception,
    Other(String),
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

    /// Java method call/return/exception data.
    JavaFrame {
        /// Fully qualified class name, e.g. "com.example.Foo"
        class_name: String,
        /// Method name, e.g. "bar"
        method_name: String,
        /// JVM descriptor, e.g. "(I)V"
        signature: Option<String>,
        /// Source file name
        file: Option<String>,
        /// Source line number
        line: Option<u32>,
        /// Captured local variables
        locals: Option<Vec<VariableInfo>>,
        /// Kind of Java event
        event_kind: JavaEventKind,
    },

    /// Go breakpoint/step/goroutine stop data.
    GoFrame {
        /// Goroutine ID
        goroutine_id: u64,
        /// Function name, e.g. "main.foo"
        function_name: String,
        /// Source file name
        file: Option<String>,
        /// Source line number
        line: Option<u32>,
        /// Captured local variables
        locals: Option<Vec<VariableInfo>>,
        /// Kind of Go event
        event_kind: GoEventKind,
    },

    /// JavaScript frame data from CDP.
    JsFrame {
        /// Function name
        function_name: String,
        /// Absolute URL/script path
        script_url: String,
        /// Source line number
        line_number: u32,
        /// Source column number
        column_number: u32,
        /// Captured local variables
        locals: Option<Vec<VariableInfo>>,
        /// Scope chain as list of scope type names
        scope_chain: Vec<String>,
        /// Kind of JavaScript event
        event_kind: JsEventKind,
    },

    /// Python stdout/stderr console output.
    PythonConsoleOutput {
        /// Output text
        text: String,
        /// Output stream: "stdout", "stderr", or "console"
        category: String,
    },

    /// JavaScript console API output.
    JsConsoleOutput {
        /// Output text
        text: String,
        /// Console level: "log", "warn", "error", "info"
        level: String,
        /// Serialized arguments
        args: Vec<String>,
    },

    /// eBPF uprobe hit data from the ring buffer.
    EbpfUprobeHit {
        /// Symbol name that was hit
        symbol_name: String,
        /// Process ID
        pid: u32,
        /// Timestamp in nanoseconds
        timestamp_ns: u64,
        /// Whether this is a return probe (exit)
        is_return: bool,
    },
}

/// x86_64 CPU register state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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

    /// Create a Java method entry event.
    pub fn java_call(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        class_name: impl Into<String>,
        method_name: impl Into<String>,
        file: Option<String>,
        line: Option<u32>,
    ) -> Self {
        let class_name = class_name.into();
        let method_name = method_name.into();
        let qualified_name = format!("{}.{}", class_name, method_name);
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                file: file.clone(),
                line,
                function: Some(qualified_name.clone()),
                ..Default::default()
            },
            data: EventData::JavaFrame {
                class_name,
                method_name,
                signature: None,
                file,
                line,
                locals: None,
                event_kind: JavaEventKind::MethodEntry,
            },
        }
    }

    /// Create a Java method exit event.
    pub fn java_return(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        class_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> Self {
        let class_name = class_name.into();
        let method_name = method_name.into();
        let qualified_name = format!("{}.{}", class_name, method_name);
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::FunctionExit,
            location: SourceLocation {
                function: Some(qualified_name),
                ..Default::default()
            },
            data: EventData::JavaFrame {
                class_name,
                method_name,
                signature: None,
                file: None,
                line: None,
                locals: None,
                event_kind: JavaEventKind::MethodExit,
            },
        }
    }

    /// Create a Go frame event.
    pub fn go_frame(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        goroutine_id: u64,
        function_name: impl Into<String>,
        file: Option<String>,
        line: Option<u32>,
        kind: GoEventKind,
    ) -> Self {
        let function_name = function_name.into();
        Self {
            event_id,
            timestamp_ns,
            thread_id: goroutine_id,
            event_type: EventType::BreakpointHit,
            location: SourceLocation {
                file: file.clone(),
                line,
                function: Some(function_name.clone()),
                ..Default::default()
            },
            data: EventData::GoFrame {
                goroutine_id,
                function_name,
                file,
                line,
                locals: None,
                event_kind: kind,
            },
        }
    }

    /// Create a JavaScript frame event.
    #[allow(clippy::too_many_arguments)]
    pub fn js_frame(
        event_id: EventId,
        timestamp_ns: TimestampNs,
        thread_id: ThreadId,
        function_name: impl Into<String>,
        script_url: String,
        line: u32,
        column: u32,
        kind: JsEventKind,
    ) -> Self {
        let function_name = function_name.into();
        Self {
            event_id,
            timestamp_ns,
            thread_id,
            event_type: EventType::BreakpointHit,
            location: SourceLocation {
                file: Some(script_url.clone()),
                line: Some(line),
                column: Some(column),
                function: Some(function_name.clone()),
                ..Default::default()
            },
            data: EventData::JsFrame {
                function_name,
                script_url,
                line_number: line,
                column_number: column,
                locals: None,
                scope_chain: Vec::new(),
                event_kind: kind,
            },
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
            EventData::Signal {
                signal_number,
                signal_name,
            } => {
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
        assert_eq!(
            call_event.location.file.as_deref(),
            Some("/path/to/script.py")
        );
        assert_eq!(call_event.location.line, Some(42));
        match &call_event.data {
            EventData::PythonFrame {
                qualified_name,
                file,
                line,
                is_generator,
                locals,
                event_kind,
            } => {
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
        let locals = vec![VariableInfo::new(
            "x",
            "42",
            "int",
            0x1000,
            VariableScope::Local,
        )];
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
            EventData::PythonFrame {
                locals: event_locals,
                event_kind,
                ..
            } => {
                assert!(event_locals.is_some());
                assert_eq!(event_locals.as_ref().unwrap().len(), 1);
                assert_eq!(event_kind, &PythonEventKind::Call);
            }
            _ => panic!("Expected PythonFrame data"),
        }
    }

    #[test]
    fn test_java_event_kind_serialization() {
        // Test JavaEventKind variants
        assert_eq!(
            serde_json::to_string(&super::JavaEventKind::MethodEntry).unwrap(),
            "\"MethodEntry\""
        );
        assert_eq!(
            serde_json::to_string(&super::JavaEventKind::MethodExit).unwrap(),
            "\"MethodExit\""
        );
        assert_eq!(
            serde_json::to_string(&super::JavaEventKind::Exception).unwrap(),
            "\"Exception\""
        );

        // Test deserialization
        let parsed: super::JavaEventKind = serde_json::from_str("\"MethodEntry\"").unwrap();
        assert_eq!(parsed, super::JavaEventKind::MethodEntry);
        let parsed: super::JavaEventKind = serde_json::from_str("\"MethodExit\"").unwrap();
        assert_eq!(parsed, super::JavaEventKind::MethodExit);
        let parsed: super::JavaEventKind = serde_json::from_str("\"Exception\"").unwrap();
        assert_eq!(parsed, super::JavaEventKind::Exception);
    }

    #[test]
    fn test_go_event_kind_serialization() {
        // Test GoEventKind variants
        assert_eq!(
            serde_json::to_string(&super::GoEventKind::Breakpoint).unwrap(),
            "\"Breakpoint\""
        );
        assert_eq!(
            serde_json::to_string(&super::GoEventKind::Step).unwrap(),
            "\"Step\""
        );
        assert_eq!(
            serde_json::to_string(&super::GoEventKind::GoroutineStop).unwrap(),
            "\"GoroutineStop\""
        );
        assert_eq!(
            serde_json::to_string(&super::GoEventKind::Exception).unwrap(),
            "\"Exception\""
        );

        // Test deserialization
        let parsed: super::GoEventKind = serde_json::from_str("\"Breakpoint\"").unwrap();
        assert_eq!(parsed, super::GoEventKind::Breakpoint);
        let parsed: super::GoEventKind = serde_json::from_str("\"GoroutineStop\"").unwrap();
        assert_eq!(parsed, super::GoEventKind::GoroutineStop);
    }

    #[test]
    fn test_java_frame_serialization_roundtrip() {
        use super::{EventData, JavaEventKind};
        use crate::VariableScope;

        let java_frame = EventData::JavaFrame {
            class_name: "com.example.Foo".to_string(),
            method_name: "bar".to_string(),
            signature: Some("(I)V".to_string()),
            file: Some("Foo.java".to_string()),
            line: Some(42),
            locals: Some(vec![crate::VariableInfo::new(
                "x",
                "10",
                "int",
                0x1000,
                VariableScope::Local,
            )]),
            event_kind: JavaEventKind::MethodEntry,
        };
        let json = serde_json::to_string(&java_frame).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(java_frame, deserialized);

        // Test without optional fields
        let java_frame_no_opts = EventData::JavaFrame {
            class_name: "com.example.Bar".to_string(),
            method_name: "baz".to_string(),
            signature: None,
            file: None,
            line: None,
            locals: None,
            event_kind: JavaEventKind::MethodExit,
        };
        let json = serde_json::to_string(&java_frame_no_opts).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(java_frame_no_opts, deserialized);
    }

    #[test]
    fn test_go_frame_serialization_roundtrip() {
        use super::{EventData, GoEventKind};
        use crate::VariableScope;

        let go_frame = EventData::GoFrame {
            goroutine_id: 12345,
            function_name: "main.foo".to_string(),
            file: Some("foo.go".to_string()),
            line: Some(100),
            locals: Some(vec![crate::VariableInfo::new(
                "count",
                "42",
                "int",
                0x2000,
                VariableScope::Local,
            )]),
            event_kind: GoEventKind::Breakpoint,
        };
        let json = serde_json::to_string(&go_frame).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(go_frame, deserialized);

        // Test without optional fields
        let go_frame_no_opts = EventData::GoFrame {
            goroutine_id: 99,
            function_name: "runtime.main".to_string(),
            file: None,
            line: None,
            locals: None,
            event_kind: GoEventKind::GoroutineStop,
        };
        let json = serde_json::to_string(&go_frame_no_opts).unwrap();
        let deserialized: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(go_frame_no_opts, deserialized);
    }

    #[test]
    fn test_java_call_constructor() {
        let event = super::TraceEvent::java_call(
            1,
            1000,
            42,
            "com.example.Foo",
            "bar",
            Some("Foo.java".to_string()),
            Some(10),
        );
        assert_eq!(event.event_id, 1);
        assert_eq!(event.timestamp_ns, 1000);
        assert_eq!(event.thread_id, 42);
        assert_eq!(event.event_type, super::EventType::FunctionEntry);
        match &event.data {
            super::EventData::JavaFrame {
                class_name,
                method_name,
                event_kind,
                ..
            } => {
                assert_eq!(class_name, "com.example.Foo");
                assert_eq!(method_name, "bar");
                assert_eq!(*event_kind, super::JavaEventKind::MethodEntry);
            }
            _ => panic!("Expected JavaFrame data"),
        }
    }

    #[test]
    fn test_java_return_constructor() {
        let event = super::TraceEvent::java_return(2, 2000, 42, "com.example.Foo", "bar");
        assert_eq!(event.event_id, 2);
        assert_eq!(event.timestamp_ns, 2000);
        match &event.data {
            super::EventData::JavaFrame {
                class_name,
                method_name,
                event_kind,
                ..
            } => {
                assert_eq!(class_name, "com.example.Foo");
                assert_eq!(method_name, "bar");
                assert_eq!(*event_kind, super::JavaEventKind::MethodExit);
            }
            _ => panic!("Expected JavaFrame data"),
        }
    }

    #[test]
    fn test_go_frame_constructor() {
        let event = super::TraceEvent::go_frame(
            1,
            1000,
            12345,
            "main.foo",
            Some("foo.go".to_string()),
            Some(42),
            super::GoEventKind::Breakpoint,
        );
        assert_eq!(event.event_id, 1);
        assert_eq!(event.timestamp_ns, 1000);
        match &event.data {
            super::EventData::GoFrame {
                goroutine_id,
                function_name,
                event_kind,
                ..
            } => {
                assert_eq!(*goroutine_id, 12345);
                assert_eq!(function_name, "main.foo");
                assert_eq!(*event_kind, super::GoEventKind::Breakpoint);
            }
            _ => panic!("Expected GoFrame data"),
        }
    }

    #[test]
    fn test_js_event_kind_serialization() {
        // Test JsEventKind variants
        assert_eq!(
            serde_json::to_string(&super::JsEventKind::Breakpoint).unwrap(),
            "\"Breakpoint\""
        );
        assert_eq!(
            serde_json::to_string(&super::JsEventKind::Step).unwrap(),
            "\"Step\""
        );
        assert_eq!(
            serde_json::to_string(&super::JsEventKind::Exception).unwrap(),
            "\"Exception\""
        );

        // Other variant serializes as JSON object
        let other_json = serde_json::to_string(&super::JsEventKind::Other("Pause".to_string())).unwrap();
        assert_eq!(other_json, "{\"Other\":\"Pause\"}");

        // Test deserialization
        let parsed: super::JsEventKind = serde_json::from_str("\"Breakpoint\"").unwrap();
        assert_eq!(parsed, super::JsEventKind::Breakpoint);
        let parsed: super::JsEventKind = serde_json::from_str("\"Step\"").unwrap();
        assert_eq!(parsed, super::JsEventKind::Step);
        let parsed: super::JsEventKind = serde_json::from_str("\"Exception\"").unwrap();
        assert_eq!(parsed, super::JsEventKind::Exception);
    }

    #[test]
    fn test_js_frame_serialization_roundtrip() {
        use crate::VariableScope;

        let js_frame = super::EventData::JsFrame {
            function_name: "myFunction".to_string(),
            script_url: "http://localhost:3000/app.js".to_string(),
            line_number: 42,
            column_number: 10,
            locals: Some(vec![crate::VariableInfo::new(
                "x",
                "10",
                "number",
                0x1000,
                VariableScope::Local,
            )]),
            scope_chain: vec!["Local".to_string(), "Closure".to_string()],
            event_kind: super::JsEventKind::Breakpoint,
        };
        let json = serde_json::to_string(&js_frame).unwrap();
        let deserialized: super::EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(js_frame, deserialized);

        // Test without optional fields
        let js_frame_no_opts = super::EventData::JsFrame {
            function_name: "anonymous".to_string(),
            script_url: "eval".to_string(),
            line_number: 1,
            column_number: 0,
            locals: None,
            scope_chain: Vec::new(),
            event_kind: super::JsEventKind::Step,
        };
        let json = serde_json::to_string(&js_frame_no_opts).unwrap();
        let deserialized: super::EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(js_frame_no_opts, deserialized);
    }

    #[test]
    fn test_js_frame_constructor() {
        let event = super::TraceEvent::js_frame(
            1,
            1000,
            42,
            "myFunction",
            "http://localhost:3000/app.js".to_string(),
            42,
            10,
            super::JsEventKind::Breakpoint,
        );
        assert_eq!(event.event_id, 1);
        assert_eq!(event.timestamp_ns, 1000);
        assert_eq!(event.thread_id, 42);
        assert_eq!(event.event_type, super::EventType::BreakpointHit);
        assert_eq!(event.location.line, Some(42));
        assert_eq!(event.location.column, Some(10));
        match &event.data {
            super::EventData::JsFrame {
                function_name,
                script_url,
                line_number,
                column_number,
                event_kind,
                ..
            } => {
                assert_eq!(function_name, "myFunction");
                assert_eq!(script_url, "http://localhost:3000/app.js");
                assert_eq!(*line_number, 42);
                assert_eq!(*column_number, 10);
                assert_eq!(*event_kind, super::JsEventKind::Breakpoint);
            }
            _ => panic!("Expected JsFrame data"),
        }
    }
}
