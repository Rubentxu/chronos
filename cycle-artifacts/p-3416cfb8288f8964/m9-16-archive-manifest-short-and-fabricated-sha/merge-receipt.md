# Merge Receipt — m9-16-archive-manifest-short-and-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-16-archive-manifest-short-and-fabricated-sha` |
| Path | B-direct |
| Branch | `fix/m9-16-archive-manifest-short-and-fabricated-sha` |
| Base SHA | `68c528ec34adc1ef5c0e049b9ab83207d710bcb6` (main @ start of cycle) |
| Head SHA | `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` |
| Tag | `v0.7.14` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` | fix(m9-16): archive-manifest short/fabricated SHAs + add cross-check #9 to vault-drift-sweep |

## What was fixed

Three archive-manifest.md files in
`.sddk-knowledge/p-3416cfb8288f8964/changes/archive/` had Head SHA
fields that were either fabricated (m9-11) or short 7-character SHAs
(m9-12, m9-13). Their corresponding apply-checkpoint.json files were
fixed by m9-14 and m9-15, but the archive-manifests were not touched
in those cycles.

Additionally:
- m9-12 archive-manifest.md cross-referenced m9-11's fabricated SHA
  in its Previous-cycles table.
- m9-13 archive-manifest.md cross-referenced m9-12's short SHA.
- `cycles/index.md` m9-14 + m9-15 Published SHA columns had short SHAs
  (authored this session for format consistency with prior cycles but
  inconsistent with the now-tightened format).

All expanded to full 40-character SHAs:
- m9-11: `cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` → `cd0115fd8f942058cde109c72a975cab7ea7473c`
- m9-12: `0012f12` → `0012f1242cef949efc4cbd4c8d419a135ee3cf8a`
- m9-13: `26848cf` → `26848cf8b26340d3fde99a3a7f398f2873943982`
- m9-14 (cycles/index): `3869906` → `38699061891b76f90ef316914d3ba15d6eb53f83`
- m9-15 (cycles/index): `2441f6f` → `2441f6f3c679555dc4106ea2e8a422ed407a26a0`

## Procedure extension

Adds **cross-check #9** to `vault-drift-sweep.md`: archive-manifest.md
Head SHA + cross-references must be 40 characters. C3 covers
apply-checkpoint.json; C9 covers archive-manifest.md. Both are needed
because they can drift independently.

## Why this matters

m9-14 and m9-15 each fixed an apply-checkpoint.json drift but missed
the corresponding archive-manifest.md in the same cycle. C3 (now
strict with `len(remote_tag_peel) == 40`) catches apply-checkpoint
drifts but not archive-manifest drifts. C9 closes this gap.
