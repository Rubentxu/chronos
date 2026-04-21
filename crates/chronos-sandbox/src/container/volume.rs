//! Volume management for chronos-sandbox.
//!
//! Manages Podman volumes for persistent data in sandbox environments.

use crate::error::SandboxError;
use std::process::Stdio;
use tokio::process::Command;

/// Represents a sandbox volume.
#[derive(Debug, Clone)]
pub struct SandboxVolume {
    pub name: String,
    pub driver: String,
}

/// Creates a new Podman volume for sandbox use.
pub async fn create_volume(name: &str) -> Result<SandboxVolume, SandboxError> {
    let output = Command::new("podman")
        .args(["volume", "create", "--driver", "local", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| SandboxError::VolumeError(format!("failed to execute podman volume create: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SandboxError::VolumeError(format!(
            "podman volume create failed: {}",
            stderr
        )));
    }

    Ok(SandboxVolume {
        name: name.to_string(),
        driver: "local".to_string(),
    })
}

/// Removes a Podman volume.
pub async fn remove_volume(name: &str) -> Result<(), SandboxError> {
    let output = Command::new("podman")
        .args(["volume", "rm", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| SandboxError::VolumeError(format!("failed to execute podman volume rm: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SandboxError::VolumeError(format!(
            "podman volume rm failed: {}",
            stderr
        )));
    }

    Ok(())
}

/// Volume manager for sandbox volumes.
#[derive(Debug, Clone)]
pub struct VolumeManager {
    default_volume_name: String,
}

impl Default for VolumeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl VolumeManager {
    /// Creates a new VolumeManager with the default volume name.
    pub fn new() -> Self {
        Self {
            default_volume_name: "chronos-data".to_string(),
        }
    }

    /// Creates the default sandbox volume.
    pub async fn create_default(&self) -> Result<SandboxVolume, SandboxError> {
        create_volume(&self.default_volume_name).await
    }

    /// Removes the default sandbox volume.
    pub async fn remove_default(&self) -> Result<(), SandboxError> {
        remove_volume(&self.default_volume_name).await
    }

    /// Lists all Podman volumes.
    pub async fn list(&self) -> Result<Vec<String>, SandboxError> {
        let output = Command::new("podman")
            .args(["volume", "ls", "--format", "{{.Name}}"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::VolumeError(format!("failed to execute podman volume ls: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::VolumeError(format!(
                "podman volume ls failed: {}",
                stderr
            )));
        }

        let volumes = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(volumes)
    }

    /// Inspects a volume and returns its details.
    pub async fn inspect(&self, name: &str) -> Result<SandboxVolume, SandboxError> {
        let output = Command::new("podman")
            .args(["volume", "inspect", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::VolumeError(format!("failed to execute podman volume inspect: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::VolumeError(format!(
                "podman volume inspect failed: {}",
                stderr
            )));
        }

        // Parse the JSON output to extract volume info
        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| SandboxError::VolumeError(format!("failed to parse volume inspect output: {}", e)))?;

        let driver = json[0]["Driver"]
            .as_str()
            .unwrap_or("local")
            .to_string();

        Ok(SandboxVolume {
            name: name.to_string(),
            driver,
        })
    }

    /// Gets the mount point of a volume on the host.
    pub async fn mount_point(&self, name: &str) -> Result<String, SandboxError> {
        let output = Command::new("podman")
            .args(["volume", "inspect", "--format", "{{.Mountpoint}}", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::VolumeError(format!("failed to execute podman volume inspect: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::VolumeError(format!(
                "podman volume inspect failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_volume_default() {
        let volume = SandboxVolume {
            name: "test".to_string(),
            driver: "local".to_string(),
        };
        assert_eq!(volume.driver, "local");
    }

    #[tokio::test]
    #[ignore] // Requires podman to be installed
    async fn test_create_remove_volume() {
        let manager = VolumeManager::new();
        let result = manager.create_default().await;
        // This test is just a smoke test
        println!("Volume creation result: {:?}", result);
    }
}
