# Change: m9-10 m9 03 apply checkpoint rebuild

## Summary

Drift closure cycle for this milestone.


## Subject

| Campo | Valor |
|---|---|
| Cycle ID | `m9-10-m9-03-apply-checkpoint-rebuild` |
| Path | `B-rebuild` |
| Status | CLOSED |
| Base SHA | `0e1474a61cf5777a6d0e22358aded55be222c784` |
| Head SHA | `69f200e2144bea2cd305c38903feb4814fb38806` |
| Tag | `v0.7.8` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `69f200e` | fix(m9-10): rebuild missing apply-checkpoint.json for m9-03 (vault-reorg gap) |
| `6dc4609` | docs(m9-10): light-verify evidence (R1 PASS, m9-03 apply-checkpoint rebuilt) |

## Scope

Trivial B-rebuild closing a **vault-reorg gap** identified by a
`findings_closed` ↔ `Terminated terms` cross-check sweep. The m9-03 cycle
closed 4 debt findings (`FIND-M9-02-DV-API-01`, `FIND-M9-02-DV-DOC-01`,
`FIND-M9-02-DV-OE-01`, `FIND-M9-02-DV-COUP-01`) and produced all the
expected cycle artifacts (merge-receipt.md, release-receipt.md,
release-report.md, verify-report.md, verify-findings.json), but the
`apply-checkpoint.json` was missing from the post-reorg path
`cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/`.

m9-10 rebuilds the missing `apply-checkpoint.json` from unmodified
pre-existing inputs:

- **head_sha / base_sha / tag / peel_sha**: lifted from `merge-receipt.md`
  and `release-receipt.md` of m9-03.
- **verified_at / released_at**: lifted from `release-report.md` of m9-03.
- **commits_since_base**: from `git log 6f375fd..2c98ce9` (3 commits: 3d72f69, 570d215, 2c98ce9).
- **findings_closed**: from `verify-report.md` table rows F1-F4 cross-referenced with the Terminated-terms table in `terms/index.md`.
- **findings_remaining_m9_plus**: snapshot at the moment m9-03 was archived (includes FIND-M9-02-DV-PERF-01 which m9-04 closed, and FIND-M9-01-DV-COUP-01/02 + FIND-M9-01-DV-OE-01 + m9-01-R4 which m9-06/m9-07/m9-08 closed).

The rebuilt file carries **cycle_id** = `m9-03-side-table-debt-cleanup`
(the cycle whose ledger it represents), while this cycle's own
`cycle_id` = `m9-10-m9-03-apply-checkpoint-rebuild`. The two are
intentionally distinct.

### Changed paths

- `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json` — created (100 lines, +100 -0).

## Findings resolved

This cycle closes **no debt findings**. It rebuilds a single missing
vault artifact that records the **already-closed** findings of m9-03.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-10-m9-03-apply-checkpoint-rebuild/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-10-m9-03-apply-checkpoint-rebuild/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Findings NOT resolved (m9+ backlog unchanged)

| ID | Cluster | Severity | Title | Status |
|---|---|---|---|---|
| m9-02 R1–R8 | disclosure | — | various scope disclosures | by-design (untouched) |
| m9-01 R1–R3 | disclosure | — | forward-compat + canonical-write disclosures | by-design (untouched) |
| m9-04 R1–R6 | disclosure | — | layout + collision + scan disclosures | by-design (untouched) |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | design-required split |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | concurrency design required |
| m8-06-R4 | disclosure | — | Cross-variant existence predicate shrinking | by-design (untouched) |
| m8-04-R-hypothesis-fallback | disclosure | — | `property_target` lost in fallback reconstruction | by-design (untouched) |
| pre-reorg m8-* apply-checkpoints | vault | — | m8-04-R4 and m8-07-R2 are terminated in terms/index.md but their apply-checkpoints never existed in this checkout | pre-vault-reorg (not auto-safe to rebuild without source artifacts) |
