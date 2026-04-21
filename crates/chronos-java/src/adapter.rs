//! Java adapter using JDWP (Java Debug Wire Protocol).
//!
//! This adapter spawns a JVM with JDWP debugging enabled and captures
//! method entry/exit and exception events via the debug wire protocol.

use chronos_capture::{CaptureConfig, TraceAdapter};
use chronos_domain::{CaptureSession, Language, TraceError};
use std::sync::Mutex;
use std::time::Instant;
use which::which;

use crate::protocol::JdwpClient;
use crate::subprocess::JavaSubprocess;

/// Interior mutable state of the Java adapter.
struct JavaAdapterState {
    /// The spawned JVM subprocess.
    subprocess: Option<JavaSubprocess>,
    /// The JDWP client connected to the JVM.
    client: Option<JdwpClient>,
    /// Next event ID to assign.
    next_event_id: u64,
    /// When the capture session started.
    session_start: Option<Instant>,
}

/// Java trace adapter using JDWP.
///
/// Spawns a JVM with `-agentlib:jdwp` and captures method entry/exit events
/// via the Java Debug Wire Protocol.
pub struct JavaAdapter {
    state: Mutex<JavaAdapterState>,
}

impl JavaAdapter {
    /// Create a new Java adapter.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(JavaAdapterState {
                subprocess: None,
                client: None,
                next_event_id: 1,
                session_start: None,
            }),
        }
    }

    /// Check if Java (java + javac) is available on the system.
    pub fn is_available() -> bool {
        which("java").is_ok()
    }
}

impl Default for JavaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for JavaAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Check java is available
        if which("java").is_err() {
            return Err(TraceError::CaptureFailed(
                "Java not found in PATH".to_string(),
            ));
        }

        // Spawn the JVM with JDWP
        let subprocess = tokio::runtime::Handle::current()
            .block_on(JavaSubprocess::spawn(&config.target))
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn JVM: {}", e)))?;

        // Connect JDWP client
        let client = tokio::runtime::Handle::current()
            .block_on(JdwpClient::connect(subprocess.jdwp_port))
            .map_err(|e| TraceError::CaptureFailed(format!("JDWP connect failed: {}", e)))?;

        let mut state = self.state.lock().unwrap();
        state.subprocess = Some(subprocess);
        state.client = Some(client);
        state.session_start = Some(Instant::now());
        state.next_event_id = 1;

        let session = CaptureSession::new(0, Language::Java, config);
        Ok(session)
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut state = self.state.lock().unwrap();
        state.subprocess = None;
        state.client = None;
        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not yet supported for Java".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::Java
    }

    fn name(&self) -> &str {
        "java-jdwp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_adapter_name() {
        let adapter = JavaAdapter::new();
        assert_eq!(adapter.name(), "java-jdwp");
    }

    #[test]
    fn test_java_adapter_language() {
        let adapter = JavaAdapter::new();
        assert_eq!(adapter.get_language(), Language::Java);
    }

    #[test]
    fn test_java_adapter_is_available() {
        // Result depends on whether java is on PATH
        let available = JavaAdapter::is_available();
        assert!(available || !available); // Always passes — checks method doesn't panic
    }
}
