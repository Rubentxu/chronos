# Release Receipt — m9-11-cycles-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-11-cycles-index-metadata-drift` |
| Path | B-direct |
| Head SHA | `cd0115f` |
| Tag | `v0.7.9` (annotated, peel matches published SHA) |
| Remote tag_peel | `cd0115f` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
  - `Total cycles` field: 22 → 26
  - `Last updated` field: 2026-09-12T08:44:00Z → 2026-09-12T08:21:30Z
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Added cross-check #5 (cycles/index.md metadata consistency)
  - Added m9-11 to the procedure reference list
  - Updated "When to escalate" wording (4 → cross-checks, generic)

## Runtime status

No production code changed. Doc-only cycle. No T1/T2/T4-smoke gates
required (per AGENTS.md §2 B-direct path = T0 + T1; T1 is a no-op
since the change is doc-only).

## Tag convention

Per chronos convention: tag peeled to the **fix commit** (`cd0115f`),
keeping runtime semantics on the tagged SHA. Docs commits (which would
follow) would fast-forward the tag peel, but this cycle has only the
fix commit — the docs (merge-receipt, release-receipt, archive-manifest,
apply-checkpoint) are added in this same release group and do not shift
the tag.

## Findings

No findings introduced or closed by this cycle. It is a vault-hygiene
cycle that closes a metadata drift the standing procedure missed.
