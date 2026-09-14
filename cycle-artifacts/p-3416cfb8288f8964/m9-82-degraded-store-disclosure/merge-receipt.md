# Merge Receipt: m9-82

> **Cycle**: `p-3416cfb8288f8964/m9-82-degraded-store-disclosure`
> **Merge SHA**: `b8694eff737293bffea4ba62f07e4b206eafc502`
> **Date**: 2026-09-14

## Merge details

| Field | Value |
|---|---|
| Branch | feat/m9-82-degraded-store-disclosure |
| Date | 2026-09-14 |
| Base SHA | a0f72c2a7fe36eaeb9c772505dfe563f85f42773 |
| Head SHA | b8694eff737293bffea4ba62f07e4b206eafc502 |
| Source branch | `feat/m9-82-degraded-store-disclosure` |
| Target branch | `main` |
| Merge strategy | `--no-ff` (preserves cycle topology; merge commit required for peel match) |
| Merge commit author | Chronos Maintainer <maintainer@chronos-rs.local> |
| Cycle head (branch tip) | `7025f06` (5 commits: T0 vault + T1 store + T2 mcp + fmt + verify artifacts) |
| Main HEAD (before merge) | `a0f72c2a7fe36eaeb9c772505dfe563f85f42773` |
| Main HEAD (after merge) | `b8694eff737293bffea4ba62f07e4b206eafc502` |
| Files merged | 12 (2 source files + 4 vault docs + 4 cycle-artifacts + 1 spec.md correction + 1 proposal.md correction) |
| Lines | 1338 inserted, 13 deleted |
## Topology preservation

```
*   b8694ef Merge branch 'feat/m9-82-degraded-store-disclosure' into main
|\
| * 7025f06 m9-82: verify-phase cycle-artifacts (apply-checkpoint + receipts)
| * 4e80517 style: cargo fmt pass on m9-82 tool envelope wrapper and helper
| * 477ee83 feat(mcp): surface degraded store mode in session-persistence tool envelopes
| * 77cd0ab feat(store): expose SessionStore kind so MCP can disclose degraded store
| * 5687346 m9-82: vault (exploration-report + proposal + spec + tasks)
|/
* a0f72c2 docs(handoff): append m9-81 closure (session 2026-09-14T09:07Z-09:42Z)
```

The merge commit makes the cycle's topology recoverable via
`git log --graph` even after the branch is deleted.

## No-conflict attestation

The merge produced no conflict markers. The two source files
(`crates/chronos-store/src/storage.rs` and
`crates/chronos-mcp/src/server.rs`) had no overlapping changes with
main since the cycle branched from `a0f72c2`. The vault files
(`spec.md`, `proposal.md`) were also unchanged on main since T0.
