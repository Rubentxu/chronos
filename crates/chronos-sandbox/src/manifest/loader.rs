//! Manifest loader for scenario definitions.

use crate::error::SandboxError;
use crate::manifest::{load_manifest, LanguageTarget, ScenarioManifest};

/// Loads a scenario manifest by language target.
pub fn load_for_language(language: LanguageTarget) -> Result<ScenarioManifest, SandboxError> {
    let fixture_path = match language {
        LanguageTarget::Rust => "fixtures/rust-hello",
        LanguageTarget::Java => "fixtures/java-petclinic",
        LanguageTarget::Go => "fixtures/go-hello",
        LanguageTarget::Python => "fixtures/python-hello",
        LanguageTarget::NodeJs => "fixtures/nodejs-hello",
        LanguageTarget::C => "fixtures/c-hello",
    };

    let manifest = ScenarioManifest {
        name: format!("{:?} scenario", language),
        description: format!("{:?} debug scenario", language),
        language: language.clone(),
        fixture: fixture_path.to_string(),
        debug_port: default_port_for_language(&language),
        env: std::collections::HashMap::new(),
        timeout_sec: 300,
        kpis: vec!["cpu".to_string(), "memory".to_string(), "latency".to_string()],
    };

    Ok(manifest)
}

fn default_port_for_language(language: &LanguageTarget) -> u16 {
    match language {
        LanguageTarget::Rust => 2345,
        LanguageTarget::Java => 5005,
        LanguageTarget::Go => 2345,
        LanguageTarget::Python => 5678,
        LanguageTarget::NodeJs => 9229,
        LanguageTarget::C => 2345,
    }
}

/// Loads all scenario manifests from a directory.
pub fn load_all_from_dir(dir: &std::path::Path) -> Result<Vec<ScenarioManifest>, SandboxError> {
    let mut manifests = Vec::new();

    if !dir.is_dir() {
        return Err(SandboxError::ManifestParseFailed(format!(
            "directory not found: {}",
            dir.display()
        )));
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
            match load_manifest(&path) {
                Ok(manifest) => manifests.push(manifest),
                Err(e) => {
                    tracing::warn!("failed to load manifest {}: {}", path.display(), e);
                }
            }
        }
    }

    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_for_language() {
        let manifest = load_for_language(LanguageTarget::Rust).unwrap();
        assert_eq!(manifest.language, LanguageTarget::Rust);
        assert_eq!(manifest.fixture, "fixtures/rust-hello");
    }
}
