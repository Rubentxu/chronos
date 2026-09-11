//! Counterexample bundle storage — persists shrink results via redb.
//!
//! Bundles have a distinct identity (`bundle_id: String`) from sessions
//! (`session_id: String`) because their lifecycle + GC + persistence
//! policy differs (m8-01 § "Bundles have a distinct identity"). They live
//! in a dedicated redb table.
//!
//! The full record envelope is stored as a single bincode blob per
//! `bundle_id`. Bundles are typically small (a shrunk subset of one
//! session's events), so blob storage is acceptable for m8-03 (R4
//! mitigation: split into CAS hashes in a follow-up if profiling demands).
//!
//! **Circular-dep disclosure**: chronos-services → chronos-store is
//! already the dependency direction. This module is intentionally
//! independent of chronos-services: the `ExistencePredicateWire` enum
//! here is a separate, line-for-line mirror of
//! `chronos_services::output::ExistencePredicate`, and the boundary
//! conversion happens in `services::counterexample`.
//!
//! See `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-scoping.md`
//! § "2. counterexample_bundles redb table" for the design.

use crate::cas::ContentHash;
use crate::error::StoreError;
use chronos_domain::property::PropertyValue;
use chronos_domain::TraceEvent;
use redb::{ReadableTable, TableDefinition, TableError};
use serde::{Deserialize, Serialize};

/// Table for counterexample bundles.
///
/// Key: `bundle_id: String` (as bytes).
/// Value: bincode-serialised [`CounterexampleBundleRecord`].
const COUNTEREXAMPLE_BUNDLES: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("counterexample_bundles");

/// What causal slice triggered the violation. Plumbed via the bundle so
/// `counterexample_get` can re-emit the relevant trace window without
/// re-running the shrink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MinimisedPayload {
    /// Minimised `PropertyValue` for an `Invariant` shrink.
    Constant(PropertyValue),
    /// Minimised predicate for an `Existence` shrink. Same wire shape as
    /// `chronos_services::output::ExistencePredicate` but a separate type
    /// to break the circular dep. The dispatcher in
    /// `services::counterexample` does a `match`-based conversion at
    /// the boundary.
    Predicate(ExistencePredicateWire),
    /// Minimised (caller, callee, optional max_depth) for a `CallPath` shrink.
    CallPath {
        caller: String,
        callee: String,
        max_depth: Option<u64>,
    },
}

/// Wire-friendly existence predicate, owned by chronos-store.
///
/// Mirrors `chronos_services::output::ExistencePredicate` line-for-line
/// (same three variants, same field names). Kept independent to break
/// the circular dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExistencePredicateWire {
    EventTypeEquals { event_type: String },
    ThreadEquals { thread_id: u64 },
    PropertyKeyEquals { target: String },
}

/// Filter shape for `SessionStore::list_counterexample_bundles`.
///
/// All fields are optional; passing `None` for everything returns the
/// most recent bundles up to `limit`. m8-03 ships single-page only
/// (`next_cursor` is m8-05 close-time work).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterexampleBundleFilter<'a> {
    pub workspace_id: Option<&'a str>,
    pub property_kind: Option<&'a str>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: u32,
}

/// Summary shape used for list responses (no events).
///
/// Mirrors `chronos_services::counterexample::CounterexampleBundleSummary`
/// but as a free-standing struct so chronos-store stays independent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
    /// String form ("invariant" | "existence" | "call_path") — avoids
    /// forcing chronos-store to depend on chronos_services::output::HypothesisKind.
    pub property_kind: String,
    pub workspace_id: String,
    pub created_at_ms: u64,
    pub rounds_used: u32,
    pub has_full_bundle: bool,
}

/// Full record stored as the redb value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised: Option<MinimisedPayload>,
    /// Snapshot of CAS hashes backing the events. Not strictly required
    /// for read (we read `events` directly), but useful for future dedup.
    #[serde(default)]
    pub event_cas_hashes: Vec<ContentHash>,
}

impl crate::storage::SessionStore {
    /// Save a counterexample bundle record. Returns the (already-existing)
    /// `bundle_id` for caller convenience (the bundle_id lives in the
    /// record itself).
    #[allow(clippy::result_large_err)]
    pub fn save_counterexample_bundle(
        &self,
        record: CounterexampleBundleRecord,
    ) -> Result<String, StoreError> {
        if record.summary.bundle_id.is_empty() {
            return Err(StoreError::Serialization(
                "bundle_id must be non-empty".to_string(),
            ));
        }
        if record.summary.bundle_id.contains('/') || record.summary.bundle_id.contains('\\') {
            return Err(StoreError::Serialization(format!(
                "bundle_id `{}` contains path separator",
                record.summary.bundle_id
            )));
        }

        let bytes =
            bincode::serialize(&record).map_err(|e| StoreError::Serialization(e.to_string()))?;

        let tx = self
            .db()
            .begin_write()
            .map_err(|e| StoreError::Database(e.into()))?;
        {
            let mut table = tx
                .open_table(COUNTEREXAMPLE_BUNDLES)
                .map_err(|e| StoreError::Database(e.into()))?;
            table
                .insert(record.summary.bundle_id.as_bytes(), bytes.as_slice())
                .map_err(|e| StoreError::Database(e.into()))?;
        }
        tx.commit().map_err(|e| StoreError::Database(e.into()))?;
        Ok(record.summary.bundle_id)
    }

