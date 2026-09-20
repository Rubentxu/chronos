//! `CounterexampleRepository` — domain-side port for counterexample
//! bundle persistence (REC-C3.3.3 / Tren B slice D).
//!
//! Mirrors `SessionArchive`'s shape (slice C) but with the bundle
//! semantics of counterexamples: a bundle has a `CounterexampleBundleRecord`
//! (envelope + events + minimised payload) and an opaque `bundle_id`,
//! plus a `CounterexampleBundleSummary` projection used for list responses.
//!
//! `CounterexampleBundleFilter` is owned here with **typed owned** fields
//! (Q3 decision — the lifetime disappears because the port no longer
//! borrows from chronos-store). The adapter does the slice borrow at
//! the storage boundary.
//!
//! **Port-shape note (B'):** `minimised` and `target_hypothesis` are
//! carried as opaque `Vec<u8>` blobs (JSON serialised payload bytes
//! from the store-side types). The store-side wire types
//! (`MinimisedPayload`, `HypothesisInputWire`, `ExistencePredicateWire`)
//! are richer than the port needs — the adapter translates at the
//! storage boundary using JSON-serialise / deserialise roundtrips.
//! See REC-C3.5-B' notes for the design rationale.

pub mod wire;

pub use wire::{
    CounterexampleBundleSummaryWire, ExistencePredicateWire, HypothesisInputWire, MinimisedPayload,
};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::TraceEvent;

/// Errors produced by `CounterexampleRepository` operations.
#[derive(Debug, thiserror::Error)]
pub enum CounterexampleRepositoryError {
    #[error("counterexample bundle not found: {0}")]
    BundleNotFound(String),
    #[error("counterexample repository store error: {0}")]
    Store(String),
}

impl CounterexampleRepositoryError {
    /// Convert any error into the `Store` variant, preserving its `Display`.
    pub fn from_store<E: std::fmt::Display>(e: E) -> Self {
        Self::Store(e.to_string())
    }
}

/// Filter for `list_bundles`. Owned-typed (no `'a` borrow) so the
/// type can live in `chronos_domain`. Adapter re-borrows string
/// slices at the storage boundary (B9: no HashMap<String, Value>
/// shape regression).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterexampleBundleFilter {
    pub workspace_id: Option<String>,
    pub property_kind: Option<String>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: u32,
    /// m8-05: opaque pagination cursor (= last `bundle_id` from
    /// previous page). `None` means first page.
    pub cursor: Option<String>,
}

/// Summary shape used for list responses (no events). Free-standing
/// here so chronos-store can re-export its own `CounterexampleBundleSummary`
/// without crossing the port boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
    /// String form ("invariant" | "existence" | "call_path") — avoids
    /// forcing chronos-domain to depend on chronos_services::output::HypothesisKind.
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub has_full_bundle: bool,
    /// Schema version of the bundle envelope (mirrors the
    /// chronos_store field; defaults to 1 for legacy loads).
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Total event count at save time. Defaults to 0 for legacy loads.
    #[serde(default)]
    pub events_count: u64,
}

fn default_schema_version() -> u32 {
    1
}

/// Full bundle record (envelope + events + minimised bytes +
/// target_hypothesis bytes + schema_version). Mirrors
/// `chronos_store::CounterexampleBundleRecord` so the adapter can
/// forward without translation at the schema level — the chronos_store
/// type remains the wire shape, the domain type is the port shape.
///
/// **Wire compatibility note (B9):** the field names and the
/// `serde` annotations mirror the chronos_store record exactly so a
/// `CounterexampleBundleRecord` round-trips through this port without
/// any conversion **at the JSON wire level**. The opaque `Vec<u8>`
/// fields for `minimised` and `target_hypothesis` are JSON-encoded
/// serialisations of the store-side types
/// (`MinimisedPayload` / `HypothesisInputWire`); the adapter does the
/// encode/decode at the storage boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised: Option<Vec<u8>>,
    #[serde(default)]
    pub event_cas_hashes: Vec<String>,
    #[serde(default)]
    pub target_hypothesis: Option<Vec<u8>>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

/// `CounterexampleRepository` — domain-side port for persisting and
/// reloading counterexample bundles.
pub trait CounterexampleRepository: Send + Sync {
    /// Persist a bundle record. Returns the `bundle_id`.
    fn save_bundle(
        &self,
        record: CounterexampleBundleRecord,
    ) -> Result<String, CounterexampleRepositoryError>;

    /// Load a bundle's events by `bundle_id`.
    fn load_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<TraceEvent>, CounterexampleRepositoryError>;

