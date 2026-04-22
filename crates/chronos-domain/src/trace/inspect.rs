//! Debug inspection types for runtime introspection.
//!
//! These types provide access to threads, stack frames, variables,
//! and runtime metadata during a capture session.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::value::VariableInfo;

/// Thread state for debugging purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum ThreadState {
    /// Thread is actively executing.
    Running,
    /// Thread is blocked (e.g., waiting on a lock).
    Blocked,
    /// Thread is waiting (e.g., condvar, I/O).
    Waiting,
    /// Thread is sleeping (e.g., timed wait).
    Sleeping,
}

/// Information about a thread in the target process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadInfo {
    /// Unique thread identifier.
    pub thread_id: u64,
    /// Human-readable thread name, if available.
    pub name: String,
    /// Current state of the thread.
    pub state: ThreadState,
}

/// A single stack frame in a call stack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StackFrame {
    /// Unique frame identifier (depth index).
    pub frame_id: u64,
    /// Name of the function at this frame.
    pub function_name: String,
    /// Source file path, if known.
    pub source_file: Option<String>,
    /// Source line number, if known.
    pub line: Option<u32>,
    /// Local and captured variables at this frame.
    pub variables: Vec<VariableInfo>,
}

/// Runtime metadata for a capture session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeInfo {
    /// Language being traced.
    pub language: String,
    /// Runtime/engine version string.
    pub runtime_version: String,
    /// Process ID of the target.
    pub pid: u32,
    /// Uptime of the target process in milliseconds at session start.
    pub uptime_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_info_clone() {
        let info = ThreadInfo {
            thread_id: 42,
            name: "worker-1".to_string(),
            state: ThreadState::Running,
        };
        let cloned = info.clone();
        assert_eq!(cloned.thread_id, 42);
        assert_eq!(cloned.name, "worker-1");
        assert_eq!(cloned.state, ThreadState::Running);
    }

    #[test]
    fn test_stack_frame_clone() {
        let frame = StackFrame {
            frame_id: 0,
            function_name: "main".to_string(),
            source_file: Some("main.rs".to_string()),
            line: Some(10),
            variables: Vec::new(),
        };
        let cloned = frame.clone();
        assert_eq!(cloned.frame_id, 0);
        assert_eq!(cloned.function_name, "main");
        assert_eq!(cloned.source_file.as_deref(), Some("main.rs"));
        assert_eq!(cloned.line, Some(10));
    }

    #[test]
    fn test_runtime_info_clone() {
        let info = RuntimeInfo {
            language: "Rust".to_string(),
            runtime_version: "1.75.0".to_string(),
            pid: 1234,
            uptime_ms: 60000,
        };
        let cloned = info.clone();
        assert_eq!(cloned.language, "Rust");
        assert_eq!(cloned.runtime_version, "1.75.0");
        assert_eq!(cloned.pid, 1234);
        assert_eq!(cloned.uptime_ms, 60000);
    }

    #[test]
    fn test_thread_state_serialization() {
        assert_eq!(serde_json::to_string(&ThreadState::Running).unwrap(), "\"Running\"");
        assert_eq!(serde_json::to_string(&ThreadState::Blocked).unwrap(), "\"Blocked\"");
        assert_eq!(serde_json::to_string(&ThreadState::Waiting).unwrap(), "\"Waiting\"");
        assert_eq!(serde_json::to_string(&ThreadState::Sleeping).unwrap(), "\"Sleeping\"");

        let parsed: ThreadState = serde_json::from_str("\"Running\"").unwrap();
        assert_eq!(parsed, ThreadState::Running);
    }

    #[test]
    fn test_thread_info_serialization() {
        let info = ThreadInfo {
            thread_id: 1,
            name: "main".to_string(),
            state: ThreadState::Running,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ThreadInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.thread_id, 1);
        assert_eq!(parsed.name, "main");
    }

    #[test]
    fn test_stack_frame_serialization() {
        let frame = StackFrame {
            frame_id: 5,
            function_name: "process".to_string(),
            source_file: Some("app.rs".to_string()),
            line: Some(42),
            variables: vec![crate::VariableInfo::new(
                "x", "10", "i32", 0x1000, crate::VariableScope::Local,
            )],
        };
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: StackFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.frame_id, 5);
        assert_eq!(parsed.function_name, "process");
        assert_eq!(parsed.line, Some(42));
        assert_eq!(parsed.variables.len(), 1);
    }

    #[test]
    fn test_runtime_info_serialization() {
        let info = RuntimeInfo {
            language: "Python".to_string(),
            runtime_version: "3.12.0".to_string(),
            pid: 9999,
            uptime_ms: 3600000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: RuntimeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.language, "Python");
        assert_eq!(parsed.pid, 9999);
    }
}
