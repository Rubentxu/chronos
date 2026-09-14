# Release Receipt: m9-82

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Tag**: `v0.7.84`
> **Date**: 2026-09-14

## Tag and merge metadata

| Field | Value |
|---|---|
| Tag | `v0.7.84` |
| Tag type | annotated |
| Tag peel (commit) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Merge commit (--no-ff) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Peel match | clean (`v0.7.84^{commit}` == merge commit) |
| Branch | `feat/m9-82-degraded-store-disclosure` |
| Cycle head (cycle branch tip) | `7025f06` |
| Main HEAD (before merge) | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` |
| Main HEAD (after merge) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Merge type | `--no-ff` (preserves cycle topology) |
| Base SHA | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` |
| Origin push | pending (next step) |

## Cycle identity

| Field | Value |
|---|---|
| Cycle record | `p-3416cfb8288f8964/m9-82-degraded-store-disclosure` |
| Remote tag | v0.7.84 |
| Remote tag_peel | b8694eff737293bffea4ba62f07e4b206eafc502 |
| Peel match | true (tag_peel == merge_commit_sha == HEAD) |
| Path | A-min |
| Tier required | T2 |
| Tiers run | T0 + T2 + focused T3 + T4-smoke |
| Carry-forward closed | FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE |
| New findings | none |

## Verdict

PASS — verified before merge. See `verify-report.md` for the full
CC-traced verdict and the per-REQ scenarios that pass.

## Behavioural baseline

- `cargo test -p chronos-store --lib --no-fail-fast`: 77 / 0 (post-cycle, +3 new is_persistent tests)
- `cargo test -p chronos-store --lib --no-fail-fast`: 74 / 0 (cycle base `a0f72c2`)
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 87 / 0 (post-cycle, +5 new degraded envelope tests)
- `cargo test -p chronos-mcp --lib --no-fail-fast`: 82 / 0 (cycle base)
- `cargo test -p chronos-services --lib --no-fail-fast`: 264 / 0 (unchanged)
- `cargo test -p chronos-store -p chronos-mcp -p chronos-services --tests --no-fail-fast`: 477 / 0 / 0 across 10 binaries (focused T3)
- T4-smoke `chronos-sandbox` subset (e2e_connectivity 1 + session_persistence 4 + session_lifecycle 8): 13 / 0
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings
- `cargo fmt --all -- --check`: 0 diffs

## Push state

Tag and merge pending push to origin (next step).
