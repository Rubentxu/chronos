//! Runtime property domain model (pure data).
//!
//! Pure, dependency-free domain types for declarative runtime properties per
//! `docs/.../RUNTIME_PROPERTIES_AND_SLICING.md`. This module defines the
//! property shape (`observe`/`when`/`invariant`) and a deterministic scalar
//! invariant evaluation that returns `Pass`, `Violation`, or
//! `UnsupportedByRecordedEvidence` — never a false `Pass` when the recorded
//! evidence lacks a required observation.

use std::fmt;

/// Stable identifier for a declared property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PropertyId(pub u64);

impl fmt::Display for PropertyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "property-{}", self.0)
    }
}

/// A scalar observed value used by scalar invariants.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PropertyValue {
    Number(f64),
    Text(String),
    Bool(bool),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyValue::Number(n) => write!(f, "{n}"),
            PropertyValue::Text(s) => write!(f, "{s:?}"),
            PropertyValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Comparison operator over two same-typed scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ComparisonOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ComparisonOp::Lt => "<",
            ComparisonOp::Le => "<=",
            ComparisonOp::Gt => ">",
            ComparisonOp::Ge => ">=",
            ComparisonOp::Eq => "==",
            ComparisonOp::Ne => "!=",
        };
        f.write_str(s)
    }
}

/// A deterministic scalar invariant from the DSL subset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum InvariantCheck {
    /// Compare the observed value against a constant of the same type.
    Comparison {
        op: ComparisonOp,
        constant: PropertyValue,
    },
    /// The observed value is present.
    Exists,
    /// The observed value differs from the previous observation.
    Changed,
    /// The observed value equals the previous observation.
    Unchanged,
    /// The signed change `after - before` compared against a threshold.
    Delta { op: ComparisonOp, threshold: f64 },
}

/// A declarative runtime property: observe a target when a trigger fires and
/// check an invariant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Property {
    pub id: PropertyId,
    pub name: String,
    pub version: u32,
    /// Target path being observed, e.g. `Order.total`.
    pub observe: String,
    /// Trigger label, e.g. `after Order.apply_discount`.
    pub trigger: String,
    pub invariant: InvariantCheck,
}

/// Result of evaluating a property against recorded observations.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PropertyOutcome {
    Pass,
    Violation {
        before: Option<PropertyValue>,
        after: PropertyValue,
        message: String,
    },
    UnsupportedByRecordedEvidence {
        reason: String,
    },
}

/// Result of evaluating a property over an ordered sequence of observations.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PropertySequenceOutcome {
    /// Every position in the sequence satisfied the invariant.
    Pass,
    /// First position where the invariant was violated.
    Violation {
        index: usize,
        before: Option<PropertyValue>,
        after: PropertyValue,
        message: String,
    },
    /// First position whose required evidence the sequence does not provide.
    UnsupportedByRecordedEvidence { index: usize, reason: String },
}

/// Display-level actor evidence for a state mutation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MutationActor {
    pub name: String,
    /// Capture-time invocation reference rendered as a string.
    pub invocation: Option<String>,
    /// file:line source label.
    pub source: Option<String>,
}

/// A value change of a target from `before` to `after` at a sequence position,
/// caused by an optional actor.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateTransition {
    pub target: String,
    pub before: Option<PropertyValue>,
    pub after: PropertyValue,
    pub index: usize,
    pub actor: Option<MutationActor>,
}

impl StateTransition {
    /// Render the Mutation-Lens transition form.
    pub fn display(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.target);
        match &self.before {
            Some(before) => s.push_str(&format!("\n{before} -> {}", self.after)),
            None => s.push_str(&format!("\n-> {}", self.after)),
        }
        if let Some(actor) = &self.actor {
            s.push_str(&format!("\nby: {}", actor.name));
            if let Some(inv) = &actor.invocation {
                s.push_str(&format!("#{inv}"));
            }
            if let Some(src) = &actor.source {
                s.push_str(&format!("\nsource: {src}"));
            }
        }
        s
    }
}

