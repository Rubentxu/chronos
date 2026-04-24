//! Process management for MCP sandbox child processes.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use super::error::McpSandboxError;

/// File path for capturing MCP server stderr output.
/// Set via `MCP_DEBUG_LOG` env var, defaults to `/tmp/chronos-mcp-debug.log`.
pub fn debug_log_path() -> std::path::PathBuf {
    std::env::var("MCP_DEBUG_LOG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/chronos-mcp-debug.log"))
}

/// A handle to a spawned MCP server process.
pub struct McpProcess {
    child: Child,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    /// Flag indicating if the server has crashed (panic detected on stderr).
    crashed: Arc<AtomicBool>,
    /// Handle to the stderr monitoring task for cleanup.
    _stderr_task: tokio::task::JoinHandle<()>,
}

impl McpProcess {
    /// Spawn a new MCP server process from the given path.
    pub async fn spawn(mcp_path: &Path) -> Result<Self, McpSandboxError> {
        let mut cmd = tokio::process::Command::new(mcp_path);
        cmd.env("RUST_LOG", "debug");
        // Pass through CHRONOS_DB_PATH if set, so sessions can persist across server restarts
        if let Ok(db_path) = std::env::var("CHRONOS_DB_PATH") {
            cmd.env("CHRONOS_DB_PATH", db_path);
        }
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| McpSandboxError::SpawnFailed(e.to_string()))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpSandboxError::SpawnFailed("Failed to take stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpSandboxError::SpawnFailed("Failed to take stdout".to_string())
        })?;

        let stderr = child.stderr.take().ok_or_else(|| {
            McpSandboxError::SpawnFailed("Failed to take stderr".to_string())
        })?;

        let crashed = Arc::new(AtomicBool::new(false));
        let crashed_clone = crashed.clone();

        // Spawn a task to monitor stderr — write to debug log file + detect panics
        let crashed_clone2 = crashed_clone.clone();
        let stderr_task = tokio::spawn(async move {
            let log_path = debug_log_path();
            let mut log_file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .await
                .ok();

            let mut stderr_lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = stderr_lines.next_line().await {
                // Write to debug log file
                if let Some(ref mut f) = log_file {
                    let _ = f.write_all(format!("{}\n", line).as_bytes()).await;
                }
                // Check for panic patterns
                if line.contains("panicked") || line.contains("FATAL") {
                    eprintln!("[MCP-SERVER-PANIC] {}", line);
                    crashed_clone2.store(true, Ordering::SeqCst);
                }
            }
        });

        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: Some(stdout),
            crashed,
            _stderr_task: stderr_task,
        })
    }

    /// Take the stdin handle from this process.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.stdin.take()
    }

    /// Take the stdout handle from this process.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    /// Detect if the server has crashed based on stderr monitoring.
    ///
    /// Returns true if a panic message or fatal error was detected on stderr.
    pub fn detect_server_crash(&self) -> bool {
        self.crashed.load(Ordering::SeqCst)
    }

    /// Shutdown the MCP server process gracefully via SIGTERM.
    pub async fn shutdown(mut self) -> Result<(), McpSandboxError> {
        // Ensure stdin/stdout are dropped
        self.stdin = None;
        self.stdout = None;

        // Use kill() on the child process
        self.child
            .kill()
            .await
            .map_err(|e| McpSandboxError::ServerCrashed(e.to_string()))?;
        Ok(())
    }

    /// Force kill the MCP server process aggressively.
    ///
    /// This uses SIGKILL to immediately terminate the process without
    /// allowing graceful shutdown. Use this when the server is unresponsive
    /// to normal shutdown attempts.
    pub async fn force_kill(&mut self) -> Result<(), McpSandboxError> {
        // Ensure stdin/stdout are dropped first
        self.stdin = None;
        self.stdout = None;

        // Use SIGKILL via kill() on Unix
        #[cfg(unix)]
        {
            use tokio::process::Command;
            if let Some(pid) = self.child.id() {
                let _ = Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output()
                    .await;
            }
        }

        // Also try the standard kill
        let _ = self.child.kill().await;

        Ok(())
    }
}

/// Ensure proper cleanup when McpProcess is dropped.
impl Drop for McpProcess {
    fn drop(&mut self) {
        // Drop stdin/stdout to close the pipes
        self.stdin = None;
        self.stdout = None;

        // Force kill the child process like shutdown() does (SIGKILL)
        // This is necessary because the server might not respond to SIGTERM
        if let Some(pid) = self.child.id() {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .output();
        }

        // The Child struct will be dropped here - tokio's Child::drop waits for the process
    }
}

/// Type alias for the stdin writer used in RPC.
pub type McpWriter = ChildStdin;

/// Type alias for the stdout reader used in RPC.
pub type McpReader = BufReader<ChildStdout>;

/// Factory for creating MCP process handles.
pub mod factory {
    use super::*;

    /// Spawn and return a new MCP process along with its stdio handles.
    /// Returns the process handle and the stdio wrappers.
    pub async fn start(mcp_path: &Path) -> Result<(McpProcess, McpWriter, McpReader), McpSandboxError> {
        let mut process = McpProcess::spawn(mcp_path).await?;

        // Take the handles from the process
        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        // Create the reader wrapper
        let reader = McpReader::new(stdout);

        Ok((process, stdin, reader))
    }
}
