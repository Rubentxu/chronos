# M9-01 scoping — Schema versioning on `CounterexampleBundleRecord` / `CounterexampleBundleSummary`

**Cycle:** `m9-01-schema-versioning`
**Branch:** `feat/m9-01-schema-versioning`
**Base:** `bceddc9` (main at m8-07 close)
**Status:** PROPOSED — 2026-09-11
**Precedence:** `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md` §6 R2

## §1 Problem statement

m8-07 §6 R2 (NEW) discloses that `CounterexampleBundleRecord` has no
on-disk schema version:

> "If a future chronos-services version adds a field to `HypothesisInput`,
> the wire mirror will silently drop it on persist. The `target_hypothesis`
> field on the redb row is not versioned. Mitigated by the m9+ plan to
> add `schema_version: u32` to `CounterexampleBundleSummary` (separate
> followup)."

Without a `schema_version`, three concrete failure modes are open:

1. **Silent field drops on legacy reads.** A bundle persisted by an older
   chronos-services build that did not know about `target_hypothesis`
   deserializes today because of `#[serde(default)]` on the field — but
   the reader has no way to distinguish "no `target_hypothesis` because
   legacy" from "no `target_hypothesis` because the user passed
   `None`". Future fields added to `HypothesisInputWire` (or any other
   wire struct in the record envelope) lose the same signal. Without a
   version, every additive change requires another `#[serde(default)]`
   band-aid with no audit trail.

2. **Forward-compatibility is undefined.** If a future chronos-services
   build writes `schema_version = 2`, the current reader cannot decide
   whether to: (a) load and trust, (b) load with a deprecation warning,
   (c) reject. m8-04 / m8-07 can only guarantee "no panic on legacy
   load" — not "act correctly on future load". A `schema_version`
   integer is the smallest unit that lets us pick a policy.

3. **Bundle GC + migration tooling is blind.** The m8-05 list path, the
   m8-04 replay path, and (m9+) a future `chronos test bundle-migrate`
   tool all need to know "which version am I reading?" before they can
   decide to upgrade, downgrade, or skip a row. Today the only signal is
   "field is Some or None" — a binary read of presence, not an integer
   version.

The chronos-log precedent (v1 → v2 coexistence via
`SegmentMetadata.schema_version: u32`, see
`crates/chronos-log/src/segment.rs:50,60` and the
`tests/v1_v2_coexistence.rs` precedent) is the template: a `u32`
monotonic counter, serialized in the record header, defaulted to `1`
for legacy bundles.

## §2 Goals & non-goals

### Goals

1. Add `schema_version: u32` to
   `chronos_store::counterexample_storage::CounterexampleBundleSummary`
   (the row-level metadata struct that all bundle paths read).
2. Add `schema_version: u32` to
   `chronos_store::counterexample_storage::CounterexampleBundleRecord`
   so the field is visible at the record envelope (the bincode blob)
   without forcing callers to walk into `summary`.
3. Both fields use `#[serde(default = "default_schema_version")]`
   where `default_schema_version() -> u32 { 1 }`. Bundles persisted
   pre-m9-01 have no such field and deserialize cleanly as
   `schema_version: 1`.
4. Define a module-level constant
   `pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 1;` in
   `counterexample_storage.rs` (the canonical "what we write today"
   value) and a `KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1];` for the
   loader to consult.
5. Loader policy on `load_counterexample_bundle`: if
   `record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION`, return
   `Err(StoreError::Serialization(format!(
   "bundle schema_version {v} is newer than supported {CURRENT}; \
   upgrade chronos-store to read this bundle")))` (a hard reject with
   an actionable message). If `record.schema_version == 1`, accept
   silently (no log, no warning — version 1 is the only known shape
   and matches the wire format we just wrote).
6. `save_counterexample_bundle` always sets
   `record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION` and
   `record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION` so future
   versions only require a constant bump.
7. `chronos_services::counterexample::save` passes the same version
   into the wire summary it builds (the chronos-services-side type
   `CounterexampleBundleSummary` does NOT carry the field — it lives
   only in the chronos-store wire shape, since the services-side
   summary is rebuilt from the wire on every `Get` / `List` /
   `events_count` path). The conversion
   `counterexample_summary_from_wire` passes
   `s.schema_version` through if/when the services-side summary grows
   the field (out of scope for m9-01; the services-side type keeps the
   same field set today).
