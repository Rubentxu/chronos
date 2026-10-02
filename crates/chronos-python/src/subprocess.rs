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

/// Build the `python3` invocation that traces `target`.
///
/// The target travels as a plain command line argument and never as part of
/// the Python source. The interpreter is invoked as
/// `python3 -u -c <bootstrap> <target>`, so `sys.argv` is `['-c', '<target>']`
/// and the bootstrap reads the target back from `sys.argv[1]`. A target
/// containing `"`, a newline or a backslash is therefore data: it cannot
/// close the string literal and append statements.
fn build_command(target: &str, capture_locals: bool) -> Command {
    let mut cmd = Command::new("python3");
    cmd.arg("-u").arg("-c").arg(bootstrap::bootstrap_code());
    cmd.arg(target);
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
    cmd
}

impl PythonSubprocess {
    /// Spawn a Python subprocess with the bootstrap tracing code.
    /// The target should be a path to a Python script.
    pub fn spawn(target: &str, capture_locals: bool) -> Result<Self, PythonError> {
        let mut child = build_command(target, capture_locals)
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

    /// The target must reach the interpreter as an argument, never as source.
    ///
    /// This asserts on the real `Command` that `spawn` runs, not on a copy of
    /// the bootstrap. It fails if anyone reintroduces embedding: an embedded
    /// target would make arg 2 differ from `bootstrap_code()`.
    #[test]
    fn test_target_is_passed_as_argv_not_embedded_in_code() {
        let target = "/tmp/example.py";
        let args: Vec<String> = build_command(target, true)
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            args.len(),
            4,
            "expected exactly [-u, -c, <bootstrap>, <target>], got {args:?}"
        );
        assert_eq!(args[0], "-u");
        assert_eq!(args[1], "-c");
        assert_eq!(
            args[2],
            bootstrap::bootstrap_code(),
            "the `-c` program must be the constant bootstrap, not one built \
             from the target"
        );
        assert_eq!(
            args[3], target,
            "the target must be the argument that lands in sys.argv[1]"
        );
    }

    /// Regression test for the injection: a target carrying a quote and a
    /// newline must stay a single opaque argument.
    ///
    /// The old code escaped backslashes only, then spliced the target into
    /// `r"{target}"`, so this payload rewrote the generated Python program.
    /// With the argument form the payload cannot terminate a string literal,
    /// because it is never inside one.
    #[test]
    fn test_malicious_target_cannot_inject_python_code() {
        let payload = "/tmp/x.py\"\nimport os; os.system(\"id\")\n";
        let args: Vec<String> = build_command(payload, true)
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        let program = args
            .get(2)
            .expect("the interpreter is always invoked as `python3 -u -c <program>`");

        // These come first and carry the diagnosis: with the old embedding the
        // payload is spliced straight into the program text.
        assert!(
            !program.contains(payload),
            "the payload must not appear inside the Python program, but it does"
        );
        assert!(
            !program.contains("os.system"),
            "injected code must not reach the program at all"
        );
        assert!(
            program.contains("sys.argv[1]"),
            "the program must read the target from argv"
        );
        assert_eq!(
            args.last().map(String::as_str),
            Some(payload),
            "the payload must arrive verbatim as the argument that lands in \
             sys.argv[1], got {args:?}"
        );

        // Whatever the payload is, the program is the same constant program.
        let other = build_command("some/other/target.py", true)
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            other.get(2),
            Some(program),
            "the program must not vary with the target"
        );
    }

    /// A target that reassigns the bootstrap's own global must not be able to
    /// disable tracing.
    ///
    /// `_chronos_trace` returns the global of that name on every event, so a
    /// target sharing the bootstrap namespace turns the tracer into a string
    /// and tracing dies with `TypeError: 'str' object is not callable`. The
    /// private `__main__` namespace stops the collision.
    #[tokio::test]
    async fn test_target_cannot_hijack_the_tracer_through_a_shared_namespace() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        write!(
            file,
            "_chronos_trace = 'hijacked by the target script'\n\
             def foo():\n    return 1\n\
             foo()\n"
        )
        .unwrap();
        file.flush().unwrap();

        let mut proc = PythonSubprocess::spawn(file.path().to_str().unwrap(), true).unwrap();

        let mut traced_foo = false;
        for _ in 0..100 {
            match proc.next_event().await {
                Ok(Some(event)) if event.name == "foo" && event.event == "call" => {
                    traced_foo = true;
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => panic!("reading a trace event should not fail: {e}"),
            }
        }

        assert!(
            traced_foo,
            "the target must not be able to disable tracing by assigning \
             _chronos_trace: it runs in its own __main__ namespace"
        );
    }

    /// The trace contract for a legitimate target is unchanged: `event`,
    /// `name`, `file`, `line`, `is_generator` and `locals` are all still
    /// emitted, and `CHRONOS_CAPTURE_LOCALS=0` still suppresses `locals`.
    #[tokio::test]
    async fn test_legitimate_target_still_emits_the_full_event_shape() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        write!(
            file,
            "def foo(value):\n    secret = value * 2\n    return secret\nfoo(21)\n"
        )
        .unwrap();
        file.flush().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let mut proc = PythonSubprocess::spawn(&path, true).unwrap();
        let mut events = Vec::new();
        for _ in 0..200 {
            match proc.next_event().await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(e) => panic!("reading a trace event should not fail: {e}"),
            }
        }

        let call = events
            .iter()
            .find(|e| e.name == "foo" && e.event == "call")
            .unwrap_or_else(|| {
                panic!("expected a `call` event for foo, got {events:#?}");
            });

        assert_eq!(call.file, path, "`file` must be the traced script");
        assert!(call.line > 0, "`line` must be populated");
        assert_eq!(call.is_generator, Some(false), "foo is not a generator");

        // `call` is the only event the tracer decorates with `locals`, and it
        // fires once the arguments are bound, so `value` is observable there.
        let call_locs = call
            .locals
            .as_ref()
            .unwrap_or_else(|| panic!("`locals` must be captured: {call:#?}"));
        assert!(
            call_locs.contains_key("value"),
            "`locals` at call time must hold the bound argument, got {call_locs:?}"
        );

        let ret = events
            .iter()
            .find(|e| e.name == "foo" && e.event == "return")
            .unwrap_or_else(|| {
                panic!("expected a `return` event for foo, got {events:#?}");
            });
        assert_eq!(ret.file, path);
        assert!(ret.line > 0);
        assert_eq!(ret.is_generator, Some(false));
        assert_eq!(
            ret.locals, None,
            "only `call` events carry locals; that is unchanged"
        );

        // Same script, locals disabled: the rest of the contract is intact.
        let mut proc = PythonSubprocess::spawn(&path, false).unwrap();
        let mut events = Vec::new();
        for _ in 0..200 {
            match proc.next_event().await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(e) => panic!("reading a trace event should not fail: {e}"),
            }
        }
        let call = events
            .iter()
            .find(|e| e.name == "foo" && e.event == "call")
            .unwrap_or_else(|| {
                panic!("expected a `call` event for foo, got {events:#?}");
            });
        assert_eq!(call.file, path);
        assert_eq!(
            call.locals, None,
            "CHRONOS_CAPTURE_LOCALS=0 must still suppress `locals`"
        );
    }

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
