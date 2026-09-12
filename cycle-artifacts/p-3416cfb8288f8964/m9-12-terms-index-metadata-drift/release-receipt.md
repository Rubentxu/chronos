# Release Receipt — m9-12-terms-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-12-terms-index-metadata-drift` |
| Path | B-direct |
| Head SHA | `0012f12` |
| Tag | `v0.7.10` (annotated, peel matches published SHA) |
| Remote tag_peel | `0012f12` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`:
  - `Last archive` field: m9-10-m9-03-apply-checkpoint-rebuild → m9-11-cycles-index-metadata-drift
  - `Last updated` field: 2026-09-12T07:24:00Z → 2026-09-12T08:52:00Z
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Added cross-check #6 (terms/index.md "Last archive" ↔ cycles/index.md most-recent)
  - Added m9-12 to the procedure reference list
  - Updated "When to escalate" wording (covers checks 5 and 6)

## Runtime status

No production code changed. Doc-only cycle. No T1/T2/T4-smoke gates
required (per AGENTS.md §2 B-direct path = T0 + T1; T1 is a no-op
since the change is doc-only).

## Tag convention

Per chronos convention: tag peeled to the **fix commit** (`0012f12`),
keeping runtime semantics on the tagged SHA. Docs commits (which would
follow) would fast-forward the tag peel, but this cycle has only the
fix commit — the docs (merge-receipt, release-receipt, archive-manifest,
apply-checkpoint) are added in this same release group and do not shift
the tag.

## Findings

No findings introduced or closed by this cycle. It is a vault-hygiene
cycle that closes a metadata drift the standing procedure missed.
