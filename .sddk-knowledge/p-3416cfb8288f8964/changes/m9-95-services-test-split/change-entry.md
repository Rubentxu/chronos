# Change: m9-95 services counterexample test-split

## Summary

Cycle closed with no follow-up debt. Schema and drift sweep clean.

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-95-services-test-split` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Status | CLOSED |
| Branch | `chore/m9-95-services-test-split` |
| Tag | `v0.7.97` |

## Subject

Extract the 2169-line `mod tests { ... }` block from
`crates/chronos-services/src/counterexample.rs` (lines 1786-3955) into
a sibling file `crates/chronos-services/src/ce_services_tests.rs`
using the `#[path = "..."]` submodule pattern established by m9-94.

After m9-95: `counterexample.rs` shrinks from 3955 → ~1788 lines
(production code only); `ce_services_tests.rs` holds the 45 test
functions + 4 test helpers.

## Problem

`cc-001-god-module` had two test-half components:
- Storage side: `crates/chronos-store/src/counterexample_storage.rs`
  (1771 lines of tests) — closed by **m9-94**.
- Services side: `crates/chronos-services/src/counterexample.rs`
  (2169 lines of tests) — closed by **m9-95** (this cycle).

m9-94's closure handoff flagged
`FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC` as the next
carry-forward for the cc-001 family. m9-95 closes it.

## Approach

B-direct mechanical refactor using the same `#[path = "..."]`
submodule pattern as m9-94:

1. Create `crates/chronos-services/src/ce_services_tests.rs` (new
   sibling file).
2. Move the entire content of `mod tests { ... }` block from
   `counterexample.rs` into `ce_services_tests.rs` verbatim (with the
   `use super::*;` + proptest imports — the sibling `#[path]`
   submodule preserves the parent's scope).
3. Replace the inline `mod tests { ... }` block in `counterexample.rs`
   with a one-line declaration.

Purely mechanical. No public API change. No behavior change.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| chronos-services/src/ce_services_tests.rs | 1 added | +2172 | verbatim copy of inline test block + doc comment header |
| chronos-services/src/counterexample.rs | 1 modified | -2168 / +11 | removed test block, added `#[path]` declaration |

**Total**: 1 file added + 1 file modified; net ~+15 LoC.

## Out-of-scope

- cc-001-god-module production code split for `counterexample.rs`
  (1785 lines of production code). Not needed at this time — the
  production code is well-organized into named sections.
- cc-004-implicit-io-toctou (P3, LOW, deferred to m10+).

## Cross-check

- **CC#3**: m9-95 cycle apply-checkpoint `head_sha`, `remote_tag_peel`,
  `tag_peel_sha`, and `main_sha` all equal the cycle-artifacts commit
  SHA; `peel_match` = `true`.
- **CC#8**: All m9-95 SHAs (`base_sha`, `head_sha`, `main_sha`,
  `remote_tag_peel`) exist in the local git object store.
- **CC#11**: m9-95 apply-checkpoint `status` = `CLOSED`,
  `archived_at` = `2026-09-14`,
  `findings_closed` = `["FIND-M9-94-SERVICES-COUNTEREXAMPLE-TESTS-MONOLITHIC"]`,
  `peel_match` = `true`.
- **CC#22**: m9-95 release-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#23**: m9-95 merge-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#39**: cycles/index.md Total cycles = 95 (matches actual row
  count).
- **CC#42**: m9-95 release-receipt Remote tag_peel matches
  `git rev-parse v0.7.97^{commit}`.
- **CC#43**: m9-95 release-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#47**: m9-95 apply-checkpoint `base_sha` exists in git as a
  commit object.
- **CC#51**: cycles/index.md cycle (m9-95) has
  cycle-artifacts/ folder.
- **CC#53**: cycle branch `chore/m9-95-services-test-split` deleted
  after `--no-ff` merge.

## Files

- Source commit: TBD (m9-95: extract tests to ce_services_tests.rs).
- Cycle-artifacts commit: TBD.
- Merge commit: TBD.
- Cascade commits: per CC#42 fixpoint-cascade workaround.
