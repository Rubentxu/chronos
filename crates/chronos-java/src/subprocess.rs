//! Spawn a JVM with JDWP enabled and parse the assigned port.

use crate::error::JavaError;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

/// A spawned JVM process with JDWP debugging enabled.
pub struct JavaSubprocess {
    /// The child process handle.
    pub child: Child,
    /// The JDWP port that was assigned (parsed from the JVM output).
    pub jdwp_port: u16,
}

impl JavaSubprocess {
    /// Spawn `java -agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:0 -cp <classpath> <main_class>`
    ///
    /// Parses the JVM output to find the JDWP port, in either the legacy
    /// "address: 127.0.0.1:<port>" form or the modern "address: <port>" one.
    /// See `parse_jdwp_port_from_line`.
    ///
    /// The target can be a `.jar` file or a `ClassName`.
    pub async fn spawn(target: &str) -> Result<Self, JavaError> {
        let mut cmd = Command::new("java");

        // Determine if target is a JAR file or a class name
        let is_jar = target.ends_with(".jar");

        // JDWP agent options: suspend=y (wait for debugger), address=127.0.0.1:0 (ephemeral port)
        cmd.arg("-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:0");

        if is_jar {
            cmd.arg("-jar").arg(target);
        } else {
            cmd.arg("-cp").arg(".").arg(target);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JavaError::JavaNotFound
            } else {
                JavaError::SpawnFailed(e.to_string())
            }
        })?;

        // Read the JVM's output looking for its JDWP port.
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| JavaError::SpawnFailed("Failed to capture stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| JavaError::SpawnFailed("Failed to capture stderr".to_string()))?;

        // Bounded on purpose: see `JDWP_PORT_TIMEOUT`.
        let jdwp_port = match tokio::time::timeout(
            JDWP_PORT_TIMEOUT,
            read_jdwp_port(
                Some(BufReader::new(stdout)),
                Some(BufReader::new(stderr)),
                &mut child,
            ),
        )
        .await
        {
            Ok(Ok(port)) => port,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(JavaError::SpawnFailed(format!(
                    "JVM did not announce a recognizable JDWP port within {:?}; \
                     its banner may have moved to another stream or changed format",
                    JDWP_PORT_TIMEOUT
                )));
            }
        };

        Ok(Self { child, jdwp_port })
    }
}

/// How long to wait for the JVM to announce its JDWP port.
///
/// The wait is bounded on purpose. With `suspend=y` the JVM stays alive waiting
/// for a debugger, so a banner this code fails to recognise does not end in a
/// clean EOF: the read simply never returns and the caller blocks forever.
/// `JavaAdapter::attach` does exactly that, inside a `block_on`, so the adapter
/// stopped responding entirely. A bounded wait turns that silent hang into an
/// error the caller can see and report.
const JDWP_PORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Read the JVM's output until it announces its JDWP port.
///
/// Both streams are watched. Which one carries the banner depends on the JVM:
/// temurin 24.0.2 writes it to stdout, while other JDKs write it to stderr.
/// Reading only stderr — as this function used to — meant watching an empty
/// stream on a modern JVM, which hangs for the reason described on
/// `JDWP_PORT_TIMEOUT`. Whichever stream speaks first wins; a stream that ends
/// stops competing so the other one can still deliver.
///
/// Returns the port, or an error if the JVM exits first. Callers are expected
/// to bound this with `JDWP_PORT_TIMEOUT`, because neither branch covers a JVM
/// that stays alive and silent forever.
async fn read_jdwp_port(
    mut stdout: Option<BufReader<ChildStdout>>,
    mut stderr: Option<BufReader<ChildStderr>>,
    child: &mut Child,
) -> Result<u16, JavaError> {
    let mut out_line = String::new();
    let mut err_line = String::new();

    loop {
        if stdout.is_none() && stderr.is_none() {
            // Both streams ended without a port — the JVM exited on us.
            let exit_status = child.wait().await?.code();
            return Err(JavaError::SpawnFailed(format!(
                "JVM exited before JDWP port was available: {:?}",
                exit_status
            )));
        }

        // A closed stream yields `pending()` instead of an immediately-ready
        // `None`, so `select!` parks on the stream that is still open rather
        // than spinning on the exhausted one.
        let from_stdout = async {
            match stdout.as_mut() {
                Some(reader) => Some(reader.read_line(&mut out_line).await),
                None => std::future::pending().await,
            }
        };
        let from_stderr = async {
            match stderr.as_mut() {
                Some(reader) => Some(reader.read_line(&mut err_line).await),
                None => std::future::pending().await,
            }
        };

        let (was_stdout, line) = tokio::select! {
            line = from_stdout => (true, line),
            line = from_stderr => (false, line),
        };

        match line {
            Some(Ok(0)) => {
                if was_stdout {
                    stdout = None;
                } else {
                    stderr = None;
                }
            }
            Some(Ok(_)) => {
                let parsed = if was_stdout {
                    parse_jdwp_port_from_line(&out_line)
                } else {
                    parse_jdwp_port_from_line(&err_line)
                };
                if let Some(port) = parsed {
                    return Ok(port);
                }
            }
            Some(Err(e)) => {
                return Err(JavaError::SpawnFailed(format!(
                    "Failed to read JVM output: {}",
                    e
                )));
            }
            None => unreachable!("a pending stream cannot complete a select! arm"),
        }
    }
}

