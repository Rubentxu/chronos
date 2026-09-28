# CURRENT.md — operating pointer

Last updated: 2026-09-28, session 3. Authority for cycle state is `sddk cycle status`, not this file.

## Goal / milestone

M10 Execution Explorer read path. The read path now has a real production entry point and a
mutation-proven E2E suite over the public JSON-RPC surface. Cycle-level SDDK bookkeeping has not been
started: there is **no active cycle**.

## Last verified state

- `origin/main` = `5feddc67`; the substantive code is at `2b162937`, the two CI fixes at `07d49e63` and `da93f6cf`.
- **Remote integration verified on `da93f6cf`: all six workflows `success`.** Architecture Contracts,
  CI, Coverage, Sandbox Debt Sentinel, Sandbox Smoke Tests, Supply chain.
- `5feddc67` (docs only) shows five workflows and no Sandbox Smoke. That is the fixed filter working
  as designed, not a regression: the commit touched only `.sddk/`, outside `chronos-sandbox/**`.
- Local on `2b162937`: E2E 2/2, `chronos-services --lib` 539, `chronos-mcp` 87, fmt and clippy clean,
  `pipelinek` run `3260a48d` with zero `StepFailed`.

## What fixing the path filter actually revealed

The `sandbox-smoke.yml` path bug was hiding more than a missing trigger. Once the workflow ran, it
failed: `uat_rec_c1_01_two_consumers_real_wire` panicked on the event_id ordering assertion. The same
test passes in `ci.yml` and passes locally, so the difference was environmental, not a product defect.

The cause is that this workflow never built `chronos-mcp`. `ci.yml` runs `cargo build --workspace`
first and gets it for free; `sandbox-smoke.yml` only ran `cargo build -p chronos-sandbox`, which does
not build a binary that lives in `crates/chronos-mcp`. The tests drive a real server process, so the
server they exercised was not guaranteed to correspond to the revision under test. `da93f6cf` adds an
explicit `cargo build --bin chronos-mcp` step. **Verified green: Sandbox Smoke Tests passed on
`da93f6cf`, with all six workflows `success`.**

**A mechanism I asserted and then disproved.** I first wrote into the commit message and the workflow
comment that the failure was a nested-`cargo build` lock deadlock. That was wrong on three counts,
each checked:

- The CI log contains no `building via cargo` line, so `build_chronos_mcp_via_cargo` never ran.
- `rust-cache` reported `Cache mode: write`, not a restore, so no stale binary was injected.
- Hiding the local `chronos-mcp` binary did **not** reproduce the failure; the suite passed via the
  nested build, taking 34 s instead of 20 s.

The comment and commit message were corrected to state only what the evidence supports. Recording this
because the wrong mechanism was plausible, matched a known prior symptom, and would have been easy to
ship as fact.

Lesson for the next run: a mechanism that reproduces a *known* failure mode is not thereby confirmed.
The 34 s versus 20 s delta was the tell that the nested build was actually succeeding locally, which
contradicted the deadlock theory.

## Open blockers

| # | Blocker | State | Evidence |
|---|---|---|---|
| 1 | `sddk lint` pack errors SDDK005/009/011/014 | OPEN, operator | do not create files just to green the counter |
| — | `.pipeline.kts` runs no tests | **CLOSED** `e1af9c4d` | journal run `22575c65`: 5 stages success, `RunFinished/success`, 0 `StepFailed`, captured output shows both E2E tests passing (2 passed, 28.11s) |
| — | `cargo test -p chronos-sandbox` does not rebuild `chronos-mcp` | **CLOSED** `3c011e8a` | resolver now refuses a stale binary naming the stale dir; mutation-proven |

**Both quality blockers from the last session are closed.** The stale-binary hazard had two distinct
demonstrations, and closing the workflow one (`da93f6cf`) did not close the direct-invocation one, so
the resolver itself now fails loud.

## Next concrete action

Remaining work is operator-scoped, not engineering-scoped: the four `sddk lint` pack errors, and
certification (UAT evidence, coverage numbers), which cannot be self-granted. The roadmap records all
10 chapters CLOSED, so there is no feature work queued.

## Do not

- Do not report `RELEASE_PENDING` or `release-uat-approved`. There is no active cycle. An earlier
  report this session did exactly that, by repeating the incoming summary instead of re-deriving from
  `sddk cycle status`.
- Do not report the full sandbox suite as green from a **truncated** run. An earlier run in this
  session was killed by a self-imposed 1200s `timeout` and showed only 2 binaries / 10 tests; that was
  not a pass. Verify the run reached its own end.
- Do not mutate a source file while a suite is running against the binary built from it. Doing so
  produced 3 `analytics_tools` failures that were the staleness guard working correctly, not a
  regression, and they invalidated the run.
- Do not re-run the E2E suite and call it green without building `chronos-mcp` first.
- Do not share one MCP server across `#[tokio::test]` functions; each test needs its own.
- Do not ship a mechanism for a failure without checking the logs for that mechanism's own marker.