8. Tests pin the wire shape:
   - `m9_01_save_writes_schema_version_1`: save a bundle, reload,
     assert `schema_version == 1` on both `summary` and `record`.
   - `m9_01_legacy_bundle_deserializes_with_schema_version_1`: hand-craft
     a bincode blob without `schema_version`, load it, assert
     `schema_version == 1`.
   - `m9_01_future_version_load_is_rejected`: hand-craft a bincode
     blob with `schema_version = 2`, load it, assert
     `Err(StoreError::Serialization(_))` with a message that mentions
     "newer than supported".

### Non-goals (deferred to m9+)

- **Bumping `CURRENT_BUNDLE_SCHEMA_VERSION` to 2.** m9-01 only adds
  the field at value 1. Any future change to the record envelope
  (e.g., adding a new wire field to `HypothesisInputWire`) bumps the
  constant. The migration policy for version 2 → 3 is a future cycle.
- **Persisting per-component versions.** A bundle record has
  ~5 nested wire types (`MinimisedPayload`, `ExistencePredicateWire`,
  `HypothesisInputWire`, `CounterexampleBundleSummary`,
  `CounterexampleBundleRecord` itself). Each could carry its own
  sub-version, but that's a v2/v3 problem; v1 lumps everything under
  one record-level counter (matches the chronos-log precedent, which
  uses one `schema_version` per segment, not per record).
- **Migrating existing bundles.** All pre-m9-01 bundles silently
  upgrade to `schema_version: 1` via `#[serde(default)]`. No rewrite
  pass. No `chronos test bundle-migrate` tool. m9+ scope.
- **Wire DTO surface for `schema_version`.** The MCP
  `CounterexampleBundleSummaryDto` and the chronos-services
  `CounterexampleBundleSummary` keep their existing field sets. The
  `schema_version` is on the redb row, not on the LLM-facing wire.
  Surfacing it on the wire is a separate decision (do we want agents
  to act on it?). m9+ scope; for m9-01 it is internal-only.
- **Rejecting bundles with `schema_version == 0`.** 0 is an
  impossible pre-m9-01 state (no record had the field, so serde
  defaults it to 1). If a future bug ever writes 0, the loader still
  loads it. We do not add a `>= 1` floor check in m9-01; the
  chronos-log precedent treats 0 as "missing / unset" rather than
  "invalid".

## §3 Architectural decisions

### D1 — Where the field lives: `CounterexampleBundleSummary` (primary) + `CounterexampleBundleRecord` (envelope mirror)

Two struct changes, both in `crates/chronos-store/src/counterexample_storage.rs`:

* `CounterexampleBundleSummary` gains `pub schema_version: u32` with
  `#[serde(default = "default_schema_version")]`. This is the
  row-metadata view returned by `list_counterexample_bundles`; the
  version is visible on every list response.
* `CounterexampleBundleRecord` gains `pub schema_version: u32` with
  the same serde default. This is the bincode envelope; the version
  is visible on every load.

The duplication is intentional: `CounterexampleBundleRecord` is the
SERIALIZED envelope (matters for serde + bincode compatibility);
`CounterexampleBundleSummary` is the row-metadata view (matters for
list responses). Both must carry the field so:

* `list_counterexample_bundles` can filter by version (m9+ scope, but
  the field has to be readable from the summary to enable it).
* `load_counterexample_bundle` can read the envelope-level version
  before deciding whether to deserialize nested wire types.

A "the summary version always equals the record version" invariant
holds by construction (the save path writes both; the load path reads
the record's version and exposes the summary's). If they ever diverge,
the record's version is authoritative.

### D2 — Serde default + constant

```rust
const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 1;
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1];

fn default_schema_version() -> u32 { 1 }
```

Both fields use `#[serde(default = "default_schema_version")]`. Bundles
persisted pre-m9-01 (no field on disk) deserialize as
`schema_version: 1`. Bundles persisted by a future build with
`schema_version: 2` deserialize as 2; the loader's policy (D3) decides
what to do.

The constant lives at module scope (not in a `pub mod versioning`) so
the import surface stays minimal and the constant is grep-able from
one file.

### D3 — Loader policy: hard-reject future versions

```rust
// in load_counterexample_bundle, after deserialization:
if record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION {
    return Err(StoreError::Serialization(format!(
        "bundle schema_version {} is newer than supported {}; \
         upgrade chronos-store to read this bundle",
        record.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
    )));
}
```

