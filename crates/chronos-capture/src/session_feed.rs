//! Durable, file-backed observation feeds.
//!
//! Bridges `chronos-log::SegmentedExecutionLog` into the `ExecutionLogBackend`
//! trait (m4b-02's `ObservationLogWriter`/replay helpers operate on
//! `ExecutionLog<B: ExecutionLogBackend>`). [`SegmentedLogBackend`] adapts the
//! fixed-session segmented log, so an observation feed written through
//! [`open_durable_feed`] is flushed to disk segments and survives a reopen.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use chronos_log::{
    ExecutionLog, ExecutionLogBackend, LogConsumerId, LogError, NewExecutionRecord, ReadResult,
    SegmentedConfig, SegmentedExecutionLog, SessionId,
};

use crate::observation_log::ObservationLogWriter;

/// Adapts a fixed-session `SegmentedExecutionLog` to the `ExecutionLogBackend`
/// trait used by the observation-log bridge.
pub struct SegmentedLogBackend {
    seg: SegmentedExecutionLog,
}

impl SegmentedLogBackend {
    /// Force buffered entries to a segment file on disk.
    pub fn flush(&self) -> Result<Option<PathBuf>, LogError> {
        self.seg.flush()
    }
}

impl ExecutionLogBackend for SegmentedLogBackend {
    fn append(&self, record: NewExecutionRecord) -> Result<chronos_log::EventSeq, LogError> {
        self.seg.append(record)
    }

    fn record_gap(&self, _session_id: SessionId, gap: chronos_log::Gap) -> Result<(), LogError> {
        self.seg.record_gap(gap)
    }

    fn read_after(
        &self,
        _session_id: SessionId,
        consumer: LogConsumerId,
        cursor: Option<chronos_log::ConsumerCursor>,
    ) -> Result<ReadResult, LogError> {
        self.seg.read_after(&consumer, cursor)
    }

    fn tail_seq(&self, _session_id: &SessionId) -> Option<chronos_log::EventSeq> {
        self.seg.tail_seq()
    }
}

/// Open a durable, file-backed observation feed for `session` in `dir`.
///
/// Entries flush to disk every `flush_every` appends (`>= 1`); reopening from
/// the same `dir` replays the segments.
pub fn open_durable_feed(
    dir: impl Into<PathBuf>,
    session: SessionId,
    flush_every: usize,
) -> Result<
    (
        Arc<ExecutionLog<SegmentedLogBackend>>,
        ObservationLogWriter<SegmentedLogBackend>,
    ),
    LogError,
> {
    let mut config = SegmentedConfig::with_dir(dir.into());
    config.flush_threshold = NonZeroUsize::new(flush_every.max(1)).expect("flush_every >= 1");
    let seg = SegmentedExecutionLog::open(session.clone(), config)?;
    let backend = Arc::new(SegmentedLogBackend { seg });
    let log = Arc::new(ExecutionLog::new(backend));
    let writer = ObservationLogWriter::new(log.clone(), session);
    Ok((log, writer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::{ComparisonOp, InvariantCheck, Property, PropertyId, PropertyValue};

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
    fn feed_survives_segmented_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let session = SessionId::new("s1");

        {
            let (log, mut writer) = open_durable_feed(dir.path(), session.clone(), 1).unwrap();
            for v in [59.0, 0.0, -35.0] {
                writer
                    .record("Order.total", PropertyValue::Number(v))
                    .unwrap();
            }
            log.backend().flush().unwrap();
        } // drop the first instance (segments already on disk)

        // Reopen a fresh segmented log from the same dir.
        let (reopen_log, _writer) = open_durable_feed(dir.path(), session.clone(), 1).unwrap();
        let replayed =
            crate::observation_log::replay_target(&reopen_log, &session, "Order.total").unwrap();
        assert_eq!(
            replayed,
            vec![
                PropertyValue::Number(59.0),
                PropertyValue::Number(0.0),
                PropertyValue::Number(-35.0),
            ]
        );

        let property = order_total_property();
        let violation =
            crate::observation_log::evaluate_property_on_session(&reopen_log, &session, &property)
                .unwrap();
        match violation {
            chronos_domain::PropertySequenceOutcome::Violation { index, after, .. } => {
                assert_eq!(index, 2);
                assert_eq!(after, PropertyValue::Number(-35.0));
            }
            other => panic!("expected violation, got {other:?}"),
        }
    }
}
