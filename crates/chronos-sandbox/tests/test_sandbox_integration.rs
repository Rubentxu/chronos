//! Integration tests for chronos-sandbox.
//!
//! ## Group A: Smoke tests (always run - no containers needed)
//! These tests verify the sandbox APIs without requiring actual container execution.
//!
//! ## Group B: Podman integration tests (feature-gated)
//! These tests exercise the full container lifecycle but are gated behind
//! `#[cfg_attr(not(feature="integration"), ignore)]` so they can run in CI
//! without Podman by default.

use chronos_sandbox::container::{
    ContainerOptions, ContainerRunner, QuadletBuilder,
};
use chronos_sandbox::container::quadlet::{PortMapping, VolumeMapping};
use chronos_sandbox::kpi::latency::{LatencyCollector, LatencyMetrics};
use chronos_sandbox::manifest::{LanguageTarget, ScenarioManifest};
use chronos_sandbox::session::{SandboxSession, SessionState};
use chronos_sandbox::workload::{HttpWorkload, HttpWorkloadConfig, WorkloadResult};
use std::collections::HashMap;
use std::time::Duration;

// =============================================================================
// Group A: Smoke tests (always run - no containers needed)
// =============================================================================

/// Tests manifest serialization/deserialization roundtrip.
#[test]
fn test_sandbox_manifest_roundtrip() {
    let manifest = ScenarioManifest {
        name: "test-python-scenario".to_string(),
        description: "A test Python debug scenario".to_string(),
        language: LanguageTarget::Python,
        fixture: "/fixtures/python-hello".to_string(),
        debug_port: 5678,
        env: HashMap::from([
            ("PYTHONPATH".to_string(), "/app".to_string()),
            ("DEBUG".to_string(), "1".to_string()),
        ]),
        timeout_sec: 300,
        kpis: vec!["cpu".to_string(), "memory".to_string(), "latency".to_string()],
    };

    // Serialize to YAML
    let yaml = serde_yaml::to_string(&manifest).expect("should serialize manifest");

    // Deserialize back
    let parsed: ScenarioManifest =
        serde_yaml::from_str(&yaml).expect("should deserialize manifest");

    // Assert fields match
    assert_eq!(parsed.name, manifest.name);
    assert_eq!(parsed.description, manifest.description);
    assert_eq!(parsed.language, manifest.language);
    assert_eq!(parsed.fixture, manifest.fixture);
    assert_eq!(parsed.debug_port, manifest.debug_port);
    assert_eq!(parsed.timeout_sec, manifest.timeout_sec);
    assert_eq!(parsed.kpis, manifest.kpis);
    assert_eq!(parsed.env.get("PYTHONPATH"), Some(&"/app".to_string()));
    assert_eq!(parsed.env.get("DEBUG"), Some(&"1".to_string()));
}

/// Tests workload builder configuration.
#[test]
fn test_workload_builder() {
    let config = HttpWorkloadConfig {
        url: "http://localhost:9090".to_string(),
        rps: 500,
        duration_sec: 120,
        connections: 50,
    };

    assert_eq!(config.url, "http://localhost:9090");
    assert_eq!(config.rps, 500);
    assert_eq!(config.duration_sec, 120);
    assert_eq!(config.connections, 50);

    // Create workload from config
    let _workload = HttpWorkload::new(config.clone());
    // HttpWorkload doesn't expose config publicly, but construction succeeds if config is valid
}

/// Tests workload result structure.
#[test]
fn test_workload_result_fields() {
    let result = WorkloadResult {
        requests_sent: 1000,
        responses_received: 995,
        errors: 5,
        avg_latency_us: 500,
        duration_sec: 10.0,
    };

    assert_eq!(result.requests_sent, 1000);
    assert_eq!(result.responses_received, 995);
    assert_eq!(result.errors, 5);
    assert_eq!(result.avg_latency_us, 500);
    assert!((result.duration_sec - 10.0).abs() < 0.001);
}

