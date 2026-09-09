//! Property projection — maps trace events to property observations and evaluates invariants.
//!
//! v1 scope: `VariableWrite` events whose `VariableInfo::name` matches `property.observe`.
//! Numeric type names (starting with 'i', 'u', or 'f') are parsed as `f64`; all others
//! become `PropertyValue::Text`.

use chronos_domain::{
    property::{Property, PropertySequenceOutcome, PropertyValue},
    trace::{EventData, TraceEvent},
    PropertyId,
};

use crate::engine::QueryEngine;

/// Report returned by a property projection run.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyProjectionReport {
    /// The property that was evaluated.
    pub property_id: PropertyId,
    /// Human-readable name of the property.
    pub property_name: String,
    /// Number of observations extracted from the trace.
    pub observations_count: usize,
    /// Result of evaluating the invariant over the observation sequence.
    pub outcome: PropertySequenceOutcome,
}

/// Zero-sized marker struct — all logic is in the associated `run` method.
pub struct PropertyProjection;

impl PropertyProjection {
    /// Run the projection: extract all matching observations and evaluate the invariant.
    ///
    /// Returns a `PropertyProjectionReport` regardless of outcome.
    /// If no matching events are found the outcome is
    /// `UnsupportedByRecordedEvidence { index: 0, reason: "…" }`.
    pub fn run(property: &Property, engine: &QueryEngine) -> PropertyProjectionReport {
        // Collect all matching observations, sorted by timestamp.
        let mut observations: Vec<_> = engine
            .events()
            .iter()
            .filter_map(|event| Self::extract_observation(event, property))
            .collect();

        observations.sort_by_key(|(_, ts, _)| *ts);

        let observations_count = observations.len();
        let values: Vec<PropertyValue> = observations.into_iter().map(|(v, _, _)| v).collect();

        let outcome = if values.is_empty() {
            PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                index: 0,
                reason: format!("no recorded events for `{}`", property.observe),
            }
        } else {
            property.evaluate_sequence(&values)
        };

        PropertyProjectionReport {
            property_id: property.id,
            property_name: property.name.clone(),
            observations_count,
            outcome,
        }
    }

    /// Extract a single observation from a trace event, if it matches the property.
    ///
    /// Only `EventData::Variable(v)` events where `v.name == property.observe` are
    /// considered. The returned tuple is `(parsed_value, timestamp_ns, event_id)`.
    fn extract_observation(
        event: &TraceEvent,
        property: &Property,
    ) -> Option<(PropertyValue, u64, u64)> {
        let EventData::Variable(var) = &event.data else {
            return None;
        };
        if var.name != property.observe {
            return None;
        }
        let value = Self::parse_value(&var.value, &var.type_name);
        Some((value, event.timestamp_ns, event.event_id))
    }

    /// Parse a `VariableInfo` string value into a `PropertyValue`.
    ///
    /// If `type_name` starts with 'i', 'u', or 'f' and the value parses as `f64`,
    /// returns `PropertyValue::Number`. Otherwise falls back to `PropertyValue::Text`.
    fn parse_value(value: &str, type_name: &str) -> PropertyValue {
        let first_char = type_name.chars().next();
        if let Some(c) = first_char {
            if c == 'i' || c == 'u' || c == 'f' {
                if let Ok(n) = value.parse::<f64>() {
                    return PropertyValue::Number(n);
                }
            }
        }
        PropertyValue::Text(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::VariableInfo;
    use chronos_domain::{
        property::{ComparisonOp, InvariantCheck},
        trace::{EventData, EventType, SourceLocation},
        VariableScope,
    };

    fn make_var_event(
        event_id: u64,
        ts: u64,
        name: &str,
        value: &str,
        type_name: &str,
    ) -> TraceEvent {
        TraceEvent::new(
            event_id,
            ts,
            1,
            EventType::VariableWrite,
            SourceLocation::new("test.rs", 10, "fn", 0x1000),
            EventData::Variable(VariableInfo::new(
                name,
                value,
                type_name,
                0x2000,
                VariableScope::Local,
            )),
        )
    }

    fn non_neg_property(observe: &str) -> Property {
        Property {
            id: PropertyId(1),
            name: "order_total_non_negative".to_string(),
            version: 1,
            observe: observe.to_string(),
            trigger: "after Order.apply".to_string(),
            invariant: InvariantCheck::Comparison {
                op: ComparisonOp::Ge,
                constant: PropertyValue::Number(0.0),
            },
        }
    }

    fn changed_property(observe: &str) -> Property {
        Property {
            id: PropertyId(2),
            name: "value_changed".to_string(),
            version: 1,
            observe: observe.to_string(),
            trigger: "always".to_string(),
            invariant: InvariantCheck::Changed,
        }
    }

    // ─── Unit tests ──────────────────────────────────────────────────────────────────

    #[test]
    fn run_with_no_matching_events_returns_unsupported() {
        let events = vec![make_var_event(1, 100, "other_var", "42", "i32")];
        let engine = QueryEngine::new(events);
        let property = non_neg_property("total");

        let report = PropertyProjection::run(&property, &engine);

        assert_eq!(report.observations_count, 0);
        assert_eq!(
            report.outcome,
            PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                index: 0,
                reason: "no recorded events for `total`".to_string()
            }
        );
    }

    #[test]
    fn run_with_text_observations_returns_pass_when_comparison_holds() {
        // Text values cannot be compared with Number(0), so the outcome is Unsupported.
        // We test the pass case by using a text comparison instead.
        let events = vec![
            make_var_event(1, 100, "status", "ok", "str"),
            make_var_event(2, 200, "status", "ok", "str"),
        ];
        let engine = QueryEngine::new(events);
        let property = Property {
            id: PropertyId(3),
            name: "status_always_ok".to_string(),
            version: 1,
            observe: "status".to_string(),
            trigger: "always".to_string(),
            invariant: InvariantCheck::Comparison {
                op: ComparisonOp::Eq,
                constant: PropertyValue::Text("ok".to_string()),
            },
        };

        let report = PropertyProjection::run(&property, &engine);

        assert_eq!(report.observations_count, 2);
        assert_eq!(report.outcome, PropertySequenceOutcome::Pass);
    }

    #[test]
    fn run_with_numeric_sequence_detects_violation() {
        let events = vec![
            make_var_event(1, 100, "total", "100", "f64"),
            make_var_event(2, 200, "total", "50", "f64"),
            make_var_event(3, 300, "total", "-50", "f64"),
        ];
        let engine = QueryEngine::new(events);
        let property = non_neg_property("total");

        let report = PropertyProjection::run(&property, &engine);

        assert_eq!(report.observations_count, 3);
        match &report.outcome {
            PropertySequenceOutcome::Violation {
                index,
                before,
                after,
                ..
            } => {
                assert_eq!(*index, 2);
                assert_eq!(before.as_ref(), Some(&PropertyValue::Number(50.0)));
                assert_eq!(after, &PropertyValue::Number(-50.0));
            }
            other => panic!("expected Violation at index 2, got {other:?}"),
        }
    }

    #[test]
    fn run_with_changed_invariant_passes_on_actual_change() {
        let events = vec![
            make_var_event(1, 100, "counter", "10", "i32"),
            make_var_event(2, 200, "counter", "20", "i32"),
        ];
        let engine = QueryEngine::new(events);
        let property = changed_property("counter");

        let report = PropertyProjection::run(&property, &engine);

        assert_eq!(report.observations_count, 2);
        assert_eq!(report.outcome, PropertySequenceOutcome::Pass);
    }
}
