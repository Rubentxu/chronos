# Change Entry — m9-12-terms-index-metadata-drift

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-12-terms-index-metadata-drift` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `f4818d1` (main @ start of cycle) |
| Head SHA | `0012f12` |
| Tag | `v0.7.10` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `0012f12` | fix(m9-12): terms/index.md Last archive metadata drift (m9-10 → m9-11) + add cross-check #6 to vault-drift-sweep |

## What changed

- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`: `Last archive`
  metadata field updated from `m9-10-m9-03-apply-checkpoint-rebuild` →
  `m9-11-cycles-index-metadata-drift`. `Last updated` updated to
  `2026-09-12T08:52:00Z`.
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  added **cross-check #6** (terms/index.md "Last archive" ↔
  cycles/index.md most-recent cycle consistency). Without this check,
  future cycles can re-introduce the same drift that m9-11 did.

## Findings

- Closed: none (this is a hygiene cycle, no findings to remediate).
- Introduced: none.

## Why this matters

The standing `vault-drift-sweep.md` procedure (introduced by m9-09/m9-10,
extended by m9-11 with cross-check #5) established that **vault drift is
a first-class maintenance surface**. This cycle closes another gap in
that procedure: the `Last archive` metadata field in `terms/index.md`
was drifting for 1 cycle (m9-11 added itself to cycles/index.md but
didn't bump terms/index.md), and the procedure had no cross-check for
this.

The new cross-check #6 closes the gap and makes the procedure
self-referentially catching: the check that caught this drift is now
part of the procedure that future sessions will run before declaring
"auto-mode exhausted".

## Vault structure impact

- New cycle folder: `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/`
  (4 receipts: merge, release, release-report, verify-report, verify-findings, apply-checkpoint)
- New change-entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-12-terms-index-metadata-drift/change-entry.md`
  (this file)
- New archive-manifest: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-12-terms-index-metadata-drift/archive-manifest.md`

## Tag convention

Tag `v0.7.10` peeled to fix commit `0012f12`, preserving runtime
semantics on the tagged SHA (chronos convention).
