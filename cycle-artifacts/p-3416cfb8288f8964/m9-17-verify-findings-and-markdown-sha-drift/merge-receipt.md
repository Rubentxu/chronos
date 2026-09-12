# Merge Receipt — m9-17-verify-findings-and-markdown-sha-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-17-verify-findings-and-markdown-sha-drift` |
| Path | B-direct |
| Branch | `fix/m9-17-verify-findings-and-markdown-sha-drift` |
| Base SHA | `0ed8f874f7b6f973505fc3997475fc398fcfa4cc` (main @ start of cycle) |
| Head SHA | `134dc7525312275c447db8f5996740ff7c102a02` |
| Tag | `v0.7.15` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `134dc7525312275c447db8f5996740ff7c102a02` | fix(m9-17): verify-findings + markdown SHA drift across 4 prior cycles + add cross-check #10 |

## What was fixed

m9-14, m9-15, and m9-16 each fixed one file type's SHA drift
(apply-checkpoint.json, archive-manifest.md, ...) but missed other
file types in the same cycle. m9-17 closes the residual drift across
4 file types × 4 prior cycles:

**verify-findings.json:**
- m9-11: fabricated `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → real `cd0115fd8f942058cde109c72a975cab7ea7473c`
- m9-12: short `0012f12` → full `0012f1242cef949efc4cbd4c8d419a135ee3cf8a`
- m9-13: short `26848cf` → full `26848cf8b26340d3fde99a3a7f398f2873943982`
- m9-03, m9-04: schema-v1 (intentional, accepted-by-design)

**cycle markdown files:**
- m9-11 release-receipt.md, merge-receipt.md, release-report.md, verify-report.md: short `cd0115f` → full
- m9-14 merge-receipt.md, release-receipt.md: short `3869906` → full (m9-14 self-drift)
- m9-11/12/13 change-entry.md: Head SHA field + fix-commit table row + "Tag peeled to fix commit" narrative

## Procedure extension

Adds **cross-check #10** to `vault-drift-sweep.md`:
- verify-findings.json `subject_sha` (or schema-v1 `subject.head`)
- release-receipt.md `Head SHA` + `Remote tag_peel`
- merge-receipt.md `Head SHA`
- change-entry.md `Head SHA`

All must be 40 characters. SHA values (where present) must match
apply-checkpoint `head_sha` or `remote_tag_peel`.

## Why this matters

The SHA-drift pattern is **pervasive**: each cycle that fixes one
file type's SHA misses 3-4 other file types that store the same SHA.
This is the third drift-class follow-up (after m9-16 archive-manifest
and m9-14 apply-checkpoint). C10 closes the gap by enumerating all
file types where SHA fields can drift independently.
