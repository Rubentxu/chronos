# REC-C1.5 closure — exploration report (refined, v2)

**Cycle**: p-3416cfb8288f8964/rec-c1-5-closure
**Path**: A-lite
**Base (declared)**: efb44989 (main) — used as reference for the roadmap doc
**Branch**: feat/rec-c1.5-closure — branched from **17367f2d** (`design/rec-c1-truth-cutover`) so the actual C1.5.4 code is present.
**Actor**: orchestrator (MiniMax-M3)
**Refinement**: 2026-09-16 — review of C1.5.4 surfaced two hardenings; on inspection both have already been merged into `17367f2d`.

## Important context: base correction

`efb44989` (main) carries the **roadmap doc** for REC-C1.5 closure, but the
**code** of sub-gates C1.5.0..C1.5.4 lives on the branch
`design/rec-c1-truth-cutover`. `git show` reports `b1ec5659` (C1.5.4) adding
4 new files in `crates/chronos-services/` and `crates/chronos-log/`, but those
files are NOT present in `efb44989`'s tree — only the doc is.

So the work this cycle must do is **on top of `17367f2d`** (HEAD of
`design/rec-c1-truth-cutover`), where the two-phase bootstrap,
`reopen_existing`, `discovery`, the `BootstrapPlan` with validated handles,
and the pure-discovery `delete_durable_execution_log` already exist.

We branch from `17367f2d` rather than from `main` so the actual product code
is in the working tree.

## Status before this cycle (as of `17367f2d`)

Sub-gates landed (chronological):

- C1.5.0 `c22732b2` — characterization of the two restart inconsistencies
- C1.5.1 `0be29519` — durable retention watermark
- C1.5.2 `3cc511ef` — strict replay (reopen publishes a validated region or nothing)
- C1.5.3 `1c4521c1` — tail state (clean / abnormal / unknown, without faking integrity)
- C1.5.4 `b1ec5659` — deterministic discovery, strict reopen, two-phase bootstrap
- **C1.5.4-fix `17367f2d`** — the two hardenings from the review (see below)
  - `BootstrapPlan` carries the validated `SessionExecutionLog` (one reopen, no TOCTOU)
  - `delete_durable_execution_log` uses `discover_execution_logs()` only
    (never reopens other sessions; honours `report.duplicates`)

The user's 11-step plan therefore **collapses**:

| # | Plan step | Status |
|---|---|---|
| 1 | BootstrapPlan carries validated handles (H-1) | **DONE** in `17367f2d` |
| 2 | delete_durable uses pure discovery (H-2) | **DONE** in `17367f2d` |
| 3 | Canonical ExecutionLog root resolver | **TODO** |
| 4 | MCP startup wiring (bootstrap before READY) | **TODO** |
| 5 | delete_session tool wiring | **TODO** |
| 6 | UAT-R1 real process resume | **TODO** |
| 7 | UAT-R2 identical stale before/after restart | **TODO** |
| 8 | UAT-R3 sealed persistence | **TODO** |
| 9 | Readiness test `MCP READY ⇒ bootstrap done` | **TODO** |
| 10 | Full regression | **TODO** |
| 11 | Ledger + REC-C1.5 CLOSED | **TODO** |

## Re-stated final order for REC-C1.5 closure (this cycle)

The 11-step plan is unchanged in shape; only steps 1 and 2 are already
delivered. The cycle delivers 3..11.

```
 3. Canonical ExecutionLog root resolver
        - one fn execution_log_root() -> PathBuf
        - one fn execution_log_dir(root, session) -> PathBuf
        - used by session_start, bootstrap, delete
        - closes "start writes A, restart reads B" bugs

 4. MCP startup wiring
        - resolve canonical root
        - open SessionStore
        - create empty ExecutionLogRegistry
        - bootstrap_execution_logs()
        - only then publish registry + announce MCP ready
        - root bootstrap failure -> typed ChronosServerInitError, no READY

 5. delete_session tool wiring
        - SessionStore delete
        - durable ExecutionLog delete (discovery-based, no reopen)
        - memory cleanup
        - partial-failure contract: durable delete fails -> tool ERROR,
          no false success
        - on success: restart discovery finds nothing

 6. UAT-R1 — resume valid (real process, 10k events)
        Process A:
          start session, 10k events, events_read up to cursor C=3000,
          flush, retention still keeps C valid, kill process
        Process B:
          bootstrap before READY, events_read(cursor C)
        Assertions:
          first event of EventSeq == expected
          no duplicates, no omissions
          same SessionId
          completeness coherent
          tail_state == Unclean (A not sealed)

 7. UAT-R2 — identical stale before/after restart
        cursor X < retained_from R
        before restart: CursorStale { requested_next_seq: X,
                                     retained_from_seq: R }
        after restart:  EXACTLY the same error
        Both numbers verified (not just "some stale error")

 8. UAT-R3 — sealed persistence
        10k events, flush, seal, restart
        Assertions:
          TailState::Sealed
          tail_seq identical
          retained_from identical
          evidence readable

 9. Readiness test
        MCP READY => bootstrap finished
        protects against future refactors reverting to background bootstrap

10. Full regression
        - T0 fmt+clippy
        - T1 lib unit (full workspace)
        - T2 per-crate integration of changed crates
          (chronos-mcp, chronos-services, chronos-store,
           chronos-log if affected)
        - T4-smoke: e2e_connectivity + analytics_tools +
                     sessions_tools + delete-related suites
        - sandbox regression on BOOT-*/TAIL-*/RET-*/REP-*/CONTROL

11. Ledger / evidence / REC-C1.5 CLOSED
        - implementation-receipt.md
        - verify-report.md
        - debt-report.json
        - release-receipt.md
        - merge-receipt.md
        - archive-manifest.md
        - commit history partitioned per work-unit (reviewable commits)
```

C1.6 is NOT opened in this cycle.

## Path selection

A-lite: explore (this doc) → specify → design → tasks → apply → verify →
debt-verify → release → archive. The remaining 9 steps fit cleanly in A-lite;
no architectural fork, but enough scope to require a real spec + design +
tasks breakdown.

## Non-goals (explicit)

- No new retention semantics (C1.5.1 closed that)
- No discovery changes (C1.5.4 closed that; `17367f2d` tightened it)
- No bootstrap TOCTOU (already fixed in `17367f2d`)
- No new tool surface
- No sandbox/S0 work in this cycle
- No C1.6

## Risk

- Touches server startup (mandatory T4-smoke per AGENTS.md §2)
- UAT requires real-process harness; sandbox helpers alone are insufficient
- The canonical root resolver must replace all three call sites atomically
  (test seam in tests stays; production path uses one shared resolver)
- Merging `feat/rec-c1.5-closure` back into main after closure should also
  propagate the design/rec-c1-truth-cutover work; consider rebasing main
  onto `design/rec-c1-truth-cutover` first or doing a clean merge