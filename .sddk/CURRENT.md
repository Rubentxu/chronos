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
| 2 | **Vault violates the `knowledge-graph` protocol, blocking `archive.complete` for v014** | OPEN, **human-admitted** | Re-verified this session, not assumed. `sddk vault validate --vault ~/.sddk-knowledge/p-3416cfb8288f8964` → `nodes: 138, errors: 60`, exit 1, digest `sha256:ee826e21…`. Gates re-evaluated with real proof: `ledger-valid` = **passed** (receipt `gate-ledger-valid-f86f75d06cd14622-2`, argv+exit 0, digest `sha256:725b4ba8…`), `vault-index-current` = **failed** (receipt `gate-vault-index-current-f86f75d06cd14622-2`). Transition re-attempted → refused `ENGINE_GATE_FAILED_WITHOUT_TARGET`. **60 errors decompose into 30 `VAULT002` id collisions (12 distinct ids across 53 files) + 30 `VAULT003` broken links (9 distinct targets).** I proved the collision half is mechanically fixable: on an isolated copy, renaming clears 60→30, and fixing the `adrs/0002-…` vs `ADR-0002-…` filename mismatch clears 9 more. **Correction to the earlier "human-only" framing:** the refusal is specific to `knowledge import` (`actor_kind System not admitted … admitted: [Human]`); `sddk vault index` is NOT refused, and `permission check` shows default-deny with no agent registered. Vault *content* repair is still not mine to perform, but the honest reason is the permission registry, not a blanket ban. Backlog `bl-bl-01M3PXC6ZV000387DTRTP6AS00` (P1). Backup `~/.jcode/scratch/vault-backup-20260929-174612`. |
| 3 | `pipelinek` without `--rerun` returns a **complete SUCCESS while executing nothing** | OPEN, upstream (cannot fix here) | **Mutation-proven.** A `panic!` injected into the `#[tokio::test]` fn `execution_log_read_over_the_wire` in `chronos-sandbox/tests/execution_log_read_e2e.rs` (run by the `test-sandbox-read-path` stage) → `cargo test` = `FAILED. 1 passed; 1 failed`. `pipelinek run` without `--rerun` → `SUCCESS`, exit 0, 5.5s, 5 stages `StageFinished/success`, **0 `StepStarted`**. With `--rerun` → `FAILURE`, exit 1. Cache key `sha256:9f26bb05…` is identical clean vs mutated, so no invalidation. `pipelinek` 0.39.0 is a distributed binary, not repo code — fix is upstream. Backlog `bl-bl-01M3PWNDVW000387DS8ZH6DX00` (P1). `--rerun` is mandatory in AGENTS.md. |
| 4 | **Two divergent knowledge bases; the repo copy is what CI actually guards, and the external vault is invisible to every gate** | OPEN, human | `.sddk-knowledge/` in the repo and `~/.sddk-knowledge/p-3416cfb8288f8964` have **zero overlapping archive manifests** (repo 102, vault 16). CI (`vault-drift.yml`) triggers on the repo copy and runs `check_vault_drift.sh` against it, so **retiring it would break the only automated drift protection** — an earlier suggestion to "reconcile or retire" was wrong and is withdrawn. The external vault is read by no gate. Both sides share the id-collision defect: vault 30 `VAULT002` errors, repo 102 duplicate `archive-manifest` + 94 `change-entry` + 19 `spec` filenames, and CI's sweep checks pinned SHAs only, never node ids. Backlog `bl-bl-01M3PXR8HC000387DVK7R57EC0` (P1). |
| 5 | ~~CI's drift gate cannot catch a broken test~~ — **WITHDRAWN, it was false** | **CLOSED, withdrawn** | I claimed `vault-drift.yml` runs no project tests so CI would stay green on a failing tree. **Refuted on evidence.** `.github/workflows/ci.yml` runs on every push/PR to `main` and executes `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build --workspace`, `cargo test --workspace --lib`, and `cargo test --workspace --tests --exclude chronos-e2e`. Verified **empirically, not just read**: `.sddk-state/test-buckets/cargo-skip.txt` is **0 bytes** so the CI-constructed `SKIP_ARGS` is empty; `chronos-sandbox` is **not** excluded (only `chronos-e2e` is); and `cargo test -p chronos-sandbox --test execution_log_read_e2e -- --list` enumerates both `execution_log_read_over_the_wire` and `execution_log_read_serves_a_real_log_over_the_wire`. The mutated test lives at `chronos-sandbox/tests/execution_log_read_e2e.rs` as a `#[tokio::test]` integration target, so CI **would have failed**. I generalised from one of nine workflow files. No CI change is warranted. |
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

1. **Human decision required — an agent cannot self-unblock this** (blocker 2).
   Either declare owners on the 178 quarantined sources and run
   `sddk knowledge import` as a `Human` actor, or amend the
   `vault-index-current` gate contract to accept a legacy non-conforming vault.
   `knowledge import` refuses a System actor, so option (a) is not reachable
   from here by design.
2. Once the vault is clean (or the gate contract is amended), re-evaluate
   `vault-index-current` and apply `archive.complete` for v014.
3. Decide the knowledge-base topology (blocker 4) with a human: the repo copy
   is CI-guarded and must be kept, while the external vault is what the SDDK
   gates and `knowledge scan` read. Either make them one base, or explicitly
   scope each (repo = CI-checked record, vault = SDDK index) and document which
   is authoritative. Do not delete the repo copy.
4. Fix the `pipelinek` cache invalidation bug properly (blocker 3). `--rerun` is
   now mandatory in AGENTS.md as the stopgap.
5. Still operator-scoped: the four `sddk lint` pack errors, and certification
   (UAT evidence, coverage numbers), which cannot be self-granted.

## Do not

- Do not delete or "clean up" the git-tracked `.sddk-knowledge/` copy. CI's
  `vault-drift.yml` triggers on it and `check_vault_drift.sh` reads it, so
  removing it would silently disable the only automated drift protection.
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
