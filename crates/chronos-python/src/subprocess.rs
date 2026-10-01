use crate::{
    bootstrap,
    error::PythonError,
    parser::{parse_line, RawPythonEvent},
};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub struct PythonSubprocess {
    #[allow(dead_code)]
    child: Child,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl PythonSubprocess {
    /// Spawn a Python subprocess with the bootstrap tracing code.
    /// The target should be a path to a Python script.
    pub fn spawn(target: &str, capture_locals: bool) -> Result<Self, PythonError> {
        let bootstrap = bootstrap::bootstrap_code_for_target(target);
        let mut cmd = Command::new("python3");
        cmd.arg("-u").arg("-c").arg(&bootstrap);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        if !capture_locals {
            cmd.env("CHRONOS_CAPTURE_LOCALS", "0");
        }

        // Without this, dropping the handle does NOT kill the interpreter:
        // `tokio::process::Child` only kills on drop when `kill_on_drop` is
        // configured. Without this line every capture of a Python target left a
        // live `python3` behind. Same reason and same pattern as chronos-js,
        // chronos-go and chronos-java.
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| PythonError::SpawnFailed(e.to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PythonError::SpawnFailed("Failed to capture stdout".to_string()))?;
        Ok(Self {
            child,
            stdout: BufReader::new(stdout),
        })
    }

    /// Read the next trace event from the subprocess.
    /// Returns None when the subprocess has finished or there's no more output.
    pub async fn next_event(&mut self) -> Result<Option<RawPythonEvent>, PythonError> {
        let mut line = String::new();
        let bytes_read = self.stdout.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None);
        }
        line = line.trim().to_string();
        if line.is_empty() {
            return Ok(None);
        }
        let event = parse_line(&line)?;
        Ok(Some(event))
    }
}

// There is no `impl Drop`: the cleanup is done by `kill_on_drop(true)` in
// `spawn`. The empty `Drop` this replaces carried the comment "SIGTERM is sent
// automatically when Child is dropped", which is false — `tokio::process::Child`
// only kills on drop when `kill_on_drop` is configured.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_spawn_python_subprocess() {
        // Create a simple Python script that runs immediately
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "print('hello')").unwrap();
        file.flush().unwrap();

        let result = PythonSubprocess::spawn(file.path().to_str().unwrap(), true);
        // Spawn should succeed - the subprocess may or may not produce events
        // depending on whether the bootstrap code is properly integrated
        assert!(result.is_ok(), "Should be able to spawn python subprocess");
    }

    #[tokio::test]
    async fn test_subprocess_reads_events() {
        // Create a Python script with a function call
        let script_content = "def foo():\n    x = 1\n    return x\nfoo()\n";
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        write!(file, "{}", script_content).unwrap();
        file.flush().unwrap();

        let mut proc = PythonSubprocess::spawn(file.path().to_str().unwrap(), true).unwrap();

        // Read output - we may get events or may get None depending on how
        // the bootstrap integration works in this MVP
        //
        // This used to collect events into a `Vec` and never look at it, so it
        // could not fail. It now asserts the two properties that actually
        // matter: reading does not error, and the traced `foo` shows up in the
        // trace stream.
        let mut events = Vec::new();
        for _ in 0..20 {
            match proc.next_event().await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(e) => panic!("reading a trace event should not fail: {e}"),
            }
        }
        assert!(
            events.iter().any(|e| e.name.contains("foo")),
            "the bootstrap should trace `foo`, got {} event(s): {:?}",
            events.len(),
            events
        );
    }

    /// Guards the process leak: dropping the handle has to terminate the
    /// interpreter, not merely forget the handle.
    ///
    /// Non-vacuous by construction: the bootstrap runs the target with
    /// `runpy.run_path`, so a target that sleeps keeps `python3` alive on its
    /// own. If this test passes it is because `kill_on_drop(true)` killed it.
    #[tokio::test]
    async fn test_subprocess_is_killed_on_drop() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "import time\ntime.sleep(30)\n").unwrap();
        file.flush().unwrap();

        let pid = {
            let proc = PythonSubprocess::spawn(file.path().to_str().unwrap(), true)
                .expect("spawning python3 should succeed");
            let pid = proc.child.id().expect("the child should have a pid");
            // Give the interpreter a moment to actually be running the target,
            // so the test cannot pass merely because it had not started yet.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            assert!(
                process_is_alive(pid),
                "the interpreter should still be running before the drop"
            );
            pid
        };
        // `proc` has been dropped here.

        // Reaping the child can take a moment; retry briefly instead of
        // demanding instantaneous death.
        for _ in 0..40 {
            if !process_is_alive(pid) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        panic!(
            "python3 process {pid} was still alive after dropping PythonSubprocess: \
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
