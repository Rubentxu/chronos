# Archive Manifest — m9-14-m9-11-fabricated-sha

| Campo | Valor |
|---|---|
| Cycle ID | `m9-14-m9-11-fabricated-sha` |
| Archived at | 2026-09-12T10:06:30Z |
| Tag | `v0.7.12` |
| Head SHA | `38699061891b76f90ef316914d3ba15d6eb53f83` |
| Release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/release-receipt.md` |

## Artifact index (SHA-256)

| File | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/merge-receipt.md` | `3f1053d48ebe62c8aad7e423951c1cc9e7852d967b2057dcfe0cc48e0ba91790` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/release-receipt.md` | `437eb492b0bba22ae5f64947c5da046d387c2e25e8890db210540275be8cd677` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/release-report.md` | `bc31226fd76fc99d6ff3ea8dde66189d7bb12e3ad3bcdb02b9cee406ed7c9df7` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/verify-report.md` | `ef359ce793ec07c20535d6234b5a729f256caf7adcca5331b4be886e07b8a91a` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/apply-checkpoint.json` | `43007a153ff193ed25ea349fd529074aee8d01eacb6c10e47683fb18a5f38fc7` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-14-m9-11-fabricated-sha/verify-findings.json` | `0bbd560707e10587dd065052ed9f678499cabd96c81fc1984d4af5b705b530fd` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-14-m9-11-fabricated-sha/change-entry.md` | `3921103baf032f9bfb8f6129fecd09660a3f038c69b9f5851d9e03799eca325d` |

## Notes

m9-14 closes a fabrication error in m9-11 (head_sha / remote_tag_peel
were a fabricated 40-char SHA). Also fixes cycles/index.md m9-11 row
(same fabricated SHA). Adds cross-check #8 to vault-drift-sweep.md
to prevent recurrence.

Two adjacent drift classes discovered while writing C8, deferred to
dedicated cycles:
- C2: 14 findings_closed / no_action IDs never added to terms/index.md
  Terminated section
- C3: m9-12 and m9-13 stored short 7-char SHAs in remote_tag_peel
