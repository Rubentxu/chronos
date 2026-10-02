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
        Self::spawn_with_env(mcp_path, std::collections::HashMap::new()).await
    }

    /// Spawn a new MCP server process from the given path with extra environment variables.
    ///
    /// Used by sandbox tests that need to control the MCP server's DB path (e.g., ce12).
    ///
    /// The ambient values of `CHRONOS_DB_PATH` and
    /// `CHRONOS_EXECUTION_LOG_DIR` are **removed**; only what
    /// `extra_env` supplies reaches the child. `Command` inherits the parent
    /// environment by default, so without the removal a developer who has
    /// either variable exported silently redirects the sandbox server at
    /// their real on-disk locations
    /// (`FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT` for the store). The
    /// execution-log root is worse than a shared store: it is resolved once
    /// per process and memoized in a `OnceLock`, so every sandbox server in
    /// the run would write its durable logs into the developer's real log
    /// root and mix test sessions in with real ones.
    pub async fn spawn_with_env(
        mcp_path: &Path,
        extra_env: std::collections::HashMap<String, String>,
    ) -> Result<Self, McpSandboxError> {
        let mut cmd = tokio::process::Command::new(mcp_path);
        for (key, value) in Self::sandbox_env() {
            match value {
                Some(v) => {
                    cmd.env(key, v);
                }
                None => {
                    cmd.env_remove(key);
                }
            }
        }
        // Apply extra environment variables (overriding any inherited ones)
        for (k, v) in extra_env {
            cmd.env(&k, &v);
        }
        let spawn_result = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match spawn_result {
            Ok(c) => c,
            Err(e) => {
                // Instrument the harness failure with the executable path
                // and exit status (CIH-B): a bare "No such file or
                // directory" leaves the operator guessing. The path is
                // included verbatim so the log line is greppable.
                return Err(McpSandboxError::SpawnFailed(format!(
                    "executable={:?} cwd={:?} error={}",
                    mcp_path,
                    std::env::current_dir().ok(),
                    e
                )));
            }
        };
        // CIH-B: if the child already exited by the time we want its
        // pipes, surface that here so the failure is visible (exit status
        // + stderr tail) instead of bubbling up as a generic "Failed to
        // take stdout" later.
        if let Ok(Some(status)) = child.try_wait() {
            let stderr_sample: String = "see McpServer stderr task output".into();
            return Err(McpSandboxError::SpawnFailed(format!(
                "executable={:?} exited before pipes were taken: status={:?} ({}).",
                mcp_path, status, stderr_sample
            )));
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpSandboxError::SpawnFailed("Failed to take stdin".to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpSandboxError::SpawnFailed("Failed to take stdout".to_string()))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| McpSandboxError::SpawnFailed("Failed to take stderr".to_string()))?;

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

    /// The environment every sandbox server starts with, as `(key, value)`.
    ///
    /// `None` means "remove from the child's environment". `Command`
    /// inherits the parent environment by default, so a location variable
    /// that is only absent because nobody wrote it down is still inherited
    /// at runtime.
    fn sandbox_env() -> Vec<(&'static str, Option<&'static str>)> {
        vec![
            ("RUST_LOG", Some("debug")),
            // Never inherit the ambient store path; callers opt in explicitly.
            ("CHRONOS_DB_PATH", None),
            // Same class of leak, same reason. The execution-log root is
            // read once per process and memoized in a `OnceLock`
            // (`chronos_log::location`), so an exported value would point
            // every sandbox server at the developer's real durable log root
            // for the whole run.
            ("CHRONOS_EXECUTION_LOG_DIR", None),
            // Pin the toolset instead of inheriting it. `debug_diff` is
            // registered in `ALL_TOOL_NAMES` and therefore appears in
            // `tools/list`, but it is in none of the seven per-profile lists,
            // so under any explicit profile the toolset guard rejects it. A
            // developer who has `CHRONOS_ACTIVE_TOOLSET` exported would
            // otherwise make the sandbox tests fail for a reason that has
            // nothing to do with the code under test. `auto` is the server
            // default (`server.rs:358-359`) and is fail-open, so this changes
            // nothing for a clean environment.
            ("CHRONOS_ACTIVE_TOOLSET", Some("auto")),
        ]
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
    pub async fn start(
        mcp_path: &Path,
    ) -> Result<(McpProcess, McpWriter, McpReader), McpSandboxError> {
        start_with_env(mcp_path, std::collections::HashMap::new()).await
    }

    /// Spawn and return a new MCP process with custom environment variables,
    /// along with its stdio handles. Used by sandbox tests that need to
    /// control the MCP server's DB path (e.g., ce12).
    pub async fn start_with_env(
        mcp_path: &Path,
        extra_env: std::collections::HashMap<String, String>,
    ) -> Result<(McpProcess, McpWriter, McpReader), McpSandboxError> {
        let mut process = McpProcess::spawn_with_env(mcp_path, extra_env).await?;

        // Take the handles from the process
        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        // Create the reader wrapper
        let reader = McpReader::new(stdout);

        Ok((process, stdin, reader))
    }
}

#[cfg(test)]
mod tests {
    use super::McpProcess;

    fn env_entry(key: &str) -> Option<Option<&'static str>> {
        McpProcess::sandbox_env()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    /// FIND-M9-72 closed this leak for the store. The execution-log root is
    /// the same defect in a different variable, and it was still inherited:
    /// every sandbox server in a run wrote its durable logs into whatever
    /// the developer running the tests had exported.
    #[test]
    fn sandbox_env_removes_both_ambient_location_variables() {
        assert_eq!(
            env_entry("CHRONOS_DB_PATH"),
            Some(None),
            "the ambient store path must be removed, not inherited"
        );
        assert_eq!(
            env_entry("CHRONOS_EXECUTION_LOG_DIR"),
            Some(None),
            "the ambient execution-log root must be removed, not inherited"
        );
    }

    /// Control: the entries that are deliberately pinned must stay pinned,
    /// so the test above cannot pass by returning an empty environment.
    #[test]
    fn sandbox_env_pins_toolset_and_log_level() {
        assert_eq!(env_entry("CHRONOS_ACTIVE_TOOLSET"), Some(Some("auto")));
        assert_eq!(env_entry("RUST_LOG"), Some(Some("debug")));
        assert_eq!(
            McpProcess::sandbox_env().len(),
            4,
            "the sandbox environment is exactly these four entries"
        );
    }
}
