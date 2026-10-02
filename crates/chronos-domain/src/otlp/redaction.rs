//! M6.5 lift: redaction + cardinality limits, generic over attribute
//! shapes.
//!
//! Lifted from `/home/rubentxu/m6-spikes/m6.5-otel-redaction/` (237L).
//!
//! The spike's `redact_and_limit_spans()` is generic over `ExportedSpan`
//! (from M6.4). This product-side lift is generic over the simplest
//! attribute representation — `Vec<(String, String)>` — so it can be
//! applied to any producer of attribute pairs without coupling to a
//! specific span type. That generic constrains the attribute shape only;
//! it does not imply that a given consumer applies the policy.
//!
//! ## What is lifted
//!   - [`RedactionPolicy`] — case-insensitive substring match against
//!     attribute keys; sensitive value → `redaction_marker` placeholder.
//!     Default key set covers common secret-bearing substrings.
//!   - [`CardinalityLimits`] — caps the number of distinct attribute
//!     keys across an export call. Overflow keys collapse to a shared
//!     `overflow_key` with a literal placeholder.
//!   - [`CardinalityBudget`] — the per-call state that cap is counted
//!     against, so the limit spans every span of one call.
//!   - [`RedactionCounters`] — observation of what was applied.
//!   - [`redact_and_limit_attributes`] — the algorithm, generic over
//!     `Vec<(String, String)>`, under a budget of its own.
//!   - [`redact_and_limit_attributes_within`] — the same algorithm
//!     charged against a budget the caller owns, which is what makes the
//!     "across the entire export call" scoping above true rather than
//!     aspirational.
//!
//! ## What is NOT lifted
//!   - The spike's `ExportedSpan` / `render_json_line` (M6.4). The product
//!     exporter `chronos-services::session_export` shares only the OTel
//!     envelope shape, so "no duplication needed" overstates the overlap:
//!     ADR-0018's opt-in filter, export limits and JSON Lines renderer have no
//!     counterpart there. That gap was closed by the M6.4 lift, which added
//!     `otlp::exporter` — so M6.5 now composes with a real exporter instead of
//!     being generic over a shape nothing produces.
//!
//! ## Where it is applied
//!
//! [`super::gates::run_service_pipeline`] calls
//! [`redact_and_limit_attributes_within`] between span construction
//! ([`super::exporter::export_spans`]) and rendering
//! ([`super::exporter::render_json_line`]), with one
//! [`CardinalityBudget`] for the whole call, so M6.5 is an applied policy
//! on the product pipeline, not a library awaiting one. `super::exporter::export`
//! deliberately does not redact: it is the renderer, and a policy it applied
//! silently would make "render this span" and "render this span under policy
//! P" the same call with no way to name which one ran.
//!
//! Not a consumer: `chronos-services::session_export` never calls this
//! policy. It serializes the `TraceEvent`s it pulls from
//! `engine.get_all_events()` as they are, and those events can carry
//! captured process memory as raw bytes in [`crate::EventData::Memory`].

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
    ///
    /// "Across the export call" is scoped by [`CardinalityBudget`], not by
    /// this value: the number only means something once a budget is being
    /// carried across spans.
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

/// The distinct-attribute-key budget of one export call.
///
/// ADR-0019 §2.3 caps distinct keys "across the entire export call (not per
/// span)", so the budget has to outlive a single attribute list: every span
/// of the same call is admitted against the same one. A per-span budget is
/// the unbounded-cardinality case the cap exists to prevent, just spread
/// over N spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardinalityBudget {
    seen: std::collections::BTreeSet<String>,
    max_distinct_keys: usize,
}

impl CardinalityBudget {
    /// An empty budget sized by `limits`. `max_distinct_keys == usize::MAX`
    /// never fills, which is how the cap is disabled (ADR-0019 §3.2);
    /// `0` refuses the first key, which is how it is made strictest (§3.3).
    pub fn new(limits: &CardinalityLimits) -> Self {
        Self {
            seen: std::collections::BTreeSet::new(),
            max_distinct_keys: limits.max_distinct_keys,
        }
    }

    /// Whether `key` still fits in the budget, consuming a slot when it does.
    /// A key already admitted stays admitted: a repeated key is not a new
    /// series for the collector.
    fn admits(&mut self, key: &str) -> bool {
        if self.seen.contains(key) {
            return true;
        }
        if self.seen.len() >= self.max_distinct_keys {
            return false;
        }
        self.seen.insert(key.to_string());
        true
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
/// This is one attribute list under its own budget — the shape the lifted
/// unit tests exercise. A caller emitting more than one span in a single
/// export call wants [`redact_and_limit_attributes_within`] with one
/// [`CardinalityBudget`] for the whole call, which is what
/// `super::gates::run_service_pipeline` does.
///
/// Sensitive keys (those that match `policy.matches()`) are ALWAYS
/// redacted regardless of cardinality — their values become the marker
/// and they do NOT count toward the distinct-keys budget.
pub fn redact_and_limit_attributes(
    attributes: Vec<(String, String)>,
    policy: &RedactionPolicy,
    limits: &CardinalityLimits,
) -> (Vec<(String, String)>, RedactionCounters) {
    let mut budget = CardinalityBudget::new(limits);
    redact_and_limit_attributes_within(attributes, policy, limits, &mut budget)
}

/// As [`redact_and_limit_attributes`], but charged against a budget the
/// caller owns, so the cap spans every attribute list it passes here.
pub fn redact_and_limit_attributes_within(
    attributes: Vec<(String, String)>,
    policy: &RedactionPolicy,
    limits: &CardinalityLimits,
    budget: &mut CardinalityBudget,
) -> (Vec<(String, String)>, RedactionCounters) {
    use std::collections::BTreeSet;

    let mut counters = RedactionCounters::default();

    // Pre-pass: classify keys.
    //   - sensitive: redacted, not counted toward cardinality budget.
    //   - non-sensitive, admitted by the shared budget: kept.
    //   - non-sensitive, beyond the budget: marked for collapse.
    let mut overflow_keys: BTreeSet<String> = BTreeSet::new();
    for (key, _) in &attributes {
        if policy.matches(key) || overflow_keys.contains(key) {
            continue;
        }
        if !budget.admits(key) {
            overflow_keys.insert(key.clone());
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
