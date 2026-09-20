//! SessionStoreBackedCounterexampleRepository — driven adapter that
//! forwards `CounterexampleRepository` calls to a
//! `chronos_store::SessionStore`'s counterexample methods.
//!
//! Part of REC-C3.3.3 (Tren B) slice D. The port uses **owned**
//! `CounterexampleBundleFilter` (Q3 decision — drops the `'a` borrow
//! that lived on the chronos_store internal type). The adapter
//! re-borrows string slices at the storage boundary via `as_str()`
//! so the store-side filter can be built without allocating new
//! strings.
//!
//! For `minimised` and `target_hypothesis`, the port carries opaque
//! `Option<Vec<u8>>` (see `ports/counterexample.rs`); the adapter
//! converts to the strongly-typed store form by bincode-round-tripping
//! the bytes. When the bytes are `None` or the typed form fails to
//! deserialise, the adapter passes `None` to the store (forward
//! compatibility).

use std::sync::Arc;

use chronos_domain::ports::counterexample::{
    CounterexampleBundleFilter, CounterexampleBundleRecord, CounterexampleBundleSummary,
    CounterexampleRepository, CounterexampleRepositoryError,
};

use crate::counterexample_storage::{
    CounterexampleBundleRecord as StoreRecord, CounterexampleBundleSummary as StoreSummary,
};
use crate::error::StoreError;
use crate::storage::SessionStore;

/// `CounterexampleRepository` driven by a `SessionStore`. Construct once
/// at the composition root and hand the `Arc<dyn CounterexampleRepository>`
/// to services.
pub struct SessionStoreBackedCounterexampleRepository {
    store: Arc<SessionStore>,
}

impl SessionStoreBackedCounterexampleRepository {
    pub fn new(store: Arc<SessionStore>) -> Self {
        Self { store }
    }

    pub fn into_arc(self) -> Arc<dyn CounterexampleRepository> {
        Arc::new(self)
    }
}

fn convert(e: StoreError) -> CounterexampleRepositoryError {
    match e {
        StoreError::SessionNotFound(id) => CounterexampleRepositoryError::BundleNotFound(id),
        other => CounterexampleRepositoryError::from_store(other),
    }
}

/// Decode opaque port-bytes into the strongly-typed store form.
///
/// Returns `None` when the port value is `None` OR the bytes fail to
/// bincode-deserialise (forward-compatible: legacy bundles written
/// before the typed form lands still load with `None`).
///
/// **B9 honesty:** `chronos_store::MinimisedPayload` and
/// `HypothesisInputWire` are **enums** (not bare byte carriers). The
/// port intentionally cannot know which variant to construct, so when
/// the bytes are present but the typed form fails to deserialise,
/// the adapter logs a warning and returns `None` (the store-side
/// `target_hypothesis: None` keeps the legacy-load semantics intact).
/// Round-tripping the strongly-typed form requires re-serialisation
/// in the adapter; that work is deferred to the slice where services
/// are rewired (TASK-TB-E), where the typed shape becomes part of
/// the contract.
fn decode_minimised(
    bytes: Option<&Vec<u8>>,
) -> Option<crate::counterexample_storage::MinimisedPayload> {
    let bytes = bytes?;
    match bincode::deserialize(bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(
                "SessionStoreBackedCounterexampleRepository: failed to decode \
                 MinimisedPayload from port bytes ({}); passing None to store. \
                 This is a forward-compat warning — rewire will land in TASK-TB-E.",
                e
            );
            None
        }
    }
}

fn decode_hypothesis(
    bytes: Option<&Vec<u8>>,
) -> Option<crate::counterexample_storage::HypothesisInputWire> {
    let bytes = bytes?;
    match bincode::deserialize(bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(
                "SessionStoreBackedCounterexampleRepository: failed to decode \
                 HypothesisInputWire from port bytes ({}); passing None to store.",
                e
            );
            None
        }
    }
}

/// Encode a strongly-typed store form into opaque port bytes.
///
/// Returns `None` for a `None` typed value, otherwise the bincode bytes.
/// Used for the round-trip from `SessionStore::load_counterexample_bundle`
/// to the port-shaped `CounterexampleBundleRecord.minimised` /
/// `.target_hypothesis`. Forward-compat is preserved: legacy bundles
/// with `minimised: None` round-trip as `None` and the port's
/// `events_count: 0` fallback applies (m9-02 D2).
fn encode_minimised_to_port(
    m: Option<&crate::counterexample_storage::MinimisedPayload>,
) -> Option<Vec<u8>> {
    m.map(|v| bincode::serialize(v).expect("MinimisedPayload is bincode-friendly"))
}