    /// List bundle summaries filtered by `filter`.
    fn list_bundles(
        &self,
        filter: &CounterexampleBundleFilter,
    ) -> Result<Vec<CounterexampleBundleSummary>, CounterexampleRepositoryError>;

    /// Number of events stored for a bundle.
    fn count_bundle_events(&self, bundle_id: &str) -> Result<u64, CounterexampleRepositoryError>;

    /// Load the full bundle record (envelope + events + minimised
    /// payload + target_hypothesis) by `bundle_id`. Returns `Ok(None)`
    /// when the bundle is absent. The port-shape record mirrors the
    /// chronos-store wire record 1:1; the adapter is a thin
    /// pass-through (see file-level note).
    fn load_bundle(
        &self,
        bundle_id: &str,
    ) -> Result<Option<CounterexampleBundleRecord>, CounterexampleRepositoryError>;
}

/// `CounterexampleRepository` backed by an in-memory map. Used by
/// unit tests and as the composition-root fallback.
#[derive(Debug, Default)]
pub struct InMemoryCounterexampleRepository {
    inner: RwLock<HashMap<String, CounterexampleBundleRecord>>,
}

impl InMemoryCounterexampleRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_arc(self) -> Arc<dyn CounterexampleRepository> {
        Arc::new(self)
    }
}

impl CounterexampleRepository for InMemoryCounterexampleRepository {
    fn save_bundle(
        &self,
        record: CounterexampleBundleRecord,
    ) -> Result<String, CounterexampleRepositoryError> {
        let bundle_id = record.summary.bundle_id.clone();
        let mut guard = self
            .inner
            .write()
            .expect("InMemoryCounterexampleRepository write-lock poisoned");
        guard.insert(bundle_id.clone(), record);
        Ok(bundle_id)
    }

    fn load_bundle_events(
        &self,
        bundle_id: &str,
    ) -> Result<Vec<TraceEvent>, CounterexampleRepositoryError> {
        let guard = self
            .inner
            .read()
            .expect("InMemoryCounterexampleRepository read-lock poisoned");
        guard
            .get(bundle_id)
            .map(|r| r.events.clone())
            .ok_or_else(|| CounterexampleRepositoryError::BundleNotFound(bundle_id.to_string()))
    }

    fn list_bundles(
        &self,
        filter: &CounterexampleBundleFilter,
    ) -> Result<Vec<CounterexampleBundleSummary>, CounterexampleRepositoryError> {
        let guard = self
            .inner
            .read()
            .expect("InMemoryCounterexampleRepository read-lock poisoned");
        let mut out: Vec<CounterexampleBundleSummary> = guard
            .values()
            .filter(|r| {
                if let Some(w) = &filter.workspace_id {
                    if r.summary.workspace_id != *w {
                        return false;
                    }
                }
                if let Some(p) = &filter.property_kind {
                    if r.summary.property_kind != *p {
                        return false;
                    }
                }
                if let Some(s) = filter.since_ms {
                    if r.summary.created_at_ms < s {
                        return false;
                    }
                }
                if let Some(u) = filter.until_ms {
                    if r.summary.created_at_ms > u {
                        return false;
                    }
                }
                if let Some(c) = &filter.cursor {
                    if r.summary.bundle_id.as_str() <= c.as_str() {
                        return false;
                    }
                }
                true
            })
            .map(|r| r.summary.clone())
            .collect();
        if filter.limit > 0 && (out.len() as u32) > filter.limit {
            out.truncate(filter.limit as usize);
        }
        Ok(out)
    }

    fn count_bundle_events(&self, bundle_id: &str) -> Result<u64, CounterexampleRepositoryError> {
        let guard = self
            .inner
            .read()
            .expect("InMemoryCounterexampleRepository read-lock poisoned");
        guard
            .get(bundle_id)
            .map(|r| r.events.len() as u64)
            .ok_or_else(|| CounterexampleRepositoryError::BundleNotFound(bundle_id.to_string()))
    }

    fn load_bundle(
        &self,
        bundle_id: &str,
    ) -> Result<Option<CounterexampleBundleRecord>, CounterexampleRepositoryError> {
        let guard = self
            .inner
            .read()
            .expect("InMemoryCounterexampleRepository read-lock poisoned");
        Ok(guard.get(bundle_id).cloned())
    }
}

impl Clone for InMemoryCounterexampleRepository {
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(
                self.inner
                    .read()
                    .expect("InMemoryCounterexampleRepository read-lock poisoned")
                    .clone(),
            ),
        }
    }
}
