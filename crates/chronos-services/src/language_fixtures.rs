//! M11.3 — Per-adapter positive + negative fixtures, fail-closed.
//!
//! ## Why this exists
//!
//! ADR-0031 §4.3 (D3): capability matrix has tiers but **must be validated**
//! against actual per-adapter fixtures. Otherwise we report CERT-3 just because
//! the adapter has tests, and the user pays the cost of discovering it doesn't
//! actually work for their runtime.
//!
//! ROADMAP §M11 §100: "no se considera 'soporte' de un lenguaje tener tests
//! unitarios — hace falta ejecutarlo contra un fixture real y verificar
//! que el evento producido cumple el contrato".
//!
//! ## Fail-closed principle
//!
//! Per ADR-0031 §4.3 + ADR-0004 (no Silent Lies):
//!
//! - If a `Cert3Certified` adapter lacks the corresponding fixture, we
//!   **downgrade** it to `Cert2Partial` with a reason — we do NOT silently
//!   keep it as CERT-3 and hope nobody checks.
//! - If a `Cert4Production` adapter is missing the perf/security evidence,
//!   we downgrade to `Cert3Certified` — same reasoning.
//!
//! The downgrade is reported via `FixtureAuditReport` with explicit reasons.
//!
//! ## Out-of-scope (per ADR-0031 §8)
//!
//! - Actually building and running the fixtures (this module is structural —
//!   it audits what fixtures exist *on disk*, what they cover, what evidence
//!   they encode). Running them end-to-end is M11.4 + UAT-M11-XX.
//! - Adding new fixtures (owners: per-adapter crate teams).
//! - Per-fixture execution timing (M11.4 perf budget territory).
//! - Cross-language fixtures (only single-adapter fixtures in this slice).

use crate::language_capabilities::{
    default_capability_matrix, tier_for, CapabilityMatrixEntry, CertificationTier,
};
use std::collections::BTreeMap;

/// Status of a single fixture relative to its expected adapter.
///
/// BTreeMap is used for deterministic iteration order in tests.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureStatus {
    /// Fixture present, content readable, captures expected events.
    Present,
    /// Fixture expected for this tier but absent on disk.
    Missing,
    /// Fixture present but stale (older than audit window) or unreadable.
    Stale,
}

/// One row in the per-adapter fixture audit.
///
/// Distinct from `CapabilityMatrixEntry` (build-time tier declaration) and
/// `LanguageAdapterStatus` (live session status). This is the **evidence**
/// that backs a tier — not the tier itself, not the runtime status.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AdapterFixtureAudit {
    pub language: String,
    pub declared_tier: CertificationTier,
    pub fixture_status: FixtureStatus,
    /// `Some(reason)` only if the audit identified a problem
    /// (missing/stale fixture for a tier that requires one).
    pub downgrade_reason: Option<String>,
}

/// Result of auditing the capability matrix against per-adapter fixtures.
///
/// Downgrades are reported here. The module does NOT mutate
/// `CapabilityMatrixEntry` in place — it returns a report, and the caller
/// (typically a build script or docs generator) decides what to do with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FixtureAuditReport {
    /// All entries audited, in declared order.
    pub audits: Vec<AdapterFixtureAudit>,
    /// Number of downgrades triggered by this audit.
    pub downgrade_count: usize,
    /// Counters indexed by `CertificationTier` (declared) AND `FixtureStatus`
    /// (observed). Two-dimensional because a CERT-3 entry may have a
    /// `Missing` fixture (downgrade to CERT-2), and a CERT-1 entry may
    /// legitimately have `Missing` fixtures (CERT-1 has no fixture
    /// requirement).
    pub summary: AuditSummary,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AuditSummary {
    pub by_declared_tier: BTreeMap<String, usize>,
    pub by_fixture_status: BTreeMap<String, usize>,
}

