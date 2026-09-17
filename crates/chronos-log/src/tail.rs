//! REC-C1.5.3 — what we know about the END of an execution.
//!
//! ## What this is, and is not
//!
//! `tail_state` describes what is known about the end of the execution. It does
//! NOT change the truth of the durable region that C1.5.2 already validated:
//!
//! ```text
//! retained region [500, 920) = intact
//! tail_state                 = Unclean
//! ```
//!
//! are perfectly compatible. A read of `[500, 600)` can still be `Complete`
//! (C1.4) because that range is fully known; what we cannot claim is that 999 was
//! really the last event of the execution. Tail state therefore belongs in
//! session lifecycle/provenance, never recycled into completeness.
//!
//! ## The rule that separates the states
//!
//! ```text
//! positive evidence of an abnormal end -> Unclean
//! lack of evidence                     -> Unknown
//! ```
//!
//! and a legacy manifest is never guessed into `Open`: that would falsely accuse
//! a historical session of having crashed.

use crate::seq::EventSeq;

/// Who we are to the tail of this execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TailState {
    /// The run is still in progress in this process.
    Open,
    /// The run ended through an explicit, durable seal.
    Sealed {
        tail_seq: Option<u64>,
        sealed_at_unix_ms: u64,
    },
    /// Positive evidence that the previous run did not end cleanly.
    Unclean {
        last_durable_seq: Option<u64>,
        reason: String,
    },
    /// No proof either way (legacy metadata, incomplete external metadata).
    Unknown { reason: String },
}

impl TailState {
    pub fn name(&self) -> &'static str {
        match self {
            TailState::Open => "open",
            TailState::Sealed { .. } => "sealed",
            TailState::Unclean { .. } => "unclean",
            TailState::Unknown { .. } => "unknown",
        }
    }

    pub fn is_sealed(&self) -> bool {
        matches!(self, TailState::Sealed { .. })
    }

    /// The tail a sealed run claims, if any.
    pub fn sealed_tail(&self) -> Option<Option<EventSeq>> {
        match self {
            TailState::Sealed { tail_seq, .. } => Some(tail_seq.map(EventSeq::new)),
            _ => None,
        }
    }

    pub fn unclean(last_durable_seq: Option<EventSeq>, reason: impl Into<String>) -> Self {
        TailState::Unclean {
            last_durable_seq: last_durable_seq.map(|s| s.0),
            reason: reason.into(),
        }
    }

    pub fn unknown(reason: impl Into<String>) -> Self {
        TailState::Unknown {
            reason: reason.into(),
        }
    }

    pub fn sealed(tail_seq: Option<EventSeq>) -> Self {
        TailState::Sealed {
            tail_seq: tail_seq.map(|s| s.0),
            sealed_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }
}

/// Why a seal was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    #[error("a live segment temp {path:?} exists; flush is incomplete")]
    LiveSegmentTemp { path: String },
}

/// A successful seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedTail {
    pub tail_seq: Option<EventSeq>,
}

/// What a reopen concluded about the previous run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailRecovery {
    pub state: TailState,
    /// True when the state was derived from evidence at open time (rather than
    /// read straight from a manifest).
    pub recovered: bool,
}

/// Derive the tail state of a reopen.
///
/// Inputs are exactly the evidence available: the persisted state (if the
/// manifest had one), whether a live segment temp exists, and the tail the
/// validated replay reconstructed.
pub fn recover_tail_state(
    persisted: Option<&TailState>,
    live_temp_present: bool,
    reconstructed_tail: Option<EventSeq>,
) -> TailRecovery {
    match persisted {
        // A sealed run reopens as sealed; C1.5.2 already validated the region and
        // the caller compares `reconstructed_tail` against the seal.
        Some(s @ TailState::Sealed { .. }) => TailRecovery {
            state: s.clone(),
            recovered: false,
        },
        // A legacy manifest without tail metadata: we simply do not know.
        Some(TailState::Unknown { .. }) | None => TailRecovery {
            state: TailState::unknown("legacy-v1: manifest carried no tail state"),
            recovered: true,
        },
        // "Open" was persisted by a previous process, so that process never
        // sealed. That is positive evidence of an abnormal end.
        Some(TailState::Open) => TailRecovery {
            state: TailState::unclean(reconstructed_tail, "previous run was not sealed"),
            recovered: true,
        },
        // An earlier recovery already concluded Unclean; keep it, refreshed with
        // the tail this reopen actually reconstructed.
        Some(TailState::Unclean { reason, .. }) => TailRecovery {
            state: TailState::unclean(reconstructed_tail, reason.clone()),
            recovered: true,
        },
    }
    .with_temp_evidence(live_temp_present, reconstructed_tail)
}

impl TailRecovery {
    /// A freshly created log is `Open`: "not sealed" cannot be blamed on a
    /// previous run that never existed.
    pub fn without_previous_run_blame(mut self) -> Self {
        if let TailState::Unclean { reason, .. } = &self.state {
            if reason.contains("not sealed") {
                self.state = TailState::Open;
                self.recovered = false;
            }
        }
        self
    }

    fn with_temp_evidence(mut self, live_temp_present: bool, tail: Option<EventSeq>) -> Self {
        if live_temp_present && !self.state.is_sealed() {
            self.state = TailState::unclean(tail, "a live segment temp survived a crash");
            self.recovered = true;
        }
        self
    }
}

/// Is a segment temp inside the live region (and therefore evidence)?
///
/// A temp entirely below `retained_from` is retired garbage and says nothing
/// about the tail. Note this is about `<session>-<seq>.tmp` from the segment
/// writer; `execution-log.manifest.json.tmp` is a different thing entirely (an
/// incomplete metadata update, not an interrupted execution tail).
pub fn temp_is_live_evidence(temp_start_seq: EventSeq, retained_from: EventSeq) -> bool {
    temp_start_seq >= retained_from
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_open_becomes_unclean_never_sealed() {
        let r = recover_tail_state(Some(&TailState::Open), false, Some(EventSeq::new(99)));
        assert!(
            matches!(r.state, TailState::Unclean { .. }),
            "{:?}",
            r.state
        );
        assert!(r.recovered);
    }

    #[test]
    fn missing_or_legacy_state_is_unknown_not_open() {
        for persisted in [None, Some(TailState::unknown("legacy-v1"))] {
            let r = recover_tail_state(persisted.as_ref(), false, Some(EventSeq::new(5)));
            assert!(
                matches!(r.state, TailState::Unknown { .. }),
                "a legacy manifest must never be guessed: {:?}",
                r.state
            );
        }
    }

    #[test]
    fn sealed_reopens_as_sealed() {
        let sealed = TailState::sealed(Some(EventSeq::new(42)));
        let r = recover_tail_state(Some(&sealed), false, Some(EventSeq::new(42)));
        assert_eq!(r.state, sealed);
        assert!(!r.recovered);
    }

    #[test]
    fn a_live_temp_marks_the_tail_unclean() {
        let r = recover_tail_state(Some(&TailState::Open), true, Some(EventSeq::new(7)));
        match r.state {
            TailState::Unclean { reason, .. } => assert!(reason.contains("temp"), "{reason}"),
            other => panic!("expected Unclean, got {other:?}"),
        }
    }

    #[test]
    fn retired_temp_is_not_tail_evidence() {
        assert!(!temp_is_live_evidence(EventSeq::new(1), EventSeq::new(500)));
        assert!(temp_is_live_evidence(
            EventSeq::new(500),
            EventSeq::new(500)
        ));
    }
}
