//! M11.3 (continued) — Per-profile OPS evidence audit integration.
//!
//! ## Why this exists
//!
//! ADR-0032 §2.2 (OPS.3): per-check, per-profile evidence files must be
//! generated and aggregatable. The aggregator (`scripts/aggregate_ops_evidence.sh`)
//! produces a JSON summary that callers (build scripts, CI) consume. This
//! module provides a **typed Rust view** of that aggregate so other Rust
//! code can introspect the OPS chapter state without re-parsing the JSON
//! schema every time.
//!
//! ## Out-of-scope (per ADR-0032 §8)
//!
//! - Live aggregation (the aggregator is a shell script that reads JSON
//!   files; this module is read-only over the *result* of that script).
//! - Modifying evidence files (write-only by humans; aggregate is the
//!   only Rust-touched artefact).
//! - OPS.4 support runbook telemetry (separate deliverable).

use serde::{Deserialize, Serialize};

/// Per-check, per-profile status as written to `evidence/ops/ops.{N}.{profile}.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Pass,
    Warn,
    Fail,
}

/// Per-profile summary entry within `evidence/ops/aggregate.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProfileSummary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub total: usize,
}

/// Tier per profile, derived from the per-profile summary using the same
/// rules as OPS.2 (`scripts/run_cert.sh`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTier {
    Cert1Stub,
    Cert2Partial,
    Cert3Certified,
    Cert4Production,
}

/// Compute the tier from a profile summary (mirrors OPS.2 logic for
/// consistency — same input → same output).
///
/// Rules (from ADR-0032 §2.2 + ADR-0004):
///
/// - 0 fail + 0 warn → `Cert4Production`
/// - 0 fail + some warn → `Cert3Certified`
/// - 1..=2 fail → `Cert2Partial`
/// - 3+ fail → `Cert1Stub`
pub fn tier_from_summary(summary: &ProfileSummary) -> ProfileTier {
    if summary.fail == 0 && summary.warn == 0 {
        ProfileTier::Cert4Production
    } else if summary.fail == 0 {
        ProfileTier::Cert3Certified
    } else if summary.fail <= 2 {
        ProfileTier::Cert2Partial
    } else {
        ProfileTier::Cert1Stub
    }
}

impl ProfileTier {
    /// Snake-case label for JSON serialization parity with OPS.2.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cert1Stub => "cert-1-stub",
            Self::Cert2Partial => "cert-2-partial",
            Self::Cert3Certified => "cert-3-certified",
            Self::Cert4Production => "cert-4-production",
        }
    }
}

/// Aggregate report schema (matches `evidence/ops/aggregate.json` produced
/// by `scripts/aggregate_ops_evidence.sh`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OpsAggregateReport {
    pub schema_version: u32,
    /// Check id (`OPS.1`..`OPS.8`) → profile → per-check summary.
    pub per_check: std::collections::BTreeMap<String, std::collections::BTreeMap<String, CheckSummary>>,
    /// Profile → overall summary (counts).
    pub per_profile: std::collections::BTreeMap<String, ProfileSummary>,
    /// Profile → overall tier (derived).
    pub tier_per_profile: std::collections::BTreeMap<String, ProfileTier>,
    /// Source count (always 16 = 8 checks × 2 profiles).
    pub source_count: usize,
    /// Wall-clock timestamp when aggregate was generated.
    pub generated_at_unix_seconds: i64,
}

/// Per-check summary (one profile's status + counts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CheckSummary {
    pub status: EvidenceStatus,
    pub gaps_count: usize,
    pub evidence_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_all_pass_is_cert4() {
        let s = ProfileSummary { pass: 8, warn: 0, fail: 0, total: 8 };
        assert_eq!(tier_from_summary(&s), ProfileTier::Cert4Production);
    }

    #[test]
    fn tier_no_fail_with_warn_is_cert3() {
        let s = ProfileSummary { pass: 6, warn: 2, fail: 0, total: 8 };
        assert_eq!(tier_from_summary(&s), ProfileTier::Cert3Certified);
    }

    #[test]
    fn tier_one_fail_is_cert2() {
        let s = ProfileSummary { pass: 5, warn: 2, fail: 1, total: 8 };
        assert_eq!(tier_from_summary(&s), ProfileTier::Cert2Partial);
    }

    #[test]
    fn tier_two_fail_is_cert2() {
        let s = ProfileSummary { pass: 4, warn: 2, fail: 2, total: 8 };
        assert_eq!(tier_from_summary(&s), ProfileTier::Cert2Partial);
    }

    #[test]
    fn tier_three_fail_is_cert1() {
        let s = ProfileSummary { pass: 3, warn: 2, fail: 3, total: 8 };
        assert_eq!(tier_from_summary(&s), ProfileTier::Cert1Stub);
    }

    #[test]
    fn tier_labels_match_ops2() {
        assert_eq!(ProfileTier::Cert1Stub.label(), "cert-1-stub");
        assert_eq!(ProfileTier::Cert2Partial.label(), "cert-2-partial");
        assert_eq!(ProfileTier::Cert3Certified.label(), "cert-3-certified");
        assert_eq!(ProfileTier::Cert4Production.label(), "cert-4-production");
    }

    #[test]
    fn aggregate_report_deserializes_example() {
        // Minimal valid aggregate report.
        let json = r#"{
            "schema_version": 1,
            "per_check": {
                "OPS.1": {
                    "local-stdio": {"status": "pass", "gaps_count": 0, "evidence_count": 2},
                    "linux-privileged": {"status": "pass", "gaps_count": 1, "evidence_count": 2}
                }
            },
            "per_profile": {
                "local-stdio": {"pass": 6, "warn": 2, "fail": 0, "total": 8},
                "linux-privileged": {"pass": 5, "warn": 3, "fail": 0, "total": 8}
            },
            "tier_per_profile": {
                "local-stdio": "cert3_certified",
                "linux-privileged": "cert3_certified"
            },
            "source_count": 16,
            "generated_at_unix_seconds": 1737604800
        }"#;
        let report: OpsAggregateReport = serde_json::from_str(json).expect("parse");
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.source_count, 16);
        assert_eq!(
            report.tier_per_profile.get("local-stdio"),
            Some(&ProfileTier::Cert3Certified)
        );
    }

    #[test]
    fn evidence_status_serde_uses_snake_case() {
        let json = serde_json::to_string(&EvidenceStatus::Pass).unwrap();
        assert_eq!(json, "\"pass\"");
        let json = serde_json::to_string(&EvidenceStatus::Warn).unwrap();
        assert_eq!(json, "\"warn\"");
    }
}
