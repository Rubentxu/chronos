# Release Receipt: m9-81

> **Cycle**: `p-3416cfb8288f8964/m9-81-counterexample-table-classifier`
> **Tag**: `v0.7.83`
> **Date**: 2026-09-14

## Tag and merge metadata

| Field | Value |
|---|---|
| Tag | `v0.7.83` |
| Tag type | annotated |
| Tag peel (commit) | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Merge commit (--no-ff) | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Peel match | clean (`v0.7.83^{commit}` == merge commit) |
| Branch | `feat/m9-81-counterexample-table-classifier` |
| Cycle head (cycle branch tip) | `93f7cf5ccc621e17ffb059047c7a515e2406f186` |
| Main HEAD (before merge) | `45b53df132186b09de75b543b87cf0bab23bd26e` |
| Main HEAD (after merge) | `fdc5accf64be1fcf780913243aec0496ad48e7fe` |
| Merge type | `--no-ff` (preserves cycle topology) |
| Base SHA | `45b53df132186b09de75b543b87cf0bab23bd26e` |
| Origin push | done — `git push origin main v0.7.83` → main `a4c4dd9`, tag `v0.7.83` clean |
| Head SHA | fdc5accf64be1fcf780913243aec0496ad48e7fe |
## Cycle identity

| Field | Value |
|---|---|
| Cycle record | `p-3416cfb8288f8964/m9-81-counterexample-table-classifier` |
| Remote tag | v0.7.83 |
| Remote tag_peel | fdc5accf64be1fcf780913243aec0496ad48e7fe |
| Peel match | true (tag_peel == merge_commit_sha == HEAD) |
| Path | B-direct |
| Tier required | T1 |
| Tiers run | T0 + T1 |
| Carry-forward closed | FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION |
| New findings | none |

## Verdict

PASS — verified before merge. See `verify-report.md` for the full
CC-traced verdict and the per-REQ scenarios that pass.

## Behavioural baseline

- `cargo test -p chronos-store --lib --no-fail-fast`: 74 / 0 (post-refactor)
- `cargo test -p chronos-store --lib --no-fail-fast`: 74 / 0 (cycle base)
- `cargo test -p chronos-services --lib --no-fail-fast`: 264 / 0 (downstream smoke)
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings
- `cargo fmt --all -- --check`: 0 diffs

## Push state

Tag and branch pending push to origin (next step).