/// Persisted evidence bundle linking a property to the transition that first
/// violated its invariant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropertyViolation {
    pub property_id: PropertyId,
    pub property_name: String,
    pub version: u32,
    pub transition: StateTransition,
    pub total_observations: usize,
}

fn same_variant(a: &PropertyValue, b: &PropertyValue) -> bool {
    matches!(
        (a, b),
        (PropertyValue::Number(_), PropertyValue::Number(_))
            | (PropertyValue::Text(_), PropertyValue::Text(_))
            | (PropertyValue::Bool(_), PropertyValue::Bool(_))
    )
}

fn compare(op: ComparisonOp, left: &PropertyValue, right: &PropertyValue) -> bool {
    match (left, right) {
        (PropertyValue::Number(l), PropertyValue::Number(r)) => compare_num(op, *l, *r),
        (PropertyValue::Text(l), PropertyValue::Text(r)) => compare_ord(op, l, r),
        (PropertyValue::Bool(l), PropertyValue::Bool(r)) => compare_ord(op, l, r),
        _ => false,
    }
}

fn compare_num(op: ComparisonOp, l: f64, r: f64) -> bool {
    use std::cmp::Ordering;
    let ord = l.partial_cmp(&r);
    match (op, ord) {
        (ComparisonOp::Eq, Some(Ordering::Equal)) => true,
        (ComparisonOp::Eq, _) => false,
        (ComparisonOp::Ne, Some(Ordering::Equal)) => false,
        (ComparisonOp::Ne, _) => true,
        (ComparisonOp::Lt, Some(Ordering::Less)) => true,
        (ComparisonOp::Lt, _) => false,
        (ComparisonOp::Le, Some(o)) => o != Ordering::Greater,
        (ComparisonOp::Le, _) => false,
        (ComparisonOp::Gt, Some(Ordering::Greater)) => true,
        (ComparisonOp::Gt, _) => false,
        (ComparisonOp::Ge, Some(o)) => o != Ordering::Less,
        (ComparisonOp::Ge, _) => false,
    }
}

fn compare_ord<T: PartialOrd>(op: ComparisonOp, l: &T, r: &T) -> bool {
    match op {
        ComparisonOp::Eq => l == r,
        ComparisonOp::Ne => l != r,
        ComparisonOp::Lt => l < r,
        ComparisonOp::Le => l <= r,
        ComparisonOp::Gt => l > r,
        ComparisonOp::Ge => l >= r,
    }
}

