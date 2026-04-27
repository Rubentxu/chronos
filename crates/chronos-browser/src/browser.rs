//! Chrome process management for browser-based debugging.
//!
//! Handles discovery of Chrome binary and spawning Chrome processes with
//! remote debugging enabled.

use crate::error::BrowserError;
use std::process::{Child, Command};
use tracing::{debug, info};

/// Environment variable for Chrome path
const CHROME_PATH_ENV: &str = "CHROME_PATH";

/// Default Chrome remote debugging port
const DEFAULT_DEBUG_PORT: u16 = 9222;

/// Chrome process manager
pub struct ChromeProcess {
    process: Option<Child>,
    debug_url: String,
    ws_url: String,
}

impl ChromeProcess {
    /// Spawn a new Chrome process with remote debugging enabled
    pub fn spawn(headless: bool) -> Result<Self, BrowserError> {
        let chrome_path = find_chrome_binary()?;

        let debug_port = DEFAULT_DEBUG_PORT;
        let user_data_dir = std::env::temp_dir();

        let mut args = vec![
            format!("--remote-debugging-port={}", debug_port),
            format!("--user-data-dir={}", user_data_dir.display()),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-extensions".to_string(),
            "--disable-popup-blocking".to_string(),
            "--disable-translate".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-sync".to_string(),
            "--disable-default-apps".to_string(),
            "--mute-audio".to_string(),
            "--no-first-run".to_string(),
            "--save-prefdrafts".to_string(),
        ];

        if headless {
            args.push("--headless".to_string());
            args.push("--disable-gpu".to_string());
        }

        // Launch Chrome
        info!("Spawning Chrome at {} with args: {:?}", chrome_path, args);

        let child = Command::new(&chrome_path)
            .args(&args)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BrowserError::ChromeNotFound(chrome_path.clone())
                } else {
                    BrowserError::ProcessError(format!("Failed to spawn Chrome: {}", e))
                }
            })?;

        // Wait for Chrome to be ready
        let ws_url = wait_for_chrome_ready(debug_port)?;

        Ok(Self {
            process: Some(child),
            debug_url: format!("http://localhost:{}", debug_port),
            ws_url,
        })
    }

    /// Attach to an existing Chrome process via WebSocket URL
    pub fn attach(ws_url: &str) -> Result<Self, BrowserError> {
        // Validate the WebSocket URL format
        if !ws_url.starts_with("ws://") && !ws_url.starts_with("wss://") {
            return Err(BrowserError::CdpConnectionFailed(
                "Invalid WebSocket URL".into(),
            ));
        }

        Ok(Self {
            process: None,
            debug_url: String::new(),
            ws_url: ws_url.to_string(),
        })
    }

    /// Attach to Chrome via the debugging port
    pub fn attach_port(port: u16) -> Result<Self, BrowserError> {
        let ws_url = format!("ws://localhost:{}/devtools/browser", port);
        Ok(Self {
            process: None,
            debug_url: format!("http://localhost:{}", port),
            ws_url,
        })
    }

    /// Get the WebSocket debugging URL
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Get the HTTP debugging URL
    pub fn debug_url(&self) -> &str {
        &self.debug_url
    }

    /// Kill the Chrome process
    pub fn kill(&mut self) -> Result<(), BrowserError> {
        if let Some(ref mut child) = self.process {
            child.kill().map_err(|e| BrowserError::ProcessError(format!("Failed to kill Chrome: {}", e)))?;
        }
        self.process = None;
        Ok(())
    }

    /// Check if the process is still running
    pub fn is_running(&self) -> bool {
        self.process.as_ref().map_or(false, |c| c.id() != 0)
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ============================================================================
// Chrome binary discovery
// ============================================================================

/// Find the Chrome binary path
fn find_chrome_binary() -> Result<String, BrowserError> {
    // Check environment variable first
    if let Ok(path) = std::env::var(CHROME_PATH_ENV) {
        if std::path::Path::new(&path).exists() {
            debug!("Using Chrome from {} env variable", CHROME_PATH_ENV);
            return Ok(path);
        }
    }

    // Try common Chrome binary paths
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ]
    } else {
        vec![
            "google-chrome",
            "chromium-browser",
            "chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
            "/snap/bin/chromium",
        ]
    };

    for candidate in candidates {
        if which::which(candidate).is_ok() {
            debug!("Found Chrome at: {}", candidate);
            return Ok(candidate.to_string());
        }
    }

    Err(BrowserError::ChromeNotFound(
        "Could not find Chrome or Chromium in PATH".into(),
    ))
}

/// Wait for Chrome to be ready and return the WebSocket URL
fn wait_for_chrome_ready(port: u16) -> Result<String, BrowserError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| BrowserError::ProcessError(format!("Failed to create HTTP client: {}", e)))?;

    let timeout = std::time::Duration::from_secs(30);
    let start = std::time::Instant::now();
    let target = format!("http://localhost:{}/json", port);

    while start.elapsed() < timeout {
        match client.get(&target).send() {
            Ok(resp) if resp.status().is_success() => {
                // Get the first target's webSocketDebuggerUrl
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(targets) = json.as_array() {
                        if let Some(first) = targets.first() {
                            if let Some(ws_url) = first.get("webSocketDebuggerUrl") {
                                if let Some(url_str) = ws_url.as_str() {
                                    return Ok(url_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // Connection failed or not ready yet
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err(BrowserError::Timeout(
        "Timed out waiting for Chrome to be ready".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_not_found_error() {
        // Set an invalid path to force ChromeNotFound error
        std::env::set_var(CHROME_PATH_ENV, "/nonexistent/path/to/chrome");
        let result = find_chrome_binary();
        std::env::remove_var(CHROME_PATH_ENV);

        assert!(matches!(result, Err(BrowserError::ChromeNotFound(_))));
    }

    #[test]
    fn test_chrome_process_creation() {
        // This test just verifies the struct can be created
        let process = ChromeProcess {
            process: None,
            debug_url: "http://localhost:9222".to_string(),
            ws_url: "ws://localhost:9222/devtools/browser".to_string(),
        };
        assert_eq!(process.debug_url, "http://localhost:9222");
        assert!(!process.is_running());
    }
}
