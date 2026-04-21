//! Example: Implementing a custom TraceAdapter for a new language.
//!
//! This shows the minimal interface needed to add language support.

use chronos_capture::TraceAdapter;
use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceError};

/// A minimal example adapter that generates synthetic events.
pub struct ExampleAdapter;

impl TraceAdapter for ExampleAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        println!("Starting capture for: {:?}", config);
        // In a real adapter: spawn subprocess, connect debugger, etc.
        // For demonstration, we create a session with the session_id set to "example".
        Ok(CaptureSession::new(0, Language::Unknown, config))
    }

    fn stop_capture(&self, session: &CaptureSession) -> Result<(), TraceError> {
        println!("Stopping session: {}", session.session_id);
        Ok(())
    }

    fn attach_to_process(
        &self,
        _pid: u32,
        _config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not implemented for this example".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::Unknown
    }

    fn name(&self) -> &str {
        "example-adapter"
    }
}

fn main() {
    println!("Custom adapter example — see source code for TraceAdapter implementation.");
    println!("To register your adapter:");
    println!("  registry.register(Arc::new(ExampleAdapter));");
}
