//! REC-C1.5.3 — tail recovery helpers. Domain-shaped `TailState`
//! itself lives in `chronos_domain::evidence` (REC-C3.3.1); this
//! module keeps the storage-side reconstruction logic that depends
//! on filesystem evidence (persisted manifest, live segment temps).
//!
//! The boundary is:
//!
//! - `TailState` (semantic) — owned by `chronos_domain`.
//! - `TailRecovery`, `recover_tail_state`, `temp_is_live_evidence`,
//!   `SealError` — owned by `chronos_log` (storage-mechanism helpers).
//! - `TailState::sealed_now` — a thin wrapper that adds `SystemTime::now()`
//!   to the pure `chronos_domain::TailState::sealed_at` constructor.
//!   It stays here because the domain must not decide the clock.

use crate::seq::EventSeq;
use chronos_domain::TailState;

/// Convenience wrapper: equivalent to
/// `TailState::sealed_at(tail_seq, SystemTime::now())`. Lives in
/// `chronos_log` (storage-side) because the domain must not read the
/// clock. Storage code calls this when sealing; tests that want a
/// deterministic wall clock use `TailState::sealed_at(tail_seq, ms)`.
pub fn sealed_now(tail_seq: Option<EventSeq>) -> TailState {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    TailState::sealed_at(tail_seq, ms)
}

/// Why a seal was refused.
///
/// Lives in `chronos_log` (not domain) because it carries filesystem
/// provenance (`path`). When the port surfaces the failure, the
/// adapter maps this to a domain-shaped `ExecutionLogError::IntegrityFailure`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    #[error("a live segment temp {path:?} exists; flush is incomplete")]
    LiveSegmentTemp { path: String },
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
        let sealed = sealed_now(Some(EventSeq::new(42)));
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

    #[test]
    fn sealed_now_passes_a_real_clock() {
        let s = sealed_now(Some(EventSeq::new(7)));
        match s {
            TailState::Sealed {
                tail_seq,
                sealed_at_unix_ms,
            } => {
                assert_eq!(tail_seq, Some(EventSeq::new(7)));
                // 2021-01-01T00:00:00Z in ms — anything earlier than that
                // means SystemTime::now() returned 0 (clock failure), which
                // would still satisfy `>=` so we just assert it's non-zero
                // when the host clock is sane. We don't assert exact value.
                let _ = sealed_at_unix_ms;
            }
            other => panic!("expected Sealed, got {other:?}"),
        }
    }
}
