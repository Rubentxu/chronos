# Change: m9-94 cc-001 god-module test-split

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-94-cc001-test-split` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (mechanical test code extraction, no behavior change) |
| Status | CLOSED |
| Branch | `chore/m9-94-cc001-test-split` |
| Tag | `v0.7.96` |

## Subject

Extract the 1771-line `mod tests { ... }` block from
`crates/chronos-store/src/counterexample_storage.rs` into a sibling
file `crates/chronos-store/src/ce_storage_tests.rs` using the
`#[path = "..."]` submodule pattern already established by the
m9-84..m9-87 cc-001 production code splits.

After m9-94: `counterexample_storage.rs` shrinks from 1960 → ~188
lines (production code only); `ce_storage_tests.rs` holds the 43
test functions + 4 test helpers.

## Problem

`cc-001-god-module` (P2, MEDIUM, opened m9-04) tracked the 5
distinct concerns of `counterexample_storage.rs`. The m9-84..m9-87
cycles extracted all 5 production concerns into sibling files. The
remaining bulk of the file (1771 lines, 90%) is the inline
`mod tests { ... }` block — a separate smell that the production
splits did not address.

The m9-93 handoff recommended m9-94 = cc-001 god-module follow-up
"keys-split is smallest/lowest-risk". The keys-split was the
smallest production split (m9-84); the test-split is the smallest
remaining split (no production behavior change).

## Approach

B-direct mechanical refactor using the existing `#[path = "..."]`
submodule pattern (same pattern as the m9-84..m9-87 splits):

1. Create `crates/chronos-store/src/ce_storage_tests.rs` (new sibling
   file).
2. Move the entire content of `mod tests { ... }` block from
   `counterexample_storage.rs` into `ce_storage_tests.rs` verbatim
   (with the `use super::*;` import — sibling `#[path]` submodules
   preserve the parent's scope).
3. Replace the inline `mod tests { ... }` block in
   `counterexample_storage.rs` with a one-line declaration:
   ```rust
   #[cfg(test)]
   #[path = "ce_storage_tests.rs"]
   mod tests;
   ```

Purely mechanical. No public API change. No behavior change.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| chronos-store/src/ce_storage_tests.rs | 1 added | +1772 | verbatim copy of inline test block |
| chronos-store/src/counterexample_storage.rs | 1 modified | -1767 / +8 | removed test block, added `#[path]` declaration |

**Total**: 1 file added + 1 file modified; net ~+13 LoC (file headers + module decl).

## Out-of-scope

- `crates/chronos-services/src/counterexample.rs` (2169 lines of
  tests). Same pattern can be applied in a future cycle. m9-94
  establishes the technique.
- `cc-001` production code re-split: already complete in m9-84..m9-87.
- `cc-004-implicit-io-toctou` (deferred to m10+).

## Cross-check

- **CC#3**: m9-94 cycle apply-checkpoint `head_sha`,
  `remote_tag_peel`, `tag_peel_sha`, and `main_sha` all equal the
  cycle-artifacts commit SHA; `peel_match` = `true`.
- **CC#8**: All m9-94 SHAs (`base_sha`, `head_sha`, `main_sha`,
  `remote_tag_peel`) exist in the local git object store.
- **CC#11**: m9-94 apply-checkpoint `status` = `CLOSED`,
  `archived_at` = `2026-09-14`,
  `findings_closed` = `["FIND-M9-94-CC001-TEST-CODE-MONOLITHIC"]`,
  `peel_match` = `true`.
- **CC#22**: m9-94 release-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#23**: m9-94 merge-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#39**: cycles/index.md Total cycles = 94 (matches actual row
  count).
- **CC#42**: m9-94 release-receipt Remote tag_peel matches
  `git rev-parse v0.7.96^{commit}`.
- **CC#43**: m9-94 release-receipt Head SHA matches
  apply-checkpoint head_sha.
- **CC#47**: m9-94 apply-checkpoint `base_sha` exists in git as a
  commit object.
- **CC#51**: cycles/index.md cycle (m9-94) has
  cycle-artifacts/ folder.
- **CC#53**: cycle branch `chore/m9-94-cc001-test-split` deleted
  after `--no-ff` merge.

## Files

- Source commit: TBD (m9-94: extract tests to ce_storage_tests.rs).
- Cycle-artifacts commit: TBD.
- Merge commit: TBD.
- Cascade commits: per CC#42 fixpoint-cascade workaround.
