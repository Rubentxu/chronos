# Change: m9-35 release-report title format normalize

## Summary

Normalize release-report.md title format across 9 cycles (m9-19..m9-27) from
`# m9-NN: Release Report` to canonical `# Release Report — m9-NN` format.

m9-28+ already used the canonical format. This cycle brings all 33 m9
cycles into alignment with the post-m9-28 convention.

## Cross-check added

- **C27**: release-report.md title format must be `# Release Report — m9-NN`.

## Files changed

- 9 release-report.md files (m9-19..m9-27): title format normalize
- 1 vault-drift-sweep.md: added cross-check #27
- 6 new cycle artifacts for m9-35

## Subject

- base_sha: `dbea58e61921eecb4b1b90f6f94dad21fea2e440`
- head_sha: `a0b0d560365315b22012325e9d416f3ef1998faa`
- cycle: m9-35
- branch: `fix/m9-35-release-report-title-format-normalize`
- date: 2026-09-12
- tag: `v0.7.33`
