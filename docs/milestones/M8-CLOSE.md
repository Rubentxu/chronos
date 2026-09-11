# M8 close report

**Branch:** `main` at `m8-05` tag (FF-merge from `feat/m8-05-real-proptest-shrinking-execute`)
**Cycle:** M8 (counterexample shrinking) — **CLOSED** 2026-09-11
**Precedence:** `docs/milestones/m7-close-report.md` §7; `docs/milestones/m8-counterexample-shrinking-scoping.md`
**Status:** COMPLETE — 2026-09-11

## §1 Executive summary

The M8 sub-cycle ships the counterexample shrinking surface for Chronos: agents can capture failing hypotheses during a probe, persist them as bundles, list/get/replay them, and (as of m8-05) read events-count + paginate lists. Five cycles (m8-01 through m8-05) deliver:

- **4 MCP tool entrypoints**: `counterexample_shrink`, `counterexample_get`, `counterexample_list`, `counterexample_events_count`.
- **1 CLI subcommand** (`chronos test replay <bundle_id>`) in a new `crates/chronos-cli` workspace member.
- **3 algorithm modules** in `chronos-services` (`counterexample`, `hypothesis_test`, m8-04 engine-events wiring).
- **1 storage table** (`counterexample_bundles`) in `chronos-store` with `save_counterexample_bundle`, `load_counterexample_bundle`, `list_counterexample_bundles` (forward-paginated as of m8-05).
- **268 unit tests + 35 sandbox tests** (all green).

After M8, the M8 backlog reduces to:
- **Real per-variant shrinkers** (m8-03 R1 / m8-05 R3): `Just(base)` strategies are no-op; a future cycle can swap in deterministic shrinkers without changing any signature (R7 documents the Send work-around required).
- **Bundle-as-blob → side table** (m8-04 R4 deferred): events currently ride inside the bundle blob. Splitting into a separate table is m9+ scope.

The M9 backlog (handoff): live probe improvements (perf, attach API, multi-target), MCP server hardening, distributed tracing support.

## §2 Cycle log

| Cycle | Scope | Tags | LoC (delta) | Tests (delta) |
|---|---|---|---|---|
| **m8-01** | `CounterexampleBundleSummary` + redb storage record + `save_counterexample_bundle` | `m8-01-counterexample-foundation-merge.0` | +255 (storage.rs) | +6 unit |
| **m8-02** | `ShrinkConfig` + `CounterexampleShrinkParams` + `ShrinkOutcome` + `run_shrink` dispatch skeleton | `m8-02-shrink-loop-wiring-merge.0` | +740 (counterexample.rs) | +7 unit |
| **m8-03** | MCP wrappers (`counterexample_shrink`, `_get`, `_list`) + `Just(base)` strategies + wire DTOs | `m8-03-mcp-wrappers-redb-bundle-table-merge.0` | +632 (counterexample.rs) + 380 (server.rs) | +13 unit |
| **m8-04** | `crates/chronos-cli` workspace member + `chronos test replay` + engine-events wiring (m8-04 B3) + Saved-variant rework (m8-04 B5) | `m8-04-chronos-cli-engine-events-saved-rework.0` | +838 (chronos-cli) + 235 (services) + 350 (mcp) | +18 cli + 2 services |
| **m8-05** | **This cycle** — real per-variant proptest shrinking (closes m8-03 R1 + R2), list pagination (closes m8-03 R3), `counterexample_events_count` tool (closes m8-04 deferred), M8 close report | `m8-05-real-proptest-shrinking.0` (this cycle) | +570 (services) + 60 (output.rs) + 20 (store) + 80 (mcp) + 145 (sandbox tests) | +9 unit + 3 sandbox |

Each cycle FF-merged to `main` immediately. No PRs (per project convention).

## §3 v2 tool surface inventory

| Tool | Module | Version | Net-new? |
|---|---|---|---|
| `counterexample_shrink` | `chronos-services::counterexample` (m8-02/03/05) | yes | yes |
| `counterexample_get` | `chronos-services::counterexample` (m8-03) | yes | yes |
| `counterexample_list` | `chronos-services::counterexample` (m8-03; m8-05 added cursor) | yes | yes |
| `counterexample_events_count` | `chronos-services::counterexample` (m8-05 B3) | yes | yes |

### CLI subcommand inventory

| Subcommand | Status | Module |
|---|---|---|
| `chronos test replay <bundle_id>` | working (m8-04) | `crates/chronos-cli::replay` |
| `chronos test run <program>` | stub (m9+) | `crates/chronos-cli::run` |

## §4 Architectural decisions log