/// What fixture status is expected for each tier.
///
/// - CERT-1 stub: no fixture required (stub compiles, doesn't execute).
/// - CERT-2 partial: missing fixture allowed (partial means "happy path
///   fixture attempted but no contract verification").
/// - CERT-3 certified: `Missing` fixture → downgrade to CERT-2.
/// - CERT-4 production: `Missing` fixture → downgrade to CERT-3.
pub fn expected_fixture_status(tier: CertificationTier) -> FixtureRequirement {
    match tier {
        CertificationTier::Cert1Stub => FixtureRequirement::NotRequired,
        CertificationTier::Cert2Partial => FixtureRequirement::Optional,
        CertificationTier::Cert3Certified => FixtureRequirement::Required,
        CertificationTier::Cert4Production => FixtureRequirement::Required,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureRequirement {
    /// A fixture is not required for this tier.
    NotRequired,
    /// A fixture is encouraged but its absence does not downgrade.
    Optional,
    /// A fixture MUST exist; absence triggers a downgrade.
    Required,
}

/// Decide the effective tier given the declared tier + observed fixture
/// status. Pure function: no I/O, no side effects, fully deterministic.
///
/// This is the **fail-closed** calculation. The intended caller is a build
/// script or pre-merge audit, NOT a request-time hot path.
///
/// Rules (from ADR-0031 §4.3 + ADR-0004):
///
/// - Declared CERT-1 + any fixture status → CERT-1 unchanged
///   (CERT-1 has no requirement).
/// - Declared CERT-2 + `Missing`/`Stale` fixture → CERT-2 unchanged
///   (CERT-2 has the requirement as `Optional`).
/// - Declared CERT-2 + `Present` fixture → CERT-2 unchanged
///   (matches the audit expectation).
/// - Declared CERT-3 + `Missing` fixture → **downgrade to CERT-2**,
///   reason "missing cert-3 fixture".
/// - Declared CERT-3 + `Stale` fixture → **downgrade to CERT-2**,
///   reason "stale cert-3 fixture".
/// - Declared CERT-3 + `Present` fixture → CERT-3 unchanged.
/// - Declared CERT-4 + `Missing`/`Stale` fixture → **downgrade to CERT-3**,
///   reason "missing cert-4 fixture evidence" (or stale).
/// - Declared CERT-4 + `Present` fixture → CERT-4 unchanged.
pub fn effective_tier(
    declared: CertificationTier,
    fixture: FixtureStatus,
) -> (CertificationTier, Option<String>) {
    match (declared, &fixture) {
        (CertificationTier::Cert1Stub, _) => (declared, None),
        (CertificationTier::Cert2Partial, FixtureStatus::Missing) => (declared, None),
        (CertificationTier::Cert2Partial, FixtureStatus::Stale) => (declared, None),
        (CertificationTier::Cert2Partial, FixtureStatus::Present) => (declared, None),
        (CertificationTier::Cert3Certified, FixtureStatus::Missing) => (
            CertificationTier::Cert2Partial,
            Some("missing cert-3 fixture, downgraded to cert-2".to_string()),
        ),
        (CertificationTier::Cert3Certified, FixtureStatus::Stale) => (
            CertificationTier::Cert2Partial,
            Some("stale cert-3 fixture, downgraded to cert-2".to_string()),
        ),
        (CertificationTier::Cert3Certified, FixtureStatus::Present) => (declared, None),
        (CertificationTier::Cert4Production, FixtureStatus::Missing) => (
            CertificationTier::Cert3Certified,
            Some("missing cert-4 fixture evidence, downgraded to cert-3".to_string()),
        ),
        (CertificationTier::Cert4Production, FixtureStatus::Stale) => (
            CertificationTier::Cert3Certified,
            Some("stale cert-4 fixture evidence, downgraded to cert-3".to_string()),
        ),
        (CertificationTier::Cert4Production, FixtureStatus::Present) => (declared, None),
    }
}

/// Audit the default capability matrix against a per-language fixture map.
///
/// The fixture map is supplied by the caller. This module does NOT walk the
/// filesystem — it accepts a precomputed map (`Language → FixtureStatus`),
/// keeping this function pure. The caller (typically `tests/` or a build
/// script) does the disk walk.
pub fn audit_capability_matrix(
    matrix: &[CapabilityMatrixEntry],
    fixtures: &BTreeMap<String, FixtureStatus>,
) -> FixtureAuditReport {
    let mut audits = Vec::with_capacity(matrix.len());
    let mut downgrade_count = 0usize;
    let mut by_declared_tier: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_fixture_status: BTreeMap<String, usize> = BTreeMap::new();

    for entry in matrix {
        let lang_label = language_label(&entry.language);
        let fixture_status = fixtures
            .get(&lang_label)
            .cloned()
            .unwrap_or(FixtureStatus::Missing);
        let (effective, reason) = effective_tier(entry.tier, fixture_status.clone());

        if reason.is_some() {
            downgrade_count += 1;
        }

        *by_declared_tier
            .entry(entry.tier.label().to_string())
            .or_insert(0) += 1;
        *by_fixture_status
            .entry(format!("{:?}", fixture_status))
            .or_insert(0) += 1;

        audits.push(AdapterFixtureAudit {
            language: lang_label,
            declared_tier: effective,
            fixture_status,
            downgrade_reason: reason,
        });
    }

    FixtureAuditReport {
        audits,
        downgrade_count,
        summary: AuditSummary {
            by_declared_tier,
            by_fixture_status,
        },
    }
}

/// Convenience helper: audit the default matrix with **all fixtures present**
/// (i.e., the optimistic baseline). No downgrades should result.
pub fn audit_default_matrix_with_all_present() -> FixtureAuditReport {
    let matrix = default_capability_matrix();
    let fixtures: BTreeMap<String, FixtureStatus> = matrix
        .iter()
        .map(|e| (language_label(&e.language), FixtureStatus::Present))
        .collect();
    audit_capability_matrix(&matrix, &fixtures)
}

/// Convenience helper: audit with **all fixtures missing** (the pessimistic
/// baseline). All CERT-3 / CERT-4 entries should downgrade.
pub fn audit_default_matrix_with_all_missing() -> FixtureAuditReport {
    let matrix = default_capability_matrix();
    let fixtures: BTreeMap<String, FixtureStatus> = matrix
        .iter()
        .map(|e| (language_label(&e.language), FixtureStatus::Missing))
        .collect();
    audit_capability_matrix(&matrix, &fixtures)
}

fn language_label(lang: &chronos_domain::Language) -> String {
    // Use Debug for stable key representation across runs.
    format!("{:?}", lang)
}

/// Look up the effective tier for a language given fixture status, by
/// going through the default matrix (per `tier_for`) and applying the
/// fail-closed audit. Does NOT mutate global state.
pub fn effective_tier_for(
    language: chronos_domain::Language,
    fixture: FixtureStatus,
) -> (CertificationTier, Option<String>) {
    let declared = tier_for(language);
    effective_tier(declared, fixture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cert1_has_no_requirement_regardless_of_fixture() {
        assert_eq!(
            effective_tier(CertificationTier::Cert1Stub, FixtureStatus::Missing),
            (CertificationTier::Cert1Stub, None)
        );
        assert_eq!(
            effective_tier(CertificationTier::Cert1Stub, FixtureStatus::Present),
            (CertificationTier::Cert1Stub, None)
        );
    }

    #[test]
    fn cert2_holds_even_with_missing_fixture() {
        let (tier, reason) =
            effective_tier(CertificationTier::Cert2Partial, FixtureStatus::Missing);
        assert_eq!(tier, CertificationTier::Cert2Partial);
        assert!(reason.is_none());
    }

    #[test]
    fn cert3_missing_fixture_downgrades_to_cert2_with_reason() {
        let (tier, reason) =
            effective_tier(CertificationTier::Cert3Certified, FixtureStatus::Missing);
        assert_eq!(tier, CertificationTier::Cert2Partial);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("cert-3"));
    }

    #[test]
    fn cert3_present_fixture_holds() {
        let (tier, reason) =
            effective_tier(CertificationTier::Cert3Certified, FixtureStatus::Present);
        assert_eq!(tier, CertificationTier::Cert3Certified);
        assert!(reason.is_none());
    }

    #[test]
    fn cert4_missing_fixture_downgrades_to_cert3() {
        let (tier, reason) =
            effective_tier(CertificationTier::Cert4Production, FixtureStatus::Missing);
        assert_eq!(tier, CertificationTier::Cert3Certified);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("cert-4"));
    }

    #[test]
    fn audit_optimistic_zero_downgrades() {
        let report = audit_default_matrix_with_all_present();
        assert_eq!(report.downgrade_count, 0, "all present = zero downgrades");
        // Default matrix has 14 entries (per Language enum audit).
        assert_eq!(report.audits.len(), 14);
    }

    #[test]
    fn audit_pessimistic_downgrades_all_cert3_and_above() {
        let report = audit_default_matrix_with_all_missing();
        // Per M11.2 initial audit: 13 declared as CERT-3 + 1 declared as CERT-1
        // (Unknown). With all missing, expect 13 downgrades — CERT-1 has no
        // requirement and never downgrades.
        assert_eq!(
            report.downgrade_count, 13,
            "all missing = 13 downgrades (one per declared CERT-3 entry; Unknown is CERT-1 and does not downgrade)"
        );
        // Every downgrade must come with a reason.
        for audit in &report.audits {
            if audit.language == "Unknown" {
                // CERT-1 + Missing = no downgrade.
                assert_eq!(audit.declared_tier, CertificationTier::Cert1Stub);
                assert!(audit.downgrade_reason.is_none());
            } else {
                assert_eq!(audit.declared_tier, CertificationTier::Cert2Partial);
                assert!(audit.downgrade_reason.is_some());
            }
        }
    }

    #[test]
    fn summary_counts_declared_tiers_and_fixture_statuses() {
        let report = audit_default_matrix_with_all_present();
        assert!(report
            .summary
            .by_declared_tier
            .contains_key("CERT-3 (certified)"));
        assert!(report.summary.by_fixture_status.contains_key("Present"));
    }

    #[test]
    fn serde_round_trip_preserves_report() {
        let report = audit_default_matrix_with_all_present();
        let json = serde_json::to_string(&report).expect("serialize");
        let parsed: FixtureAuditReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, report);
    }

    #[test]
    fn effective_tier_for_unknown_language_returns_cert1() {
        // Unknown → CERT-1 declared (per `tier_for`).
        // CERT-1 + anything = CERT-1 unchanged.
        let (tier, reason) =
            effective_tier_for(chronos_domain::Language::Unknown, FixtureStatus::Missing);
        assert_eq!(tier, CertificationTier::Cert1Stub);
        assert!(reason.is_none());
    }
}
