//! Manifest system for scenario definition and scoring.
//!
//! Provides YAML-based scenario definitions and scoring.

pub mod loader;
pub mod runner;
pub mod scoring;

use serde::{Deserialize, Serialize};

/// Language target in a scenario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LanguageTarget {
    Rust,
    Java,
    Go,
    Python,
    NodeJs,
    C,
}

/// A scenario manifest defining a test scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioManifest {
    /// Scenario name.
    pub name: String,
    /// Scenario description.
    pub description: String,
    /// Language target.
    pub language: LanguageTarget,
    /// Fixture path (directory with the program to debug).
    pub fixture: String,
    /// Debug port for the target.
    pub debug_port: u16,
    /// Environment variables.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_sec: u64,
    /// KPIs to collect.
    #[serde(default)]
    pub kpis: Vec<String>,
}

fn default_timeout() -> u64 {
    300
}

/// Loads a scenario manifest from a YAML file.
pub fn load_manifest(path: &std::path::Path) -> Result<ScenarioManifest, crate::error::SandboxError> {
    let content = std::fs::read_to_string(path)?;
    let manifest: ScenarioManifest = serde_yaml::from_str(&content)?;
    Ok(manifest)
}

/// Saves a scenario manifest to a YAML file.
pub fn save_manifest(
    manifest: &ScenarioManifest,
    path: &std::path::Path,
) -> Result<(), crate::error::SandboxError> {
    let content = serde_yaml::to_string(manifest)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_manifest_serde() {
        let manifest = ScenarioManifest {
            name: "test-scenario".to_string(),
            description: "A test scenario".to_string(),
            language: LanguageTarget::Rust,
            fixture: "/fixtures/rust-hello".to_string(),
            debug_port: 2345,
            env: std::collections::HashMap::new(),
            timeout_sec: 300,
            kpis: vec!["cpu".to_string(), "memory".to_string()],
        };

        let yaml = serde_yaml::to_string(&manifest).unwrap();
        let parsed: ScenarioManifest = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.name, manifest.name);
        assert_eq!(parsed.language, manifest.language);
    }
}
