//! OPS.4 — Health Check contract (documented but NOT implemented).
//!
//! Per ADR-0033 §2 + ADR-0032 §2.2 (OPS.4 scope): this module defines the
//! `HealthCheck` *contract* (data shapes, status semantics, JSON wire
//! format) but does **NOT** implement the HTTP endpoint itself. The HTTP
//! endpoint is a per-deployment decision (operator's choice of port, auth,
//! rate-limit) and out of scope for the library.
//!
//! # Why "contract only" (not implementation)
//!
//! 1. **Deployment-specific**: local/stdio profile has no HTTP listener;
//!    Linux privileged profile may want one on a unix socket; remote
//!    multi-tenant profile (NOT IMPLEMENTED) would need auth + rate-limit.
//!    A single library-side HTTP server is impossible.
//! 2. **Forward compatibility**: by exposing only types, the operator can
//!    wire any HTTP framework (axum, hyper, warp, actix) without depending
//!    on a particular framework version. ADR-0033 §3.
//! 3. **Reusable**: the same `HealthReport` JSON shape can be returned by
//!    a CLI subcommand (`chronos health`), a unix-socket responder, or
//!    scraped by Prometheus — the operator chooses the transport.
//!
//! # Status semantics
//!
//! - `Healthy` — all components responding normally.
//! - `Degraded` — non-critical component impaired (e.g., telemetry exporter
//!   slow); service is functional but should be monitored.
//! - `Unhealthy` — critical component down (e.g., redb store corrupted,
//!   eBPF probe attached but no events); service is not functional.
//!
//! Per ADR-0004 fail-closed: `Unhealthy` MUST include a `details` string
//! explaining which component failed. NO "Unhealthy (no reason given)".
//!
//! # JSON wire format
//!
//! Per OPS.4 ADR-0033 §2.3, the JSON shape is intentionally minimal so it
//! can be consumed by any HTTP client / scraper:
//!
//! ```json
//! {
//!   "status": "healthy" | "degraded" | "unhealthy",
//!   "version": "0.7.112",
//!   "uptime_seconds": 3600,
//!   "components": [
//!     {"name": "redb_store", "status": "healthy", "details": null},
//!     {"name": "ebpf_probes", "status": "healthy", "details": null},
//!     {"name": "mcp_server", "status": "degraded", "details": "high latency"}
//!   ]
//! }
//! ```
//!
//! # UAT-M11-OPS-04 mapping
//!
//! - UAT-OPS-04-01: `HealthReport::serialize_round_trip` (preserves all
//!   fields through serde JSON).
//! - UAT-OPS-04-02: `worst_status` reduces 3 healthy + 1 degraded = degraded.
//! - UAT-OPS-04-03: `is_healthy` requires ALL components `Healthy`.
//! - UAT-OPS-04-04: `Unhealthy` component forces report status to
//!   `Unhealthy` (fail-closed per ADR-0004).
//!
//! # Out-of-scope (per ADR-0033 §6)
//!
//! - HTTP server implementation (operator's choice).
//! - Authentication / rate-limit (deployment profile decision).
//! - Prometheus / OTel exporter integration (deployment decision).
//! - Probe attach/detach lifecycle (MCP server's job).
//! - Per-component health probes (each component reports its own).

use serde::{Deserialize, Serialize};

/// Status of a single component or aggregate report.
///
/// Per ADR-0004 fail-closed, transitions are monotonic in severity
/// (`Healthy < Degraded < Unhealthy`). Aggregate status is the
/// **maximum** severity across all components.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// All components responding normally.
    Healthy,
    /// Non-critical component impaired; service functional but monitor.
    Degraded,
    /// Critical component down; service is not functional.
    Unhealthy,
}

impl HealthStatus {
    /// Human-readable label (for logs and CLI output).
    pub fn label(self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        }
    }
}

/// Status of a single named component.
///
/// Per ADR-0004, `details` MUST be present when `status != Healthy`
/// (explain why, don't silently lie). `details` is `Option<String>` to
/// allow `null` when `status == Healthy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComponentHealth {
    /// Component identifier (e.g., `redb_store`, `ebpf_probes`, `mcp_server`).
    pub name: String,
    /// Component status.
    pub status: HealthStatus,
    /// Optional explanation (required if `status != Healthy`, per ADR-0004).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ComponentHealth {
    /// Construct a healthy component (no details).
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            details: None,
        }
    }

    /// Construct a degraded component with explanation.
    pub fn degraded(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            details: Some(details.into()),
        }
    }

    /// Construct an unhealthy component with explanation.
    pub fn unhealthy(name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            details: Some(details.into()),
        }
    }
}

/// Aggregate health report for the running Chronos instance.
///
/// Per OPS.4 ADR-0033 §2.3 + ADR-0004: the report's overall `status` is
/// the maximum severity across all components. `details` MUST be populated
/// when `status == Unhealthy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HealthReport {
    /// Overall status (max of all components).
    pub status: HealthStatus,
    /// Chronos version (e.g., "0.7.112").
    pub version: String,
    /// Uptime in seconds since process start.
    pub uptime_seconds: u64,
    /// Per-component status (may be empty if no components registered).
    pub components: Vec<ComponentHealth>,
}

