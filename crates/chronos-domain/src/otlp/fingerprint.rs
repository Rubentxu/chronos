//! M7.3 lift: BehaviourFingerprint — session-level summary of M7.2 alignment.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.3-behaviour-fingerprint/` (282L).
//!
//! Builds a session-level fingerprint from M7.2's `AlignmentReport` so two runs
//! (possibly across hosts, possibly at different times) can be compared for
//! behavioural equivalence without re-traversing individual invocations.
//!
//! ## Duplication avoidance
//!
//! FNV-1a 64-bit: the spike has its own `pub fn fnv1a` re-implementing the
//! algorithm. The product already has `fnv1a_64` in
//! `chronos_domain::trace::event` (made public during the M7.1 lift). The
//! lift imports `fnv1a_64` from `trace::event` rather than redeclaring it.
//!
//! M7.1 hashes: M7.3 does NOT recompute them. The hashes already live on
//! each `InvocationAlignment` (filled by M7.2's `compute_pair` which
//! delegates to M7.1's `hash_invocation_canonical`). M7.3 only consumes
//! those values.
//!
//! M7.2 alignment: M7.3 reads `AlignmentReport` and `InvocationAlignment`
//! from `super::alignment`.

use super::alignment::{AlignmentReport, InvocationAlignment, InvocationStatus};
use crate::trace::event::fnv1a_64;

const NONE_DELTA_SENTINEL: u64 = 0x8000_0000_0000_0000;

/// Compact session-level fingerprint derived from M7.2's `AlignmentReport`.
///
/// `aggregate_hash` covers the *content* of every classified invocation using
/// M7.2's already-computed M7.1 hashes (`hash_a`, `hash_b`).
///
/// `fingerprint_hash` covers the *shape + canonical hashes*: per-pair
/// `(status, hash_a_or_b)` sorted and hashed.
///
/// Both hashes are FNV-1a 64-bit (matching M7.1's choice). NOT cryptographic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BehaviourFingerprint {
    /// FNV-1a over `(status, hash_a, hash_b, delta)` per pair, sorted.
    pub aggregate_hash: u64,
    /// FNV-1a over `(status, primary_key, delta)` per pair, sorted.
    pub fingerprint_hash: u64,
    /// Number of matched invocations.
    pub matched_count: usize,
    /// Number of mismatched invocations.
    pub mismatched_count: usize,
    /// Number of invocations only in A.
    pub only_in_a_count: usize,
    /// Number of invocations only in B.
    pub only_in_b_count: usize,
}

/// Errors raised when computing a fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintError {
    /// The supplied `AlignmentReport` had no classified pairs; nothing to hash.
    EmptyReport,
}

/// Human-readable summary of a fingerprint, for logging and dashboards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerprintSummary {
    /// 16-char hex of `aggregate_hash`.
    pub aggregate_hash_hex: String,
    /// 16-char hex of `fingerprint_hash`.
    pub fingerprint_hash_hex: String,
    /// One-line counts summary.
    pub counts_line: String,
}

fn push_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn status_tag(s: &InvocationStatus) -> u8 {
    match s {
        InvocationStatus::Matched => 0x01,
        InvocationStatus::Mismatched => 0x02,
        InvocationStatus::OnlyInA => 0x03,
        InvocationStatus::OnlyInB => 0x04,
    }
}

/// For `Matched`/`Mismatched`: delta (i64 cast to u64 bits).
/// For `OnlyInA`/`OnlyInB`: i64::MIN sentinel.
fn delta_as_u64(p: &InvocationAlignment) -> u64 {
    match p.delta_event_count {
        Some(d) => d as u64,
        None => NONE_DELTA_SENTINEL,
    }
}

/// Primary key per pair for sorting:
/// - `Matched`/`Mismatched`: smaller of `hash_a` and `hash_b`.
/// - `OnlyInA`: `hash_a` (hash_b is None).
/// - `OnlyInB`: `hash_b` (hash_a is None).
fn primary_key(p: &InvocationAlignment) -> u64 {
    match (p.hash_a, p.hash_b) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => 0, // shouldn't happen
    }
}

/// Stream of bytes for the aggregate hash.
///
/// Layout: `[status_tag: u8][hash_a: u64][hash_b: u64][delta_or_sentinel: u64]`
///
/// Sort key per pair is `(status_tag, primary_key)` for determinism across
/// insertion order (M7.2 lists pairs in walk order; we re-sort here).
fn aggregate_bytes_for_pair(p: &InvocationAlignment) -> Vec<u8> {
    let mut buf = Vec::with_capacity(25);
    buf.push(status_tag(&p.status));
    let (ha, hb) = match (&p.hash_a, &p.hash_b) {
        (Some(a), Some(b)) => (*a, *b),
        (Some(a), None) => (*a, 0),
        (None, Some(b)) => (0, *b),
        (None, None) => (0, 0),
    };
    push_u64(&mut buf, ha);
    push_u64(&mut buf, hb);
    push_u64(&mut buf, delta_as_u64(p));
    buf
}