    /// Load a counterexample bundle by id.
    ///
    /// Returns `Ok(None)` when the bundle does not exist (idempotent —
    /// let the caller decide between LoadFailed vs NotFound). Also
    /// returns `Ok(None)` when the `counterexample_bundles` table has
    /// never been written (a fresh in-memory DB or a live DB on which
    /// no shrink has ever run); redb's read-only `open_table` errors
    /// `TableDoesNotExist` in that case.
    #[allow(clippy::result_large_err)]
    pub fn load_counterexample_bundle(
        &self,
        bundle_id: &str,
    ) -> Result<Option<CounterexampleBundleRecord>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table_result = tx.open_table(COUNTEREXAMPLE_BUNDLES);
        let table = match table_result {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        let bytes_opt = match table.get(bundle_id.as_bytes()) {
            Ok(o) => o,
            Err(e) => return Err(StoreError::Database(e.into())),
        };
        match bytes_opt {
            None => Ok(None),
            Some(bytes_guard) => {
                let bytes: &[u8] = bytes_guard.value();
                let record: CounterexampleBundleRecord = bincode::deserialize(bytes)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(record))
            }
        }
    }

    /// List bundle summaries matching `filter`, oldest-first (uuid::v7
    /// lexicographic order matches chronological).
    ///
    /// Returns `Ok(vec![])` when the `counterexample_bundles` table has
    /// never been written (redb's read-only `open_table` errors
    /// `TableDoesNotExist` in that case; we collapse to empty per the
    /// `load_counterexample_bundle` precedent).
    #[allow(clippy::result_large_err)]
    pub fn list_counterexample_bundles(
        &self,
        filter: CounterexampleBundleFilter<'_>,
    ) -> Result<Vec<CounterexampleBundleSummary>, StoreError> {
        let tx = self
            .db()
            .begin_read()
            .map_err(|e| StoreError::Database(e.into()))?;
        let table = match tx.open_table(COUNTEREXAMPLE_BUNDLES) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Database(e.into())),
        };

        let mut out: Vec<CounterexampleBundleSummary> = Vec::new();
        let iter = table.iter().map_err(|e| StoreError::Database(e.into()))?;
        let limit_usize: Option<usize> = if filter.limit == 0 {
            None
        } else {
            Some(filter.limit as usize)
        };

        for entry in iter {
            let (_k, v) = entry.map_err(|e| StoreError::Database(e.into()))?;
            let bytes: &[u8] = v.value();
            let record: CounterexampleBundleRecord = match bincode::deserialize(bytes) {
                Ok(r) => r,
                // Skip corrupt rows but don't fail the whole list (best-effort).
                Err(_) => continue,
            };
            let s = &record.summary;
            if let Some(w) = filter.workspace_id {
                if s.workspace_id != w {
                    continue;
                }
            }
            if let Some(k) = filter.property_kind {
                if s.property_kind != k {
                    continue;
                }
            }
            if let Some(since) = filter.since_ms {
                if s.created_at_ms < since {
                    continue;
                }
            }
            if let Some(until) = filter.until_ms {
                if s.created_at_ms > until {
                    continue;
                }
            }
            out.push(s.clone());
            if let Some(l) = limit_usize {
                if out.len() >= l {
                    break;
                }
            }
        }

        // Sort by uuid::v7 ordering: bundle_id is uuid::v7 (m8-02).
        // Lexicographic sort on uuid::v7 ≈ chronological order.
        out.sort_by(|a, b| a.bundle_id.cmp(&b.bundle_id));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> crate::storage::SessionStore {
        crate::storage::SessionStore::in_memory().expect("in_memory store")
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b1".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 1000,
                rounds_used: 4,
                has_full_bundle: true,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(2.0))),
            event_cas_hashes: vec![],
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store.load_counterexample_bundle("b1").unwrap().unwrap();
        assert_eq!(loaded.summary.bundle_id, "b1");
        assert_eq!(loaded.summary.rounds_used, 4);
        assert!(matches!(
            loaded.minimised,
            Some(MinimisedPayload::Constant(_))
        ));
    }

    #[test]
    fn load_unknown_returns_none() {
        let store = make_store();
        let r = store.load_counterexample_bundle("nonexistent").unwrap();
        assert_eq!(r, None);
    }

    #[test]
    fn list_filters_by_property_kind() {
        let store = make_store();
        for (id, kind) in [
            ("b-inv", "invariant"),
            ("b-ext", "existence"),
            ("b-cp", "call_path"),
        ] {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: id.into(),
                        property_kind: kind.into(),
                        workspace_id: "ws".into(),
                        created_at_ms: 100,
                        rounds_used: 1,
                        has_full_bundle: true,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: Some("invariant"),
            since_ms: None,
            until_ms: None,
            limit: 100,
        };
        let summaries = store.list_counterexample_bundles(filter).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].bundle_id, "b-inv");
    }

    #[test]
    fn list_with_limit_truncates() {
        let store = make_store();
        for i in 0..10 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("b-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i,
                        rounds_used: 1,
                        has_full_bundle: true,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: None,
            since_ms: None,
            until_ms: None,
            limit: 3,
        };
        let summaries = store.list_counterexample_bundles(filter).unwrap();
        assert_eq!(summaries.len(), 3);
    }

    #[test]
    fn save_rejects_empty_bundle_id() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 0,
                has_full_bundle: false,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
        };
        let r = store.save_counterexample_bundle(rec);
        assert!(r.is_err(), "empty bundle_id must be rejected");
    }
}