Rationale for hard-reject (not "load with warning"):

* The bundles are small (one record per shrink run) and redb is a
  local on-disk store. No remote reader needs to be tolerant.
* A silently-loaded future-versioned bundle may have field shapes we
  don't understand (e.g., a new `MinimisedPayload` variant) and
  downstream code (`counterexample_summary_from_wire`,
  `MinimisedPayload` match arms, `reconstruct_hypothesis`) would
  panic on the unknown variant. Hard-reject fails closed.
* The error message names the version and points to the fix
  (`upgrade chronos-store`). m9-02 / m9-03 cycles can relax this to
  "load with `serde(deny_unknown_fields)` off + a deprecation log" if
  we ever decide the bundles are large enough to matter.

We do NOT add a "version too old" reject. `schema_version: 0` (which
serde will never produce via the default) loads with whatever fields
happen to be present; the `#[serde(default)]` machinery is the
"version too old" tolerance.

### D4 — Services-side summary is unchanged

`chronos_services::counterexample::CounterexampleBundleSummary` (the
typed struct returned by `Got` / `Saved` / `Shrunk` / `Listed`) does
NOT carry `schema_version` in m9-01. The chronos-services summary is
rebuilt on every read from the wire `CounterexampleBundleSummary`
(which DOES carry it). The conversion
`counterexample_summary_from_wire` ignores
`s.schema_version` for now (it could plumb it through later if the
services-side type grows the field).

This keeps the LLM-facing wire (`CounterexampleBundleSummaryDto` in
`output.rs`) unchanged — no MCP tool surface changes, no agent-visible
behaviour change. The version is internal bookkeeping.

### D5 — `save_counterexample_bundle` always writes the current version

The save function takes the user's `record: CounterexampleBundleRecord`
and overwrites both `summary.schema_version` and `record.schema_version`
with `CURRENT_BUNDLE_SCHEMA_VERSION` before bincode-serializing. This
makes the version a controlled output: even if a future caller
accidentally constructs a record with `schema_version: 0` (e.g., from
a struct literal), the persisted version is always current.

