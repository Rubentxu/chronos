//! REC-C3.3.1 — Codec for `TripwireFiredEvidence`.
//!
//! The application-shape type lives in `chronos_domain::evidence`; the
//! storage codec lives here because `serde_json` is a dependency of
//! `chronos_log` (it is *only* a dev-dep of `chronos_domain`).
//!
//! The on-disk segment format depends on these functions: changing
//! the JSON field names or the payload tag would break
//! `rec_c2_1_tripwire_evidence` and every segment already written.

use chronos_domain::evidence::{
    ExecutionPayload, TripwireFiredEvidence, TRIPWIRE_FIRED_EVIDENCE_TAG,
};

/// Encode a tripwire-firing evidence into an `ExecutionPayload` that can
/// be carried by an `ExecutionRecord` of kind `TripwireFired`.
pub fn encode(evidence: &TripwireFiredEvidence) -> Result<ExecutionPayload, serde_json::Error> {
    Ok(ExecutionPayload::new(
        serde_json::to_vec(evidence)?,
        TRIPWIRE_FIRED_EVIDENCE_TAG,
    ))
}

/// Decode an `ExecutionPayload` into a `TripwireFiredEvidence`.
///
/// Returns `Ok(None)` when the payload tag is not the tripwire tag
/// (i.e. the record is a `Raw` or `GapMarker`).
pub fn decode(
    payload: &ExecutionPayload,
) -> Result<Option<TripwireFiredEvidence>, serde_json::Error> {
    if payload.tag != TRIPWIRE_FIRED_EVIDENCE_TAG {
        return Ok(None);
    }
    let evidence: TripwireFiredEvidence = serde_json::from_slice(&payload.bytes)?;
    Ok(Some(evidence))
}
