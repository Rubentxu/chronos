//! Persist in-process state observations into the ExecutionLog.
//!
//! Bridges the m4b-01 [`StateObservationRecorder`] model to `chronos-log`:
//! [`ObservationLogWriter`] appends each typed observation as an
//! `ExecutionRecord` (payload tag = target, payload bytes = serde_json of the
//! `PropertyValue`), and the `replay_*` helpers read a session back in seq
//! order so the M3 property evaluator can consume a durable, replayable feed.

use std::sync::Arc;

use chronos_domain::{Property, PropertySequenceOutcome, PropertyValue, PropertyViolation};
use chronos_log::{
    ExecutionLog, ExecutionLogBackend, LogConsumerId, LogError, ReadResult, SessionId,
};

/// Errors surfaced by the observation log bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    /// Underlying execution-log error.
    Log(String),
    /// Failed to encode a `PropertyValue` for storage.
    Encode(String),
    /// Failed to decode a stored value.
    Decode(String),
    /// The session has no records.
    SessionNotFound,
    /// A consumer cursor became stale mid-replay.
    CursorStale,
}

impl From<LogError> for PersistError {
    fn from(e: LogError) -> Self {
        PersistError::Log(e.to_string())
    }
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Log(m) => write!(f, "log error: {m}"),
            PersistError::Encode(m) => write!(f, "encode error: {m}"),
            PersistError::Decode(m) => write!(f, "decode error: {m}"),
            PersistError::SessionNotFound => f.write_str("session not found"),
            PersistError::CursorStale => f.write_str("stale consumer cursor"),
        }
    }
}

impl std::error::Error for PersistError {}

/// Appends typed state observations into an execution-log session.
#[derive(Debug, Clone)]
pub struct ObservationLogWriter<B: ExecutionLogBackend + ?Sized> {
    log: Arc<ExecutionLog<B>>,
    session: SessionId,
    next_ns: u64,
}

impl<B: ExecutionLogBackend + ?Sized> ObservationLogWriter<B> {
    /// Create a writer for `session` that appends into `log`.
    pub fn new(log: Arc<ExecutionLog<B>>, session: SessionId) -> Self {
        Self {
            log,
            session,
            next_ns: 0,
        }
    }

    /// The session this writer appends to.
    pub fn session_id(&self) -> &SessionId {
        &self.session
    }

    /// Append one typed observation for `target`, returning its assigned seq.
    pub fn record(
        &mut self,
        target: impl Into<String>,
        value: PropertyValue,
    ) -> Result<chronos_log::EventSeq, PersistError> {
        let tag = target.into();
        let bytes = serde_json::to_vec(&value).map_err(|e| PersistError::Encode(e.to_string()))?;
        let payload = chronos_log::ExecutionPayload::new(bytes, tag);
        let seq = self
            .log
            .append_record(self.session.clone(), self.next_ns, payload)?;
        self.next_ns += 1;
        Ok(seq)
    }
}

fn read_all<B: ExecutionLogBackend + ?Sized>(
    log: &ExecutionLog<B>,
    session: &SessionId,
) -> Result<Vec<chronos_log::ExecutionRecord>, PersistError> {
    let consumer = LogConsumerId::new("observation-replay");
    let mut cursor = None;
    let mut out = Vec::new();
    loop {
        match log.read_after(session.clone(), consumer.clone(), cursor)? {
            ReadResult::Ok {
                records,
                gaps: _,
                next_cursor,
            } => {
                if records.is_empty() {
                    return Ok(out);
                }
                out.extend(records);
                cursor = Some(next_cursor);
            }
            ReadResult::SessionNotFound => {
                if out.is_empty() {
                    return Err(PersistError::SessionNotFound);
                }
                return Ok(out);
            }
            ReadResult::CursorStale { .. } => return Err(PersistError::CursorStale),
        }
    }
}

fn decode(record: &chronos_log::ExecutionRecord) -> Option<PropertyValue> {
    if record.payload.tag.is_empty() {
        return None;
    }
    serde_json::from_slice(&record.payload.bytes).ok()
}

/// Read every decoded observation in a session in seq order.
pub fn replay_observations<B: ExecutionLogBackend + ?Sized>(
    log: &ExecutionLog<B>,
    session: &SessionId,
) -> Result<Vec<(String, PropertyValue)>, PersistError> {
    let records = read_all(log, session)?;
    Ok(records
        .iter()
        .filter_map(|r| decode(r).map(|v| (r.payload.tag.clone(), v)))
        .collect())
}

/// Read the ordered decoded values for one target in a session.
pub fn replay_target<B: ExecutionLogBackend + ?Sized>(
    log: &ExecutionLog<B>,
    session: &SessionId,
    target: &str,
) -> Result<Vec<PropertyValue>, PersistError> {
    Ok(replay_observations(log, session)?
        .into_iter()
        .filter(|(tag, _)| tag == target)
        .map(|(_, value)| value)
        .collect())
}