impl Property {
    /// Evaluate this property against recorded observations.
    ///
    /// Never returns `Pass` when the evidence required by the invariant is not
    /// recorded; insufficient evidence yields `UnsupportedByRecordedEvidence`.
    pub fn evaluate(
        &self,
        observed: Option<&PropertyValue>,
        previous: Option<&PropertyValue>,
    ) -> PropertyOutcome {
        match &self.invariant {
            InvariantCheck::Exists => match observed {
                Some(_) => PropertyOutcome::Pass,
                None => PropertyOutcome::UnsupportedByRecordedEvidence {
                    reason: format!("no recorded value for `{}`", self.observe),
                },
            },
            InvariantCheck::Changed | InvariantCheck::Unchanged => {
                let (Some(prev), Some(obs)) = (previous, observed) else {
                    return PropertyOutcome::UnsupportedByRecordedEvidence {
                        reason: format!(
                            "{} requires a previous and current observation of `{}`",
                            match self.invariant {
                                InvariantCheck::Changed => "changed()",
                                _ => "unchanged()",
                            },
                            self.observe
                        ),
                    };
                };
                let differs = prev != obs;
                let holds = match self.invariant {
                    InvariantCheck::Changed => differs,
                    _ => !differs,
                };
                outcome_for(holds, Some(prev), obs, self.observe.as_str())
            }
            InvariantCheck::Comparison { op, constant } => {
                let Some(obs) = observed else {
                    return PropertyOutcome::UnsupportedByRecordedEvidence {
                        reason: format!("no recorded value for `{}`", self.observe),
                    };
                };
                if !same_variant(obs, constant) {
                    return PropertyOutcome::UnsupportedByRecordedEvidence {
                        reason: format!(
                            "type mismatch comparing observed `{obs}` against constant `{constant}` for `{}`",
                            self.observe
                        ),
                    };
                }
                let holds = compare(*op, obs, constant);
                outcome_for(holds, None, obs, self.observe.as_str())
            }
            InvariantCheck::Delta { op, threshold } => {
                let (Some(prev), Some(obs)) = (previous, observed) else {
                    return PropertyOutcome::UnsupportedByRecordedEvidence {
                        reason: format!(
                            "delta() requires a previous and current observation of `{}`",
                            self.observe
                        ),
                    };
                };
                let (PropertyValue::Number(prev_n), PropertyValue::Number(obs_n)) = (prev, obs)
                else {
                    return PropertyOutcome::UnsupportedByRecordedEvidence {
                        reason: format!(
                            "delta() requires numeric observations of `{}`",
                            self.observe
                        ),
                    };
                };
                let change = obs_n - prev_n;
                let holds = compare_num(*op, change, *threshold);
                outcome_for(holds, Some(prev), obs, self.observe.as_str())
            }
        }
    }

    /// Evaluate this property over an ordered sequence of observations.
    ///
    /// Single-observation invariants (`Comparison`, `Exists`) check every
    /// position. Pair invariants (`Changed`, `Unchanged`, `Delta`) check each
    /// transition from observation `i-1` to `i`, and require at least two
    /// observations; a shorter sequence yields
    /// `UnsupportedByRecordedEvidence` (never a false `Pass`).
    pub fn evaluate_sequence(&self, observations: &[PropertyValue]) -> PropertySequenceOutcome {
        match &self.invariant {
            InvariantCheck::Comparison { .. } | InvariantCheck::Exists => {
                for (i, obs) in observations.iter().enumerate() {
                    match self.evaluate(Some(obs), None) {
                        PropertyOutcome::Pass => {}
                        PropertyOutcome::Violation { after, message, .. } => {
                            return PropertySequenceOutcome::Violation {
                                index: i,
                                before: previous_at(observations, i),
                                after,
                                message,
                            };
                        }
                        PropertyOutcome::UnsupportedByRecordedEvidence { reason } => {
                            return PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                                index: i,
                                reason,
                            };
                        }
                    }
                }
            }
            InvariantCheck::Changed | InvariantCheck::Unchanged | InvariantCheck::Delta { .. } => {
                if observations.len() < 2 {
                    return PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                        index: 0,
                        reason: format!(
                            "property `{}` requires at least two observations to \
                             evaluate a change",
                            self.observe
                        ),
                    };
                }
                for i in 1..observations.len() {
                    let prev = &observations[i - 1];
                    let obs = &observations[i];
                    match self.evaluate(Some(obs), Some(prev)) {
                        PropertyOutcome::Pass => {}
                        PropertyOutcome::Violation { after, message, .. } => {
                            return PropertySequenceOutcome::Violation {
                                index: i,
                                before: Some(prev.clone()),
                                after,
                                message,
                            };
                        }
                        PropertyOutcome::UnsupportedByRecordedEvidence { reason } => {
                            return PropertySequenceOutcome::UnsupportedByRecordedEvidence {
                                index: i,
                                reason,
                            };
                        }
                    }
                }
            }
        }
        PropertySequenceOutcome::Pass
    }

    /// Map a `Violation` from `evaluate_sequence` into a `StateTransition`
    /// evidence record. Returns `None` for `Pass`/`UnsupportedByRecordedEvidence`.
    pub fn violation_transition(
        &self,
        outcome: &PropertySequenceOutcome,
    ) -> Option<StateTransition> {
        match outcome {
            PropertySequenceOutcome::Violation {
                index,
                before,
                after,
                ..
            } => Some(StateTransition {
                target: self.observe.clone(),
                before: before.clone(),
                after: after.clone(),
                index: *index,
                actor: None,
            }),
            PropertySequenceOutcome::Pass
            | PropertySequenceOutcome::UnsupportedByRecordedEvidence { .. } => None,
        }
    }

    /// Evaluate this property over a sequence and, on the first violation,
    /// return a persisted `PropertyViolation` bundle. Returns `None` when the
    /// sequence passes or evaluation is unsupported.
    pub fn evaluate_violation(&self, observations: &[PropertyValue]) -> Option<PropertyViolation> {
        let outcome = self.evaluate_sequence(observations);
        let transition = self.violation_transition(&outcome)?;
        Some(PropertyViolation {
            property_id: self.id,
            property_name: self.name.clone(),
            version: self.version,
            transition,
            total_observations: observations.len(),
        })
    }
}