/// Stream of bytes for the shape+content fingerprint hash.
///
/// Layout: `[status_tag: u8][hash_a_or_b: u64][delta_or_sentinel: u64]`
///
/// Sort key per pair is `(status_tag, primary_key)`.
fn shape_bytes_for_pair(p: &InvocationAlignment) -> Vec<u8> {
    let mut buf = Vec::with_capacity(17);
    buf.push(status_tag(&p.status));
    push_u64(&mut buf, primary_key(p));
    push_u64(&mut buf, delta_as_u64(p));
    buf
}

/// Compute the session-level `BehaviourFingerprint` from M7.2's
/// `AlignmentReport`. Pure function. Deterministic.
///
/// Uses M7.2's already-computed per-invocation hashes (M7.1 FNV-1a).
/// No dependency on caller-side `EquivalenceSpec` because M7.2 already
/// normalised via the spec used when `align_sessions` was called.
pub fn session_fingerprint(
    report: &AlignmentReport,
) -> Result<BehaviourFingerprint, FingerprintError> {
    let total = report.total_pairs();
    if total == 0 {
        return Err(FingerprintError::EmptyReport);
    }

    let all_pairs: Vec<&InvocationAlignment> = report
        .matched
        .iter()
        .chain(report.mismatched.iter())
        .chain(report.only_in_a.iter())
        .chain(report.only_in_b.iter())
        .collect();

    // ---- aggregate stream (sorted) ----
    let mut agg_entries: Vec<(u8, u64, Vec<u8>)> = all_pairs
        .iter()
        .map(|p| {
            let bytes = aggregate_bytes_for_pair(p);
            (bytes[0], primary_key(p), bytes)
        })
        .collect();
    agg_entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut agg_stream: Vec<u8> = Vec::new();
    for (_, _, bytes) in &agg_entries {
        agg_stream.extend_from_slice(bytes);
    }
    let aggregate_hash = fnv1a_64(&agg_stream);

    // ---- shape stream (sorted) ----
    let mut shape_entries: Vec<(u8, u64, Vec<u8>)> = all_pairs
        .iter()
        .map(|p| {
            let bytes = shape_bytes_for_pair(p);
            (bytes[0], primary_key(p), bytes)
        })
        .collect();
    shape_entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut shape_stream: Vec<u8> = Vec::new();
    for (_, _, bytes) in &shape_entries {
        shape_stream.extend_from_slice(bytes);
    }
    let fingerprint_hash = fnv1a_64(&shape_stream);

    Ok(BehaviourFingerprint {
        aggregate_hash,
        fingerprint_hash,
        matched_count: report.matched_count(),
        mismatched_count: report.mismatched_count(),
        only_in_a_count: report.only_in_a_count(),
        only_in_b_count: report.only_in_b_count(),
    })
}

/// `true` if two fingerprints denote the same behaviour under their spec.
pub fn fingerprint_equiv(a: &BehaviourFingerprint, b: &BehaviourFingerprint) -> bool {
    a.fingerprint_hash == b.fingerprint_hash && a.aggregate_hash == b.aggregate_hash
}

/// `true` if two fingerprints have the same shape (counts + structural hashes)
/// but possibly different specific content.
pub fn fingerprint_shape_only_equiv(a: &BehaviourFingerprint, b: &BehaviourFingerprint) -> bool {
    a.fingerprint_hash == b.fingerprint_hash
}

/// One-line summary, suitable for log output.
pub fn summary(f: &BehaviourFingerprint) -> FingerprintSummary {
    FingerprintSummary {
        aggregate_hash_hex: format!("{:016x}", f.aggregate_hash),
        fingerprint_hash_hex: format!("{:016x}", f.fingerprint_hash),
        counts_line: format!(
            "matched={} mismatched={} only_in_a={} only_in_b={}",
            f.matched_count, f.mismatched_count, f.only_in_a_count, f.only_in_b_count
        ),
    }
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn fnv1a_64_known_vector_via_trace_event() {
        // Standard FNV-1a 64-bit: empty → offset.
        assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
        // Known: FNV-1a of "foobar" = 0x85944171f73967e8.
        assert_eq!(fnv1a_64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn status_tag_returns_unique_codes() {
        use std::collections::BTreeSet;
        let s = [
            InvocationStatus::Matched,
            InvocationStatus::Mismatched,
            InvocationStatus::OnlyInA,
            InvocationStatus::OnlyInB,
        ];
        let tags: BTreeSet<u8> = s.iter().map(status_tag).collect();
        assert_eq!(tags.len(), 4);
    }
}
