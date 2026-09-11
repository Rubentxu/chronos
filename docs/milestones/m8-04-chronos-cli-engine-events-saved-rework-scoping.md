# m8-04 — chronos-cli + engine events + Saved-variant rework scoping (PROPOSED)

**Cycle:** M8 §4, fourth execute cycle of the M8 multi-cycle plan.
**Predecessor doc:** `docs/milestones/m8-counterexample-shrinking-scoping.md` (parent M8 plan, m8-04 line 170).
**Predecessor merge:** `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-merge.md` (last execute cycle, merged 2026-09-11).
**Branch (planned):** `feat/m8-04-chronos-cli-engine-events-saved-rework`.
**Status:** PROPOSED — awaiting execute.

## Why this cycle exists

m8-03 shipped the dispatcher stubs-as-real + MCP wire + redb persistence + sandbox smoke. Three things remain before M8 is acceptance-ready:

1. **A4 (M8 scoping): `chronos test` CLI.** The dispatcher has no driver. The UAT acceptance criterion (`MILESTONE_ACCEPTANCE.md` § M8 line 78) is "a generated failing input shrinks while preserving the same property violation" — but nothing in the workspace can drive that end-to-end except via the MCP wire (which is what m8-03's smoke already does). The CLI is the canonical UAT harness the scoping doc mandated.

2. **Engine events → save() plumbing.** `ChronosCounterexampleService::save()` accepts a `Vec<TraceEvent>` (m8-03 §3), but the only caller today (`shrink()`) passes `vec![]` as a placeholder. m8-03 disclosed this as a known gap. Without real events in the persisted bundle, `chronos test replay` cannot replay the trace — the bundle is half-blind.

3. **B5 (m8-03 B-decisions): Saved-variant rework.** Today `Saved` serialises to `{"saved": <summary>}` (stopgap envelope), but `Shrunk` serialises to `{bundle, rounds_used, minimised_constant, minimised_predicate, minimised_call_path}` (full envelope). Wire-shape inconsistency. Rework `Saved` to match the Shrunk shape family so consumers only need one parser.

**What is deferred** (this cycle is A-min; do not include):
- **R4 (m8-03 B-decisions): split events into a side redb table.** Performance optimisation for 10k+ events. Bundle-as-blob is correct for M8; m9+ concern.
- **m8-05 (M8 close) work:** real `proptest::strategy::Map` shrinker, `spawn_blocking` wrap, pagination on `list()`, final M8 close report. This is m8-05 scope.

## Architectural decisions (B-decisions)

| # | Decision | Justification |
|---|---|---|
| B1 | `chronos-cli` is a single binary `chronos` with subcommand dispatch via `std::env::args` (no clap this cycle) | A4 (M8 scoping) says ≤ 200 LoC. clap is a workspace dep candidate but it adds 3-5 transitive deps. The two subcommands (`test run`, `test replay`) each take ≤ 3 positional args + 1-2 flags; manual parse is 20 LoC and zero deps. m9+ can adopt clap when we add `chronos repl` / `chronos inspect`. |
| B2 | CLI calls into `chronos-services` directly (no MCP wire round-trip) | The CLI is a driver, not a wire consumer. The MCP wire path already exists (m8-03) and is exercised by the sandbox smoke; re-driving it from CLI would require spawning `chronos-mcp` as a subprocess and using the rmcp client crate, which is heavier and slower. Direct dispatch is one `tokio::main` + one `Arc<SessionStore>` + one `tokio::sync::Mutex<HashMap<String, QueryEngine>>` (or `mpsc` for thread-safe engines map). |
| B3 | Engine events pulled in `shrink()` step 4, not in `save()` | `shrink()` is the only `save()` caller today (CLI is the second one). `shrink()` already has `target_hypothesis.session_id` in scope — pulling events there is one `ctx.hypothesis_ctx.engines.lock().await.get(...).get_all_events()`. `save()` stays pure-persistence (no engine lookup). Cost: `save()` becomes engine-unaware, which is correct per A-min scoping (saves shouldn't need the live engine map). |
| B4 | `Saved` serialises to `{bundle, events_count}` (mirrors `Shrunk`/`Got` family) | B5 rework. `events_count` is `events.len()` from the now-real Vec, not 0. `bundle` carries the same `CounterexampleBundleSummaryDto` as the rest of the wire. Consumers that handle `Saved` today update to the new shape — there are no external consumers yet (m8-03 just shipped), so the cut is contained. |
| B5 | CLI opens `SessionStore::open(path)` from a path arg (`--db`) — defaults to `~/.local/share/chronos/bundles.redb` | Mirror the v2 session lifecycle precedent (`session_load{path}` takes a path). Default is the conventional XDG path. Tilde expansion via `std::env::home_dir()` for simplicity (no `dirs` crate). |
| B6 | CLI uses an in-memory engines map (not a live session_start) | The CLI does not start sessions — that's an MCP-scope operation. The CLI replays already-persisted bundles by re-running `hypothesis_test` against the events in the bundle. No live probe is involved. The engines map is populated with a synthetic `QueryEngine` built from the bundle's `events` field. |

## Concrete deliverables

### Deliverable 1: `crates/chronos-cli` workspace member (~250 LoC)

```
crates/chronos-cli/
├── Cargo.toml                # depends on chronos-services, chronos-store, chronos-domain, chronos-query
└── src/
    ├── main.rs               # subcommand dispatch (~50 LoC)
    ├── args.rs               # manual arg parser (~30 LoC)
    ├── replay.rs             # chronos test replay (~80 LoC)
    └── run.rs                # chronos test run stub (no algorithm — calls services) (~90 LoC)
```

The binary is `chronos`. Subcommands:
- `chronos test run --db <path> <property_kdl> <target_binary>` — **stubbed in this cycle** (returns `Err(NotImplemented)` with a clear message; the run path needs the v2 session lifecycle + probe integration which is a m9+ concern). The dispatch path is wired (`chronos-services::hypothesis_test::ChronosHypothesisTestService::test`), but the live probe plumbing is not — that's a separate cycle.
- `chronos test replay --db <path> <bundle_id>` — **fully working**. Reads the bundle from redb via `ChronosCounterexampleService::get`, rebuilds the `QueryEngine` from `bundle.events`, re-runs `hypothesis_test::test` against the persisted `target_hypothesis`, prints the verdict + bundle summary as JSON.

### Deliverable 2: Engine events wiring (`shrink()` step 4)

`crates/chronos-services/src/counterexample.rs` — `shrink()` step 4 currently calls:
```rust
ChronosCounterexampleService::save(ctx, "ws-default", property_kind, (...), Vec::new())?;
```

Replace `Vec::new()` with a function call that pulls events from the live engine:
```rust
let events = pull_engine_events(ctx, target.session_id.clone()).await?;
ChronosCounterexampleService::save(ctx, "ws-default", property_kind, (...), events)?;
```

Where:
```rust
async fn pull_engine_events(
    ctx: &CounterexampleContext<'_>,
    session_id: String,
) -> Result<Vec<chronos_domain::TraceEvent>, ServiceError> {
    let guard = ctx.hypothesis_ctx.engines.lock().await;
    match guard.get(&session_id) {
        Some(engine) => Ok(engine.get_all_events()),
        None => Err(ServiceError::SessionNotFound(session_id)),
    }
}
```

The function name `pull_engine_events` and its `SessionNotFound` propagation mirror `hypothesis_test::test()` line 92-95. No new `ServiceError` variant.

### Deliverable 3: Saved-variant rework (B5)

`crates/chronos-mcp/src/server.rs` — `serialize_counterexample_output` today maps:
```rust
COut::Saved { summary } => json!({"saved": <summary>})
```

After m8-04:
```rust
COut::Saved { summary, events_count } => json!({
    "bundle": <CounterexampleBundleSummaryDto>,
    "events_count": events_count,
})
```

Requires:
- Adding `events_count: usize` to `CounterexampleOutput::Saved`.
- Plumbing `events.len()` through `save()` → `Saved` variant.
- Updating the sandbox smoke `ce4_list_after_shrink_includes_bundle` test (if it asserts on Saved variant shape — currently it doesn't).

Plus `output.rs`: drop the `Saved`-specific stopgap comments; the variant now mirrors the `Shrunk` shape family.

### Deliverable 4: Sandbox smoke update (chronos-sandbox/tests/counterexample_tools.rs)

Update CE4 / CE6 tests to assert the new Saved-variant shape:
- After shrink, fetch the freshly-saved bundle via `counterexample_get(bundle_id)` and confirm `events_count >= 1` (was `>= 0`). Today this fails because m8-03 ships `events_count = 0`. After m8-04, real events flow through.
- Add CE7: a smoke that asserts the Saved envelope shape via raw JSON parse of the shrink response (verifying `bundle` field is present, not `saved`).

### Deliverable 5: Cycle-close docs

Same pattern as m8-03: `docs/milestones/m8-04-chronos-cli-engine-events-saved-rework-merge.md` (~200 LoC) + `sddk/changes/m8-04-chronos-cli-engine-events-saved-rework-merge/apply-checkpoint.json`.

## Test plan

| Layer | Test | Notes |
|---|---|---|
| unit (chronos-services) | `engine_events_pull_returns_session_events` | Pulls events from a mock engine map and asserts the Vec matches. |
| unit (chronos-services) | `engine_events_pull_missing_session_errors` | SessionNotFound when engine map lacks the session_id. |
| unit (chronos-services) | `saved_variant_carries_events_count` | Confirms `Saved { summary, events_count }` shape. |
| unit (chronos-services) | `shrink_persists_real_events_not_empty_placeholder` | After shrink on a session with 3 events, `get()` returns a record with `events.len() == 3`. Replaces the m8-03 placeholder assertion. |
| unit (chronos-cli) | `test_replay_help` | `chronos test replay --help` prints usage (no exit-code assertion — just doesn't panic). |
| unit (chronos-cli) | `test_replay_unknown_bundle_errors` | `chronos test replay --db /tmp/x.redb unknown-id` exits non-zero with a clear stderr message. |
| sandbox (chronos-sandbox) | CE7 (new): saved envelope shape | Asserts shrink response has `bundle` (not `saved`) and `events_count >= 1`. |
| sandbox | CE1-CE6 (existing, refreshed) | Same as m8-03 plus the events_count assertions. |
| smoke (manual) | `chronos test replay` end-to-end | Spawn a real `chronos-mcp` server, run shrink via sandbox-style flow, kill server, then `chronos test replay --db /tmp/bundles.redb <bundle_id>` and assert the CLI prints a Violation verdict. |
| sandbox | CE4/CE6 (modified) | `events_count >= 1` instead of `>= 0`. |

**No new external deps.** No `clap` (B1), no `dirs` (B5), no `rmcp` client (B2).

## Honest disclosures (carry-over + new)

- **R1 (m8-02, m8-03): `rounds_used == 1`.** Still present. Real shrinker is m8-05 scope.
- **R2 (m8-03): no `spawn_blocking` wrap.** Still present. The events-pull lock in `shrink()` is a fast in-memory operation (no await inside the critical section beyond `lock()` itself).
- **R3 (m8-03): `next_cursor: None`.** Still present. Pagination is m8-05 scope.
- **R4 (m8-03): bundle-as-blob storage.** **Deferred to m9+.** Acceptable: real events are small in the smoke fixtures (test_busyloop has ~10 events); the inefficiency only matters for 10k+ events scenes which are not in M8 acceptance.
- **R5 (m8-03): wire-mirror conversion.** Still present. One struct, one line each direction.
- **B5 (m8-03): Saved-variant stopgap envelope.** **Resolved by m8-04 deliverable 3.**
- **NEW (m8-04): `chronos test run` is a stub.** Only `chronos test replay` works end-to-end. `run` needs live probe plumbing (session_start, probe_start, drain, stop) which is a much larger surface — likely a m9 cycle of its own (the M8 scoping doc line 170 says "CLI smoke drives a Go + Rust fixture through shrink end-to-end", which we'll meet via the sandbox smoke + CLI `replay` for m8-04, deferring `run` to m9+).
- **NEW (m8-04): CLI uses in-memory engines map.** `replay` rebuilds the QueryEngine from bundle.events on each invocation. m9+ may add an LRU cache.
- **NEW (m8-04): tilde expansion is `std::env::home_dir()` only.** No support for `~user/...`. Documented in `--help`.

## Risks and out-of-scope

- **MCP wire round-trip from CLI not built (B2).** If a future consumer needs the CLI to drive a remote `chronos-mcp`, that's an m9+ scope. Direct services-call is correct for the local UAT harness.
- **CLI is single-binary `chronos`.** No `chronos repl` / `chronos inspect` / `chronos migrate`. m9+ backlog.
- **CLI does not extend KDL.** Per A4 (M8 scoping line 86), the CLI consumes the existing grammar; it does not add new KDL keys. The `replay` subcommand does not parse KDL at all — it takes `bundle_id` directly.

## Cross-references

- `docs/milestones/m8-counterexample-shrinking-scoping.md` — parent doc, m8-04 line 170 (CLI scope).
- `docs/milestones/m8-03-mcp-wrappers-redb-bundle-table-merge.md` — predecessor merge doc, followups § lists m8-04 work.
- `docs/milestones/m8-01-counterexample-foundation-merge.md` — m8-01 sibling (signature + service foundation).
- `docs/chronos-agentic-reconstruction/docs/specs/RUNTIME_PROPERTIES_AND_SLICING.md` § "Counterexample shrinking" — spec subsection.
- `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` § M8 line 78 — end-to-end acceptance criterion.
- `crates/chronos-services/src/counterexample.rs` — shrink() step 4 today (Vec::new() placeholder, line ~523).
- `crates/chronos-services/src/hypothesis_test.rs` line 92-95 — pattern reference for SessionNotFound propagation.

## Cycle classification

A-min. Bounded, no architectural fork. CLI is a thin driver (≤ 250 LoC target), engine events plumbing is a 5-line `async fn` + one save() arg change, Saved-variant rework is one wire-shape edit.

**Pre-merge gate**: T0 (fmt + clippy) + T1 (workspace lib) + T2 (services/store/cli) + T4-smoke (sandbox counterexample_tools). No new sandbox T4 work beyond updated CE4/CE6 + new CE7.

---

— Submitted 2026-09-11. Awaiting execute.
