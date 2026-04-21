//! Container runner for managing Podman containers.
//!
//! Uses `podman` CLI via `std::process::Command` for container operations.
//! Testcontainers is used for development and testing where applicable.

use crate::container::quadlet::ContainerOptions;
use crate::error::SandboxError;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// A running container instance.
#[derive(Debug, Clone)]
pub struct RunningContainer {
    pub id: String,
    pub name: String,
    pub mapped_ports: HashMap<u16, u16>,
}

/// Manages container lifecycle using Podman CLI.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ContainerRunner {
    /// Default timeout for container operations.
    default_timeout: Duration,
}

impl ContainerRunner {
    /// Creates a new ContainerRunner with default settings.
    pub fn new() -> Self {
        Self {
            default_timeout: Duration::from_secs(60),
        }
    }

    /// Creates a new ContainerRunner with a custom default timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            default_timeout: timeout,
        }
    }

    /// Starts a container from the given options.
    ///
    /// Uses `podman run` to create and start the container.
    pub async fn start(&self, opts: &ContainerOptions) -> Result<RunningContainer, SandboxError> {
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(), // Detached mode
            "--name".to_string(),
            opts.name.clone(),
        ];

        // Add capabilities
        for cap in &opts.caps {
            args.push("--cap-add".to_string());
            args.push(cap.clone());
        }

        // Add security options
        for opt in &opts.security_opt {
            args.push("--security-opt".to_string());
            args.push(opt.clone());
        }

        // Add port mappings
        for port in &opts.ports {
            args.push("-p".to_string());
            args.push(format!("{}:{}/{}", port.host, port.container, port.protocol));
        }

        // Add volume mappings
        for vol in &opts.volumes {
            args.push("-v".to_string());
            args.push(format!(
                "{}:{}:{}",
                vol.host_path.display(),
                vol.container_path.display(),
                vol.options
            ));
        }

        // Add environment variables
        for (key, value) in &opts.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add network
        if let Some(ref network) = opts.network {
            args.push("--network".to_string());
            args.push(network.clone());
        }

        // Add pod
        if let Some(ref pod) = opts.pod {
            args.push("--pod".to_string());
            args.push(pod.clone());
        }

        // Add user
        if let Some(ref user) = opts.user {
            args.push("-u".to_string());
            args.push(user.clone());
        }

        // Add workdir
        if let Some(ref workdir) = opts.workdir {
            args.push("-w".to_string());
            args.push(workdir.clone());
        }

        // Add image
        args.push(opts.image.clone());

        // Add command
        if !opts.command.is_empty() {
            args.extend(opts.command.clone());
        }

        // Execute podman run
        let output = Command::new("podman")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::ContainerStartFailed(format!(
                "podman run failed: {}",
                stderr
            )));
        }

        let container_id = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        // Build port mappings
        let mut mapped_ports = HashMap::new();
        for port in &opts.ports {
            mapped_ports.insert(port.container, port.host);
        }

        Ok(RunningContainer {
            id: container_id,
            name: opts.name.clone(),
            mapped_ports,
        })
    }

    /// Stops a running container.
    pub async fn stop(&self, id: &str) -> Result<(), SandboxError> {
        let output = Command::new("podman")
            .args(["stop", id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman stop: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::ContainerStartFailed(format!(
                "podman stop failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Removes a container.
    pub async fn rm(&self, id: &str) -> Result<(), SandboxError> {
        let output = Command::new("podman")
            .args(["rm", "-f", id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman rm: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::ContainerStartFailed(format!(
                "podman rm failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Waits for a container to be ready by checking if the port is accepting connections.
    pub async fn wait_ready(
        &self,
        id: &str,
        port: u16,
        timeout_duration: Duration,
    ) -> Result<(), SandboxError> {
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout_duration {
                return Err(SandboxError::ContainerReadyTimeout {
                    port,
                    message: format!(
                        "container {} did not become ready within {:?}",
                        id, timeout_duration
                    ),
                });
            }

            // Check if container is still running
            let status = self.inspect_status(id).await.ok();
            if status.as_deref() != Some("running") {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Try to connect to the port using nc (netcat)
            let result = tokio::time::timeout(
                Duration::from_secs(1),
                self.check_port_available(id, port),
            )
            .await;

            if result.is_ok() {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Checks if a port is available/accepting connections on localhost.
    async fn check_port_available(&self, container_name: &str, port: u16) -> Result<(), SandboxError> {
        // Use podman port to get the actual mapped port
        let output = Command::new("podman")
            .args(["port", container_name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman port: {}", e)))?;

        if !output.status.success() {
            // Try direct connection
            let child = Command::new("sh")
                .args([
                    "-c",
                    &format!("nc -z localhost {} > /dev/null 2>&1", port),
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if let Ok(mut c) = child {
                let status = c.wait().await;
                if status.map(|s| s.success()).unwrap_or(false) {
                    return Ok(());
                }
            }
            return Err(SandboxError::ContainerReadyTimeout {
                port,
                message: "port check failed".to_string(),
            });
        }

        Ok(())
    }

    /// Inspects a container and returns its status.
    pub async fn inspect_status(&self, id: &str) -> Result<String, SandboxError> {
        let output = Command::new("podman")
            .args(["inspect", "--format", "{{.State.Status}}", id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman inspect: {}", e)))?;

        if !output.status.success() {
            return Err(SandboxError::ContainerNotFound(id.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Gets the logs of a container.
    pub async fn logs(&self, id: &str) -> Result<String, SandboxError> {
        let output = Command::new("podman")
            .args(["logs", id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman logs: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Executes a command in a running container.
    pub async fn exec(
        &self,
        id: &str,
        args: &[&str],
    ) -> Result<std::process::Output, SandboxError> {
        let mut cmd_args = vec!["exec".to_string(), id.to_string()];
        cmd_args.extend(args.iter().map(|s| s.to_string()));

        let output = Command::new("podman")
            .args(&cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::ContainerStartFailed(format!("failed to execute podman exec: {}", e)))?;

        Ok(output)
    }
}

impl Default for ContainerRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Wait for container to be ready on a port with a default timeout.
///
/// This is a convenience function that creates a default runner and waits.
pub async fn wait_ready(id: &str, port: u16, timeout: Duration) -> Result<(), SandboxError> {
    let runner = ContainerRunner::new();
    runner.wait_ready(id, port, timeout).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_running_container_default() {
        let container = RunningContainer {
            id: "abc123".to_string(),
            name: "test".to_string(),
            mapped_ports: HashMap::new(),
        };
        assert_eq!(container.id, "abc123");
    }

    #[tokio::test]
    #[ignore] // Requires podman to be installed
    async fn test_podman_available() {
        let output = Command::new("podman")
            .args(["--version"])
            .output()
            .await;
        assert!(output.is_ok());
    }
}
