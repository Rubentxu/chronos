# Change Entry — m9-13-change-entry-base-sha-drift

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-13-change-entry-base-sha-drift` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `bfb9ede` (main @ start of cycle) |
| Head SHA | `26848cf8b26340d3fde99a3a7f398f2873943982` |
| Tag | `v0.7.11` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `26848cf8b26340d3fde99a3a7f398f2873943982` | fix(m9-13): m9-11 change-entry Base SHA drift (cd0115f → 6120e98) + add cross-check #7 to vault-drift-sweep |

## What changed

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`:
  `Base SHA` field corrected from `cd0115f` (the short-prefix that pointed at the fix commit, wrong)
  to `6120e98` (the parent of the fix, main HEAD before m9-11).
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  added **cross-check #7** (change-entry Head/Base SHA ↔ apply-checkpoint
  consistency). Without this check, future cycles can re-introduce the
  same drift that m9-11 did.

## Findings

- Closed: none (this is a hygiene cycle, no findings to remediate).
- Introduced: none.

## Why this matters

The standing `vault-drift-sweep.md` procedure (introduced by m9-09/m9-10,
extended by m9-11..m9-12 with cross-checks #5/#6) established that
**vault drift is a first-class maintenance surface**. This cycle closes
another gap: change-entry SHA fields can drift away from apply-checkpoint
SHA fields due to copy-paste errors. The new cross-check #7 catches
this by parsing change-entry SHA fields and verifying they are prefixes
of the corresponding apply-checkpoint SHA fields.

## Vault structure impact

- New cycle folder: `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/`
- New change-entry: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-13-change-entry-base-sha-drift/change-entry.md`
  (this file)
- New archive-manifest: `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-13-change-entry-base-sha-drift/archive-manifest.md`

## Tag convention

Tag `v0.7.11` peeled to fix commit `26848cf8b26340d3fde99a3a7f398f2873943982`, preserving runtime
semantics on the tagged SHA (chronos convention).
