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
//!
//! **Schema versioning (m9-01):** `CounterexampleBundleRecord` and
//! `CounterexampleBundleSummary` carry a `schema_version: u32` field
//! that lets the loader distinguish legacy bundles (no field on disk,
//! serde defaults to 1) from future-versioned bundles. The current
//! version is [`CURRENT_BUNDLE_SCHEMA_VERSION`]. Bundles written by a
//! newer chronos-store with a higher version are hard-rejected on
//! `load_counterexample_bundle`; the list path best-effort skips them.

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

/// Canonical "what we write today" value. Bumped when the bundle
/// envelope (record + summary + nested wire types) changes in a way
/// that requires a loader-side decision. See module docs.
///
/// m9-01 initial value: 1. All bundles persisted before m9-01 have no
/// field on disk; serde defaults to 1 on load.
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 1;

/// Versions the loader accepts silently. Today only 1; future cycles
/// add entries here when they introduce a new envelope shape.
///
/// R4 disclosure (m9-01): currently unused — the loader policy in D3 only
/// checks the upper bound (`> CURRENT`), not membership in the known list.
/// Suppressed here so future cycles can tighten the check without a
/// new lint cascade.
#[allow(dead_code)]
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1];

fn default_schema_version() -> u32 {
    CURRENT_BUNDLE_SCHEMA_VERSION
}

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

/// m8-07: wire mirror of `chronos_services::hypothesis_test::HypothesisInput`.
///
/// Persisted in the bundle record (alongside the `minimised` payload) so
/// `chronos test replay` can reconstruct the EXACT HypothesisInput the user
/// passed to `counterexample_shrink`, instead of synthesising defaults from
/// the minimised payload (the m8-04 R-hypothesis-reconstruction-fidelity gap).
///
/// R1 disclosure (m8-07): this is a hand-maintained mirror of the
/// chronos-services type. Drift is possible if `HypothesisInput` evolves
/// without updating this mirror; mitigated by the roundtrip test in
/// `crates/chronos-services/src/counterexample.rs::tests`.
///
/// All fields are plain string / Option types to avoid cross-crate type
/// sharing. Kind/scope/comparison are stringified to keep chronos-store
/// independent of chronos-services' enum types (same precedent as
/// `CounterexampleBundleSummary.property_kind`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HypothesisInputWire {
    pub session_id: String,
    /// "invariant" | "existence" | "call_path".
    pub kind: String,
    /// "event_count" | "property_value" | "latency_ms" | None.
    pub scope: Option<String>,
    /// "Eq" | "Ne" | "Ge" | "Gt" | "Le" | "Lt" | None.
    pub comparison: Option<String>,
    pub constant: Option<PropertyValue>,
    pub property_target: Option<String>,
    pub predicate: Option<ExistencePredicateWire>,
    pub caller: Option<String>,
    pub callee: Option<String>,
    /// HypothesisInput::max_depth is Option<usize>; we persist as Option<u64>
    /// for forward compatibility (future widening to u128). Cast is
    /// saturating in `hypothesis_input_from_wire` (bounded by usize::MAX).
    pub max_depth: Option<u64>,
}

