//! Pod management for chronos-sandbox.
//!
//! Manages Podman pods for grouping related containers.

use crate::error::SandboxError;
use std::process::Stdio;
use tokio::process::Command;

/// Pod manager for sandbox pods.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PodManager {
    default_pod_name: String,
}

impl Default for PodManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PodManager {
    /// Creates a new PodManager with the default pod name.
    pub fn new() -> Self {
        Self {
            default_pod_name: "chronos-pod".to_string(),
        }
    }

    /// Creates a new Pod.
    pub async fn create(&self, name: &str) -> Result<String, SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "create", "--name", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod create: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod create failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Starts a pod.
    pub async fn start(&self, name: &str) -> Result<(), SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "start", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod start: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod start failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Stops a pod.
    pub async fn stop(&self, name: &str) -> Result<(), SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "stop", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod stop: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod stop failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Removes a pod.
    pub async fn rm(&self, name: &str) -> Result<(), SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "rm", "-f", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod rm: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod rm failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Lists all pods.
    pub async fn list(&self) -> Result<Vec<String>, SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "ls", "--format", "{{.Name}}"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod ls: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod ls failed: {}",
                stderr
            )));
        }

        let pods = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(pods)
    }

    /// Inspects a pod and returns its status.
    pub async fn inspect_status(&self, name: &str) -> Result<String, SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "inspect", "--format", "{{.State}}", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod inspect: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod inspect failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Lists all containers in a pod.
    pub async fn ps(&self, name: &str) -> Result<Vec<String>, SandboxError> {
        let output = Command::new("podman")
            .args(["pod", "ps", "--format", "{{.Containers}}", "--pod", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::PodError(format!("failed to execute podman pod ps: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::PodError(format!(
                "podman pod ps failed: {}",
                stderr
            )));
        }

        let containers = String::from_utf8_lossy(&output.stdout)
            .lines()
            .flat_map(|line| {
                line.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .collect();

        Ok(containers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires podman to be installed
    async fn test_pod_operations() {
        let manager = PodManager::new();
        let result = manager.list().await;
        println!("Pod list result: {:?}", result);
    }
}
