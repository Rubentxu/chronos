//! Spawn Delve DAP server as a subprocess.

use crate::error::GoError;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

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

        // Without this, dropping the handle does NOT kill dlv: `tokio::process::Child`
        // only kills on drop when `kill_on_drop` is configured. Without this
        // line every attach to a Go target left a live DAP server holding its
        // port and its memory. Same reason as chronos-js using
        // `.kill_on_drop(true)` in `NodeProcess::spawn`.
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                GoError::DelveNotFound
            } else {
                GoError::SpawnFailed(e.to_string())
            }
        })?;

        // Read stdout to find the DAP port
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GoError::SpawnFailed("Failed to capture stdout".to_string()))?;

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

// There is no `impl Drop`: the cleanup is done by `kill_on_drop(true)` in `spawn`.
// An empty `Drop` whose comment claims tokio sends SIGTERM would be false, and
// it was: `tokio::process::Child` only kills on drop when `kill_on_drop` is
// configured. See crates/chronos-go/src/subprocess.rs::spawn.

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

    /// Verifies the real contract of `DelveSubprocess::spawn`: that dlv publishes
    /// a DAP port on stdout and that the port ends up actually listening.
    ///
    /// The target is missing on purpose: dlv announces the port *before* trying
    /// to load the program, which is exactly the guarantee the adapter needs —
    /// obtain a port, not compile the target.
    ///
    /// This test used to discard the result with `let _result = ...` and assert
    /// nothing; its own comment said "we just verify it doesn't panic". It
    /// covered nothing, and dropping the handle left a `dlv` alive, which is how
    /// the leak now pinned by `kill_on_drop` was found.
    #[tokio::test]
    #[ignore = "requires dlv on PATH; run by the gate's opt-in tier"]
    async fn test_delve_spawn_publishes_a_reachable_dap_port() {
        if which::which("dlv").is_err() {
            // Declared skip: this test can assert nothing without dlv, and it
            // must not quietly become a green.
            eprintln!("DECLARED SKIP: dlv not on PATH, nothing to assert");
            return;
        }

        let target = std::env::temp_dir().join("chronos-dlv-target-inexistente");
        let sub = DelveSubprocess::spawn(target.to_str().expect("utf-8 path"))
            .await
            .expect("dlv should publish a DAP port even when the target is missing");

        assert!(
            sub.port > 0,
            "the DAP port must be a positive integer, got {:?}",
            sub.port
        );

        // Parsed is not the same as listening. If dlv announced a port nobody
        // then opened, this test has to fail: that is the difference between
        // "printed a number" and "I can attach".
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], sub.port));
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("DAP port {addr} should accept a connection: {e}"));

        // On leaving scope, `kill_on_drop(true)` must terminate dlv.
    }

    /// Guards the process leak: dropping the handle has to terminate the `dlv`,
    /// not merely forget the handle.
    ///
    /// This is the test that was missing and that made the bug possible: the
    /// old test dropped the handle without checking anything and the `dlv`
    /// survived, reparented to systemd --user.
    #[tokio::test]
    #[ignore = "requires dlv on PATH; run by the gate's opt-in tier"]
    async fn test_delve_subprocess_is_killed_on_drop() {
        if which::which("dlv").is_err() {
            eprintln!("DECLARED SKIP: dlv not on PATH, nothing to assert");
            return;
        }

        let target = std::env::temp_dir().join("chronos-dlv-target-inexistente");
        let pid = {
            let sub = DelveSubprocess::spawn(target.to_str().expect("utf-8 path"))
                .await
                .expect("dlv should publish a DAP port");
            sub.child.id().expect("the child should have a pid")
        };
        // `sub` has been dropped here.

        // Reaping the child can take a moment; retry briefly instead of
        // demanding instantaneous death.
        for _ in 0..40 {
            if !process_is_alive(pid) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        panic!(
            "dlv process {pid} was still alive after dropping DelveSubprocess: \
             kill_on_drop(true) is not taking effect"
        );
    }

    /// `kill(pid, 0)` sends no signal: it only reports whether the process
    /// exists. A zombie still occupies the table, so its state is checked too,
    /// to avoid calling an already-reaped child alive.
    fn process_is_alive(pid: u32) -> bool {
        // /proc/<pid>/stat: field 3 is the state ('Z' = zombie).
        let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => s,
            // Without /proc (macOS, BSD) fall back to the simpler criterion.
            Err(_) => return unsafe { libc_kill(pid) == 0 },
        };
        // The process name sits in parentheses and may contain spaces, so split
        // after the last ')'.
        let state = match stat.rfind(')') {
            Some(i) => stat[i + 1..].split_whitespace().next().unwrap_or(""),
            None => "",
        };
        state != "Z"
    }

    unsafe fn libc_kill(pid: u32) -> i32 {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        kill(pid as i32, 0)
    }
}