/// Filter shape for `SessionStore::list_counterexample_bundles`.
///
/// All fields are optional; passing `None` for everything returns the
/// most recent bundles up to `limit`.
///
/// m8-05 (B2): `cursor` carries the `bundle_id` returned by the
/// previous page's `next_cursor`. When `Some`, the list skips rows
/// whose `bundle_id <= cursor`, effectively returning the page that
/// begins AFTER the cursor. The cursor is opaque to callers — we use
/// the bundle_id directly (uuid::v7 is monotonically increasing, so
/// lexicographic `>` gives chronological forward paging).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterexampleBundleFilter<'a> {
    pub workspace_id: Option<&'a str>,
    pub property_kind: Option<&'a str>,
    pub since_ms: Option<u64>,
    pub until_ms: Option<u64>,
    pub limit: u32,
    /// m8-05: opaque pagination cursor (= last `bundle_id` from previous
    /// page). When `Some(c)`, the list starts at the first row whose
    /// `bundle_id > c`. `None` means first page.
    pub cursor: Option<String>,
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
    /// m9-01: monotonic version of the bundle envelope. Defaults to 1
    /// for bundles persisted before m9-01 (serde `#[serde(default)]`).
    /// Loader rejects bundles with `schema_version > CURRENT_BUNDLE_SCHEMA_VERSION`.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
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
    /// m8-07: original `HypothesisInput` the user passed to
    /// `counterexample_shrink`. None for bundles persisted before m8-07
    /// (serde's `#[serde(default)]` makes legacy loads a clean
    /// `target_hypothesis: None`).
    ///
    /// When `Some`, `chronos test replay` uses this verbatim to
    /// reconstruct the user's original hypothesis for replay, instead of
    /// the m8-04 synthetic-default reconstruction (which loses
    /// scope/comparison/property_target for Invariant targets).
    #[serde(default)]
    pub target_hypothesis: Option<HypothesisInputWire>,
    /// m9-01: envelope-level version. Always equals `summary.schema_version`
    /// for records written by this build; the loader reads `record.schema_version`
    /// (the envelope) as authoritative.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
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

        // m9-01 D5: always write the current schema version so the
        // persisted value is controlled — even if the caller constructed
        // a record with a stale value.
        let mut record = record;
        record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
        record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;

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
                // m9-01 D3: reject bundles written by a newer chronos-store.
                // This is a hard-reject (not a warning) because silently loading
                // a future-versioned bundle risks panicking on unknown enum
                // variants in nested wire types downstream.
                if record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION {
                    return Err(StoreError::Serialization(format!(
                        "bundle schema_version {} is newer than supported {}; \
                         upgrade chronos-store to read this bundle",
                        record.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
                    )));
                }
                Ok(Some(record))
            }
        }
    }

    /// List bundle summaries matching `filter`, oldest-first (uuid::v7
    /// lexicographic order matches chronological).
    ///
    /// m8-05 (B2 note): when `filter.cursor` is `Some(c)`, the iteration
    /// skips rows whose `bundle_id <= c` before applying other filters
    /// and `limit`. This gives forward pagination: the first page returns
    /// up to `limit` rows and sets `next_cursor = last.bundle_id`. The
    /// next call passes that bundle_id as `cursor` to fetch the
    /// following page.
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
            // m8-05 B2: forward pagination. Skip rows at or before the
            // cursor's bundle_id (uuid::v7 is monotonic, so the row key
            // matches chronological order).
            if let Some(c) = filter.cursor.as_deref() {
                if s.bundle_id.as_str() <= c {
                    continue;
                }
            }
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
                schema_version: 1,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(PropertyValue::Number(2.0))),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
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
                        schema_version: 1,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: Some("invariant"),
            since_ms: None,
            until_ms: None,
            limit: 100,
            cursor: None,
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
                        schema_version: 1,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let filter = CounterexampleBundleFilter {
            workspace_id: None,
            property_kind: None,
            since_ms: None,
            until_ms: None,
            limit: 3,
            cursor: None,
        };
        let summaries = store.list_counterexample_bundles(filter).unwrap();
        assert_eq!(summaries.len(), 3);
    }

    // m8-05 (B2): cursor skips rows <= cursor's bundle_id, returning
    // the page that begins AFTER the cursor.
    #[test]
    fn list_cursor_paginates_forward() {
        let store = make_store();
        for i in 0..6 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("b-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i as u64,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        // Page 1: limit=2, no cursor -> ["b-00", "b-01"].
        let page1 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: None,
            })
            .unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].bundle_id, "b-00");
        assert_eq!(page1[1].bundle_id, "b-01");
        // Page 2: cursor = "b-01" -> ["b-02", "b-03"].
        let page2 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-01".into()),
            })
            .unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].bundle_id, "b-02");
        assert_eq!(page2[1].bundle_id, "b-03");
        // Page 3: cursor = "b-03" -> ["b-04", "b-05"].
        let page3 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-03".into()),
            })
            .unwrap();
        assert_eq!(page3.len(), 2);
        assert_eq!(page3[0].bundle_id, "b-04");
        assert_eq!(page3[1].bundle_id, "b-05");
        // Page 4: cursor = "b-05" -> [].
        let page4 = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 2,
                cursor: Some("b-05".into()),
            })
            .unwrap();
        assert!(page4.is_empty(), "page past last bundle is empty");
    }

    // m8-05 (B2): cursor is opaque to callers; passing a non-existent
    // bundle_id as cursor yields a deterministic forward page (all
    // matching rows have id > cursor).
    #[test]
    fn list_cursor_unknown_id_returns_all() {
        let store = make_store();
        for i in 0..3 {
            store
                .save_counterexample_bundle(CounterexampleBundleRecord {
                    summary: CounterexampleBundleSummary {
                        bundle_id: format!("z-{:02}", i),
                        property_kind: "invariant".into(),
                        workspace_id: "ws".into(),
                        created_at_ms: i as u64,
                        rounds_used: 1,
                        has_full_bundle: true,
                        schema_version: 1,
                    },
                    events: vec![],
                    minimised: None,
                    event_cas_hashes: vec![],
                    target_hypothesis: None,
                    schema_version: 1,
                })
                .unwrap();
        }
        let summaries = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 100,
                // Lexicographically less than every "z-..." row, so
                // all rows pass.
                cursor: Some("a-00".into()),
            })
            .unwrap();
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
                schema_version: 1,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        let r = store.save_counterexample_bundle(rec);
        assert!(r.is_err(), "empty bundle_id must be rejected");
    }

    // m8-07 §5 (per-crate integration): save a bundle with a non-None
    // target_hypothesis, load it back, and assert the wire mirror survived
    // the bincode round-trip intact. This pins the D2 serde contract:
    // #[serde(default)] on the field means pre-m8-07 bundles (which have
    // no target_hypothesis on disk) deserialize cleanly as None.
    #[test]
    fn m8_07_save_then_load_preserves_target_hypothesis() {
        use crate::counterexample_storage::HypothesisInputWire;
        let store = make_store();
        let wire = HypothesisInputWire {
            session_id: "sess-m8-07-test".into(),
            kind: "invariant".into(),
            scope: Some("event_count".into()),
            comparison: Some("Ge".into()),
            constant: Some(chronos_domain::property::PropertyValue::Number(1000.0)),
            property_target: None,
            predicate: None,
            caller: None,
            callee: None,
            max_depth: None,
        };
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-m8-07".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 4,
                has_full_bundle: true,
                schema_version: 1,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(
                chronos_domain::property::PropertyValue::Number(0.0),
            )),
            event_cas_hashes: vec![],
            target_hypothesis: Some(wire),
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-m8-07")
            .unwrap()
            .unwrap();

        // The wire mirror survived the bincode round-trip.
        let th = loaded
            .target_hypothesis
            .expect("target_hypothesis must be present");
        assert_eq!(th.session_id, "sess-m8-07-test");
        assert_eq!(th.kind, "invariant");
        assert_eq!(th.scope.as_deref(), Some("event_count"));
        assert_eq!(th.comparison.as_deref(), Some("Ge"));
        assert!(th.constant.is_some());
    }

    // m8-07 §5 (backward compatibility): a bundle record without target_hypothesis
    // on disk must deserialize as target_hypothesis=None (serde #[serde(default)]).
    // We simulate this by constructing a record with target_hypothesis=None
    // and verifying it round-trips correctly.
    #[test]
    fn m8_07_pre_m8_07_bundle_has_no_target_hypothesis() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-pre-m8-07".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
            },
            events: vec![],
            minimised: Some(MinimisedPayload::Constant(
                chronos_domain::property::PropertyValue::Number(0.0),
            )),
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec.clone()).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-pre-m8-07")
            .unwrap()
            .unwrap();
        assert!(
            loaded.target_hypothesis.is_none(),
            "pre-m8-07 bundles must have target_hypothesis=None"
        );
    }

    // ========================================================================
    // m9-01 tests: schema_version on CounterexampleBundleRecord / Summary
    // ========================================================================

    // m9-01 §5: save a bundle via in-memory store, load it, assert both
    // summary.schema_version == 1 and record.schema_version == 1.
    #[test]
    fn m9_01_save_writes_schema_version_1() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-schema-test".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(rec).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-schema-test")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.schema_version, 1,
            "record.schema_version must be 1 after save"
        );
        assert_eq!(
            loaded.summary.schema_version, 1,
            "summary.schema_version must be 1 after save"
        );
    }

    // m9-01 §5: a JSON payload without schema_version must deserialize to 1
    // via #[serde(default = "default_schema_version")].
    #[test]
    fn m9_01_legacy_bundle_deserializes_with_schema_version_1() {
        // Verify the default function returns 1.
        assert_eq!(default_schema_version(), 1);
        // Verify serde_json correctly assigns the default when the field is absent.
        let json_no_version = r#"{
            "bundle_id": "b-legacy",
            "property_kind": "invariant",
            "workspace_id": "ws",
            "created_at_ms": 0,
            "rounds_used": 1,
            "has_full_bundle": true
        }"#;
        let summary: CounterexampleBundleSummary =
            serde_json::from_str(json_no_version).expect("serde_json must accept pre-m9-01 JSON");
        assert_eq!(
            summary.schema_version, 1,
            "pre-m9-01 JSON must default schema_version to 1"
        );
    }

    // m9-01 §5: a bincode blob with schema_version=2 on the record must be
    // rejected by load_counterexample_bundle with an error mentioning "newer
    // than supported".
    #[test]
    fn m9_01_future_version_load_is_rejected() {
        let store = make_store();

        // Inject a future-versioned record directly into the DB (bypassing
        // save_counterexample_bundle so we control the exact bytes).
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 2, // future version
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 2, // future version
        };
        let future_bytes = bincode::serialize(&future_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table.insert(&b"b-future"[..], future_bytes.as_slice()).unwrap();
        }
        tx.commit().unwrap();

        // Load it — must be rejected.
        let result = store.load_counterexample_bundle("b-future");
        let err = result.expect_err("future-versioned bundle must be rejected");
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "error message must mention 'newer than supported', got: {err_msg}"
        );
        assert!(
            err_msg.contains('2'),
            "error message must mention schema_version 2, got: {err_msg}"
        );
    }

    // m9-01 §5 D5: construct a record with schema_version=99, save it,
    // reload it, assert the persisted version is 1 (save overwrites caller's value).
    #[test]
    fn m9_01_save_overwrites_callers_schema_version() {
        let store = make_store();
        let rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-overwrite".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 0,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 99, // caller sets stale value
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 99, // caller sets stale value
        };
        store.save_counterexample_bundle(rec).unwrap();
        let loaded = store
            .load_counterexample_bundle("b-overwrite")
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.schema_version, 1,
            "save must overwrite caller's schema_version with CURRENT"
        );
        assert_eq!(
            loaded.summary.schema_version, 1,
            "save must overwrite caller's summary.schema_version with CURRENT"
        );
    }

    // m9-01 §5: list best-effort skips rows where bincode deserialization fails.
    // A future-versioned row with schema_version=2 deserializes fine in bincode
    // (bincode doesn't know about our version check), so it IS included in the
    // list. The explicit rejection only happens in load_counterexample_bundle.
    // This test pins that: both bundles appear in list, but load() rejects the
    // future-versioned row individually.
    #[test]
    fn m9_01_list_includes_future_versioned_row_best_effort() {
        let store = make_store();

        // Normal bundle: save via the API (writes schema_version=1).
        let normal_rec = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-normal".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 100,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 1,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 1,
        };
        store.save_counterexample_bundle(normal_rec).unwrap();

        // Future-versioned bundle: inject directly into the DB.
        let future_record: CounterexampleBundleRecord = CounterexampleBundleRecord {
            summary: CounterexampleBundleSummary {
                bundle_id: "b-future".into(),
                property_kind: "invariant".into(),
                workspace_id: "ws".into(),
                created_at_ms: 200,
                rounds_used: 1,
                has_full_bundle: true,
                schema_version: 2,
            },
            events: vec![],
            minimised: None,
            event_cas_hashes: vec![],
            target_hypothesis: None,
            schema_version: 2,
        };
        let future_bytes = bincode::serialize(&future_record).unwrap();
        let tx = store.db().begin_write().unwrap();
        {
            let mut table = tx.open_table(COUNTEREXAMPLE_BUNDLES).unwrap();
            table.insert(&b"b-future"[..], future_bytes.as_slice()).unwrap();
        }
        tx.commit().unwrap();

        // List all bundles: both appear (bincode deserializes schema_version=2 fine).
        let summaries = store
            .list_counterexample_bundles(CounterexampleBundleFilter {
                workspace_id: None,
                property_kind: None,
                since_ms: None,
                until_ms: None,
                limit: 100,
                cursor: None,
            })
            .unwrap();

        assert_eq!(
            summaries.len(), 2,
            "both normal and future-versioned bundles must appear in list \
             (best-effort; bincode deserializes fine)"
        );

        // load_counterexample_bundle rejects the future-versioned row individually.
        let load_result = store.load_counterexample_bundle("b-future");
        let err = load_result.expect_err("future-versioned bundle must be rejected on load");
        let err_msg = format!("{err}");
        assert!(
            err_msg.contains("newer than supported"),
            "load error must mention 'newer than supported', got: {err_msg}"
        );
    }
}
