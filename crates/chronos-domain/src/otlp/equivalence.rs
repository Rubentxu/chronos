//! M7.1 lift: Differential execution v2 — semantic equivalence primitives.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.1-differential-equivalence/`
//! (228L + tests). Three stable, deterministic hierarchical hashes:
//!
//!   - [`hash_event_canonical`] — per-event canonical bytes → FNV-1a 64-bit.
//!   - [`hash_invocation_canonical`] — per-invocation, **order-independent**
//!     over its event hashes.
//!   - [`hash_session_canonical`] — per-session, **order-independent**
//!     over its invocation hashes.
//!
//! ## Duplication avoidance
//!
//! FNV-1a already exists in `chronos_domain::trace::event::fnv1a_64`
//! (made public on 2026-09-22 for this lift). We import that function
//! rather than re-implementing. The IETF FNV constants
//! (`0xcbf29ce484222325`, `0x100000001b3`) are stable across compilers
//! and produce byte-identical output.
//!
//! [`ChronosEvent`] / [`EventField`] come from
//! `chronos_domain::otlp::correlation` (M6.3 lift).

use std::collections::BTreeMap;

use super::correlation::{ChronosEvent, EventField};
use crate::trace::event::fnv1a_64;

/// What counts as "the same" when comparing two runs.
///
/// Defaults are **lenient**: ignore timestamps and field order. Use
/// [`equivalence_spec_strict`] when you need byte-exact match (e.g. when
/// reproducing a recorded incident).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivalenceSpec {
    /// If true, event `ts_micros` is **not** included in the canonical
    /// bytes. Two events that differ only in timestamp will hash equal.
    pub ignore_timestamps: bool,
    /// If true, the `fields` map is sorted by key before hashing. Two
    /// events whose `fields` differ only in key insertion order will
    /// hash equal.
    pub ignore_field_order: bool,
}

/// Lenient default: ignore timestamps and field order.
pub fn equivalence_spec_default() -> EquivalenceSpec {
    EquivalenceSpec {
        ignore_timestamps: true,
        ignore_field_order: true,
    }
}

/// Strict spec: timestamps and field order count toward the hash.
pub fn equivalence_spec_strict() -> EquivalenceSpec {
    EquivalenceSpec {
        ignore_timestamps: false,
        ignore_field_order: false,
    }
}

/// Canonical byte representation of a single [`EventField`].
///
/// Format is opaque but stable across runs and Rust versions.
/// Length-prefixed encoding to disambiguate field boundaries:
/// `<tag:u8> <len:u32 LE> <bytes>`.
pub fn canonical_field_bytes(field: &EventField) -> Vec<u8> {
    let mut out = Vec::new();
    match field {
        EventField::Str(s) => {
            out.push(0x01);
            let bytes = s.as_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        EventField::Int(i) => {
            out.push(0x02);
            out.extend_from_slice(&i.to_le_bytes());
        }
        EventField::Bool(b) => {
            out.push(0x03);
            out.push(if *b { 0x01 } else { 0x00 });
        }
        EventField::DomainRef(r) => {
            out.push(0x04);
            let bytes = r.as_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
        }
    }
    out
}

/// Canonical byte representation of a [`ChronosEvent`] under a spec.
///
/// Two events whose canonical bytes differ **only** in timestamps or
/// field insertion order will produce equal canonical bytes **only when**
/// the corresponding flags in `spec` are set.
pub fn canonical_event_bytes(event: &ChronosEvent, spec: &EquivalenceSpec) -> Vec<u8> {
    let mut out = Vec::new();

    // 1. probe name (always included).
    let probe_bytes = event.probe.as_bytes();
    out.extend_from_slice(&(probe_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(probe_bytes);

    // 2. timestamp (skipped when ignore_timestamps).
    if !spec.ignore_timestamps {
        out.extend_from_slice(&event.ts_micros.to_le_bytes());
    }

    // 3. fields (canonicalised).
    if spec.ignore_field_order {
        let mut sorted: BTreeMap<&String, &EventField> = BTreeMap::new();
        for (k, v) in &event.fields {
            sorted.insert(k, v);
        }
        out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
        for (k, v) in sorted {
            let kb = k.as_bytes();
            out.extend_from_slice(&(kb.len() as u32).to_le_bytes());
            out.extend_from_slice(kb);
            out.extend_from_slice(&canonical_field_bytes(v));
        }
    } else {
        // Preserve insertion order (Vec iteration order).
        out.extend_from_slice(&(event.fields.len() as u32).to_le_bytes());
        for (k, v) in &event.fields {
            let kb = k.as_bytes();
            out.extend_from_slice(&(kb.len() as u32).to_le_bytes());
            out.extend_from_slice(kb);
            out.extend_from_slice(&canonical_field_bytes(v));
        }
    }

    out
}

/// FNV-1a hash of the canonical bytes of `event` under `spec`.
///
/// **Deterministic** across runs and Rust versions. Two events that
/// produce equal canonical bytes will produce equal hashes.
pub fn hash_event_canonical(event: &ChronosEvent, spec: &EquivalenceSpec) -> u64 {
    fnv1a_64(&canonical_event_bytes(event, spec))
}

/// Per-invocation canonical hash.
///
/// Order-independent: the events of an invocation are sorted by their
/// own canonical hash before being concatenated, so reordering events
/// inside an invocation does **not** change the invocation hash.
pub fn hash_invocation_canonical(events: &[ChronosEvent], spec: &EquivalenceSpec) -> u64 {
    let mut per_event_hashes: Vec<u64> = events
        .iter()
        .map(|e| hash_event_canonical(e, spec))
        .collect();
    per_event_hashes.sort_unstable();
    per_event_hashes.dedup();

    let mut out = Vec::with_capacity(per_event_hashes.len() * 8);
    for h in &per_event_hashes {
        out.extend_from_slice(&h.to_le_bytes());
    }
    fnv1a_64(&out)
}

/// Per-session canonical hash.
///
/// Order-independent over invocation hashes.
pub fn hash_session_canonical(invocation_hashes: &[u64], _spec: &EquivalenceSpec) -> u64 {
    let mut sorted = invocation_hashes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out = Vec::with_capacity(sorted.len() * 8);
    for h in &sorted {
        out.extend_from_slice(&h.to_le_bytes());
    }
    fnv1a_64(&out)
}

/// Result of comparing two sessions under a spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffVerdict {
    /// Hashes are equal under the spec.
    Equivalent,
    /// Hashes differ.
    Differ,
}

/// Compare two session-level hash lists under a spec.
///
/// Returns `Equivalent` iff the canonical session hashes are equal.
pub fn diff_session_hashes(a: &[u64], b: &[u64], spec: &EquivalenceSpec) -> DiffVerdict {
    let ha = hash_session_canonical(a, spec);
    let hb = hash_session_canonical(b, spec);
    if ha == hb {
        DiffVerdict::Equivalent
    } else {
        DiffVerdict::Differ
    }
}
