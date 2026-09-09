//! UAT end-to-end test: negative order total detected via property projection.
//!
//! Verifies the full M3 data pipeline (Property → PropertyValue extraction →
//! PropertySequenceOutcome → PropertyViolation) for the canonical M3 bug:
//! `apply_discount` with discount > total causes total to go negative.

use chronos_domain::{
    property::{ComparisonOp, InvariantCheck, Property, PropertySequenceOutcome, PropertyValue},
    trace::{EventData, EventType, SourceLocation},
    PropertyId, PropertyViolation, StateTransition, VariableInfo, VariableScope,
};
use chronos_query::{PropertyProjection, PropertyProjectionReport, QueryEngine};

fn make_var_event(
    event_id: u64,
    timestamp_ns: u64,
    thread_id: u64,
    name: &str,
    value: &str,
    type_name: &str,
    address: u64,
) -> chronos_domain::TraceEvent {
    chronos_domain::TraceEvent::new(
        event_id,
        timestamp_ns,
        thread_id,
        EventType::VariableWrite,
        SourceLocation::from_address(address),
        EventData::Variable(VariableInfo::new(
            name,
            value,
            type_name,
            address,
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
        trigger: "after Order::apply_discount".to_string(),
        invariant: InvariantCheck::Comparison {
            op: ComparisonOp::Ge,
            constant: PropertyValue::Number(0.0),
        },
    }
}

#[test]
fn m3_uat_negative_order_total_detected_via_property_projection() {
    // Arrange: 2 VariableWrite events for "total":
    // 1. total = 100.00 (order created)
    // 2. total = -50.00 (apply_discount with discount > total: no min-zero check)
    let events = vec![
        make_var_event(1, 1000, 1, "total", "100.00", "f64", 0x1000),
        make_var_event(2, 2000, 1, "total", "-50.00", "f64", 0x1000),
    ];
    let engine = QueryEngine::new(events);
    let property = order_total_non_negative();

    // Act
    let report: PropertyProjectionReport = PropertyProjection::run(&property, &engine);

    // Assert: report reflects 2 observations with violation at index 1
    assert_eq!(report.observations_count, 2);

    let PropertySequenceOutcome::Violation {
        index,
        before: _,
        after,
        message: _,
    } = &report.outcome
    else {
        panic!("expected Violation outcome, got {:?}", report.outcome);
    };

    assert_eq!(
        *index, 1,
        "violation must be at index 1 (second observation)"
    );
    assert_eq!(*after, PropertyValue::Number(-50.0), "after must be -50.0");

    // Build PropertyViolation bundle from the report
    let (violation_index, violation_before, violation_after) = match &report.outcome {
        PropertySequenceOutcome::Violation {
            index,
            before,
            after,
            message: _,
        } => (*index, before.clone(), after.clone()),
        other => panic!("expected Violation outcome, got {other:?}"),
    };

    let bundle = PropertyViolation {
        property_id: report.property_id,
        property_name: report.property_name.clone(),
        version: 1,
        transition: StateTransition {
            target: property.observe.clone(),
            before: violation_before,
            after: violation_after,
            index: violation_index,
            actor: None,
        },
        total_observations: report.observations_count,
    };

    // Assert: violation bundle has correct target and negative after value
    assert_eq!(bundle.transition.target, "total");
    assert_eq!(bundle.transition.after, PropertyValue::Number(-50.0));
    assert_eq!(bundle.total_observations, 2);
}
