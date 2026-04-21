//! Network management for chronos-sandbox.
//!
//! Manages Podman networks for sandbox isolation.

use crate::error::SandboxError;
use std::process::Stdio;
use tokio::process::Command;

/// Represents a sandbox network.
#[derive(Debug, Clone)]
pub struct SandboxNetwork {
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
}

/// Creates a new Podman network for sandbox isolation.
///
/// The network is named `chronos-network` by default and uses the bridge driver.
pub async fn create_network(name: &str) -> Result<SandboxNetwork, SandboxError> {
    let output = Command::new("podman")
        .args(["network", "create", "--driver", "bridge", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| SandboxError::NetworkError(format!("failed to execute podman network create: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SandboxError::NetworkError(format!(
            "podman network create failed: {}",
            stderr
        )));
    }

    Ok(SandboxNetwork {
        name: name.to_string(),
        driver: "bridge".to_string(),
        subnet: None,
    })
}

/// Removes a Podman network.
pub async fn remove_network(name: &str) -> Result<(), SandboxError> {
    let output = Command::new("podman")
        .args(["network", "rm", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| SandboxError::NetworkError(format!("failed to execute podman network rm: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SandboxError::NetworkError(format!(
            "podman network rm failed: {}",
            stderr
        )));
    }

    Ok(())
}

/// Network manager for sandbox networks.
#[derive(Debug, Clone)]
pub struct NetworkManager {
    default_network_name: String,
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkManager {
    /// Creates a new NetworkManager with the default network name.
    pub fn new() -> Self {
        Self {
            default_network_name: "chronos-network".to_string(),
        }
    }

    /// Creates the default sandbox network.
    pub async fn create_default(&self) -> Result<SandboxNetwork, SandboxError> {
        create_network(&self.default_network_name).await
    }

    /// Removes the default sandbox network.
    pub async fn remove_default(&self) -> Result<(), SandboxError> {
        remove_network(&self.default_network_name).await
    }

    /// Lists all Podman networks.
    pub async fn list(&self) -> Result<Vec<String>, SandboxError> {
        let output = Command::new("podman")
            .args(["network", "ls", "--format", "{{.Name}}"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::NetworkError(format!("failed to execute podman network ls: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::NetworkError(format!(
                "podman network ls failed: {}",
                stderr
            )));
        }

        let networks = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(networks)
    }

    /// Inspects a network and returns its details.
    pub async fn inspect(&self, name: &str) -> Result<SandboxNetwork, SandboxError> {
        let output = Command::new("podman")
            .args(["network", "inspect", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SandboxError::NetworkError(format!("failed to execute podman network inspect: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxError::NetworkError(format!(
                "podman network inspect failed: {}",
                stderr
            )));
        }

        // Parse the JSON output to extract network info
        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| SandboxError::NetworkError(format!("failed to parse network inspect output: {}", e)))?;

        let driver = json[0]["Driver"]
            .as_str()
            .unwrap_or("bridge")
            .to_string();

        let subnet = json[0]["IPAM"]["Config"]
            .as_array()
            .and_then(|configs| configs.first())
            .and_then(|config| config["Subnet"].as_str())
            .map(|s| s.to_string());

        Ok(SandboxNetwork {
            name: name.to_string(),
            driver,
            subnet,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_network_default() {
        let network = SandboxNetwork {
            name: "test".to_string(),
            driver: "bridge".to_string(),
            subnet: Some("10.89.0.0/16".to_string()),
        };
        assert_eq!(network.driver, "bridge");
    }

    #[tokio::test]
    #[ignore] // Requires podman to be installed
    async fn test_create_remove_network() {
        let manager = NetworkManager::new();
        let result = manager.create_default().await;
        // This test is just a smoke test
        println!("Network creation result: {:?}", result);
    }
}