fn previous_at(observations: &[PropertyValue], i: usize) -> Option<PropertyValue> {
    i.checked_sub(1).and_then(|p| observations.get(p).cloned())
}

fn outcome_for(
    holds: bool,
    before: Option<&PropertyValue>,
    after: &PropertyValue,
    observe: &str,
) -> PropertyOutcome {
    if holds {
        PropertyOutcome::Pass
    } else {
        PropertyOutcome::Violation {
            before: before.cloned(),
            after: after.clone(),
            message: format!("invariant violated on `{observe}`: after = {after}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn property_model_serializes_and_round_trips() {
        let p = order_total_property();
        let json = serde_json::to_string(&p).unwrap();
        let back: Property = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        assert_eq!(PropertyId(1).to_string(), "property-1");
        assert_eq!(ComparisonOp::Ge.to_string(), ">=");
        assert!(matches!(
            p.invariant,
            InvariantCheck::Comparison {
                op: ComparisonOp::Ge,
                ..
            }
        ));
    }

    #[test]
    fn non_negative_invariant_passes_then_violates() {
        let p = order_total_property();
        assert_eq!(
            p.evaluate(Some(&PropertyValue::Number(59.0)), None),
            PropertyOutcome::Pass
        );
        let violation = p.evaluate(Some(&PropertyValue::Number(-35.0)), None);
        match violation {
            PropertyOutcome::Violation { after, before, .. } => {
                assert_eq!(after, PropertyValue::Number(-35.0));
                assert_eq!(before, None);
            }
            other => panic!("expected Violation, got {other:?}"),
        }
    }

    #[test]
    fn missing_observation_never_pass() {
        let p = order_total_property();
        let out = p.evaluate(None, None);
        assert!(
            matches!(out, PropertyOutcome::UnsupportedByRecordedEvidence { .. }),
            "missing observation must be Unsupported, got {out:?}"
        );

        // Changed requires previous -> never Pass when previous absent.
        let changed = Property {
            invariant: InvariantCheck::Changed,
            ..order_total_property()
        };
        let out = changed.evaluate(Some(&PropertyValue::Number(1.0)), None);
        assert!(
            matches!(out, PropertyOutcome::UnsupportedByRecordedEvidence { .. }),
            "Changed with no previous must be Unsupported, got {out:?}"
        );
    }

    #[test]
    fn cross_type_comparison_is_unsupported() {
        let p = order_total_property();
        let out = p.evaluate(Some(&PropertyValue::Text("high".into())), None);
        assert!(matches!(
            out,
            PropertyOutcome::UnsupportedByRecordedEvidence { .. }
        ));
    }

    #[test]
    fn changed_and_unchanged() {
        let base = Property {
            invariant: InvariantCheck::Changed,
            ..order_total_property()
        };
        assert_eq!(
            base.evaluate(
                Some(&PropertyValue::Number(2.0)),
                Some(&PropertyValue::Number(1.0))
            ),
            PropertyOutcome::Pass
        );
        let unch = Property {
            invariant: InvariantCheck::Unchanged,
            ..order_total_property()
        };
        assert_eq!(
            unch.evaluate(
                Some(&PropertyValue::Number(1.0)),
                Some(&PropertyValue::Number(1.0))
            ),
            PropertyOutcome::Pass
        );
        // Equal under Changed is a Violation, not Pass.
        let changed_same = base.evaluate(
            Some(&PropertyValue::Number(1.0)),
            Some(&PropertyValue::Number(1.0)),
        );
        assert!(matches!(changed_same, PropertyOutcome::Violation { .. }));
    }

    #[test]
    fn delta_cap_pass_and_violation() {
        let cap = Property {
            invariant: InvariantCheck::Delta {
                op: ComparisonOp::Le,
                threshold: 10.0,
            },
            ..order_total_property()
        };
        // change = -50 - 100 = -150 <= 10 -> Pass.
        assert_eq!(
            cap.evaluate(
                Some(&PropertyValue::Number(-50.0)),
                Some(&PropertyValue::Number(100.0))
            ),
            PropertyOutcome::Pass
        );
        // change = 25 - 5 = 20 > 10 -> Violation.
        let violation = cap.evaluate(
            Some(&PropertyValue::Number(25.0)),
            Some(&PropertyValue::Number(5.0)),
        );
        match violation {
            PropertyOutcome::Violation { before, after, .. } => {
                assert_eq!(before, Some(PropertyValue::Number(5.0)));
                assert_eq!(after, PropertyValue::Number(25.0));
            }
            other => panic!("expected Violation, got {other:?}"),
        }
    }

    #[test]
    fn delta_with_missing_or_non_number_is_unsupported() {
        let cap = Property {
            invariant: InvariantCheck::Delta {
                op: ComparisonOp::Le,
                threshold: 10.0,
            },
            ..order_total_property()
        };
        // missing previous
        let missing = cap.evaluate(Some(&PropertyValue::Number(5.0)), None);
        assert!(matches!(
            missing,
            PropertyOutcome::UnsupportedByRecordedEvidence { .. }
        ));
        // non-number observed
        let non_num = cap.evaluate(
            Some(&PropertyValue::Text("high".into())),
            Some(&PropertyValue::Number(5.0)),
        );
        assert!(matches!(
            non_num,
            PropertyOutcome::UnsupportedByRecordedEvidence { .. }
        ));
    }

    #[test]
    fn sequence_detects_violating_transition_with_before() {
        let p = order_total_property();
        let out = p.evaluate_sequence(&[
            PropertyValue::Number(59.0),
            PropertyValue::Number(0.0),
            PropertyValue::Number(-35.0),
        ]);
        match out {
            PropertySequenceOutcome::Violation {
                index,
                before,
                after,
                ..
            } => {
                assert_eq!(index, 2);
                assert_eq!(before, Some(PropertyValue::Number(0.0)));
                assert_eq!(after, PropertyValue::Number(-35.0));
            }
            other => panic!("expected Violation at index 2, got {other:?}"),
        }
    }

    #[test]
    fn sequence_all_pass() {
        let p = order_total_property();
        let out = p.evaluate_sequence(&[
            PropertyValue::Number(59.0),
            PropertyValue::Number(10.0),
            PropertyValue::Number(0.0),
        ]);
        assert_eq!(out, PropertySequenceOutcome::Pass);
    }

    #[test]
    fn sequence_delta_short_is_unsupported_never_pass() {
        let cap = Property {
            invariant: InvariantCheck::Delta {
                op: ComparisonOp::Le,
                threshold: 10.0,
            },
            ..order_total_property()
        };
        let out = cap.evaluate_sequence(&[PropertyValue::Number(5.0)]);
        match out {
            PropertySequenceOutcome::UnsupportedByRecordedEvidence { index, .. } => {
                assert_eq!(index, 0);
            }
            other => panic!("expected Unsupported at index 0, got {other:?}"),
        }
    }

    #[test]
    fn transition_line_renders_mutation_lens_form() {
        let t = StateTransition {
            target: "Order.total".to_string(),
            before: Some(PropertyValue::Number(100.0)),
            after: PropertyValue::Number(-50.0),
            index: 7,
            actor: Some(MutationActor {
                name: "Discount.apply".to_string(),
                invocation: Some("inv-8281".to_string()),
                source: Some("discount.rs:118".to_string()),
            }),
        };
        let s = t.display();
        assert!(s.contains("Order.total"), "got: {s}");
        assert!(s.contains("-> -50"), "got: {s}");
        assert!(s.contains("Discount.apply"), "got: {s}");
        assert!(s.contains("inv-8281"), "got: {s}");
        assert!(s.contains("discount.rs:118"), "got: {s}");
    }

    #[test]
    fn state_transition_serializes_round_trip() {
        let t = StateTransition {
            target: "Order.total".to_string(),
            before: Some(PropertyValue::Number(59.0)),
            after: PropertyValue::Number(-35.0),
            index: 2,
            actor: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: StateTransition = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn violation_transition_maps_replay_violation() {
        let p = order_total_property();
        let seq = p.evaluate_sequence(&[
            PropertyValue::Number(59.0),
            PropertyValue::Number(0.0),
            PropertyValue::Number(-35.0),
        ]);
        let t = p.violation_transition(&seq).expect("expected a transition");
        assert_eq!(t.target, "Order.total");
        assert_eq!(t.before, Some(PropertyValue::Number(0.0)));
        assert_eq!(t.after, PropertyValue::Number(-35.0));
        assert_eq!(t.index, 2);
        assert_eq!(t.actor, None);
    }

    #[test]
    fn violation_transition_non_violation_is_none() {
        let p = order_total_property();
        let pass = p.evaluate_sequence(&[PropertyValue::Number(59.0), PropertyValue::Number(10.0)]);
        assert_eq!(pass, PropertySequenceOutcome::Pass);
        assert!(p.violation_transition(&pass).is_none());
        let short = p.evaluate_sequence(&[PropertyValue::Number(59.0)]);
        assert!(p.violation_transition(&short).is_none());
    }

    #[test]
    fn evaluate_violation_builds_bundle_on_violation() {
        let p = order_total_property();
        let v = p
            .evaluate_violation(&[
                PropertyValue::Number(59.0),
                PropertyValue::Number(0.0),
                PropertyValue::Number(-35.0),
            ])
            .expect("expected a violation bundle");
        assert_eq!(v.property_id, PropertyId(1));
        assert_eq!(v.property_name, "order_total_non_negative");
        assert_eq!(v.version, 1);
        assert_eq!(v.transition.before, Some(PropertyValue::Number(0.0)));
        assert_eq!(v.transition.after, PropertyValue::Number(-35.0));
        assert_eq!(v.transition.index, 2);
        assert_eq!(v.total_observations, 3);
    }

    #[test]
    fn evaluate_violation_passing_sequence_is_none() {
        let p = order_total_property();
        let v = p.evaluate_violation(&[
            PropertyValue::Number(59.0),
            PropertyValue::Number(10.0),
            PropertyValue::Number(0.0),
        ]);
        assert_eq!(v, None);
    }

    #[test]
    fn property_violation_serializes_round_trip() {
        let p = order_total_property();
        let v = p
            .evaluate_violation(&[
                PropertyValue::Number(59.0),
                PropertyValue::Number(0.0),
                PropertyValue::Number(-35.0),
            ])
            .unwrap();
        let json = serde_json::to_string(&v).unwrap();
        let back: PropertyViolation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }
}
