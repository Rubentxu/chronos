# Change: m9-03 side table debt cleanup

## Summary

Drift closure cycle for this milestone.


## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-03-side-table-debt-cleanup` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `6f375fd96dbc0c03fe36b473d306f1e54d078b08` |
| Head SHA | `2c98ce9a1df65d44ae865376fee46eb0d95ac425` |
| Tag | `v0.7.1` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `570d215` | fix(m9-03): add bundle_events_count_or_legacy chokepoint, update services callers |
| `3d72f69` | fix(m9-03): delete dead save_counterexample_bundle_events API + fix count doc drift + extract collect_bundle_chunks helper |
| `2c98ce9` | docs(m9-03): light-verify evidence |

## Scope

Bounded debt-cleanup targeting four low/medium-priority findings from the m9-02 archive:

1. **FIND-M9-02-DV-API-01** — `save_counterexample_bundle_events` dead API eliminated.
2. **FIND-M9-02-DV-DOC-01** — `count_counterexample_bundle_events` doc drift fixed; docstring
   no longer claims the pre-m9-02 invariant that no longer holds.
3. **FIND-M9-02-DV-OE-01** — `collect_bundle_chunks` private helper consolidates three
   duplicate `open_table + iter + decode` loops from `load`, `count`, and the deleted
   `save` paths.
4. **FIND-M9-02-DV-COUP-01** — `bundle_events_count_or_legacy` chokepoint (D5) introduced;
   both `chronos-services` call sites now route through it. The fallback logic is co-located
   with the D5 canonical writer boundary.

FIND-M9-02-DV-PERF-01 (key layout forces full-scan) is explicitly out of scope and left
in the m9+ backlog.

### Changed paths

- `crates/chronos-store/src/counterexample_storage.rs`
- `crates/chronos-services/src/counterexample.rs`

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| FIND-M9-02-DV-API-01 | api | `save_counterexample_bundle_events` dead API: no caller uses it standalone | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-DOC-01 | doc | `events_count` doc drift: fallback branch not documented at call site | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-OE-01 | overeng | Side-table chunk-iteration skeleton duplicated in 3 places | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-COUP-01 | coupling | Fallback outside D5 chokepoint: wrong module boundary | m9-03-side-table-debt-cleanup (`v0.7.1`) |

Verdict: `PASS` (4/4 findings resolved) · FIND-M9-02-DV-PERF-01 → m9+ backlog

## Artefactos

| Kind | Path |
|---|---|
| Apply checkpoint | `apply-checkpoint.json` |
| Merge receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/merge-receipt.md` |
| Release receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/release-receipt.md` |
| Verify report | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/verify-report.md` |
| Archive manifest | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-03-side-table-debt-cleanup/archive-manifest.md` |