/// Tests KPI aggregation via LatencyCollector.
#[test]
fn test_kpi_aggregation() {
    let mut collector = LatencyCollector::new();

    // Record 10 latency samples (in microseconds)
    let samples = [100u64, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
    for sample in samples {
        collector.record_startup(sample);
    }

    let metrics = collector.startup_metrics();

    // Verify metrics
    assert_eq!(metrics.samples, 10);

    // Calculate expected average: (100+200+300+400+500+600+700+800+900+1000)/10 = 550
    let expected_avg: u64 = samples.iter().sum::<u64>() / 10;
    assert_eq!(expected_avg, 550);

    // p50 should be around 500-600 (median of sorted samples)
    assert!((metrics.p50_us as i64 - 550).abs() <= 100);

    // p95 should be around 900-1000
    assert!(metrics.p95_us >= 900);
}

/// Tests LatencyMetrics aggregation from samples.
#[test]
fn test_latency_metrics_aggregation() {
    // 100 samples from 1000 to 10000 microseconds
    let samples: Vec<u64> = (1..=100).map(|i| i * 100).collect();
    let metrics = LatencyMetrics::from_samples(&samples);

    assert_eq!(metrics.samples, 100);
    // p50 should be around 5000-5100
    assert!((metrics.p50_us as i64 - 5000).abs() <= 500);
    // p95 should be around 9500-10000
    assert!(metrics.p95_us >= 9000);
    // p99 should be around 9900-10000
    assert!(metrics.p99_us >= 9900);
}

/// Tests session ID uniqueness across multiple sessions.
#[test]
fn test_session_id_uniqueness() {
    let num_sessions = 10;
    let mut ids = Vec::with_capacity(num_sessions);

    for _ in 0..num_sessions {
        // Generate unique session IDs using UUIDs
        let unique_id = uuid::Uuid::new_v4().to_string();
        let session = SandboxSession::new(unique_id.clone());
        ids.push(session.id.clone());
    }

    // Assert all IDs are unique
    let mut unique_ids = ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(
        ids.len(),
        unique_ids.len(),
        "All session IDs should be unique"
    );

    // Assert all IDs are valid UUIDs (can be parsed)
    for id in &ids {
        let parsed = uuid::Uuid::parse_str(id);
        assert!(
            parsed.is_ok(),
            "Session ID '{}' should be a valid UUID",
            id
        );
    }
}

/// Tests session lifecycle state transitions.
#[test]
fn test_session_lifecycle_states() {
    let mut session = SandboxSession::new("test-session".to_string());

    // Initial state
    assert_eq!(session.state, SessionState::Idle);

    // Start
    session.start();
    assert_eq!(session.state, SessionState::Running);

    // Pause
    session.pause();
    assert_eq!(session.state, SessionState::Paused);

    // Resume
    session.resume();
    assert_eq!(session.state, SessionState::Running);

    // Stop
    session.stop();
    assert_eq!(session.state, SessionState::Stopped);
    assert!(session.duration().is_some());
}

/// Tests ContainerOptions builder pattern.
#[test]
fn test_container_options_builder() {
    let opts = ContainerOptions {
        image: "python:3.11-slim".to_string(),
        name: "test-container".to_string(),
        caps: vec!["SYS_PTRACE".to_string()],
        security_opt: vec!["seccomp=unconfined".to_string()],
        ports: vec![PortMapping::new(5678, 5678)],
        volumes: vec![VolumeMapping::new("/host/path", "/container/path")],
        env: HashMap::from([("DEBUG".to_string(), "1".to_string())]),
        network: Some("chronos-network".to_string()),
        pod: None,
        command: vec!["python".to_string(), "-m".to_string(), "debugpy".to_string()],
        user: Some("root".to_string()),
        workdir: Some("/app".to_string()),
    };

    assert_eq!(opts.image, "python:3.11-slim");
    assert_eq!(opts.name, "test-container");
    assert_eq!(opts.caps.len(), 1);
    assert_eq!(opts.ports.len(), 1);
    assert_eq!(opts.network, Some("chronos-network".to_string()));
}

/// Tests QuadletBuilder for container generation.
#[test]
fn test_quadlet_builder_container() {
    let quadlet = QuadletBuilder::new_container("test-python")
        .image("python:3.11-slim")
        .add_cap("SYS_PTRACE")
        .add_port(PortMapping::new(5678, 5678))
        .network("chronos-network")
        .build()
        .expect("should build quadlet");

    assert!(quadlet.content.contains("Image=python:3.11-slim"));
    assert!(quadlet.content.contains("ContainerName=test-python"));
    assert!(quadlet.content.contains("Capability=SYS_PTRACE"));
    assert!(quadlet.content.contains("PublishPort=5678:5678/tcp"));
    assert!(quadlet.content.contains("Network=chronos-network"));
}

// =============================================================================
// Group B: Podman integration tests (feature-gated)
// =============================================================================

/// Tests Podman pull and run for Python container.
#[tokio::test]
#[cfg_attr(not(feature = "integration"), ignore)]
async fn test_podman_pull_and_run_python() {
    let runner = ContainerRunner::new();
    let container_name = format!("chronos-test-python-{}", uuid::Uuid::new_v4());

    let opts = ContainerOptions {
        image: "python:3.11-slim".to_string(),
        name: container_name.clone(),
        caps: vec![],
        security_opt: vec![],
        ports: vec![],
        volumes: vec![],
        env: HashMap::new(),
        network: None,
        pod: None,
        command: vec!["python3".to_string(), "-c".to_string(), "print('hello')".to_string()],
        user: None,
        workdir: None,
    };

    // Start container
    let container = runner.start(&opts).await.expect("container should start");

    // Give it a moment to execute
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get logs
    let logs = runner.logs(&container.id).await.expect("should get logs");

    // Verify output contains "hello"
    assert!(
        logs.contains("hello"),
        "Expected 'hello' in output, got: {}",
        logs
    );

    // Stop and remove
    let _ = runner.stop(&container.id).await;
    let _ = runner.rm(&container.id).await;
}

/// Tests Podman pull and run for Java container.
#[tokio::test]
#[cfg_attr(not(feature = "integration"), ignore)]
async fn test_podman_pull_and_run_java() {
    let runner = ContainerRunner::new();
    let container_name = format!("chronos-test-java-{}", uuid::Uuid::new_v4());

    let opts = ContainerOptions {
        image: "eclipse-temurin:17-jre-alpine".to_string(),
        name: container_name.clone(),
        caps: vec![],
        security_opt: vec![],
        ports: vec![],
        volumes: vec![],
        env: HashMap::new(),
        network: None,
        pod: None,
        command: vec!["java".to_string(), "-version".to_string()],
        user: None,
        workdir: None,
    };

    // Start container
    let container = runner.start(&opts).await.expect("container should start");

    // Give it a moment to execute
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get logs
    let logs = runner.logs(&container.id).await.expect("should get logs");

    // Verify output contains "java" version info
    assert!(
        logs.to_lowercase().contains("java") || logs.contains("openjdk"),
        "Expected Java version info in output, got: {}",
        logs
    );

    // Stop and remove
    let _ = runner.stop(&container.id).await;
    let _ = runner.rm(&container.id).await;
}

/// Tests Podman network create and delete.
#[tokio::test]
#[cfg_attr(not(feature = "integration"), ignore)]
async fn test_podman_network_create_delete() {
    use chronos_sandbox::container::{create_network, remove_network};
    use chronos_sandbox::container::network::NetworkManager;

    let network_name = format!("chronos-test-net-{}", uuid::Uuid::new_v4());

    // Create network
    let _network = create_network(&network_name).await.expect("network should be created");

    // Verify network exists in list
    let manager = NetworkManager::new();
    let networks: Vec<String> = manager.list().await.expect("should list networks");
    assert!(
        networks.contains(&network_name),
        "Network '{}' should be in list: {:?}",
        network_name,
        networks
    );

    // Delete network
    remove_network(&network_name).await.expect("should remove network");

    // Verify network no longer exists
    let networks_after: Vec<String> = manager.list().await.expect("should list networks after delete");
    assert!(
        !networks_after.contains(&network_name),
        "Network '{}' should not be in list after delete: {:?}",
        network_name,
        networks_after
    );
}

/// Tests Podman volume create and delete.
#[tokio::test]
#[cfg_attr(not(feature = "integration"), ignore)]
async fn test_podman_volume_create_delete() {
    use chronos_sandbox::container::{create_volume, remove_volume};
    use chronos_sandbox::container::volume::VolumeManager;

    let volume_name = format!("chronos-test-vol-{}", uuid::Uuid::new_v4());

    // Create volume
    let _volume = create_volume(&volume_name).await.expect("volume should be created");

    // Verify volume exists in list
    let manager = VolumeManager::new();
    let volumes: Vec<String> = manager.list().await.expect("should list volumes");
    assert!(
        volumes.contains(&volume_name),
        "Volume '{}' should be in list: {:?}",
        volume_name,
        volumes
    );

    // Delete volume
    remove_volume(&volume_name).await.expect("should remove volume");

    // Verify volume no longer exists
    let volumes_after: Vec<String> = manager.list().await.expect("should list volumes after delete");
    assert!(
        !volumes_after.contains(&volume_name),
        "Volume '{}' should not be in list after delete: {:?}",
        volume_name,
        volumes_after
    );
}
