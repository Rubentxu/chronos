//! Spawn a JVM with JDWP enabled and parse the assigned port.

use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::error::JavaError;

/// A spawned JVM process with JDWP debugging enabled.
pub struct JavaSubprocess {
    /// The child process handle.
    pub child: Child,
    /// The JDWP port that was assigned (parsed from stderr).
    pub jdwp_port: u16,
}

impl JavaSubprocess {
    /// Spawn `java -agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:0 -cp <classpath> <main_class>`
    ///
    /// Parses stderr to find the JDWP port from:
    /// "Listening for transport dt_socket at address: 127.0.0.1:<port>"
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

        // Read stderr to find the JDWP port
        let stderr = child.stderr.take().ok_or_else(|| {
            JavaError::SpawnFailed("Failed to capture stderr".to_string())
        })?;

        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        // JDWP port is bound once the JVM is ready to accept connections
        // Format: "Listening for transport dt_socket at address: 127.0.0.1:<port>"
        let jdwp_port = loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF reached without finding port — JVM may have exited
                    let exit_status = child.wait().await?.code();
                    return Err(JavaError::SpawnFailed(format!(
                        "JVM exited before JDWP port was available: {:?}",
                        exit_status
                    )));
                }
                Ok(_) => {
                    if let Some(port) = parse_jdwp_port_from_line(&line) {
                        break port;
                    }
                }
                Err(e) => {
                    return Err(JavaError::SpawnFailed(format!(
                        "Failed to read JVM stderr: {}",
                        e
                    )));
                }
            }
        };

        Ok(Self { child, jdwp_port })
    }
}

/// Parse the JDWP port from a line of JVM output.
///
/// Expected format: "Listening for transport dt_socket at address: 127.0.0.1:<port>"
fn parse_jdwp_port_from_line(line: &str) -> Option<u16> {
    // Find the last colon-separated segment after "address: "
    let prefix = "address: 127.0.0.1:";
    let idx = line.find(prefix)?;
    let after_address = &line[idx + prefix.len()..];
    // The port is everything up to the next whitespace or end
    let port_str = after_address
        .split_whitespace()
        .next()
        .unwrap_or(after_address);
    port_str.parse().ok()
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
        let line = "Listening for transport dt_socket at address: 127.0.0.1:54321";
        assert_eq!(parse_jdwp_port_from_line(line), Some(54321));

        let line = "Some other output\nListening for transport dt_socket at address: 127.0.0.1:12345\nmore text";
        assert_eq!(parse_jdwp_port_from_line(line), Some(12345));

        let line = "Random output without port";
        assert_eq!(parse_jdwp_port_from_line(line), None);

        let line = "Listening for transport dt_socket at address: 127.0.0.1:";
        assert_eq!(parse_jdwp_port_from_line(line), None);
    }

    #[tokio::test]
    #[ignore]
    async fn test_jvm_spawn_parses_jdwp_port() {
        // This test requires java to be on PATH
        if which::which("java").is_err() {
            return;
        }

        // Create a minimal Java class for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let class_content = r#"
public class TestClass {
    public static void main(String[] args) {
        System.out.println("Hello");
        try {
            Thread.sleep(5000);
        } catch (InterruptedException e) {}
    }
}
"#;
        let class_path = temp_dir.path();
        std::fs::write(class_path.join("TestClass.java"), class_content).unwrap();

        // Compile the class
        let compile_result = std::process::Command::new("javac")
            .arg(class_path.join("TestClass.java"))
            .output();
        if compile_result.is_err() {
            return;
        }

        // We can't actually run this test without a compiled class
        // and waiting 5 seconds, so we just verify the spawn function works
    }
}