/// Parse the JDWP port from a line of JVM output.
///
/// The JVM announces the bound port in two shapes and both occur in the wild:
///
/// * `Listening for transport dt_socket at address: 127.0.0.1:54321`
///   — the legacy form, which carries the host prefix.
/// * `Listening for transport dt_socket at address: 54321`
///   — the modern form, which dropped it.
///
/// Verified empirically against temurin 24.0.2, which emits the second one.
/// Matching only the legacy form left the port unparsed on every supported
/// JDK, and the unit test below used to assert only the legacy shape, which is
/// why the suite stayed green while the production path hung.
fn parse_jdwp_port_from_line(line: &str) -> Option<u16> {
    const PREFIX: &str = "at address:";
    let idx = line.find(PREFIX)?;
    let after_address = line[idx + PREFIX.len()..].trim_start();
    // The port is everything up to the next whitespace or end of line.
    let candidate = after_address
        .split_whitespace()
        .next()
        .unwrap_or(after_address);
    // Strip the optional host prefix, keeping whatever follows the last colon.
    let digits = candidate.rsplit_once(':').map_or(candidate, |(_, d)| d);
    digits.parse().ok()
}

impl Drop for JavaSubprocess {
    fn drop(&mut self) {
        // SIGTERM is sent automatically when Child is dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jdwp_port_from_line() {
        // Legacy form: host prefix present.
        let line = "Listening for transport dt_socket at address: 127.0.0.1:54321";
        assert_eq!(parse_jdwp_port_from_line(line), Some(54321));

        // Modern form: no host prefix. This is what temurin 24.0.2 emits, and
        // it is the case the parser used to miss, hanging `spawn` forever.
        let line = "Listening for transport dt_socket at address: 56375";
        assert_eq!(parse_jdwp_port_from_line(line), Some(56375));

        let line =
            "Some other output\nListening for transport dt_socket at address: 127.0.0.1:12345\nmore text";
        assert_eq!(parse_jdwp_port_from_line(line), Some(12345));

        let line =
            "Some other output\nListening for transport dt_socket at address: 12345\nmore text";
        assert_eq!(parse_jdwp_port_from_line(line), Some(12345));

        let line = "Random output without port";
        assert_eq!(parse_jdwp_port_from_line(line), None);

        // Truncated banners must not panic or yield a bogus port.
        let line = "Listening for transport dt_socket at address: 127.0.0.1:";
        assert_eq!(parse_jdwp_port_from_line(line), None);

        let line = "Listening for transport dt_socket at address:";
        assert_eq!(parse_jdwp_port_from_line(line), None);
    }