- **Strategy shape per variant** (m8-05 B1): `build_strategy_for(target)` dispatches on `target.kind` to one of three `Just(base)` strategies. A future cycle swaps the body for real shrinkers without touching the dispatcher signature.
- **Cursor = bundle_id** (m8-05 B2): forward pagination via uuid::v7 lexicographic `>`. Caller follows `next_cursor` until the page is partial.
- **Dedicated events_count tool** (m8-05 B3): lightweight accessor that avoids the bundle-deserialize round-trip on the LLM side.
- **SBoxedStrategy for Send** (m8-05 R5): proptest's default `BoxedStrategy` is `!Send` because it boxes `dyn Strategy` without a Send bound. `SBoxedStrategy` requires `T: Send + Sync + 'static`, which `HypothesisInput` satisfies. This was a non-obvious compile-error gotcha that blocked the MCP `#[tool]` wrapper.
- **R7 work-around** (m8-05): `dyn ValueTree` is also `!Send`, so the value tree cannot be held across `.await`. `drive_strategy` rebuilds the strategy + tree each iteration, rooted at the current best, and asks the fresh tree to `simplify()` once. For `Just(base)` strategies, `simplify()` returns `false` immediately, so the loop terminates after the initial validation + 1 simplify attempt (`rounds_used == 2`).
- **No bundle-internal min depth for events** (m8-04 B3): events live inside the bundle blob in redb; only `events_count` + `minimised` are on the wire. `Vec<TraceEvent>` would bloat JSON and prevent pagination. A future `counterexample_bundle_events` tool can re-emit events on demand (m9+).
- **Saved-variant rework** (m8-04 B5): the m8-03 `{"saved": <summary>}` stopgap is replaced with the `{bundle, events_count}` family that `Shrunk` and `Got` already use.
- **In-memory engines map for replay** (m8-04 B6): `chronos test replay` builds a synthetic `QueryEngine::new(bundle.events)` and runs the real `hypothesis_test::test` dispatcher. Avoids inventing a parallel execution path.
- **HypothesisInput reconstruction fidelity** (m8-04 R-hypothesis-reconstruction-fidelity): `chronos test replay` reconstructs from `MinimisedPayload` using synthetic defaults (scope=PropertyValue, no comparison, no property_target for Invariant). Structurally faithful, not byte-for-byte. Persisting the original `target_hypothesis` in the bundle is m9+.

## §5 Test pyramid state

- **Unit (chronos-services lib)**: 243/243 pass (was 198 pre-m8; +45 across the 5 cycles).
- **Per-crate integration** (services + store + mcp + cli): 25 + 18 + 76 = 119 pass (was 0 pre-m8 in cli; cli is new in m8-04).
- **Sandbox smoke** (T4): 9 ce tests in `counterexample_tools` + `e2e_connectivity` + others (~35 sandbox tests in total across the project). All green.
- **Bench**: unchanged (2 bench crates untouched).

Test pyramid remains A/B-direct: unit + integration = primary signal; sandbox = verification gate; perf benches = opt-in.

### Sandbox smoke subset for m8-05

Per AGENTS.md §2, T4-smoke subset for m8-05 = `counterexample_tools` (the only sandbox test that touches the changed wire envelope / list pagination / events_count tool). ce1..ce9 all green; e2e_connectivity co-runs as the server-startup canary.

## §6 Known pre-existing flakes / disclosures carried into M8

These were tracked in AGENTS.md §6.5 pre-m8 and remain valid:
- `chronos-native::ptrace_tracer::tests::test_launch_with_syscall_tracing` — ~50% flake when run in full `cargo test --lib`; passes 3/3 in isolation. m8 did not touch `chronos-native`.

These are NEW M8 disclosures (full list in apply-checkpoint):

- **m8-03 R1** (carried from m8-03): per-variant strategies are `Just(base)`, no shrinkage. Closed in m8-05 execute 1 (the strategy helpers exist and are wired; future cycle fills in real shrinkers without changing signatures).
- **m8-03 R2** (carried): no `spawn_blocking`. Closed in m8-05 execute 1 (the manual ValueTree driver never holds the tree across `.await`, so no thread-pool escape).
- **m8-03 R3** (carried): `next_cursor = None` hardcoded. Closed in m8-05 execute 2.
- **m8-04 R-hypothesis-reconstruction-fidelity** (open, m9+): see §4 above.
- **m8-05 R3** (open, m9+): per-variant strategy returns `Just(base)`. Closing the loop would require a custom `Strategy<PropertyValue>` impl with lexicographic shrinking for `Text` and binary-search shrinking for `Number` (with a precision policy).
- **m8-05 R5** (open, accepted): `SBoxedStrategy` requirement is a constraint on the strategy's shape, not a bug.
- **m8-05 R7** (open, accepted): `dyn ValueTree: !Send` means the strategy tree is rebuilt each iteration instead of evolved in place. A future real-shrinker cycle must address this (e.g., `ValueTree: Send` feature flag, or move the loop into `spawn_blocking`).
- **m8-05 R6** (open, accepted): shrink requires a populated engines map; empty map returns `SessionNotFound` and the target unchanged.

## §7 m9+ handoff

The m9 backlog (to be scoped in the next cycle):

- **`attach` domain API** — implement `chronos_domain::attach` (m7+ scope per m7-04 decision). Unblocks `session_start{action=attach}`.
- **Single-call `session_stop`** — add `events` + `language` to `SessionStopOutput` (m7-07). Removes the v1 shim's double-call.
- **Live probe performance** — currently single-process; m9 could explore multi-process fan-out.
- **Real proptest shrinkers** (m8-05 R3) — `Strategy<PropertyValue>` with lexicographic `Text` shrinking + binary-search `Number` shrinking. Must also address m8-05 R7 (Send work-around).
- **Bundle-as-blob → side table** (m8-04 R4) — events currently ride inside the bundle blob. Splitting into a separate `counterexample_bundle_events` table enables event-level queries without deserializing the whole blob.
- **Persisting original `target_hypothesis`** (m8-04 R-hypothesis-reconstruction-fidelity) — enables byte-faithful `chronos test replay` round-trips for non-default Invariant options (scope, comparison, property_target).
- **Live probe plumbing for `chronos test run`** — the stub from m8-04 needs real probe integration.
- **Distributed tracing** — out of scope until m9+.
- **MCP server hardening** — observability, rate limiting, etc.

See `docs/chronos-agentic-reconstruction/docs/specs/AGENT_API_V2.md` for the v2 spec that motivated this sub-cycle.