(Equivalent precedent: `chronos_log::segment::SEGMENT_VERSION` is set
on encode, not trusted from the caller's record.)

### D6 — No MCP / CLI surface change

Same as m8-07 D5: this is an internal schema change. No new MCP tool,
no new CLI flag, no new field in the agent-facing wire. The change is
observable only via:

* A new test asserting `record.schema_version == 1` after save/load.
* The error path when loading a future-versioned bundle (which is
  reachable only if a future chronos-services build is mixed with a
  current chronos-store — an operator error, not an agent error).

## §4 Implementation sketch

### chronos-store (1 file, ~30 LoC)

`crates/chronos-store/src/counterexample_storage.rs`:

```rust
/// Canonical "what we write today" value. Bumped when the bundle
/// envelope (record + summary + nested wire types) changes in a way
/// that requires a loader-side decision. See module docs.
pub const CURRENT_BUNDLE_SCHEMA_VERSION: u32 = 1;

/// Versions the loader accepts silently. Today only 1; future cycles
/// add entries here when they introduce a new envelope shape.
const KNOWN_BUNDLE_SCHEMA_VERSIONS: &[u32] = &[1];

fn default_schema_version() -> u32 { 1 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterexampleBundleSummary {
    pub bundle_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterexampleBundleRecord {
    pub summary: CounterexampleBundleSummary,
    pub events: Vec<TraceEvent>,
    pub minimised: Option<MinimisedPayload>,
    #[serde(default)]
    pub event_cas_hashes: Vec<ContentHash>,
    #[serde(default)]
    pub target_hypothesis: Option<HypothesisInputWire>,
    /// m9-01: envelope-level version. Always equals `summary.schema_version`
    /// for records written by this build; the loader reads `record.schema_version`
    /// (the envelope) as authoritative.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}
```

`save_counterexample_bundle` overwrites both fields before serialize:

```rust
let mut record = record;
record.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
record.summary.schema_version = CURRENT_BUNDLE_SCHEMA_VERSION;
let bytes = bincode::serialize(&record)...
```

`load_counterexample_bundle` rejects future versions after deserialize:

```rust
let record: CounterexampleBundleRecord = bincode::deserialize(bytes)
    .map_err(|e| StoreError::Serialization(e.to_string()))?;
if record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION {
    return Err(StoreError::Serialization(format!(
        "bundle schema_version {} is newer than supported {}; \
         upgrade chronos-store to read this bundle",
        record.schema_version, CURRENT_BUNDLE_SCHEMA_VERSION,
    )));
}
Ok(Some(record))
```

The list path (`list_counterexample_bundles`) does NOT reject — it
best-effort skips rows it cannot deserialize, same precedent as today
(see line ~290 of the existing file). Future-versioned bundles would
be skipped silently in the list path; the next caller's `Get` on the
same `bundle_id` would fail with the explicit error.

### chronos-services (no change)

`save()` constructs the wire summary and currently does:

```rust
let summary = CounterexampleBundleSummaryWire {
    bundle_id: ...,
    property_kind: ...,
    workspace_id: ...,
    created_at_ms,
    rounds_used,
    has_full_bundle: true,
};
```

m9-01 adds one line:

```rust
let summary = CounterexampleBundleSummaryWire {
    ...,
    schema_version: chronos_store::counterexample_storage::CURRENT_BUNDLE_SCHEMA_VERSION,
};
```

The constant is re-exported through chronos-store's public surface
(`pub const`) so chronos-services can read it without copying the
literal. This keeps a single source of truth — the next time the
version bumps, only `counterexample_storage.rs` changes.

The services-side `CounterexampleBundleSummary` (the typed DTO) is
unchanged. `counterexample_summary_from_wire` ignores
`s.schema_version` for now.

### Sandbox (no new test)

The smoke subset re-runs the existing `counterexample_tools` (ce1..ce12).
ce12 (added in m8-07) covers the `target_hypothesis` round-trip; the
`schema_version` field is co-tested by the same fixtures because every
shrink in those tests now persists `schema_version: 1` and reloads
with `schema_version: 1`. No new sandbox test needed.

### chronos-cli (no change)

`chronos test replay <bundle_id>` reads the bundle, runs the
hypothesis_test dispatcher, prints the report. None of that code
touches `schema_version`. If a future-versioned bundle were loaded,
the new error surfaces as the existing
`ServiceError::LoadFailed(format!("counterexample reload: {e}"))`
path.

## §5 Tests

### Unit (chronos-store lib)

* `m9_01_save_writes_schema_version_1` — save a bundle via
  `SessionStore::in_memory`, load it, assert both
  `loaded.summary.schema_version == 1` and
  `loaded.schema_version == 1`.
* `m9_01_legacy_bundle_deserializes_with_schema_version_1` —
  hand-craft a bincode blob from a pre-m9-01-shaped record
  (`schema_version` field absent), deserialize it, assert
  `record.schema_version == 1` and `record.summary.schema_version == 1`.
* `m9_01_future_version_load_is_rejected` — hand-craft a bincode blob
  with `schema_version: 2` on the record, load it, assert
  `Err(StoreError::Serialization(_))` whose message contains
  `"newer than supported"`.
* `m9_01_save_overwrites_callers_schema_version` — construct a record
  with `schema_version: 99`, save it, reload it, assert the persisted
  version is `1` (not `99`). Pins D5.
* `m9_01_list_skips_future_versioned_rows_best_effort` — save a
  normal-versioned bundle and a future-versioned bundle in the same
  store, list, assert only the normal one appears. Pins the list path's
  best-effort tolerance.

### Per-crate integration

* `services::counterexample::tests::save_persists_schema_version_1` —
  call `ChronosCounterexampleService::save` end-to-end (with a real
  in-memory `SessionStore`), then `load_counterexample_bundle`,
  assert `schema_version: 1`. Confirms the chronos-services save path
  passes the version into the wire summary it builds.

### Sandbox (T4-smoke subset re-runs)

* `counterexample_tools` — ce1..ce12 all green; the ce12
  `target_hypothesis` round-trip implicitly covers `schema_version`
  because every shrink in the suite persists version 1 and reloads
  version 1.

## §6 Disclosures

### R1 (NEW) — `schema_version` is silently overwritten on save

If a caller constructs a `CounterexampleBundleRecord` with
`schema_version != CURRENT_BUNDLE_SCHEMA_VERSION` (e.g., from a struct
literal that hard-codes a stale value), `save_counterexample_bundle`
overwrites both `record.schema_version` and
`record.summary.schema_version` with `CURRENT`. This is the intended
behaviour (single source of truth — chronos-store is the canonical
writer), but it means the field is not a faithful round-trip of
whatever the caller constructed. Mitigated by D5 + a unit test
(`m9_01_save_overwrites_callers_schema_version`).

### R2 (NEW) — Future-versioned bundles are silently skipped in `list`

The `list_counterexample_bundles` path best-effort skips rows that
fail to deserialize, but `load_counterexample_bundle` rejects future
versions. If a future chronos-services build writes
`schema_version = 2` rows into a shared redb, the current reader's
list view omits them entirely (no error, no warning). The next caller's
`counterexample_get <bundle_id>` on the same row returns the explicit
"newer than supported" error. Mitigation: if m9+ ever decides list
visibility matters for future versions, add a `with_warnings` flag to
`list_counterexample_bundles`. Out of scope for m9-01.

### R3 (NEW) — `schema_version` is internal-only

The MCP wire (`CounterexampleBundleSummaryDto`) and the
chronos-services-side `CounterexampleBundleSummary` do NOT expose
`schema_version` in m9-01. Agents cannot read the version through any
tool. This is intentional (D4) but means the field is invisible to
the agent surface. If a future cycle decides agents should act on the
version (e.g., to decide whether to upgrade chronos), the field would
need to be promoted to the services-side summary + the MCP DTO. m9+
scope.

### R4 (NEW) — `KNOWN_BUNDLE_SCHEMA_VERSIONS` is currently unused

The constant is declared (D2) but the loader policy in D3 only checks
the upper bound (`> CURRENT`), not membership in the known list.
Future cycles that add a non-contiguous version (e.g., v3 published
without v2 ever existing) would tighten the check to
`!KNOWN_BUNDLE_SCHEMA_VERSIONS.contains(&record.schema_version)`. For
m9-01, the upper-bound check is sufficient because we only ever bump
sequentially from 1.

## §7 Tier & gate plan

This is **A-min** scope:

* T0: fmt + clippy
* T1: chronos-store lib (the only crate touched at production-code
  level; chronos-services has a one-line constant reference).
* T2: chronos-store + chronos-services integration tests.
* T4-smoke: `counterexample_tools` (full file, ce1..ce12) +
  `e2e_connectivity` (server-startup canary, no behaviour change but
  keeps the smoke subset honest).

No chronos-e2e (D bucket), no benches (E bucket).

## §8 Smoke subset chosen

Per AGENTS.md §2, the cycle touches the bincode envelope of the
`counterexample_bundles` table (every save / load goes through
chronos-store), so the smoke subset is:

- `counterexample_tools` (full file, 12 tests) — ce1..ce12 cover the
  save / get / list / shrink / events_count / replay path. Every
  fixture in this file now round-trips `schema_version` implicitly.
- `e2e_connectivity` — confirms the MCP server starts and accepts the
  first tool call (no regression in the chronos-mcp wiring).

The selection matches the m8-07 subset + adds the chronos-store lib
unit tests in §5 (those run at T1, not T4, but they pin the new
contract).

## §9 What's NOT closed by m9-01

- **m8-04 R-hypothesis-reconstruction-fidelity** — closed by m8-07
  (target_hypothesis is now persisted). NOT a m9-01 deliverable.
- **m8-04 R4** (bundle-as-blob → side table) — still open, m9+ scope.
  Events ride inside the bundle blob; m9-01's `schema_version` lives
  on the summary, not on the events table.
- **m8-06 R4** (cross-variant existence predicate shrinking) — still
  open, m9+ scope. m9-01's `schema_version` is independent of variant
  evolution.
- **Migration tooling** — `chronos test bundle-migrate` (or equivalent)
  for upgrading pre-m9-01 bundles from `schema_version: 0`-equivalent
  to a future `schema_version: 2`. m9+ scope; for m9-01 the
  `#[serde(default)]` machinery handles legacy loads transparently.
- **Wire DTO exposure** — surfacing `schema_version` on the
  LLM-facing wire (`CounterexampleBundleSummaryDto`). m9+ scope (R3).
- **Future `schema_version: 2` work** — any change that requires
  bumping the constant (e.g., adding a new field to
  `MinimisedPayload`) is its own cycle, with its own loader policy
  decision.