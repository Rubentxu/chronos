# rec-c3.3-train-b — cycle skeleton

Cycle opened 2026-09-19T10:45Z.

## What

REC-C3.3.3 (Tren B): Expand the chronos MCP tool surface so AI agents
can drive the native probe to investigate a target process: capture,
query, extract, advance, and step.

## Why

Tren A (REC-C3.3.2) closed with `capture_session` finalized. The
remaining Tren B surface lets agents run anything beyond a single
capture: query captured probes, extract specific events, advance the
target, and single-step.

## Locked rules (operator 2026-09-18)

- **B1**: Reuse existing service-port abstractions; no `NativeProbeServicePort`.
- **B2**: No `dyn Any`, downcast, store helper, or `chronos_store` DTO crossing the port. `SessionRepository` stays lifecycle/registry only; Tren B introduces `SessionArchive` + `CounterexampleRepository` + moves `SessionMetadata` to `chronos-domain`.
- **B3**: `store->native` direction belongs to REC-C3.4, NOT Tren B.
- **B4**: Composition root in `chronos-mcp::composition`; bootstrap-scoped vs session-scoped factory distinction mandatory.
- **B5**: No `#[ignore]` and no `--skip` ever.
- **B6**: REC-C1.5.2 strict replay semantics stay untouched.
- **B7**: No squash, no rebase, no merge commit; full SHA chain preserved.
- **B8**: Findings `no_action_in_current_cycle`, not permanent waivers.
- **B9**: Honesty over coverage.

## Active gate

`reconstruction-contracts.toml::active_gate = REC-C3`.

## Branch / SHA

- Branch: `feat/rec-c3.3-train-b` off `fa5eb582`.
- Initial commit: opening artifact only; first slice commit lands after `sddk-explore` + `sddk-propose` + `sddk-spec` + `sddk-tasks` finish.

## Cycle phase plan (A-min path)

1. sddk-explore (deepseek/deepseek-chat)
2. sddk-propose (MiniMax-M3)
3. sddk-spec (MiniMax-M3)
4. sddk-tasks (MiniMax-M3)
5. sddk-apply (MiniMax-M2.7)
6. sddk-verify (MiniMax-M3)
7. sddk-debt-verify (Z.ai GLM-5)
8. sddk-release (MiniMax-M2.7)
9. sddk-archive (MiniMax-M2.7)

## Smoke subset

`e2e_connectivity` + `analytics_tools` + `program_scenarios` + `native_probe_tools`
