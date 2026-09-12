# Release Receipt — m9-15-m9-12-m9-13-short-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-15-m9-12-m9-13-short-sha` |
| Path | B-direct |
| Head SHA | `2441f6f3c679555dc4106ea2e8a422ed407a26a0` |
| Tag | `v0.7.13` (annotated, peel matches published SHA) |
| Remote tag_peel | `2441f6f3c679555dc4106ea2e8a422ed407a26a0` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/apply-checkpoint.json`:
  - `head_sha`: `0012f12` → `0012f1242cef949efc4cbd4c8d419a135ee3cf8a`
  - `remote_tag_peel`: same correction
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/*.md`:
  - All short-SHA references (merge-receipt, release-receipt, release-report, verify-report) expanded
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/apply-checkpoint.json`:
  - `head_sha`: `26848cf` → `26848cf8b26340d3fde99a3a7f398f2873943982`
  - `remote_tag_peel`: same correction
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/*.md`:
  - All short-SHA references expanded
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
  - m9-12 row `Published SHA`: `0012f12` → full
  - m9-13 row `Published SHA`: `26848cf` → full
  - Added m9-15 row
  - `Last updated` updated to 2026-09-12T10:09:00Z
  - `Total cycles`: 30 → 31
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`:
  - `Last archive`: m9-14 → m9-15
  - `Last updated` updated to 2026-09-12T10:09:00Z
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Cross-check #3 tightened: now requires `head_sha == remote_tag_peel`,
    `peel_match == True`, AND `len(remote_tag_peel) == 40`. Closes the
    "vacuously valid short-SHA" gap.

## Runtime status

No production code changed. Doc-only cycle. No T1/T2/T4-smoke gates
required.

## Tag convention

Per chronos convention: tag peeled to the **fix commit**
(`2441f6f3c679555dc4106ea2e8a422ed407a26a0`), keeping runtime
semantics on the tagged SHA.

## Findings

No findings introduced or closed by this cycle that affect runtime.
Drift fixed is a documentation/audit vault format inconsistency.

The `vacuous-peel-match-prior-cycles` placeholder finding introduced
by m9-14 is now superseded by the tightened C3 (which catches the same
class of drift via `len(remote_tag_peel) == 40`).
