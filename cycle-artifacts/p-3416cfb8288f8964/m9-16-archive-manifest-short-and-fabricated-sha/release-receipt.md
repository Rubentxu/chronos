# Release Receipt — m9-16-archive-manifest-short-and-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-16-archive-manifest-short-and-fabricated-sha` |
| Path | B-direct |
| Head SHA | `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` |
| Tag | `v0.7.14` (annotated, peel matches published SHA) |
| Remote tag_peel | `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` |
| peel_match | true |
| Status | RELEASED |

## Release contents

- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-11-cycles-index-metadata-drift/archive-manifest.md`:
  - Head SHA: `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → `cd0115fd8f942058cde109c72a975cab7ea7473c`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-12-terms-index-metadata-drift/archive-manifest.md`:
  - Head SHA: `0012f12` → `0012f1242cef949efc4cbd4c8d419a135ee3cf8a`
  - Previous-cycles cross-ref to m9-11: same fabrication fix
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-13-change-entry-base-sha-drift/archive-manifest.md`:
  - Head SHA: `26848cf` → `26848cf8b26340d3fde99a3a7f398f2873943982`
  - Previous-cycles cross-ref to m9-12: same short-SHA fix
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`:
  - m9-14 Published SHA: `3869906` → full
  - m9-15 Published SHA: `2441f6f` → full
  - Added m9-16 row
  - `Last updated` updated to 2026-09-12T10:15:00Z
  - `Total cycles`: 31 → 32
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`:
  - `Last archive`: m9-15 → m9-16
  - `Last updated` updated to 2026-09-12T10:15:00Z
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`:
  - Added cross-check #9 (archive-manifest.md Head SHA + cross-references)
  - Added m9-15 + m9-16 to the procedure reference list
  - Updated "When to escalate" wording to cover check 9

## Runtime status

No production code changed. Doc-only cycle.

## Tag convention

Per chronos convention: tag peeled to the **fix commit**
(`eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384`).

## Findings

No findings introduced or closed. This is a hygiene cycle.
