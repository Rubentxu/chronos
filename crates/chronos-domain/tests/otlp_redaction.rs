//! Integration tests for `chronos-domain::otlp::redaction` (M6.5 lift).
//!
//! Tests are adapted from the durable M6.5 spike
//! (`/home/rubentxu/m6-spikes/m6.5-otel-redaction/tests/cases.rs`). The
//! spike tests operate on `ExportedSpan` (M6.4 type); the product lift is
//! generic over `Vec<(String, String)>` so the contract assertions are
//! preserved at the attribute-pair level.

use chronos_domain::otlp::redaction::*;

/// Helper: build an attribute list from `&str` literals.
fn attrs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn redaction_marks_sensitive_keys() {
    let policy = RedactionPolicy::default_secrets();
    let input = attrs(&[("password", "hunter2"), ("user", "alice")]);
    let (out, counters) =
        redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "[REDACTED]", "password value redacted");
    assert_eq!(out[1].1, "alice", "non-sensitive value preserved");
    assert_eq!(counters.redacted_fields, 1);
    assert_eq!(counters.collapsed_cardinality, 0);
}

#[test]
fn redaction_is_case_insensitive() {
    let policy = RedactionPolicy::default_secrets();
    let input = attrs(&[("Password", "x"), ("AUTH", "y"), ("apiKey", "z")]);
    let (out, counters) =
        redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "[REDACTED]");
    assert_eq!(out[1].1, "[REDACTED]");
    assert_eq!(out[2].1, "[REDACTED]");
    assert_eq!(counters.redacted_fields, 3);
}

#[test]
fn redaction_uses_custom_keys() {
    let policy = RedactionPolicy::with_keys(vec!["internal_id".to_string()]);
    let input = attrs(&[("internal_id", "secret-123"), ("user", "alice")]);
    let (out, _) = redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "[REDACTED]");
    assert_eq!(out[1].1, "alice");
}

#[test]
fn redaction_disabled_with_empty_policy() {
    let policy = RedactionPolicy::empty();
    let input = attrs(&[("password", "hunter2"), ("api_key", "abc")]);
    let (out, counters) =
        redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "hunter2");
    assert_eq!(out[1].1, "abc");
    assert_eq!(counters.redacted_fields, 0);
}

#[test]
fn redaction_does_not_count_toward_cardinality() {
    // Sensitive keys DO NOT consume the cardinality budget.
    // All five input keys match default_secrets; "user" is NOT in the
    // default set so we replace it with "auth" (which IS in the set).
    let policy = RedactionPolicy::default_secrets();
    let limits = CardinalityLimits {
        max_distinct_keys: 2,
        ..CardinalityLimits::default()
    };
    let input = attrs(&[
        ("password", "x"),
        ("secret", "y"),
        ("token", "z"),
        ("auth", "u"),
        ("session_id", "s"),
    ]);
    let (_, counters) = redact_and_limit_attributes(input, &policy, &limits);
    assert_eq!(counters.redacted_fields, 5, "all 5 sensitive keys redacted");
    assert_eq!(
        counters.collapsed_cardinality, 0,
        "sensitive keys never collapse — they're redacted instead"
    );
}

#[test]
fn cardinality_caps_distinct_keys() {
    let policy = RedactionPolicy::empty();
    let limits = CardinalityLimits {
        max_distinct_keys: 3,
        ..CardinalityLimits::default()
    };
    let input = attrs(&[("a", "1"), ("b", "2"), ("c", "3"), ("d", "4"), ("e", "5")]);
    let (out, counters) = redact_and_limit_attributes(input, &policy, &limits);
    assert_eq!(counters.collapsed_cardinality, 2, "d and e overflow");
    // a/b/c preserved.
    assert_eq!(out[0], ("a".to_string(), "1".to_string()));
    assert_eq!(out[1], ("b".to_string(), "2".to_string()));
    assert_eq!(out[2], ("c".to_string(), "3".to_string()));
    // d/e collapsed to overflow key.
    assert_eq!(out[3].0, limits.overflow_key);
    assert_eq!(out[3].1, "[cardinality-collapsed]");
    assert_eq!(out[4].0, limits.overflow_key);
    assert_eq!(out[4].1, "[cardinality-collapsed]");
}

