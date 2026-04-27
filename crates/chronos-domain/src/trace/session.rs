//! Capture session management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// Programming language of the target program.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Language {
    C,
    Cpp,
    Rust,
    Java,
    Kotlin,
    Scala,
    Python,
    JavaScript,
    Go,
    CSharp,
    Ebpf,
    WebAssembly,
    Unknown,
}

impl Language {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "c" => Language::C,
            "cpp" | "cc" | "cxx" | "c++" => Language::Cpp,
            "rs" => Language::Rust,
            "java" => Language::Java,
            "kt" | "kts" => Language::Kotlin,
            "scala" | "sc" => Language::Scala,
            "py" | "pyw" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "go" => Language::Go,
            "cs" => Language::CSharp,
            "wasm" => Language::WebAssembly,
            _ => Language::Unknown,
        }
    }

    /// Detect language from a file path.
    pub fn from_path(path: &str) -> Self {
        std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(Language::from_extension)
            .unwrap_or(Language::Unknown)
    }

    /// Detect language from a string name.
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "c" => Language::C,
            "cpp" | "c++" | "cc" | "cxx" => Language::Cpp,
            "rust" | "rs" => Language::Rust,
            "java" => Language::Java,
            "kotlin" | "kt" | "kts" => Language::Kotlin,
            "scala" | "sc" => Language::Scala,
            "python" | "py" | "pyw" => Language::Python,
            "javascript" | "js" | "mjs" | "cjs" => Language::JavaScript,
            "go" => Language::Go,
            "csharp" | "cs" | "c#" => Language::CSharp,
            "ebpf" | "bpf" => Language::Ebpf,
            "wasm" | "webassembly" | "wasm-bytecode" => Language::WebAssembly,
            _ => Language::Unknown,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::C => write!(f, "c"),
            Language::Cpp => write!(f, "cpp"),
            Language::Rust => write!(f, "rust"),
            Language::Java => write!(f, "java"),
            Language::Kotlin => write!(f, "kotlin"),
            Language::Scala => write!(f, "scala"),
            Language::Python => write!(f, "python"),
            Language::JavaScript => write!(f, "javascript"),
            Language::Go => write!(f, "go"),
            Language::CSharp => write!(f, "csharp"),
            Language::Ebpf => write!(f, "ebpf"),
            Language::WebAssembly => write!(f, "WebAssembly"),
            Language::Unknown => write!(f, "unknown"),
        }
    }
}

/// State of a capture session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    /// Session created, capture not yet started.
    Created,
    /// Capture is actively recording.
    Active,
    /// Capture finished, trace being finalized (indices being built).
    Finalizing,
    /// Trace finalized and ready for queries.
    Finalized,
    /// An error occurred during capture.
    Error,
}

/// Configuration for a capture session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Path to the target binary or script.
    pub target: String,
    /// Command-line arguments for the target.
    pub args: Vec<String>,
    /// Environment variables (None = inherit from current process).
    pub env: Option<HashMap<String, String>>,
    /// Working directory for the target.
    pub cwd: Option<PathBuf>,
    /// Language of the target (None = auto-detect from file extension).
    pub language: Option<Language>,
    /// Whether to capture syscall enter/exit events.
    pub capture_syscalls: bool,
    /// Whether to capture variable values.
    pub capture_variables: bool,
    /// Whether to capture call stack information.
    pub capture_stack: bool,
    /// Whether to capture memory writes (expensive).
    pub capture_memory: bool,
    /// Whether to capture function exit events (returns).
    pub capture_function_exit: bool,
    /// Only trace functions matching these patterns (None = all).
    pub function_filter: Option<Vec<String>>,
    /// Maximum capture duration in milliseconds (None = unlimited).
    pub max_duration_ms: Option<u64>,
}

impl CaptureConfig {
    /// Create a config with sensible defaults.
    pub fn new(target: impl Into<String>) -> Self {
        let target_str = target.into();
        let language = Language::from_path(&target_str);
        Self {
            target: target_str,
            args: Vec::new(),
            env: None,
            cwd: None,
            language: if language == Language::Unknown {
                None
            } else {
                Some(language)
            },
            capture_syscalls: true,
            capture_variables: true,
            capture_stack: true,
            capture_memory: false,
            capture_function_exit: false,
            function_filter: None,
            max_duration_ms: None,
        }
    }
}

