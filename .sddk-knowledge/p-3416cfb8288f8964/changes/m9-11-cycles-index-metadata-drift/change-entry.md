# Change: m9-11 cycles index metadata drift

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-11-cycles-index-metadata-drift` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `6120e98` (parent of fix commit, main HEAD before m9-11) |
| Head SHA | `cd0115fd8f942058cde109c72a975cab7ea7473c` |
| Tag | `v0.7.9` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `cd0115fd8f942058cde109c72a975cab7ea7473c` | fix(m9-11): cycles/index.md Total cycles metadata drift (22→26) + add cross-check #5 to vault-drift-sweep procedure |

## What changed

- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`: `Total cycles`
  metadata field updated from `22` → `26` to reflect actual data-row count.
  `Last updated` updated to `2026-09-12T08:21:30Z`.
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  added **cross-check #5** (cycles/index.md metadata consistency). Without
  this check, future cycles can re-introduce the same drift the previous
  m9-07..m9-10 cycles did.

## Findings

- Closed: none (this is a hygiene cycle, no findings to remediate).
- Introduced: none.

## Why this matters

The standing `vault-drift-sweep.md` procedure (introduced by m9-09/m9-10)
established that **vault drift is a first-class maintenance surface**.
This cycle closes a gap in that procedure: the `Total cycles` metadata
field was drifting for 4 cycles without being caught, because the
procedure had no cross-check for it.

The new cross-check #5 closes the gap and makes the procedure
self-referentially catching: the check that caught this drift is now part
of the procedure that future sessions will run before declaring
"auto-mode exhausted".

## Vault structure impact

- New cycle folder: `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/`
  (4 receipts: merge, release, release-report, verify-report, verify-findings, apply-checkpoint)
- New change-entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`
  (this file)
- New archive-manifest: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-11-cycles-index-metadata-drift/archive-manifest.md`

## Tag convention

Tag `v0.7.9` peeled to fix commit `cd0115fd8f942058cde109c72a975cab7ea7473c`, preserving runtime semantics
on the tagged SHA (chronos convention).