#[test]
fn cardinality_does_not_count_repeats() {
    let policy = RedactionPolicy::empty();
    let limits = CardinalityLimits {
        max_distinct_keys: 2,
        ..CardinalityLimits::default()
    };
    // 5 attributes but only 2 distinct keys (a, b). Neither should overflow.
    let input = attrs(&[("a", "1"), ("a", "2"), ("b", "3"), ("a", "4"), ("b", "5")]);
    let (out, counters) = redact_and_limit_attributes(input, &policy, &limits);
    assert_eq!(counters.collapsed_cardinality, 0);
    assert_eq!(out.len(), 5);
}

#[test]
fn cardinality_zero_cap_collapses_everything() {
    let policy = RedactionPolicy::empty();
    let limits = CardinalityLimits {
        max_distinct_keys: 0,
        ..CardinalityLimits::default()
    };
    let input = attrs(&[("a", "1"), ("b", "2")]);
    let (out, counters) = redact_and_limit_attributes(input, &policy, &limits);
    assert_eq!(counters.collapsed_cardinality, 2);
    for (k, v) in &out {
        assert_eq!(k, &limits.overflow_key);
        assert_eq!(v, "[cardinality-collapsed]");
    }
}

#[test]
fn counters_default_is_zero() {
    let c = RedactionCounters::default();
    assert_eq!(c.redacted_fields, 0);
    assert_eq!(c.collapsed_cardinality, 0);
}

#[test]
fn counters_total_unchanged_regardless_of_input_order() {
    let policy = RedactionPolicy::default_secrets();
    let limits = CardinalityLimits {
        max_distinct_keys: 2,
        ..CardinalityLimits::default()
    };
    let fwd = attrs(&[("a", "1"), ("b", "2"), ("c", "3"), ("password", "x")]);
    let rev = attrs(&[("password", "x"), ("c", "3"), ("b", "2"), ("a", "1")]);
    let (_, cf) = redact_and_limit_attributes(fwd, &policy, &limits);
    let (_, cr) = redact_and_limit_attributes(rev, &policy, &limits);
    assert_eq!(cf.redacted_fields, cr.redacted_fields);
    assert_eq!(cf.collapsed_cardinality, cr.collapsed_cardinality);
}

#[test]
fn redaction_with_custom_marker() {
    let policy = RedactionPolicy {
        sensitive_keys: vec!["secret".to_string()],
        redaction_marker: "***".to_string(),
    };
    let input = attrs(&[("secret_value", "x")]);
    let (out, _) = redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "***");
}

#[test]
fn empty_attributes_returns_empty() {
    let (out, counters) = redact_and_limit_attributes(
        Vec::new(),
        &RedactionPolicy::default(),
        &CardinalityLimits::default(),
    );
    assert!(out.is_empty());
    assert_eq!(counters.redacted_fields, 0);
    assert_eq!(counters.collapsed_cardinality, 0);
}

#[test]
fn sensitive_key_substring_match() {
    // "auth" is in default_secrets; "authority" contains "auth" => matches.
    let policy = RedactionPolicy::default_secrets();
    let input = attrs(&[("authority", "admin"), ("user", "alice")]);
    let (out, counters) =
        redact_and_limit_attributes(input, &policy, &CardinalityLimits::default());
    assert_eq!(out[0].1, "[REDACTED]", "substring match triggers redaction");
    assert_eq!(out[1].1, "alice");
    assert_eq!(counters.redacted_fields, 1);
}

#[test]
fn policy_default_matches_default_secrets() {
    let p1 = RedactionPolicy::default();
    let p2 = RedactionPolicy::default_secrets();
    assert_eq!(p1, p2);
}

#[test]
fn cardinality_default_has_64_keys_and_overflow_key() {
    let l = CardinalityLimits::default();
    assert_eq!(l.max_distinct_keys, 64);
    assert_eq!(l.overflow_key, "chronos._cardinality_collapsed");
}
