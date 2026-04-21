//! Scenario runner for executing sandbox scenarios.

use crate::error::SandboxError;
use crate::manifest::ScenarioManifest;

/// Result of a scenario run.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Scenario name.
    pub name: String,
    /// Whether the scenario passed.
    pub passed: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Duration in seconds.
    pub duration_sec: f64,
}

impl ScenarioResult {
    /// Creates a successful result.
    pub fn success(name: String, duration_sec: f64) -> Self {
        Self {
            name,
            passed: true,
            error: None,
            duration_sec,
        }
    }

    /// Creates a failed result.
    pub fn failure(name: String, error: String, duration_sec: f64) -> Self {
        Self {
            name,
            passed: false,
            error: Some(error),
            duration_sec,
        }
    }
}

/// Scenario runner that executes scenarios.
#[derive(Debug, Clone, Default)]
pub struct ScenarioRunner {
    /// Registered scenarios.
    scenarios: std::collections::HashMap<String, ScenarioManifest>,
}

impl ScenarioRunner {
    /// Creates a new ScenarioRunner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a scenario.
    pub fn register(&mut self, manifest: ScenarioManifest) {
        self.scenarios.insert(manifest.name.clone(), manifest);
    }

    /// Runs a scenario by name.
    pub async fn run(&self, name: &str) -> Result<ScenarioResult, SandboxError> {
        let manifest = self
            .scenarios
            .get(name)
            .ok_or_else(|| SandboxError::ScenarioFailed(format!("scenario not found: {}", name)))?;

        self.run_manifest(manifest).await
    }

    /// Runs a scenario from a manifest.
    pub async fn run_manifest(&self, manifest: &ScenarioManifest) -> Result<ScenarioResult, SandboxError> {
        let start = std::time::Instant::now();

        // Stub implementation - real implementation would:
        // 1. Start the target container
        // 2. Attach the debugger
        // 3. Execute workload
        // 4. Collect KPIs
        // 5. Score the results

        tracing::info!(
            "Running scenario: {} (fixture: {}, port: {})",
            manifest.name,
            manifest.fixture,
            manifest.debug_port
        );

        // Simulate scenario execution
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let duration = (std::time::Instant::now() - start).as_secs_f64();

        Ok(ScenarioResult::success(manifest.name.clone(), duration))
    }

    /// Lists all registered scenario names.
    pub fn list(&self) -> Vec<String> {
        self.scenarios.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_runner_creation() {
        let runner = ScenarioRunner::new();
        assert!(runner.list().is_empty());
    }
}
