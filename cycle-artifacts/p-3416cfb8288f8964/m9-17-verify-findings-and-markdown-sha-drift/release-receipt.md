# Release Receipt — m9-17-verify-findings-and-markdown-sha-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-17-verify-findings-and-markdown-sha-drift` |
| Path | B-direct |
| Head SHA | `134dc7525312275c447db8f5996740ff7c102a02` |
| Tag | `v0.7.15` (annotated, peel matches published SHA) |
| Remote tag_peel | `134dc7525312275c447db8f5996740ff7c102a02` |
| peel_match | true |
| Status | RELEASED |

## Release contents

**verify-findings.json subject_sha corrections:**
- `cycle-artifacts/p-3416cfb8288f8964/m9-11-cycles-index-metadata-drift/verify-findings.json`: `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → `cd0115fd8f942058cde109c72a975cab7ea7473c`
- `cycle-artifacts/p-3416cfb8288f8964/m9-12-terms-index-metadata-drift/verify-findings.json`: `0012f12` → `0012f1242cef949efc4cbd4c8d419a135ee3cf8a`
- `cycle-artifacts/p-3416cfb8288f8964/m9-13-change-entry-base-sha-drift/verify-findings.json`: `26848cf` → `26848cf8b26340d3fde99a3a7f398f2873943982`

**Cycle markdown Head SHA corrections:**
- m9-11 release-receipt.md, merge-receipt.md, release-report.md, verify-report.md: `cd0115f` → full
- m9-14 merge-receipt.md, release-receipt.md: `3869906` → full (m9-14 self-drift)

**change-entry.md Head SHA corrections:**
- m9-11/12/13 change-entry.md Head SHA field + fix-commit table row + "Tag peeled to fix commit" narrative expanded to full SHAs

**Index updates:**
- `cycles/index.md`: added m9-17 row, Total cycles 32 → 33, Last updated → 2026-09-12T10:29:00Z
- `terms/index.md`: Last archive m9-16 → m9-17

**Procedure extension:**
- `vault-drift-sweep.md`: added cross-check #10 (cycle artifact SHA fields must be 40-char + match apply-checkpoint)
- Updated "When to escalate" to cover check 10
- Updated "Reference" list with m9-17

## Runtime status

No production code changed. Doc-only cycle.

## Tag convention

Per chronos convention: tag peeled to the **fix commit** (`134dc75`).

## Findings

No findings introduced or closed. This is a hygiene cycle.

The `schema-v1-verify-findings` accepted-by-design drift (m9-03, m9-04)
is documented as `findings_remaining_m9_plus` for future dedicated
cycle handling.
