//! Chrome process management for browser-based debugging.
//!
//! Handles discovery of Chrome binary and spawning Chrome processes with
//! remote debugging enabled.

use crate::error::BrowserError;
use std::process::{Child, Command};
use tracing::info;

/// Environment variable for Chrome path
const CHROME_PATH_ENV: &str = "CHROME_PATH";

/// Default Chrome remote debugging port
const DEFAULT_DEBUG_PORT: u16 = 9222;

/// Chrome process manager
pub struct ChromeProcess {
    process: Option<Child>,
    debug_port: u16,
    debug_url: String,
    ws_url: String,
    /// Keep temp dir alive for session isolation (SIG 6)
    _user_data_dir: tempfile::TempDir,
}

impl ChromeProcess {
    /// Spawn Chrome and return immediately (non-blocking).
    /// Call `wait_for_ready` afterwards to get the WS URL.
    pub fn spawn(headless: bool, chrome_path: Option<&str>) -> Result<Self, BrowserError> {
        let chrome_path = match chrome_path {
            Some(p) => p.to_string(),
            None => find_chrome_binary()?,
        };

        let debug_port = DEFAULT_DEBUG_PORT;

        // Use a unique temp directory for session isolation (SIG 6)
        let user_data_dir = tempfile::TempDir::new()
            .map_err(|e| BrowserError::ProcessError(format!("Failed to create temp dir: {}", e)))?;
        let user_data_path = user_data_dir.path().to_path_buf();

        let mut args = vec![
            format!("--remote-debugging-port={}", debug_port),
            format!("--user-data-dir={}", user_data_path.display()),
            "--no-default-browser-check".to_string(), // Remove duplicate --no-first-run (SIG 8)
            "--disable-extensions".to_string(),
            "--disable-popup-blocking".to_string(),
            "--disable-translate".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-sync".to_string(),
            "--disable-default-apps".to_string(),
            "--mute-audio".to_string(),
            // Remove --save-prefdrafts (SIG 7 - typo, doesn't exist)
            // Remove duplicate --no-first-run (SIG 8)
        ];

        if headless {
            args.push("--headless".to_string());
            args.push("--disable-gpu".to_string());
            // Docker compatibility: prevents /dev/shm size issues
            args.push("--disable-dev-shm-usage".to_string());
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

        Ok(Self {
            process: Some(child),
            debug_port,
            debug_url: format!("http://localhost:{}", debug_port),
            ws_url: String::new(), // Set after wait_for_ready
            _user_data_dir: user_data_dir,
        })
    }

    /// Get the debug port
    pub fn port(&self) -> u16 {
        self.debug_port
    }

    /// Attach to an existing Chrome process via WebSocket URL
    pub fn attach(ws_url: &str) -> Result<Self, BrowserError> {
        // Validate the WebSocket URL format
        if !ws_url.starts_with("ws://") && !ws_url.starts_with("wss://") {
            return Err(BrowserError::CdpConnectionFailed(
                "Invalid WebSocket URL".into(),
            ));
        }

        let user_data_dir = tempfile::TempDir::new()
            .map_err(|e| BrowserError::ProcessError(format!("Failed to create temp dir: {}", e)))?;

        Ok(Self {
            process: None,
            debug_port: DEFAULT_DEBUG_PORT,
            debug_url: String::new(),
            ws_url: ws_url.to_string(),
            _user_data_dir: user_data_dir,
        })
    }

    /// Attach to Chrome via the debugging port
    pub fn attach_port(port: u16) -> Result<Self, BrowserError> {
        let user_data_dir = tempfile::TempDir::new()
            .map_err(|e| BrowserError::ProcessError(format!("Failed to create temp dir: {}", e)))?;

        Ok(Self {
            process: None,
            debug_port: port,
            debug_url: format!("http://localhost:{}", port),
            ws_url: format!("ws://localhost:{}/devtools/browser", port),
            _user_data_dir: user_data_dir,
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

    /// Wait for Chrome's CDP endpoint to be ready. Returns WS URL.
    /// This is an async method that polls the CDP JSON endpoint.
    pub async fn wait_for_ready(port: u16) -> Result<String, BrowserError> {
        let target = format!("http://localhost:{}/json", port);
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                BrowserError::ProcessError(format!("Failed to create HTTP client: {}", e))
            })?;

        while start.elapsed() < timeout {
            match client.get(&target).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        // Extract webSocketDebuggerUrl from first target
                        if let Some(url) = json
                            .as_array()
                            .and_then(|t| t.first())
                            .and_then(|t| t.get("webSocketDebuggerUrl"))
                            .and_then(|u| u.as_str())
                        {
                            return Ok(url.to_string());
                        }
                    }
                }
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        Err(BrowserError::Timeout(
            "Timed out waiting for Chrome CDP".into(),
        ))
    }

    /// Set the WS URL after wait_for_ready completes
    pub fn set_ws_url(&mut self, ws_url: String) {
        self.ws_url = ws_url;
    }

    /// Kill the Chrome process with timeout
    ///
    /// Waits up to 5 seconds for the process to exit after sending SIGKILL.
    pub fn kill(&mut self) -> Result<(), BrowserError> {
        if let Some(ref mut child) = self.process {
            child
                .kill()
                .map_err(|e| BrowserError::ProcessError(format!("Failed to kill Chrome: {}", e)))?;

            // Wait with timeout for process to exit
            let start = std::time::Instant::now();
            while start.elapsed() < std::time::Duration::from_secs(5) {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        self.process = None;
        Ok(())
    }

    /// Check if the process is still running
    pub fn is_running(&self) -> bool {
        self.process.as_ref().is_some_and(|c| c.id() != 0)
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

/// Locate the Chrome binary within a configured environment.
///
/// Decoupled from the host environment (PATH, well-known paths) so the
/// search can be controlled from tests without depending on what is
/// installed on the host. `find_chrome_binary` is the default entry
/// point that constructs a `ChromeLocator` from the host.
#[derive(Debug, Clone)]
pub struct ChromeLocator {
    /// Override resolved from `CHROME_PATH` env var, if present and the
    /// file exists on disk.
    pub env_override: Option<String>,
    /// Candidate paths / basenames to look up via `which`.
    pub candidates: Vec<String>,
}

impl ChromeLocator {
    /// Build a locator that mirrors the host search: env var first,
    /// then the platform-default candidate list.
    pub fn from_host() -> Self {
        let env_override = std::env::var(CHROME_PATH_ENV)
            .ok()
            .filter(|p| std::path::Path::new(p).exists());
        let candidates = if cfg!(target_os = "macos") {
            vec![
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string(),
                "/Applications/Chromium.app/Contents/MacOS/Chromium".to_string(),
            ]
        } else {
            vec![
                "google-chrome".to_string(),
                "chromium-browser".to_string(),
                "chromium".to_string(),
                "/usr/bin/google-chrome".to_string(),
                "/usr/bin/chromium-browser".to_string(),
                "/usr/bin/chromium".to_string(),
                "/snap/bin/chromium".to_string(),
            ]
        };
        Self {
            env_override,
            candidates,
        }
    }

    /// Resolve a Chrome binary path using only the candidates provided.
    /// Returns `ChromeNotFound` if neither the env override (if set)
    /// nor any of the candidates resolves to an executable on disk.
    pub fn resolve(&self) -> Result<String, BrowserError> {
        if let Some(path) = &self.env_override {
            if std::path::Path::new(path).exists() {
                return Ok(path.clone());
            }
            // Env override points to a non-existent path; fall through to
            // candidate search. This matches the previous behaviour and
            // ensures a misconfigured CHROME_PATH does not silently win.
        }
        for candidate in &self.candidates {
            if which::which(candidate).is_ok() {
                return Ok(candidate.clone());
            }
        }
        Err(BrowserError::ChromeNotFound(
            "Could not find Chrome or Chromium in the configured locator".into(),
        ))
    }
}

/// Find the Chrome binary path using the default host locator.
fn find_chrome_binary() -> Result<String, BrowserError> {
    ChromeLocator::from_host().resolve()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_not_found_error() {
        // Construct a locator with NO env override and NO candidates.
        // Resolution must deterministically return ChromeNotFound,
        // regardless of whether the host has Chrome installed.
        let locator = ChromeLocator {
            env_override: None,
            candidates: Vec::new(),
        };
        let result = locator.resolve();
        assert!(matches!(result, Err(BrowserError::ChromeNotFound(_))));
    }

    #[test]
    fn test_chrome_locator_with_candidate_resolves_to_that_candidate() {
        // /bin/true exists on every Linux/macOS CI runner and is non-Chrome,
        // so we use it as a stand-in to verify candidate-driven resolution
        // is honoured without depending on the host PATH.
        let locator = ChromeLocator {
            env_override: None,
            candidates: vec!["/bin/true".to_string()],
        };
        let result = locator.resolve();
        assert!(matches!(result, Ok(ref p) if p == "/bin/true"));
    }

    #[test]
    fn test_chrome_locator_env_override_path_must_exist() {
        // An env override pointing to a non-existent path is treated as
        // "no override" — fall back to the candidate list.
        let locator = ChromeLocator {
            env_override: Some("/nonexistent/path/to/chrome".to_string()),
            candidates: Vec::new(),
        };
        let result = locator.resolve();
        assert!(matches!(result, Err(BrowserError::ChromeNotFound(_))));
    }

    #[test]
    fn test_chrome_process_creation() {
        // This test just verifies the struct can be created
        let user_data_dir = tempfile::TempDir::new().unwrap();
        let process = ChromeProcess {
            process: None,
            debug_port: 9222,
            debug_url: "http://localhost:9222".to_string(),
            ws_url: "ws://localhost:9222/devtools/browser".to_string(),
            _user_data_dir: user_data_dir,
        };
        assert_eq!(process.debug_url, "http://localhost:9222");
        assert_eq!(process.port(), 9222);
        assert!(!process.is_running());
    }

    #[test]
    fn test_attach_valid_ws_url() {
        // CRIT-002 fix: attach() should succeed with valid ws:// URL and create TempDir
        let result = ChromeProcess::attach("ws://localhost:9222/devtools/browser");
        assert!(
            result.is_ok(),
            "attach() should succeed with valid ws:// URL"
        );
        let process = result.unwrap();
        assert_eq!(process.ws_url(), "ws://localhost:9222/devtools/browser");
        assert!(!process.is_running());
    }

    #[test]
    fn test_attach_valid_wss_url() {
        // CRIT-002 fix: attach() should succeed with valid wss:// URL
        let result = ChromeProcess::attach("wss://remote:9222/devtools/browser");
        assert!(
            result.is_ok(),
            "attach() should succeed with valid wss:// URL"
        );
        let process = result.unwrap();
        assert_eq!(process.ws_url(), "wss://remote:9222/devtools/browser");
    }

    #[test]
    fn test_attach_invalid_url() {
        // CRIT-002 fix: attach() should reject non-ws URLs
        let result = ChromeProcess::attach("http://localhost:9222");
        assert!(result.is_err());
        match result {
            Err(BrowserError::CdpConnectionFailed(msg)) => {
                assert!(
                    msg.contains("Invalid WebSocket URL"),
                    "Expected 'Invalid WebSocket URL' error, got: {}",
                    msg
                );
            }
            Err(other) => panic!("Expected CdpConnectionFailed, got: {:?}", other),
            Ok(_) => panic!("Expected error, got success"),
        }
    }

    #[test]
    fn test_attach_port_creates_process() {
        // CRIT-003 fix: attach_port() should succeed and create a proper process
        let result = ChromeProcess::attach_port(9222);
        assert!(result.is_ok(), "attach_port() should succeed");
        let process = result.unwrap();
        assert_eq!(process.port(), 9222);
        assert_eq!(process.debug_url(), "http://localhost:9222");
        assert!(process.ws_url().contains("9222"));
        assert!(process.ws_url().contains("devtools/browser"));
        assert!(!process.is_running());
    }

    #[test]
    fn test_attach_port_different_port() {
        // Verify port is correctly set
        let process = ChromeProcess::attach_port(9333).unwrap();
        assert_eq!(process.port(), 9333);
        assert_eq!(process.debug_url(), "http://localhost:9333");
        assert!(process.ws_url().contains("9333"));
    }
}
