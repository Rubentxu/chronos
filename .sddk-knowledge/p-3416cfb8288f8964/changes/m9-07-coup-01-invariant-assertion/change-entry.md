# Change: m9-07 coup 01 invariant assertion

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-07-coup-01-invariant-assertion` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `aa96e5a51b5424a844f1f153d7ce719a53017119` |
| Head SHA | `3edb01f0a17c3ac8877df1e7868d4e3cca374217` |
| Tag | `v0.7.5` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `3edb01f` | fix(m9-07): loader rejects envelope/summary schema_version mismatch (closes FIND-M9-01-DV-COUP-01) |
| `5c5b3ec` | docs(m9-07): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-01 closed) |

## Scope

Trivial B-direct cleanup closing the long-outstanding **FIND-M9-01-DV-COUP-01**
(coupling · MEDIUM P2) finding from the m9-01 cycle. The debt-report prescribed
two remediation options:

> **Remediation (backlog):** add a 3-line loader assertion
> `record.schema_version == summary.schema_version`, or drop the nested field
> at the next wire-format break.

m9-07 takes **option A** — the loader-assertion path:

- `load_counterexample_bundle` previously hard-rejected only records with
  `record.schema_version > CURRENT_BUNDLE_SCHEMA_VERSION`. A hand-constructed
  record (e.g. a future migration tool) with `record.schema_version == 3`
  (in-range) but `record.summary.schema_version == 1` (drifted) would silently
  pass.
- The new guard (5 lines + doc) compares the envelope and summary versions
  after the in-range check passes. On mismatch, returns
  `StoreError::Serialization` with both version numbers in the message so the
  caller can diagnose the drift.
- `save()` already canonicalizes both fields to `CURRENT_BUNDLE_SCHEMA_VERSION`
  (lines 577-578 of `counterexample_storage.rs`), so legitimate writes never
  trip the new check. The guard exists only to surface malformed records
  produced by tools that bypass the public save API.

A new lib test, `m9_07_loader_rejects_envelope_summary_version_mismatch`,
injects an envelope=3/summary=1 record via the m9-05 chokepoint
`insert_bundle_record_for_test` and asserts the loader rejects it with the
new `"disagrees with summary"` message. The 57 pre-existing tests on the file
remain green (T1: 58/58).

### Changed paths

- `crates/chronos-store/src/counterexample_storage.rs` — added 5-line guard
  in `load_counterexample_bundle` + 1 doc comment + 1 lib test (33 lines
  incl. setup + assertions). Plus 2 lines incidental `assert!` reformat that
  `cargo fmt` repaired on the m9-06 const-eval block.

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | Duplicated `schema_version` on record + summary with no equality enforcement at load time | m9-07-coup-01-invariant-assertion (`v0.7.5`) |

Verdict: **PASS** (1/1 finding closed)

## Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Title | Reason |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | coupling | LOW P3 | List/load policy asymmetry shipped as an error-kind overload | Requires list-side change with a dedicated `SchemaTooNew` error variant; natural target = the next wire-format break |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | Splits require design pass + compat shim; growth +57 LoC this cycle (+114 net m9-04..m9-07) |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | `property_target` lost in fallback reconstruction | Deferred from m8-04 |