fn encode_hypothesis_to_port(
    h: Option<&crate::counterexample_storage::HypothesisInputWire>,
) -> Option<Vec<u8>> {
    h.map(|v| bincode::serialize(v).expect("HypothesisInputWire is bincode-friendly"))
}

impl CounterexampleRepository for SessionStoreBackedCounterexampleRepository {
    fn save_bundle(
        &self,
        record: CounterexampleBundleRecord,
    ) -> Result<String, CounterexampleRepositoryError> {
        // Convert the domain `CounterexampleBundleRecord` to the
        // chronos_store type. Field shape is identical for `summary`,
        // `events`, `event_cas_hashes`, `schema_version`. Opaque
        // fields are bincode-encoded into the typed form.
        // B9 honesty: the port's `minimised` and `target_hypothesis` are
        // opaque bytes; the store types are strongly-typed enums. When
        // the bytes decode cleanly we pass the typed form; otherwise we
        // pass `None` (legacy semantics). See `decode_minimised` /
        // `decode_hypothesis` for the warnings.
        let minimised = decode_minimised(record.minimised.as_ref());
        let target_hypothesis = decode_hypothesis(record.target_hypothesis.as_ref());
        let store_record = StoreRecord {
            summary: StoreSummary {
                bundle_id: record.summary.bundle_id,
                property_kind: record.summary.property_kind,
                workspace_id: record.summary.workspace_id,
                created_at_ms: record.summary.created_at_ms,
                rounds_used: record.summary.rounds_used,
                has_full_bundle: record.summary.has_full_bundle,
                schema_version: record.summary.schema_version,
                events_count: record.summary.events_count,
            },
            events: record.events,
            minimised,
            event_cas_hashes: record.event_cas_hashes,
            target_hypothesis,
            schema_version: record.schema_version,
        };
        self.store
            .save_counterexample_bundle(store_record)
            .map_err(convert)
    }

    fn load_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<chronos_domain::TraceEvent>, CounterexampleRepositoryError> {
        self.store
            .load_counterexample_bundle_events(bundle_id)
            .map_err(convert)
    }

    fn list_bundles(
        &self,
        filter: &CounterexampleBundleFilter,
    ) -> Result<Vec<CounterexampleBundleSummary>, CounterexampleRepositoryError> {
        let store_filter = crate::counterexample_storage::CounterexampleBundleFilter {
            workspace_id: filter.workspace_id.as_deref(),
            property_kind: filter.property_kind.as_deref(),
            since_ms: filter.since_ms,
            until_ms: filter.until_ms,
            limit: filter.limit,
            cursor: filter.cursor.clone(),
        };
        let summaries = self
            .store
            .list_counterexample_bundles(store_filter)
            .map_err(convert)?;
        Ok(summaries
            .into_iter()
            .map(|s| CounterexampleBundleSummary {
                bundle_id: s.bundle_id,
                property_kind: s.property_kind,
                workspace_id: s.workspace_id,
                created_at_ms: s.created_at_ms,
                rounds_used: s.rounds_used,
                has_full_bundle: s.has_full_bundle,
                schema_version: s.schema_version,
                events_count: s.events_count,
            })
            .collect())
    }

    fn count_bundle_events(&self, bundle_id: &str) -> Result<u64, CounterexampleRepositoryError> {
        self.store
            .count_counterexample_bundle_events(bundle_id)
            .map_err(convert)
    }

    fn load_bundle(
        &self,
        bundle_id: &str,
    ) -> Result<Option<CounterexampleBundleRecord>, CounterexampleRepositoryError> {
        let store_record = self
            .store
            .load_counterexample_bundle(bundle_id)
            .map_err(convert)?;
        Ok(store_record.map(|r| CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: r.summary.bundle_id,
                property_kind: r.summary.property_kind,
                workspace_id: r.summary.workspace_id,
                created_at_ms: r.summary.created_at_ms,
                rounds_used: r.summary.rounds_used,
                has_full_bundle: r.summary.has_full_bundle,
                schema_version: r.summary.schema_version,
                events_count: r.summary.events_count,
            },
            events: r.events,
            minimised: encode_minimised_to_port(r.minimised.as_ref()),
            event_cas_hashes: r.event_cas_hashes,
            target_hypothesis: encode_hypothesis_to_port(r.target_hypothesis.as_ref()),
            schema_version: r.schema_version,
        }))
    }
}
