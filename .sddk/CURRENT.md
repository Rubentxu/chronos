# CURRENT.md — operating pointer

Last updated: 2026-09-28, session 3. Authority for cycle state is `sddk cycle status`, not this file.

## Goal / milestone

M10 Execution Explorer read path. The read path now has a real production entry point and a
mutation-proven E2E suite over the public JSON-RPC surface. Cycle-level SDDK bookkeeping has not been
started: there is **no active cycle**.

## Last verified state

- `origin/main` = `1970da9a`; the substantive code is at `2b162937`, the CI fix at `07d49e63`.
- Remote CI on `2b162937`: CI, Architecture Contracts, Supply chain, Sandbox Debt Sentinel all
  `success`. Coverage was still running.
- Remote workflows on `07d49e63`: 3 of 6 completed; CI, Sandbox Smoke Tests, Coverage pending.
  Sandbox Smoke Tests **did** trigger, which is the proof that the path-filter fix works.
- Local on `2b162937`: E2E 2/2, `chronos-services --lib` 539, `chronos-mcp` 87, fmt and clippy clean,
  `pipelinek` run `3260a48d` with zero `StepFailed`.

## Open blockers

| # | Blocker | Owner | Next move |
|---|---|---|---|
| 1 | `sddk lint` pack errors SDDK005/009/011/014 | operator | decide; do not create files to green the counter |
| 2 | `.pipeline.kts` runs no tests, so `SUCCESS` certifies compilation only | operator-approval needed | add a test stage; see below |
| 3 | `cargo test -p chronos-sandbox` does not rebuild `chronos-mcp`; false green possible | me, on approval | fix in the same stage as #2 |

Blocker 3 has a demonstrated consequence: mutating `decode_for_session` left the suite **passing**.
The two obvious repairs both fail, `cargo build --dry-run` emits an empty stream either way and a real
nested build deadlocks on the target-dir lock. Details and the rejected mtime heuristic are in
`CLOSE-OUT-2026-09-28.md`.

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
