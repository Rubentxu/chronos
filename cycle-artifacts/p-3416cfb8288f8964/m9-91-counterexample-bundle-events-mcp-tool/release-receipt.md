# Release Receipt — m9-91-counterexample-bundle-events-mcp-tool

## Identification

| Field | Value |
|---|---|
| Cycle | m9-91-counterexample-bundle-events-mcp-tool |
| Path | A-min |
| Branch | feat/m9-91-counterexample-bundle-events-mcp-tool |
| Date | 2026-09-14 |
| Head SHA | c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d |
| Cycle HEAD (pre-merge) | 18e2a28d0fc1ade564d1c884d9c7228f5864a6f0 |
| Merge commit | 40a7ba23082c11f5dc2ec6a25a9a8d12cb9a4cfa |
| Cycle-artifacts commit | fdcb0dc4d9260d387dd8e7ee2eab4b022d10118c |
| Main SHA (post-cycle-artifacts, tag-peel) | c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467 |
| Final fixpoint HEAD (after SHA cascade) | TBD |
| Remote tag | v0.7.93 |
| Remote tag_peel | c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467 |
| Peel match | TBD (filled post-cascade) — initial peel = cycle-artifacts commit fdcb0dc |

## Release notes

- Tag `v0.7.93` moved to the post-cascade HEAD `c7cdaf54872a9b37bd4a70b8f1fbf497b80d4467`.
- Initial peel was at the cycle-artifacts commit `fdcb0dc4d9260d387dd8e7ee2eab4b022d10118c` (pre-cascade).
- New MCP tool `counterexample_bundle_events` exposed at the wire.
  Closes m9-02-R4 — long-deferred since m9-04 documented the
  deferred-tool policy.
- New service method `ChronosCounterexampleService::events` exposed
  for callers outside the MCP.
- New `CounterexampleOutput::Events` variant in the service-layer
  enum.
- New wire DTO `CounterexampleBundleEventsOutputDto` published under
  `chronos_services::output`.
- 3 new unit tests pin the spec scenarios (full-stream read,
  paginated read across 4 pages, missing-bundle LoadFailed).
- Public surface added: tool (1), DTO (1), service method (1),
  variant (1). No public API removed.

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p chronos-services --lib`: 267/267 (was 264 pre, +3 new).
- `cargo test --workspace --lib -- --test-threads=1`: all suites pass.
- `cargo build --workspace`: success, no warnings.
- `cargo build --bin chronos-mcp`: success, no warnings.
- T4 sandbox smoke (counterexample_tools + e2e_connectivity +
  analytics_tools): 17/17 passed.
- `bash scripts/check_vault_drift.sh`: TBD (run post-cascade).

## Peel location rationale

Tag `v0.7.93` will be placed at the cycle-artifacts commit (TBD SHA),
following the **CC#42 fixpoint-cascade workaround** documented in
m9-83 handoff:

- The cascade commits (regen-manifest-shas, bump-Head-SHAs) change
  files inside the vault but introduce no source code changes.
- Tagging the cascade commits would mean the tag points at a SHA
  whose only effect is metadata bookkeeping.
- Tagging the cycle-artifacts commit (the last "real" commit before
  the metadata cascade) gives the tag a stable, content-bearing
  anchor.

The cascade commits update `apply-checkpoint.head_sha` and
`cycles/index.md` Total cycles to point at the final fixpoint HEAD.

## Carry-forward

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK remains external-deferred
  (carried from m9-88).
- The `cc-001-god-module` debt remains open (separate from m9-91);
  not addressed by this cycle (out of scope).
