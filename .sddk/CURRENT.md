# CURRENT.md — operating pointer

Last updated: 2026-09-29, session 4. Authority for cycle state is `sddk cycle status`, not this file.

## Goal / milestone

Release-pipeline honesty (M10-era roadmap, all 10 chapters recorded CLOSED; no feature work queued).
v0.1.4 is published. The active work is closing out cycle v014, which is **blocked in `archive`**.

## Last verified state

- `HEAD == origin/main` = `84c42bd97cc7e951d4662232b725a2cd98b8c6b3`, working tree clean.
- Cycle `p-3416cfb8288f8964/release-pipeline-honesty-v014`: **RELEASED**, phase `archive`, no lease.
  It is **not CLOSED** — see the vault blocker below.
- Release v0.1.4 published: annotated tag, tag object `d617c177…`, peel `98c4cd23`, remote verified.
  `main` is intentionally ahead of the tag by two docs-only commits. **Do not move or recreate v0.1.4.**
- Ledger verified: 417 events, `sha256:524d744eda0ad3d6733733c705d533767e61f32133f34869277ea8d07c18a40d`.
- Pipelinek PASS (run `0b8662a3`, forced with `--rerun`); vault drift sweep PASS.

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
| 2 | **Vault has 60 validation errors, blocking `archive.complete` for v014** | OPEN, engineering | `sddk vault validate` → `errors: 60` (VAULT002 x30 duplicate `archive-manifest` node ids, VAULT003 x30 dangling wikilinks). Gate receipt `gate-vault-index-current-f86f75d06cd14622-1` = **failed**. Runtime refused `archive.complete` with `ENGINE_GATE_FAILED_WITHOUT_TARGET`. Backlog `bl-bl-01M3PXC6ZV000387DTRTP6AS00`. Full evidence in `cycle-artifacts/…/v014/archive-manifest.md`. |
| 3 | `pipelinek` can return SUCCESS from a stale cache | OPEN, engineering | Backlog `bl-bl-01M3PWNDVW000387DS8ZH6DX00`. Every pipeline claim must use `--rerun`. |
| — | `.pipeline.kts` runs no tests | **CLOSED** `e1af9c4d` | journal run `22575c65`: 5 stages success, `RunFinished/success`, 0 `StepFailed`, captured output shows both E2E tests passing (2 passed, 28.11s) |
| — | `cargo test -p chronos-sandbox` does not rebuild `chronos-mcp` | **CLOSED** `3c011e8a` | resolver now refuses a stale binary naming the stale dir; mutation-proven |

**The two quality blockers from session 3 are closed.** The stale-binary hazard had two distinct
demonstrations, and closing the workflow one (`da93f6cf`) did not close the direct-invocation one, so
the resolver itself now fails loud.

**The v014 archive blocker is real and not cosmetic.** None of the 60 vault errors are attributable to
v014 (it added no vault node at all), but the archive contract makes a vault validation failure
blocking and no local ledger holds a prior `vault-index-current` receipt to justify a narrower
reading. The gate was recorded `failed` rather than quietly redefined.

## Next concrete action

1. Open a **vault repair cycle**: namespace the 30 colliding `archive-manifest` node ids per cycle,
   and create or correct the 10 missing wikilink targets. Target: `sddk vault validate` → 0 errors.
2. Re-evaluate `vault-index-current`, then apply `archive.complete` for v014 with the new receipt.
3. Force `--rerun` for pipelinek in `AGENTS.md` so no future session can read a stale SUCCESS.
4. Still operator-scoped: the four `sddk lint` pack errors, and certification (UAT evidence,
   coverage numbers), which cannot be self-granted.

## Do not

- Do not report v014 as CLOSED or archived. It is RELEASED/phase=archive with a failed
  `vault-index-current` gate. Closing it would require a pass that was never observed.
- Do not move, recreate, or re-point tag `v0.1.4`. It correctly peels to `98c4cd23`; `main` is
  ahead by docs-only commits.
- Do not report `release-uat-approved` as an executed UAT. It passed by policy skip (no `uat.toml`).
- Do not trust a `pipelinek` SUCCESS without `--rerun`.
- Do not report `RELEASE_PENDING` or repeat an incoming summary instead of re-deriving from
  `sddk cycle status`.
- Do not report the full sandbox suite as green from a **truncated** run. Verify the run reached its end.
- Do not mutate a source file while a suite is running against the binary built from it. Doing so
  produced 3 `analytics_tools` failures that were the staleness guard working correctly, not a
  regression, and they invalidated the run.
- Do not re-run the E2E suite and call it green without building `chronos-mcp` first.
- Do not share one MCP server across `#[tokio::test]` functions; each test needs its own.
- Do not ship a mechanism for a failure without checking the logs for that mechanism's own marker.
