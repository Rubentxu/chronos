# m8-04 — chronos-cli + engine events wiring + Saved-variant rework merge doc

**Cycle:** M8 §4, fourth execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-04-chronos-cli-engine-events-saved-rework-scoping.md` (PROPOSED, FF-merged to `main` as commit `34d0b81` before any code landed).
**Branch:** `feat/m8-04-chronos-cli-engine-events-saved-rework-execute` (FF-merged to `main` after this cycle).
**Status:** MERGED — 2026-09-11

## What shipped

m8-04 closes three strands from the m8-04 scoping doc:

1. **Engine events wiring** — `shrink()` no longer returns `vec![]` for events; it pulls them out of the live `QueryEngine` via a new `pull_engine_events(session_id)` method that locks the engines `TokioMutex<HashMap<String, QueryEngine>>`, looks up the session, and calls `engine.get_all_events()`. Missing sessions surface as `Err(ServiceError::SessionNotFound(session_id))` (mirrors `hypothesis_test::test` line 92-95; no new error variant).
2. **Saved-variant rework + `events_count` plumbing** — `CounterexampleOutput::Saved` and `::Shrunk` now carry `events_count: usize`. The MCP `serialize_counterexample_output` wrapper emits the new `{bundle, events_count}` envelope (replacing the m8-03 `{"saved": <summary>}` stopgap). The sandbox's `CounterexampleShrinkOutputWire` gained a custom `Deserialize` impl that reads top-level `events_count` first and falls back to `full.events_count` (smoke tests use the new shape).
3. **`crates/chronos-cli` workspace member** — a new binary `chronos` with two subcommands:
   - `chronos test replay <bundle_id>` — fully working: reads a bundle from the redb store, rebuilds a `QueryEngine::new(bundle.events)`, reconstructs a `HypothesisInput` from the bundle's `MinimisedPayload`, runs `ChronosHypothesisTestService::test`, prints a JSON `ReplayReport`.
   - `chronos test run` — STUB (m9+): returns `Err(anyhow!("...not yet implemented (m9+ scope)..."))` so operators get a sane failure with the milestone named, not a cryptic "command not found".

### 1. Engine events wiring

`crates/chronos-services/src/counterexample.rs`. New method:

```rust
pub async fn ChronosCounterexampleService::pull_engine_events(
    ctx: &CounterexampleContext,
    session_id: &str,
) -> Result<Vec<TraceEvent>, ServiceError>
```

`shrink()` step 4 now calls `Self::pull_engine_events(target.session_id)` instead of `vec![]`. This is what makes the `Saved` envelope's `events_count` non-zero in practice — without the wiring, every saved bundle would have had `events_count == 0` and the field would have been a footgun.

**Disclosure:** missing-session errors re-use `ServiceError::SessionNotFound(String)`. No new variant added (we already use it in `hypothesis_test::test` for the same lookup pattern).

### 2. Saved-variant rework + `events_count` plumbing

The m8-03 wire envelope was a stopgap — `serialize_counterexample_output` emitted `{"saved": <summary>}` for the Saved variant and hardcoded `0` for `full.events_count`. m8-04 closes both gaps:

- `CounterexampleOutput::Saved` now carries `{ summary, events_count }`. `save()` re-reads the just-persisted record via `load_counterexample_bundle` to surface `events.len()` — one extra redb read per save, which is acceptable today. (m9+ may surface `events_count` from the shrinker's local view to avoid the second read; noted in commit message.)
- `CounterexampleOutput::Shrunk` propagates `Saved.events_count` up. The MCP envelope for Shrunk is now `{bundle: <bundle>, rounds_used, minimised_*, full: {…, events_count: <n>}}`.
- Sandbox wrapper `CounterexampleShrinkOutputWire` gained `events_count: usize` + `Option<CounterexampleBundleFullWire>`. Because serde rejects both flat-and-nested and explicit-and-tagging, the wrapper uses a **custom `Deserialize` impl** that reads top-level `events_count` first and falls back to `full.events_count`. Smoke test CE7 (`ce7_shrink_response_uses_new_saved_envelope`) exercises this.

**Disclosure (m9+):** the one-extra-redb-read in `save()` is logged but not yet optimised. The cleaner fix — having `save()` accept an `events_count` argument and trust the shrinker rather than re-reading — is m9+ scope.

### 3. `crates/chronos-cli` workspace member

New workspace member, single binary `chronos`. Added to `Cargo.toml` `members`.

| File | LoC | Purpose |
|---|---|---|
| `crates/chronos-cli/Cargo.toml` | 27 | depends on chronos-domain, chronos-query, chronos-store, chronos-services; no clap (B1). |
| `crates/chronos-cli/src/args.rs` | 252 | manual arg parser; `parse(argv) -> Result<Command, ArgsError>`; XDG default + `~` expansion. |
| `crates/chronos-cli/src/main.rs` | 96 | tracing init, anyhow error funnel, JSON ReplayReport on stdout. |
| `crates/chronos-cli/src/replay.rs` | 358 | the working subcommand: load bundle, rebuild engine, reconstruct hypothesis, run dispatcher, project to report. |
| `crates/chronos-cli/src/run.rs` | 59 | the stub: returns `Err(anyhow!("...not yet implemented (m9+ scope)..."))`. |

**Decisions (from scoping doc):**
- **B1 — no clap:** hand-rolled parser in `args.rs`. Two subcommands don't justify a third-party framework.
- **B2 — direct services dispatch:** the CLI does NOT open a JSON-RPC channel to `chronos-mcp`. It dispatches into chronos-services + chronos-store directly, which keeps the CLI dependency-free at runtime and avoids round-tripping through a separate process for what is fundamentally a local-disk read + in-process computation.
- **B5 — XDG default:** `--db` defaults to `$XDG_DATA_HOME/chronos/chronos.db`, falling back to `$HOME/.local/share/chronos/chronos.db`. Manual `~` expansion in the parser (R-tilde-expansion disclosure).
- **B6 — in-memory engines map for replay:** the replay subcommand builds a synthetic `QueryEngine::new(bundle.events)` and parks it under a fixed session id in a `HashMap<String, QueryEngine>` wrapped in `tokio::sync::Mutex`. This is the same shape the MCP server uses for live sessions, which lets us call the real `hypothesis_test::test` dispatcher without inventing a parallel execution path.

**Disclosures:**
- **R-run-stub:** `chronos test run` is a milestone-pinned stub (m9+). Operators get a sane `Err(anyhow!("...not yet implemented (m9+ scope)..."))` chain.
- **R-in-memory-engines:** the temporary engines map lives in the CLI's own `HashMap`; `SessionStore::open` is read-only from CLI's perspective.
- **R-tilde-expansion:** only `~` and `~/...` are expanded; `~user/...` is intentionally not supported (would require `/etc/passwd`).
- **R-hypothesis-reconstruction-fidelity (new, m8-04):** `replay` reconstructs `HypothesisInput` from `MinimisedPayload` using synthetic defaults (`scope = PropertyValue`, no `comparison`, no `property_target` for Invariant). Structurally faithful but **not byte-for-byte**: a bundle minimised with non-default Invariant options cannot round-trip exactly. Fixing this requires storing the original `target_hypothesis` in the bundle (m9+ scope).

## Test counts

| Bucket | Count | Status |
|---|---|---|
| chronos-cli unit tests | 18 (8 args + 8 replay + 2 run) | green |
| chronos-services lib | 232 | green |
| chronos-store lib | 23 | green |
| T0 fmt + clippy workspace | n/a | clean |
| T4-smoke `counterexample_tools` | 7 (CE1–CE7) | green in 39s |
| T4-smoke `e2e_connectivity` | 1 | green |

T4-smoke subset: `counterexample_tools` + `e2e_connectivity` — picked because they are the only sandbox tests that touch the changed wire envelope and the MCP server lifecycle. AGENTS.md §2 calls this out as the standard pattern for "cycle changes probe/mcp plumbing".

## Honest limitations

- **Bundle round-trip is structurally faithful, not byte-for-byte.** A user who passes `scope = LatencyMs` to a shrink and then replays the saved bundle will see the dispatcher re-test on `scope = PropertyValue`. The round-trip is sufficient to validate that the bundle still represents a violation (the M8 acceptance criterion), but not sufficient to reproduce the exact minimised input the user typed.
- **One extra redb read per save.** `save()` calls `load_counterexample_bundle` to surface `events_count`. m9+ may short-circuit this by having the shrinker pass `events_count` directly.
- **No live probe in `chronos test run`.** The CLI replays persisted bundles; starting a fresh probe is m9+.

## Out-of-scope (deferred)

These were explicitly listed as m9+ in the m8-04 scoping doc and remain there:
- R4 — bundle-as-blob → side table for events (m9+ split)
- Storing the original `target_hypothesis` in the bundle (m9+; closes R-hypothesis-reconstruction-fidelity)
- `chronos test run` live-probe plumbing (m9+)
- Pagination on `list_counterexample_bundles` (m8-05)
- Real per-variant proptest shrinking (m8-05; closes m8-03 R1)

## Closing checklist

- [x] Branch clean, FF-merged to main with `--no-ff` merge commit per cycle pattern.
- [x] Tag `m8-04-chronos-cli-engine-events-saved-rework.0` on the FF-merge SHA.
- [x] Apply-checkpoint under `sddk/changes/m8-04-chronos-cli-engine-events-saved-rework-merge/apply-checkpoint.json`.
- [x] All disclosures (R-run-stub, R-in-memory-engines, R-tilde-expansion, R-hypothesis-reconstruction-fidelity) recorded.
- [x] T0 + T2 + T4-smoke gates clean.
- [ ] m8-05 cycle picks up next: real proptest shrinking, pagination, events_count tool exposure, M8 close report.
