# Release Receipt — m9-13-change-entry-base-sha-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-13-change-entry-base-sha-drift` |
| Path | B-direct |
| Head SHA | `26848cf` |
| Tag | `v0.7.11` (annotated, peel matches published SHA) |
| Remote tag_peel | `26848cf` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`:
  - `Base SHA` field: `cd0115f` → `6120e98` (parent of fix, main HEAD before m9-11)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Added cross-check #7 (change-entry Head/Base SHA ↔ apply-checkpoint consistency)
  - Added m9-13 to the procedure reference list
  - Updated "When to escalate" wording to cover checks 5, 6, and 7

## Runtime status

No production code changed. Doc-only cycle. No T1/T2/T4-smoke gates
required.

## Tag convention

Per chronos convention: tag peeled to the **fix commit** (`26848cf`),
keeping runtime semantics on the tagged SHA.

## Findings

No findings introduced or closed by this cycle. It is a vault-hygiene
cycle that closes a documentation drift the standing procedure missed.
