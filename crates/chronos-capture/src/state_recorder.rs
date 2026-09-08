//! In-process typed state-observation recorder.
//!
//! This is the source-instrumentation seam for Chronos semantic state probes
//! (ADAPTIVE L4): instrumented application code calls [`StateObservationRecorder::record`]
//! at a mutation site with the target's typed value, and the recorder accumulates
//! per-target ordered [`PropertyValue`] observations. Those observations feed the
//! M3 property evaluator (`Property::evaluate_sequence` / `evaluate_violation`),
//! giving a real in-process observation feed without DWARF/ptrace.

use std::collections::HashMap;

use chronos_domain::{Property, PropertySequenceOutcome, PropertyValue, PropertyViolation};

/// Records ordered typed observations per target in call order.
///
/// Pure in-memory, single-threaded. It deliberately keeps no I/O so it can be
/// used directly by instrumented code and exercised in unit tests.
#[derive(Debug, Clone, Default)]
pub struct StateObservationRecorder {
    values: HashMap<String, Vec<PropertyValue>>,
}

impl StateObservationRecorder {
    /// Create an empty recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a typed observation for `target` in call order.
    pub fn record(&mut self, target: impl Into<String>, value: PropertyValue) {
        self.values.entry(target.into()).or_default().push(value);
    }

    /// Recorded values for `target`, or `&[]` when none were recorded.
    pub fn observations(&self, target: &str) -> &[PropertyValue] {
        self.values.get(target).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Targets that have at least one recorded observation (any order).
    pub fn targets(&self) -> Vec<&str> {
        self.values.keys().map(String::as_str).collect()
    }

    /// Evaluate `property` over the observations recorded for `property.observe`.
    ///
    /// Returns `UnsupportedByRecordedEvidence` when the feed has no observations
    /// for the property's target, so an empty feed is never reported as `Pass`.
    pub fn evaluate_outcome(&self, property: &Property) -> PropertySequenceOutcome {
        let observations = self.observations(&property.observe);
        if observations.is_empty() {
            return PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                index: 0,
                reason: format!("no recorded observations for `{}`", property.observe),
            };
        }
        property.evaluate_sequence(observations)
    }

    /// Evaluate `property` over the recorded observations and return the
    /// persisted violation bundle when the invariant is violated, else `None`.
    pub fn evaluate_violation(&self, property: &Property) -> Option<PropertyViolation> {
        let observations = self.observations(&property.observe).to_vec();
        if observations.is_empty() {
            return None;
        }
        property.evaluate_violation(&observations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{ComparisonOp, InvariantCheck, Property, PropertyId};

    fn order_total_property() -> Property {
        Property {
            id: PropertyId(1),
            name: "order_total_non_negative".to_string(),
            version: 1,
            observe: "Order.total".to_string(),
            trigger: "after Order.apply_discount".to_string(),
            invariant: InvariantCheck::Comparison {
                op: ComparisonOp::Ge,
                constant: PropertyValue::Number(0.0),
            },
        }
    }

    #[test]
    fn discount_feed_drives_non_negative_invariant() {
        let mut rec = StateObservationRecorder::new();
        rec.record("Order.total", PropertyValue::Number(59.0));
        rec.record("Order.total", PropertyValue::Number(0.0));
        rec.record("Order.total", PropertyValue::Number(-35.0));

        let property = order_total_property();
        let violation = rec
            .evaluate_violation(&property)
            .expect("expected a violation");
        assert_eq!(violation.transition.after, PropertyValue::Number(-35.0));
        assert_eq!(violation.transition.index, 2);
        assert_eq!(violation.total_observations, 3);
    }

    #[test]
    fn empty_feed_is_unsupported_never_false_pass() {
        let rec = StateObservationRecorder::new();
        let property = order_total_property();
        let outcome = rec.evaluate_outcome(&property);
        assert!(
            matches!(
                outcome,
                PropertySequenceOutcome::UnsupportedByRecordedEvidence { .. }
            ),
            "empty feed must be Unsupported, got {outcome:?}"
        );
        assert!(rec.evaluate_violation(&property).is_none());
    }

    #[test]
    fn observations_accumulate_and_targets_listed() {
        let mut rec = StateObservationRecorder::new();
        assert_eq!(rec.observations("Order.total"), &[] as &[PropertyValue]);
        rec.record("Order.total", PropertyValue::Number(1.0));
        rec.record("Order.total", PropertyValue::Number(2.0));
        rec.record("Other", PropertyValue::Bool(true));
        assert_eq!(rec.observations("Order.total").len(), 2);
        assert_eq!(rec.targets().len(), 2);
    }
}
