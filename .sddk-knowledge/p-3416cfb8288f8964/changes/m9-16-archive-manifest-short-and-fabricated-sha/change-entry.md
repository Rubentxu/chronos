# Change: m9-16 archive manifest short and fabricated sha

## Summary

Drift closure cycle for this milestone.


| Campo | Valor |
|---|---|
| Cycle ID | `m9-16-archive-manifest-short-and-fabricated-sha` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `68c528ec34adc1ef5c0e049b9ab83207d710bcb6` (main HEAD before cycle) |
| Head SHA | `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` (fix commit on main) |
| Tag | `v0.7.14` |
| Published | 2026-09-12T10:16:30Z |

## What changed

m9-14 (v0.7.12) fixed m9-11 apply-checkpoint.json fabricated SHA but
missed the corresponding archive-manifest.md (which still had the
same fabricated value).

m9-15 (v0.7.13) fixed m9-12 + m9-13 apply-checkpoint.json short SHAs
but missed their archive-manifest.md files. Plus m9-12 archive-manifest
cross-referenced m9-11's fabricated SHA in its Previous-cycles table.

m9-16 expands Head SHA in m9-11, m9-12, m9-13 archive-manifest.md to
full 40-char SHAs, expands cross-references in m9-12 + m9-13
Previous-cycles tables, and expands cycles/index.md m9-14 + m9-15
Published SHA to full 40-char (they were authored short in this
session for format consistency with prior cycles, but inconsistent
with the now-tightened format).

Adds cross-check #9 to vault-drift-sweep.md: archive-manifest.md
Head SHA + cross-references must be 40-char. C3 covers
apply-checkpoint.json; C9 covers archive-manifest.md. Both are needed
because they can drift independently.

Also extends cross-check #8 to cover archive-manifest.md Head SHA
fields via `git cat-file -e` (closes the gap that C9 does not catch
40-char fabrications).

## Files touched

- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-11-cycles-index-metadata-drift/archive-manifest.md` (Head SHA)
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-12-terms-index-metadata-drift/archive-manifest.md` (Head SHA + cross-ref to m9-11)
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-13-change-entry-base-sha-drift/archive-manifest.md` (Head SHA + cross-ref to m9-12)
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (m9-14, m9-15 Published SHA expansion + add m9-16 row + bump counters)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (Last archive → m9-16)
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md` (cross-check #9 + extend C8 for archive-manifest Head SHA)