    /// Feeds canned JVM output through the real read loop, without needing a
    /// JVM installed. Unlike the `spawn` test these run in the default gate.
    ///
    /// Both streams are piped exactly as `spawn` does, so each test decides
    /// which one carries the banner. The shell is safe here: CI is ubuntu-only
    /// (`.github/workflows/ci.yml`).
    async fn read_over(script: &str) -> Result<u16, JavaError> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sh should start");
        let stdout = child.stdout.take().expect("stdout captured");
        let stderr = child.stderr.take().expect("stderr captured");
        read_jdwp_port(
            Some(BufReader::new(stdout)),
            Some(BufReader::new(stderr)),
            &mut child,
        )
        .await
    }

    /// The regression that mattered most: temurin 24.0.2 writes the banner to
    /// **stdout**, and this code used to read stderr only, so it watched an
    /// empty stream and hung until the timeout.
    #[tokio::test]
    async fn read_loop_accepts_the_modern_banner_on_stdout() {
        let port = read_over("echo 'Listening for transport dt_socket at address: 56375'; sleep 5")
            .await
            .expect("the stdout banner must yield its port");
        assert_eq!(port, 56375);
    }

    /// The other stream must keep working, so the fix does not trade one JDK's
    /// behaviour for another's.
    #[tokio::test]
    async fn read_loop_accepts_the_legacy_banner_on_stderr() {
        let port = read_over(
            "echo 'Listening for transport dt_socket at address: 127.0.0.1:41234' >&2; sleep 5",
        )
        .await
        .expect("the stderr banner must yield its port");
        assert_eq!(port, 41234);
    }

    /// The banner is not always the first line; the loop has to keep reading.
    #[tokio::test]
    async fn read_loop_skips_noise_before_the_banner() {
        let port = read_over(
            "echo 'Picked up JAVA_TOOL_OPTIONS: -Xmx1g'; \
             echo 'Listening for transport dt_socket at address: 41234'; sleep 5",
        )
        .await
        .expect("the loop must skip noise until the banner");
        assert_eq!(port, 41234);
    }

    /// A process that exits without ever announcing a port must produce an
    /// error, not a hang and not a bogus port.
    #[tokio::test]
    async fn read_loop_errors_when_the_jvm_exits_silently() {
        let err = read_over("echo 'Error: Could not find or load main class' >&2; exit 1")
            .await
            .expect_err("a process dying without a banner must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("exited before JDWP port was available"),
            "unexpected message: {msg}"
        );
    }

    /// Callers bound this with `JDWP_PORT_TIMEOUT`; this pins that contract so
    /// the bound is exercised at least once. A live, silent process must yield
    /// the timeout rather than hanging the test.
    #[tokio::test]
    async fn read_loop_is_bounded_against_a_silent_jvm() {
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            // Kept short so the helper process cleans itself up promptly.
            read_over("sleep 2"),
        )
        .await;

        assert!(
            result.is_err(),
            "a live silent process must exhaust the timeout, not return Ok"
        );
    }

    /// Verifies the real contract of `JavaSubprocess::spawn`: that the JVM
    /// publishes a reachable JDWP port.
    ///
    /// The target is intentionally missing. With `suspend=y` the JVM announces
    /// the port *before* trying to load the program and then stays suspended
    /// waiting for a debugger, which is exactly the contract the adapter needs
    /// (obtain a port, not run the target) and keeps the test independent of
    /// `javac` and of a real classpath.
    ///
    /// This test used to compile a class and then assert nothing: its body
    /// ended on "so we just verify the spawn function works" with no
    /// expectation. It covered nothing.
    #[tokio::test]
    #[ignore = "requires java on PATH; run by the gate's opt-in tier"]
    async fn test_jvm_spawn_publishes_a_reachable_jdwp_port() {
        if which::which("java").is_err() {
            // Declared skip: this test can assert nothing without java, and it
            // must not quietly become a green.
            eprintln!("DECLARED SKIP: java not on PATH, nothing to assert");
            return;
        }

        let sub = JavaSubprocess::spawn("NoSuchClassParaChronos")
            .await
            .expect("the JVM should publish a JDWP port even when the target is missing");

        assert!(
            sub.jdwp_port > 0,
            "the JDWP port must be a positive integer, got {:?}",
            sub.jdwp_port
        );

        // Parsed is not the same as listening. If the JVM announced a port
        // nobody then opened, this test has to fail: that is the difference
        // between "printed a number" and "I can attach".
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], sub.jdwp_port));
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("JDWP port {addr} should accept a connection: {e}"));
    }
}
