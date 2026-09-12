# Archive Manifest — m9-33

| Field | Value |
|---|---|
| Cycle | m9-33-change-entry-title-format-normalize |
| Head SHA | `837bc5c44a501e4c2253cc6fd257cc578bebf8e0` |
| Base SHA | `5bfcfedaf9687050a8090a31f999738ff082a081` |
| Tag | `v0.7.31` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Normalizes change-entry.md title format to
`# Change: m9-NN <human-readable>` across 18 cycles (m9-01..m9-18).
Adds cross-check #25 enforcing this format.

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/apply-checkpoint.json` | `6a6de5ac86764fd9da130cbeeb53efa107c10720d761b15db74a667409b4961a` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/merge-receipt.md` | `e8b3e423b05ee1992971616c33b75467910d739bff134397de38217f466b45b1` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/release-receipt.md` | `9ece2bc72e206a14e34547531b6b3a36f5daa1d66acc9dac453df8034c524612` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/release-report.md` | `85dbb7e73cfad68f92540773df87462217de4a2028249979c7713d4c317e1d48` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/verify-findings.json` | `c0bffeff245bddd0bc0c62a859649609b6ec1fb80270b7102a43097aeb17e2bc` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-33-change-entry-title-format-normalize/verify-report.md` | `3e920efb934aebd3836a576a7c7dbc72b662b4fc4655b24eb0cd3b19dbff1e5c` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-33-change-entry-title-format-normalize/change-entry.md` | `21f9e2c59f67137b842cefc6977ccc5e255692c99a3990f060c542365b920592` |

## Evidence bindings

Vault metadata only. Each cycle artifact listed with its computed
SHA-256 in the Artifact index above.

## Drift summary

18 change-entry.md files (m9-01..m9-18) used `# Change Entry — <slug>`
format. m9-33 normalizes all to canonical `# Change: m9-NN <human-readable>`.

## Cross-checks added

- #25 (`vault-drift-sweep.md`): change-entry.md title format must be
  `# Change: m9-NN <human-readable>`.