/// Evaluate `property` over the values recorded for `property.observe` in a
/// session. A session with no values for that target yields
/// `UnsupportedByRecordedEvidence` (never a false `Pass`).
pub fn evaluate_property_on_session<B: ExecutionLogBackend + ?Sized>(
    log: &ExecutionLog<B>,
    session: &SessionId,
    property: &Property,
) -> Result<PropertySequenceOutcome, PersistError> {
    let values = replay_target(log, session, &property.observe)?;
    if values.is_empty() {
        return Ok(PropertySequenceOutcome::UnsupportedByRecordedEvidence {
            index: 0,
            reason: format!("no recorded observations for `{}`", property.observe),
        });
    }
    Ok(property.evaluate_sequence(&values))
}

/// Return the persisted violation bundle when `property` is violated by the
/// values recorded for its target in a session, else `None`.
pub fn property_violation_on_session<B: ExecutionLogBackend + ?Sized>(
    log: &ExecutionLog<B>,
    session: &SessionId,
    property: &Property,
) -> Result<Option<PropertyViolation>, PersistError> {
    let values = replay_target(log, session, &property.observe)?;
    if values.is_empty() {
        return Ok(None);
    }
    Ok(property.evaluate_violation(&values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{ComparisonOp, InvariantCheck, Property, PropertyId};
    use chronos_log::InMemoryExecutionLog;

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
    fn discount_feed_is_durably_appended() {
        let backend = Arc::new(InMemoryExecutionLog::new());
        let log = Arc::new(ExecutionLog::new(backend));
        let session = SessionId::new("s1");
        let mut writer = ObservationLogWriter::new(log.clone(), session.clone());

        writer
            .record("Order.total", PropertyValue::Number(59.0))
            .unwrap();
        writer
            .record("Order.total", PropertyValue::Number(0.0))
            .unwrap();
        writer
            .record("Order.total", PropertyValue::Number(-35.0))
            .unwrap();
        writer.record("Other", PropertyValue::Bool(true)).unwrap();

        let replayed = replay_target(&log, &session, "Order.total").unwrap();
        assert_eq!(
            replayed,
            vec![
                PropertyValue::Number(59.0),
                PropertyValue::Number(0.0),
                PropertyValue::Number(-35.0),
            ]
        );

        let all = replay_observations(&log, &session).unwrap();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn replayed_feed_drives_non_negative_property() {
        let backend = Arc::new(InMemoryExecutionLog::new());
        let log = Arc::new(ExecutionLog::new(backend));
        let session = SessionId::new("s1");
        let mut writer = ObservationLogWriter::new(log.clone(), session.clone());
        for v in [59.0, 0.0, -35.0] {
            writer
                .record("Order.total", PropertyValue::Number(v))
                .unwrap();
        }

        let values = replay_target(&log, &session, "Order.total").unwrap();
        let property = order_total_property();
        let outcome = property.evaluate_sequence(&values);
        match outcome {
            chronos_domain::PropertySequenceOutcome::Violation { index, after, .. } => {
                assert_eq!(index, 2);
                assert_eq!(after, PropertyValue::Number(-35.0));
            }
            other => panic!("expected a violation, got {other:?}"),
        }
    }

    #[test]
    fn empty_session_replays_empty() {
        let backend = Arc::new(InMemoryExecutionLog::new());
        let log = Arc::new(ExecutionLog::new(backend));
        let session = SessionId::new("never-written");
        let replayed = replay_target(&log, &session, "Order.total").unwrap();
        assert!(replayed.is_empty());
    }

    #[test]
    fn session_evaluation_detects_violation_on_replay() {
        let backend = Arc::new(InMemoryExecutionLog::new());
        let log = Arc::new(ExecutionLog::new(backend));
        let session = SessionId::new("s1");
        let mut writer = ObservationLogWriter::new(log.clone(), session.clone());
        for v in [59.0, 0.0, -35.0] {
            writer
                .record("Order.total", PropertyValue::Number(v))
                .unwrap();
        }

        let property = order_total_property();
        let outcome = evaluate_property_on_session(&log, &session, &property).unwrap();
        match outcome {
            chronos_domain::PropertySequenceOutcome::Violation { index, after, .. } => {
                assert_eq!(index, 2);
                assert_eq!(after, PropertyValue::Number(-35.0));
            }
            other => panic!("expected a violation, got {other:?}"),
        }
        let bundle = property_violation_on_session(&log, &session, &property)
            .unwrap()
            .expect("expected a bundle");
        assert_eq!(bundle.transition.after, PropertyValue::Number(-35.0));
        assert_eq!(bundle.total_observations, 3);
    }

    #[test]
    fn session_evaluation_empty_target_feed_is_unsupported() {
        let backend = Arc::new(InMemoryExecutionLog::new());
        let log = Arc::new(ExecutionLog::new(backend));
        let session = SessionId::new("s2");
        let mut writer = ObservationLogWriter::new(log.clone(), session.clone());
        writer.record("Other", PropertyValue::Bool(true)).unwrap();

        let property = order_total_property();
        let outcome = evaluate_property_on_session(&log, &session, &property).unwrap();
        assert!(
            matches!(
                outcome,
                chronos_domain::PropertySequenceOutcome::UnsupportedByRecordedEvidence { .. }
            ),
            "empty target feed must be Unsupported, got {outcome:?}"
        );
        assert!(property_violation_on_session(&log, &session, &property)
            .unwrap()
            .is_none());
    }
}
