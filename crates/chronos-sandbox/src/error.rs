//! Sandbox error types.

use thiserror::Error;

/// Errors that can occur during sandbox operations.
#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("failed to start container: {0}")]
    ContainerStartFailed(String),

    #[error("ptrace is blocked or unavailable: {0}")]
    PtraceBlocked(String),

    #[error("failed to connect to debug target: {0}")]
    DebugTargetConnectFailed(String),

    #[error("workload execution failed: {0}")]
    WorkloadFailed(String),

    #[error("KPI collection failed: {0}")]
    KpiCollectionFailed(String),

    #[error("failed to parse manifest: {0}")]
    ManifestParseFailed(String),

    #[error("scenario execution failed: {0}")]
    ScenarioFailed(String),

    #[error("container not found: {0}")]
    ContainerNotFound(String),

    #[error("network operation failed: {0}")]
    NetworkError(String),

    #[error("volume operation failed: {0}")]
    VolumeError(String),

    #[error("pod operation failed: {0}")]
    PodError(String),

    #[error("Quadlet generation failed: {0}")]
    QuadletGenerationFailed(String),

    #[error("timeout waiting for container to be ready on port {port}: {message}")]
    ContainerReadyTimeout { port: u16, message: String },

    #[error("invalid container configuration: {0}")]
    InvalidContainerConfig(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("UUID error: {0}")]
    UuidError(#[from] uuid::Error),
}
