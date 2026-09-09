//! Smoke test for `PropertyProjection` — integration test verifying the full
//! round-trip from trace events through the projection to a `Violation` outcome.

use chronos_domain::{
    property::{ComparisonOp, InvariantCheck, Property, PropertySequenceOutcome, PropertyValue},
    trace::{EventData, EventType, SourceLocation},
    PropertyId, VariableInfo, VariableScope,
};
use chronos_query::{PropertyProjection, PropertyProjectionReport, QueryEngine};

fn make_var_event(
    event_id: u64,
    ts: u64,
    name: &str,
    value: &str,
    type_name: &str,
) -> chronos_domain::TraceEvent {
    chronos_domain::TraceEvent::new(
        event_id,
        ts,
        1,
        EventType::VariableWrite,
        SourceLocation::new("test.rs", 10, "apply_discount", 0x1000),
        EventData::Variable(VariableInfo::new(
            name,
            value,
            type_name,
            0x2000,
            VariableScope::Local,
        )),
    )
}

fn order_total_non_negative() -> Property {
    Property {
        id: PropertyId(1),
        name: "order_total_non_negative".to_string(),
        version: 1,
        observe: "total".to_string(),
        trigger: "after Order.apply_discount".to_string(),
        invariant: InvariantCheck::Comparison {
            op: ComparisonOp::Ge,
            constant: PropertyValue::Number(0.0),
        },
    }
}

#[test]
fn property_projection_smoke_violation_at_third_observation() {
    // Arrange: 3 VariableWrite events for "total" with values 100, 50, -50.
    // The invariant is `total >= 0`. The third observation (-50) should trigger
    // a Violation at index 2.
    let events = vec![
        make_var_event(1, 100, "total", "100", "f64"),
        make_var_event(2, 200, "total", "50", "f64"),
        make_var_event(3, 300, "total", "-50", "f64"),
    ];
    let engine = QueryEngine::new(events);
    let property = order_total_non_negative();

    // Act
    let report: PropertyProjectionReport = PropertyProjection::run(&property, &engine);

    // Assert
    assert_eq!(report.property_id, PropertyId(1));
    assert_eq!(report.property_name, "order_total_non_negative");
    assert_eq!(report.observations_count, 3);

    match report.outcome {
        PropertySequenceOutcome::Violation {
            index,
            before,
            after,
            ..
        } => {
            assert_eq!(index, 2, "violation must be at index 2 (third observation)");
            assert_eq!(
                before,
                Some(PropertyValue::Number(50.0)),
                "before must be the value at index 1"
            );
            assert_eq!(
                after,
                PropertyValue::Number(-50.0),
                "after must be -50.0"
            );
        }
        other => panic!(
            "expected Violation{{ index: 2, before: Number(50.0), after: Number(-50.0) }}, got {other:?}"
        ),
    }
}
