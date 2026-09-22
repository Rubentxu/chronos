//! Integration tests for `chronos-domain::otlp::equivalence` (M7.1 lift).
//!
//! Tests are adapted from the durable M7.1 spike
//! (`/home/rubentxu/m7-spikes/m7.1-differential-equivalence/tests/cases.rs`).

use chronos_domain::otlp::correlation::{ChronosEvent, EventField};
use chronos_domain::otlp::equivalence::*;

fn ev(ts: u64, probe: &str, fields: Vec<(&str, EventField)>) -> ChronosEvent {
    ChronosEvent {
        ts_micros: ts,
        probe: probe.to_string(),
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    }
}

#[test]
fn fnv1a_known_vector() {
    // Standard FNV-1a 64 test vector: empty string => offset basis.
    let h = chronos_domain::trace::event::fnv1a_64(b"");
    assert_eq!(h, 0xcbf29ce484222325);
    // "a" => 0xaf63dc4c8601ec8c.
    let h = chronos_domain::trace::event::fnv1a_64(b"a");
    assert_eq!(h, 0xaf63dc4c8601ec8c);
}

#[test]
fn canonical_field_bytes_is_deterministic() {
    let bytes = canonical_field_bytes(&EventField::Str("hello".to_string()));
    let bytes2 = canonical_field_bytes(&EventField::Str("hello".to_string()));
    assert_eq!(bytes, bytes2);
}

#[test]
fn canonical_field_bytes_different_types_have_different_tags() {
    let s = canonical_field_bytes(&EventField::Str("42".to_string()));
    let i = canonical_field_bytes(&EventField::Int(42));
    let b = canonical_field_bytes(&EventField::Bool(true));
    assert_ne!(s[0], i[0]);
    assert_ne!(s[0], b[0]);
    assert_ne!(i[0], b[0]);
}