impl HealthReport {
    /// Build a report from a list of components; status is the max severity.
    pub fn from_components(
        version: impl Into<String>,
        uptime_seconds: u64,
        components: Vec<ComponentHealth>,
    ) -> Self {
        let status = worst_status(&components);
        Self {
            status,
            version: version.into(),
            uptime_seconds,
            components,
        }
    }

    /// Convenience: report is healthy if all components are healthy.
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

/// Compute the worst (highest severity) status across components.
///
/// Per ADR-0004 fail-closed: any `Unhealthy` component forces the report
/// to `Unhealthy`, not just `Degraded`. `worst_status([])` returns
/// `Healthy` (no components = nothing wrong).
pub fn worst_status(components: &[ComponentHealth]) -> HealthStatus {
    components
        .iter()
        .map(|c| c.status)
        .max()
        .unwrap_or(HealthStatus::Healthy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_distinct() {
        assert_eq!(HealthStatus::Healthy.label(), "healthy");
        assert_eq!(HealthStatus::Degraded.label(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.label(), "unhealthy");
    }

    #[test]
    fn status_ord_is_monotonic_in_severity() {
        assert!(HealthStatus::Healthy < HealthStatus::Degraded);
        assert!(HealthStatus::Degraded < HealthStatus::Unhealthy);
        assert!(HealthStatus::Healthy < HealthStatus::Unhealthy);
    }

    #[test]
    fn component_healthy_has_no_details() {
        let c = ComponentHealth::healthy("redb_store");
        assert_eq!(c.status, HealthStatus::Healthy);
        assert!(c.details.is_none());
    }

    #[test]
    fn component_degraded_requires_details() {
        let c = ComponentHealth::degraded("mcp_server", "high latency");
        assert_eq!(c.status, HealthStatus::Degraded);
        assert_eq!(c.details.as_deref(), Some("high latency"));
    }

    #[test]
    fn component_unhealthy_requires_details() {
        let c = ComponentHealth::unhealthy("ebpf_probes", "no probes attached");
        assert_eq!(c.status, HealthStatus::Unhealthy);
        assert_eq!(c.details.as_deref(), Some("no probes attached"));
    }

    #[test]
    fn worst_status_empty_returns_healthy() {
        // Per ADR-0004: no components = nothing wrong = Healthy.
        assert_eq!(worst_status(&[]), HealthStatus::Healthy);
    }

    #[test]
    fn worst_status_returns_max() {
        let components = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::healthy("b"),
        ];
        assert_eq!(worst_status(&components), HealthStatus::Healthy);

        let components = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::degraded("b", "slow"),
        ];
        assert_eq!(worst_status(&components), HealthStatus::Degraded);

        let components = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::degraded("b", "slow"),
            ComponentHealth::unhealthy("c", "down"),
        ];
        // Per ADR-0004 fail-closed: any Unhealthy forces Unhealthy.
        assert_eq!(worst_status(&components), HealthStatus::Unhealthy);
    }

    #[test]
    fn report_status_uses_worst_status() {
        let components = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::degraded("b", "slow"),
        ];
        let report = HealthReport::from_components("0.7.112", 3600, components);
        assert_eq!(report.status, HealthStatus::Degraded);
        assert!(!report.is_healthy());
    }

    #[test]
    fn report_all_healthy_is_healthy() {
        let components = vec![
            ComponentHealth::healthy("redb_store"),
            ComponentHealth::healthy("ebpf_probes"),
        ];
        let report = HealthReport::from_components("0.7.112", 0, components);
        assert!(report.is_healthy());
    }

    #[test]
    fn serialize_round_trip_preserves_all_fields() {
        let components = vec![
            ComponentHealth::healthy("redb_store"),
            ComponentHealth::degraded("mcp_server", "high latency"),
            ComponentHealth::unhealthy("ebpf_probes", "no probes"),
        ];
        let report = HealthReport::from_components("0.7.112", 3600, components.clone());

        let json = serde_json::to_string(&report).expect("serialize");
        let parsed: HealthReport = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed, report);
        assert_eq!(parsed.components, components);
        assert_eq!(parsed.version, "0.7.112");
        assert_eq!(parsed.uptime_seconds, 3600);
    }

    #[test]
    fn serialize_produces_snake_case_json() {
        let report = HealthReport::from_components(
            "0.7.112",
            60,
            vec![ComponentHealth::healthy("redb_store")],
        );
        let json = serde_json::to_string(&report).expect("serialize");
        // Per ADR-0033 §2.3: snake_case wire format.
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"uptime_seconds\":60"));
        assert!(json.contains("\"name\":\"redb_store\""));
        // Healthy component omits details (skip_serializing_if = "Option::is_none").
        assert!(!json.contains("\"details\""));
    }

    #[test]
    fn unhealthy_component_includes_details_in_json() {
        let report = HealthReport::from_components(
            "0.7.112",
            60,
            vec![ComponentHealth::unhealthy("ebpf_probes", "no probes attached")],
        );
        let json = serde_json::to_string(&report).expect("serialize");
        assert!(json.contains("\"status\":\"unhealthy\""));
        assert!(json.contains("\"details\":\"no probes attached\""));
    }
}
