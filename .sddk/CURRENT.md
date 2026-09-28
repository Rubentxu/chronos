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

| # | Blocker | Owner | Next move |
|---|---|---|---|
| 1 | `sddk lint` pack errors SDDK005/009/011/014 | operator | decide; do not create files to green the counter |
| 2 | `.pipeline.kts` runs no tests, so `SUCCESS` certifies compilation only | operator-approval needed | add a test stage; see below |
| 3 | `cargo test -p chronos-sandbox` does not rebuild `chronos-mcp`; false green possible | me, on approval | fix in the same stage as #2 |

Blocker 3 now has a second, independent demonstration beyond the local mutation: `sandbox-smoke.yml`
was shipping without a `chronos-mcp` build step at all, which is how a stale or unbuilt server reached
the tests in CI. Fixed in `da93f6cf` for that workflow, but the hazard remains for anyone invoking
`cargo test -p chronos-sandbox` directly.

Blocker 2 is now evidence-backed rather than a read of the file. Enumerating the commands in
`.pipeline.kts` yields exactly `cargo check --workspace`, `cargo build -p chronos-domain`, and a
`cargo check --workspace` in a retry step. No `cargo test` appears anywhere.

## Next concrete action

Add a test stage to `.pipeline.kts` that builds `chronos-mcp` before running the E2E suite, which
makes the local gate meaningful and closes blocker 3 as a side effect. This edits a versioned pipeline
contract, so it needs operator approval before landing.

## Do not

- Do not report `RELEASE_PENDING` or `release-uat-approved`. There is no active cycle. An earlier
  report this session did exactly that, by repeating the incoming summary instead of re-deriving from
  `sddk cycle status`.
- Do not re-run the E2E suite and call it green without building `chronos-mcp` first.
- Do not share one MCP server across `#[tokio::test]` functions; each test needs its own.
- Do not ship a mechanism for a failure without checking the logs for that mechanism's own marker.