#[test]
fn hash_event_default_ignores_timestamps() {
    let spec = equivalence_spec_default();
    let a = ev(100, "probe.x", vec![("k", EventField::Int(1))]);
    let b = ev(200, "probe.x", vec![("k", EventField::Int(1))]);
    assert_eq!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_event_default_ignores_field_order() {
    let spec = equivalence_spec_default();
    let a = ev(
        0,
        "p",
        vec![("a", EventField::Int(1)), ("b", EventField::Int(2))],
    );
    let b = ev(
        0,
        "p",
        vec![("b", EventField::Int(2)), ("a", EventField::Int(1))],
    );
    assert_eq!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_event_strict_detects_timestamp_drift() {
    let spec = equivalence_spec_strict();
    let a = ev(100, "p", vec![("k", EventField::Int(1))]);
    let b = ev(200, "p", vec![("k", EventField::Int(1))]);
    assert_ne!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_event_strict_detects_field_order() {
    let spec = equivalence_spec_strict();
    let a = ev(
        0,
        "p",
        vec![("a", EventField::Int(1)), ("b", EventField::Int(2))],
    );
    let b = ev(
        0,
        "p",
        vec![("b", EventField::Int(2)), ("a", EventField::Int(1))],
    );
    assert_ne!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_event_strict_same_event_same_hash() {
    let spec = equivalence_spec_strict();
    let a = ev(100, "p", vec![("k", EventField::Int(1))]);
    let b = ev(100, "p", vec![("k", EventField::Int(1))]);
    assert_eq!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_event_different_probes_yield_different_hashes() {
    let spec = equivalence_spec_strict();
    let a = ev(100, "probe.a", vec![("k", EventField::Int(1))]);
    let b = ev(100, "probe.b", vec![("k", EventField::Int(1))]);
    assert_ne!(
        hash_event_canonical(&a, &spec),
        hash_event_canonical(&b, &spec)
    );
}

#[test]
fn hash_invocation_default_order_independent() {
    let spec = equivalence_spec_default();
    let a = vec![
        ev(1, "p", vec![("k", EventField::Int(1))]),
        ev(2, "q", vec![]),
    ];
    let b = vec![
        ev(2, "q", vec![]),
        ev(1, "p", vec![("k", EventField::Int(1))]),
    ];
    assert_eq!(
        hash_invocation_canonical(&a, &spec),
        hash_invocation_canonical(&b, &spec)
    );
}

#[test]
fn hash_invocation_default_detects_added_event() {
    let spec = equivalence_spec_default();
    let a = vec![ev(1, "p", vec![("k", EventField::Int(1))])];
    let b = vec![
        ev(1, "p", vec![("k", EventField::Int(1))]),
        ev(2, "q", vec![]),
    ];
    assert_ne!(
        hash_invocation_canonical(&a, &spec),
        hash_invocation_canonical(&b, &spec)
    );
}

#[test]
fn hash_invocation_default_detects_changed_value() {
    let spec = equivalence_spec_default();
    let a = vec![ev(0, "p", vec![("k", EventField::Int(1))])];
    let b = vec![ev(0, "p", vec![("k", EventField::Int(2))])];
    assert_ne!(
        hash_invocation_canonical(&a, &spec),
        hash_invocation_canonical(&b, &spec)
    );
}

#[test]
fn hash_session_default_order_independent() {
    let spec = equivalence_spec_default();
    let a = vec![10u64, 20, 30];
    let b = vec![30, 10, 20];
    assert_eq!(
        hash_session_canonical(&a, &spec),
        hash_session_canonical(&b, &spec)
    );
}

#[test]
fn hash_session_default_dedupes_identical_invocations() {
    let spec = equivalence_spec_default();
    let a = vec![10u64, 10, 10];
    let b = vec![10u64];
    assert_eq!(
        hash_session_canonical(&a, &spec),
        hash_session_canonical(&b, &spec)
    );
}

#[test]
fn diff_session_hashes_equivalent_default() {
    let spec = equivalence_spec_default();
    let a = vec![10u64, 20, 30];
    let b = vec![30, 10, 20];
    assert_eq!(diff_session_hashes(&a, &b, &spec), DiffVerdict::Equivalent);
}

#[test]
fn diff_session_hashes_differ_default() {
    let spec = equivalence_spec_default();
    let a = vec![10u64, 20];
    let b = vec![10u64, 30];
    assert_eq!(diff_session_hashes(&a, &b, &spec), DiffVerdict::Differ);
}

#[test]
fn canonical_event_bytes_changes_when_spec_changes() {
    let a = ev(100, "p", vec![("k", EventField::Int(1))]);
    let bytes_default = canonical_event_bytes(&a, &equivalence_spec_default());
    let bytes_strict = canonical_event_bytes(&a, &equivalence_spec_strict());
    // Default omits timestamp, strict includes it => bytes differ.
    assert_ne!(bytes_default, bytes_strict);
}

#[test]
fn equivalence_spec_default_differs_from_strict() {
    assert_ne!(equivalence_spec_default(), equivalence_spec_strict());
}

#[test]
fn equivalence_spec_clone_independent() {
    let mut a = equivalence_spec_default();
    let b = a.clone();
    a.ignore_timestamps = false;
    assert!(b.ignore_timestamps);
    // Use `a` so the assignment is not flagged as unused.
    let _ = a.ignore_field_order;
}

// --- Defensive coverage tests beyond the spike ---

#[test]
fn empty_invocation_has_stable_hash() {
    let spec = equivalence_spec_default();
    let h1 = hash_invocation_canonical(&[], &spec);
    let h2 = hash_invocation_canonical(&[], &spec);
    assert_eq!(h1, h2);
}

#[test]
fn empty_session_has_stable_hash() {
    let spec = equivalence_spec_default();
    let h1 = hash_session_canonical(&[], &spec);
    let h2 = hash_session_canonical(&[], &spec);
    assert_eq!(h1, h2);
}

#[test]
fn canonical_bytes_for_empty_event_is_just_probe() {
    let e = ev(0, "only_probe", vec![]);
    let bytes = canonical_event_bytes(&e, &equivalence_spec_default());
    // Under default spec: timestamp is omitted. fields count (4 bytes) + probe
    // length prefix (4 bytes) + probe name bytes. Assert the probe name and
    // length prefix are present at the head.
    let probe_bytes = b"only_probe";
    let len_bytes = (probe_bytes.len() as u32).to_le_bytes();
    assert!(bytes.len() >= 4 + probe_bytes.len());
    assert_eq!(&bytes[..4], &len_bytes);
    assert_eq!(&bytes[4..4 + probe_bytes.len()], probe_bytes);
}

#[test]
fn all_field_variants_produce_distinct_tags() {
    let all = [
        canonical_field_bytes(&EventField::Str("x".to_string())),
        canonical_field_bytes(&EventField::Int(0)),
        canonical_field_bytes(&EventField::Bool(false)),
        canonical_field_bytes(&EventField::DomainRef("x".to_string())),
    ];
    let tags: Vec<u8> = all.iter().map(|b| b[0]).collect();
    let mut sorted = tags.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), tags.len(), "all 4 tags must be distinct");
}
