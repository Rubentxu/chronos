//! Compatibility re-exports (REC-C3.3.1).
//!
//! `ExecutionRecord`, `ExecutionKind`, `ExecutionPayload`,
//! `TripwireFiredEvidence`, and the tripwire tag live in
//! `chronos_domain::evidence` because the storage port
//! (`chronos_domain::ports::execution_log::ExecutionLogProvider`)
//! needs them in its signature. The inherent impls
//! (`ExecutionRecord::schema_version`, etc.) live on the canonical
//! domain types — they cannot stay here without breaking the
//! orphan/inherent-impl rule.
//!
//! `SessionId` is the canonical identity newtype from
//! `chronos_domain::session_id` (REC-C3.3.1, single owner).

pub use chronos_domain::evidence::{
    ExecutionKind, ExecutionPayload, ExecutionRecord, TripwireFiredEvidence,
    TRIPWIRE_FIRED_EVIDENCE_TAG,
};
pub use chronos_domain::session_id::SessionId;

// Local use so `crate::record::*` paths inside this crate keep
// resolving to the same canonical types without depending on
// `crate::lib`'s re-export order.
#[allow(unused_imports)]
use crate as _crate;