/// An active or finalized capture session.
#[derive(Debug, Clone)]
pub struct CaptureSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Process ID of the target.
    pub pid: u32,
    /// Detected or configured language.
    pub language: Language,
    /// When the session was created (not serialized — for runtime use only).
    pub started_at: Instant,
    /// Capture configuration.
    pub config: CaptureConfig,
    /// Current state of the session.
    pub state: SessionState,
}

impl CaptureSession {
    /// Create a new capture session.
    pub fn new(pid: u32, language: Language, config: CaptureConfig) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            pid,
            language,
            started_at: Instant::now(),
            config,
            state: SessionState::Created,
        }
    }

    /// Create a minimal capture session with only session_id and language.
    ///
    /// This is useful for evaluation purposes where only the language is needed.
    /// Other fields are set to dummy values that are sufficient for routing.
    pub fn minimal(session_id: String, language: Language) -> Self {
        Self {
            session_id,
            pid: 0,
            language,
            started_at: Instant::now(),
            config: CaptureConfig::new(""),
            state: SessionState::Finalized,
        }
    }

    /// Transition the session to active state.
    pub fn activate(&mut self) {
        self.state = SessionState::Active;
    }

    /// Transition the session to finalizing state.
    pub fn begin_finalize(&mut self) {
        self.state = SessionState::Finalizing;
    }

    /// Transition the session to finalized state.
    pub fn finalize(&mut self) {
        self.state = SessionState::Finalized;
    }

    /// Mark the session as errored.
    pub fn error(&mut self) {
        self.state = SessionState::Error;
    }

    /// Returns the elapsed time since session creation.
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("c"), Language::C);
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("go"), Language::Go);
        assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    }

    #[test]
    fn test_language_from_path() {
        assert_eq!(Language::from_path("/usr/bin/test"), Language::Unknown);
        assert_eq!(Language::from_path("src/main.rs"), Language::Rust);
        assert_eq!(Language::from_path("app.cpp"), Language::Cpp);
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::C.to_string(), "c");
    }

    #[test]
    fn test_capture_config_new() {
        let config = CaptureConfig::new("target_program");
        assert_eq!(config.target, "target_program");
        assert!(config.args.is_empty());
        assert!(config.capture_syscalls);
        assert!(!config.capture_memory);
        assert!(!config.capture_function_exit);
        // Auto-detect should return Unknown for no extension
        assert!(config.language.is_none());
    }

    #[test]
    fn test_capture_config_new_with_extension() {
        let config = CaptureConfig::new("main.rs");
        assert_eq!(config.language, Some(Language::Rust));
    }

    #[test]
    fn test_session_lifecycle() {
        let config = CaptureConfig::new("test.rs");
        let mut session = CaptureSession::new(12345, Language::Rust, config);

        assert_eq!(session.state, SessionState::Created);
        session.activate();
        assert_eq!(session.state, SessionState::Active);
        session.begin_finalize();
        assert_eq!(session.state, SessionState::Finalizing);
        session.finalize();
        assert_eq!(session.state, SessionState::Finalized);
    }

    #[test]
    fn test_session_error() {
        let config = CaptureConfig::new("test.c");
        let mut session = CaptureSession::new(9999, Language::C, config);
        session.activate();
        session.error();
        assert_eq!(session.state, SessionState::Error);
    }

    #[test]
    fn test_session_elapsed() {
        let config = CaptureConfig::new("test.rs");
        let session = CaptureSession::new(1, Language::Rust, config);
        // Just verify it doesn't panic
        let _ = session.elapsed();
    }

    #[test]
    fn test_session_has_id() {
        let config = CaptureConfig::new("test.rs");
        let session = CaptureSession::new(42, Language::Rust, config);
        // Verify session_id is a valid UUID format
        assert!(uuid::Uuid::parse_str(&session.session_id).is_ok());
        assert_eq!(session.pid, 42);
        assert_eq!(session.language, Language::Rust);
    }
}
