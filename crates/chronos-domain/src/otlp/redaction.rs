//! M6.5 lift: redaction + cardinality limits, generic over attribute
//! shapes.
//!
//! Lifted from `/home/rubentxu/m6-spikes/m6.5-otel-redaction/` (237L).
//!
//! The spike's `redact_and_limit_spans()` is generic over `ExportedSpan`
//! (from M6.4). This product-side lift is generic over the simplest
//! attribute representation — `Vec<(String, String)>` — so it can be
//! applied to ANY exporter (including `chronos-services::session_export`)
//! without coupling to a specific span type.
//!
//! ## What is lifted
//!   - [`RedactionPolicy`] — case-insensitive substring match against
//!     attribute keys; sensitive value → `redaction_marker` placeholder.
//!     Default key set covers common secret-bearing substrings.
//!   - [`CardinalityLimits`] — caps the number of distinct attribute
//!     keys across an export call. Overflow keys collapse to a shared
//!     `overflow_key` with a literal placeholder.
//!   - [`RedactionCounters`] — observation of what was applied.
//!   - [`redact_and_limit_attributes`] — the algorithm, generic over
//!     `Vec<(String, String)>`.
//!
//! ## What is NOT lifted
//!   - The spike's `ExportedSpan` / `render_json_line` (M6.4). The
//!     product already has `chronos-services::session_export` with the
//!     same wire-format shape — no duplication needed.

/// Redaction policy. Sensitive key patterns are matched
/// case-insensitive against the attribute key name; the value of any
/// matched key is replaced with the configured `redaction_marker`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPolicy {
    /// Attribute key substrings that mark a value as secret.
    /// Case-insensitive. Default set covers common secret-bearing
    /// substrings (`"password"`, `"token"`, `"api_key"`, `"auth"`,
    /// `"bearer"`, etc.).
    pub sensitive_keys: Vec<String>,
    /// Placeholder substituted for any redacted value. Default:
    /// `"[REDACTED]"`.
    pub redaction_marker: String,
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self::default_secrets()
    }
}

impl RedactionPolicy {
    /// Common secret-bearing key substrings (conservative defaults).
    pub fn default_secrets() -> Self {
        Self {
            sensitive_keys: vec![
                "password".to_string(),
                "passwd".to_string(),
                "secret".to_string(),
                "token".to_string(),
                "api_key".to_string(),
                "apikey".to_string(),
                "auth".to_string(),
                "bearer".to_string(),
                "private_key".to_string(),
                "privatekey".to_string(),
                "credential".to_string(),
                "credit_card".to_string(),
                "creditcard".to_string(),
                "ssn".to_string(),
                "session_id".to_string(),
                "sessionid".to_string(),
                "email".to_string(),
            ],
            redaction_marker: "[REDACTED]".to_string(),
        }
    }

    /// Empty policy: no redaction. Use explicitly to disable redaction.
    pub fn empty() -> Self {
        Self {
            sensitive_keys: Vec::new(),
            redaction_marker: "[REDACTED]".to_string(),
        }
    }

    /// Custom sensitive keys (case-insensitive substring match).
    pub fn with_keys(keys: Vec<String>) -> Self {
        Self {
            sensitive_keys: keys,
            redaction_marker: "[REDACTED]".to_string(),
        }
    }

    /// `true` iff `key` matches any sensitive substring (case-insensitive).
    pub fn matches(&self, key: &str) -> bool {
        let lower = key.to_lowercase();
        self.sensitive_keys.iter().any(|p| lower.contains(p))
    }
}

/// Cardinality limits. Distinct attribute keys beyond `max_distinct_keys`
/// collapse to `overflow_key` with the literal value
/// `"[cardinality-collapsed]"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardinalityLimits {
    /// Maximum distinct attribute keys across the export call. Default: 64.
    pub max_distinct_keys: usize,
    /// The shared key used when a key is collapsed.
    /// Default: `"chronos._cardinality_collapsed"`.
    pub overflow_key: String,
}

impl Default for CardinalityLimits {
    fn default() -> Self {
        Self {
            max_distinct_keys: 64,
            overflow_key: "chronos._cardinality_collapsed".to_string(),
        }
    }
}

/// Counters emitted by [`redact_and_limit_attributes`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactionCounters {
    /// Attribute values redacted because their key matched a sensitive
    /// pattern.
    pub redacted_fields: usize,
    /// Attribute values collapsed to the overflow key because the
    /// cardinality cap was hit.
    pub collapsed_cardinality: usize,
}

/// Apply redaction + cardinality limits to a slice of attribute pairs.
///
/// Returns the modified pairs and counters. Pure function, no I/O.
///
/// Sensitive keys (those that match `policy.matches()`) are ALWAYS
/// redacted regardless of cardinality — their values become the marker
/// and they do NOT count toward the distinct-keys budget.
pub fn redact_and_limit_attributes(
    attributes: Vec<(String, String)>,
    policy: &RedactionPolicy,
    limits: &CardinalityLimits,
) -> (Vec<(String, String)>, RedactionCounters) {
    use std::collections::{BTreeMap, BTreeSet};

    let mut counters = RedactionCounters::default();

    // Pre-pass: classify keys.
    //   - sensitive: redacted, not counted toward cardinality budget.
    //   - non-sensitive, first-occurrence distinct: counted toward budget.
    //   - non-sensitive, distinct beyond budget: marked for collapse.
    let mut distinct_keys_seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut overflow_keys: BTreeSet<String> = BTreeSet::new();

    for (key, _) in &attributes {
        if policy.matches(key) {
            continue;
        }
        if !distinct_keys_seen.contains_key(key) {
            if distinct_keys_seen.len() >= limits.max_distinct_keys {
                overflow_keys.insert(key.clone());
            } else {
                distinct_keys_seen.insert(key.clone(), 1);
            }
        } else if let Some(v) = distinct_keys_seen.get_mut(key) {
            *v += 1;
        }
    }

    // Apply pass: replace values per the classifications above.
    let mut new_attributes: Vec<(String, String)> = Vec::with_capacity(attributes.len());
    for (key, value) in attributes {
        if policy.matches(&key) {
            counters.redacted_fields += 1;
            new_attributes.push((key, policy.redaction_marker.clone()));
        } else if overflow_keys.contains(&key) {
            counters.collapsed_cardinality += 1;
            new_attributes.push((
                limits.overflow_key.clone(),
                "[cardinality-collapsed]".to_string(),
            ));
        } else {
            new_attributes.push((key, value));
        }
    }

    (new_attributes, counters)
}
