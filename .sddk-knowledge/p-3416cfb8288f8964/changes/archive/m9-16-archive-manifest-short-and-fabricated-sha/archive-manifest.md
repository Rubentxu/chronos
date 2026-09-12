# Archive Manifest — m9-16-archive-manifest-short-and-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle | `m9-16-archive-manifest-short-and-fabricated-sha` |
| Archived at | 2026-09-12T10:16:45Z |
| Tag | `v0.7.14` |
| Head SHA | `eb5110dfe1f1d14eab85e6052f1f7cbb86e2f384` |
| Base SHA | `68c528ec34adc1ef5c0e049b9ab83207d710bcb6` |
| Release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/release-receipt.md` |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1-C25: pass
## Evidence bindings

Vault metadata cycles only (or vault + minimal code). Each cycle artifact bound to its SHA-256:

- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/apply-checkpoint.json` → `a4a55cf8f6d9d20b66e508c85a4f23775000ba61d7b3e7707f911253e93f48b4`
- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/merge-receipt.md` → `cb5d2c3f8388c7007506be08f6785d1b06e1272f7f2c118eaf76eab1528288f2`
- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/release-receipt.md` → `41c956a5fc8a0d118459785c319551e7511462a90026638ab83ea05849e11a26`
- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/release-report.md` → `171088818f6c64356935ddf4bd03a3c2910976504f8ece0eb876ab3ddb2535e8`
- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/verify-findings.json` → `86ca3528227d4298507b44e95aaf249371085a10ddc348d7dccbc00681f7685d`
- `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/verify-report.md` → `11be21c63783cf097b021afd6e94e5aa4d2e98d6511956a3e8cd2f2bd8e16384`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-16-archive-manifest-short-and-fabricated-sha/change-entry.md` → `78956d69aa9bad0583cb287df7200e4ae1b02732f576dd068b166b64994c3170`

## Artifact index (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/merge-receipt.md` | `cb5d2c3f8388c7007506be08f6785d1b06e1272f7f2c118eaf76eab1528288f2` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/release-receipt.md` | `41c956a5fc8a0d118459785c319551e7511462a90026638ab83ea05849e11a26` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/release-report.md` | `171088818f6c64356935ddf4bd03a3c2910976504f8ece0eb876ab3ddb2535e8` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/verify-report.md` | `11be21c63783cf097b021afd6e94e5aa4d2e98d6511956a3e8cd2f2bd8e16384` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/apply-checkpoint.json` | `5677df24c1de6ac78ce1026321f13fd73f490dd607577f7a4fedf1e4c8e8df29` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-16-archive-manifest-short-and-fabricated-sha/verify-findings.json` | `35c79bf88df0094ca9fe6deccfa6e622daef2f664b3bed124898b22cb3d742c7` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-16-archive-manifest-short-and-fabricated-sha/change-entry.md` | `78956d69aa9bad0583cb287df7200e4ae1b02732f576dd068b166b64994c3170` |

## Notes

m9-16 closes a follow-up drift left over by m9-14 and m9-15: both
fixed apply-checkpoint.json SHA fields but missed the corresponding
archive-manifest.md files. m9-16 expands three archive-manifests'
Head SHA fields plus cross-references, plus expands two
cycles/index.md Published SHA columns to full 40-char SHAs, and adds
cross-check #9 + extends cross-check #8 to cover archive-manifest
SHA fields.
