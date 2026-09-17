# REC-C2.3 — Tasks

## Pre-conditions

- `legacy-evb-inventory.json` shows 21 COMPATIBILITY uses (20 `event_bus_type`,
  1 `read_since`).
- `scripts/check_legacy_evb.py --strict` PASSES with CANONICAL=0.
- Cycle base: `f02ab311` (REC-C2.2 release).

## Commits

| # | Subject | Touches | Tied invariants |
|---|---|---|---|
| C2.3.0 | `services`: drop EventBus from probe constructors | `chronos-services/src/probe.rs`, `chronos-mcp/src/server.rs` | canonical authority stops constructing the mirror |
| C2.3.1 | `native`: drop `event_bus` field, signature, default helper | `chronos-native/src/probe_backend.rs` | native's only job is `accept_raw` |
| C2.3.2 | `domain`: drop `ProbeBackend::read_since`; delete `bus.rs` + re-exports | `chronos-domain/src/adapter.rs`, `chronos-domain/src/lib.rs`, `chronos-domain/src/bus.rs` | trait stops claiming a non-destructive cursor read; module gone |
| C2.3.3 | `browser` + `ebpf`: drop `read_since` impls + dead `CursorDto` | `chronos-browser/src/adapter.rs`, `chronos-ebpf/src/lib.rs`, `chronos-mcp/src/server.rs` | every adapter agrees the trait no longer has the method |
| C2.3.4 | `sandbox`: drop `bus_capacity` from wire + tests | `chronos-sandbox/src/client/types.rs`, `chronos-sandbox/src/client/tools.rs`, sandbox tests | wire stops pretending the ring is a knob |
| C2.3.5 | ratchet: regenerate inventory, run all gates | `legacy-evb-inventory.json`, full T3 + T4-smoke | tracked=0, --strict green |

## Why this ordering

- C2.3.0 must come before C2.3.1: services is the only caller that mints a
  bus today; once it stops, C2.3.1 can safely remove the field. Both compile
  together.
- C2.3.2 follows: by then nothing references the bus module, the trait can
  shrink, and `bus.rs` has no remaining caller.
- C2.3.3 follows: dropping `read_since` from the trait means every impl
  must drop it too. Doing them together avoids a "missing trait method"
  window.
- C2.3.4 is the wire cleanup.
- C2.3.5 is the ratchet proof, not a feature commit.

## Acceptance per commit

| Commit | Must compile | Must test | Comment |
|---|---|---|---|
| C2.3.0 | yes | `cargo test -p chronos-services --lib` | no EventBus in `ProbeService::*` |
| C2.3.1 | yes | `cargo test -p chronos-native --lib --test-threads=1` | no `event_bus` field, `new()` no args |
| C2.3.2 | yes | `cargo test -p chronos-domain --lib` | `bus.rs` removed; `read_since` gone from `ProbeBackend` |
| C2.3.3 | yes | `cargo test --workspace --lib --exclude chronos-sandbox --exclude chronos-e2e` | all impls updated; `CursorDto` deleted |
| C2.3.4 | yes | 3 sandbox suites (`e2e_connectivity`, `program_scenarios`, `probe_drain_tools`) | `bus_capacity` removed from wire |
| C2.3.5 | yes | T0+T3+T4-smoke | `tracked=0`, `--strict` green |

## Risks

- Wire change in C2.3.4: clients that set `bus_capacity` will see it silently
  ignored (or rejected, depending on serde mode). Documented in proposal.
- Test rewrites in C2.3.0/1: tests that previously took `EventBus` for
  setup now use the canonical seam. No semantic change.

## Out-of-cycle

- `BrowserAdapter::take_semantic_events` is still destructive; this is
  FIND-C2.2-04, deferred to `rec-c2.4` (browser persistence).
- CC#18/CC#39 — gov-cycle follow-ups, unchanged.
