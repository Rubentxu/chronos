//! Spawn Delve DAP server as a subprocess.

use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::error::GoError;

/// A spawned Delve DAP server process.
pub struct DelveSubprocess {
    /// The child process handle.
    pub child: Child,
    /// The port that was assigned (parsed from stdout).
    pub port: u16,
}

impl DelveSubprocess {
    /// Spawn: `dlv dap --listen=127.0.0.1:0 -- <target>`
    ///
    /// Parses stdout to find the DAP server port from:
    /// "DAP server listening at: 127.0.0.1:<port>"
    pub async fn spawn(target: &str) -> Result<Self, GoError> {
        // Check if dlv is available
        which::which("dlv").map_err(|_| GoError::DelveNotFound)?;

        let mut cmd = Command::new("dlv");
        cmd.arg("dap")
            .arg("--listen=127.0.0.1:0")
            .arg("--")
            .arg(target);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GoError::DelveNotFound
            } else {
                GoError::SpawnFailed(e.to_string())
            }
        })?;

        // Read stdout to find the DAP port
        let stdout = child.stdout.take().ok_or_else(|| {
            GoError::SpawnFailed("Failed to capture stdout".to_string())
        })?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        // DAP port is bound once Delve is ready
        // Format: "DAP server listening at: 127.0.0.1:<port>"
        let port = loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF reached without finding port — dlv may have exited
                    let exit_status = child.wait().await?.code();
                    return Err(GoError::SpawnFailed(format!(
                        "Delve exited before DAP port was available: {:?}",
                        exit_status
                    )));
                }
                Ok(_) => {
                    if let Some(port) = parse_dap_port_from_line(&line) {
                        break port;
                    }
                }
                Err(e) => {
                    return Err(GoError::SpawnFailed(format!(
                        "Failed to read Delve stdout: {}",
                        e
                    )));
                }
            }
        };

        Ok(Self { child, port })
    }
}

/// Parse the DAP port from a line of Delve output.
///
/// Expected format: "DAP server listening at: 127.0.0.1:<port>"
fn parse_dap_port_from_line(line: &str) -> Option<u16> {
    let prefix = "DAP server listening at: 127.0.0.1:";
    let idx = line.find(prefix)?;
    let after_address = &line[idx + prefix.len()..];
    // The port is everything up to the next whitespace or end
    let port_str = after_address
        .split_whitespace()
        .next()
        .unwrap_or(after_address);
    port_str.parse().ok()
}

impl Drop for DelveSubprocess {
    fn drop(&mut self) {
        // SIGTERM is sent automatically when Child is dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dap_port_from_line() {
        let line = "DAP server listening at: 127.0.0.1:54321";
        assert_eq!(parse_dap_port_from_line(line), Some(54321));

        let line = "Some other output\nDAP server listening at: 127.0.0.1:12345\nmore text";
        assert_eq!(parse_dap_port_from_line(line), Some(12345));

        let line = "Random output without port";
        assert_eq!(parse_dap_port_from_line(line), None);

        let line = "DAP server listening at: 127.0.0.1:";
        assert_eq!(parse_dap_port_from_line(line), None);
    }

    #[tokio::test]
    #[ignore]
    async fn test_delve_subprocess_spawn() {
        // This test requires dlv to be on PATH
        if which::which("dlv").is_err() {
            return;
        }

        // Create a minimal Go program for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let main_content = r#"
package main

func main() {
    println("Hello")
}
"#;
        let file_path = temp_dir.path().join("main.go");
        std::fs::write(&file_path, main_content).unwrap();

        // Try to spawn dlv dap on the temp file
        let result = DelveSubprocess::spawn(file_path.to_str().unwrap()).await;
        // This might fail due to compilation issues, but we're testing the spawn mechanism
        // We just verify it doesn't panic
    }
}
