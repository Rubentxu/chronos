# Session handoff — 2026-09-18

> A copy of the full handoff lives at `~/.jcode/scratch/handoff-2026-09-17.md`.
> This note in the repo survives `git pull` if you arrive tomorrow without
> that scratch directory accessible.

## Repo state at end of day

- `main` @ `ab863cf1` (chore merge)
- Predecessor: `d435557e` (REC-C2.3 merge)
- Tags pushed: `rec-c2.3-eventbus-removal`, `retire-stale-bus-doc-mentions`
- Ratchet (`python3 scripts/check_legacy_evb.py`): PASS, baseline 0
- Worktree: clean

## Two closed cycles today

**REC-C2.3 (architectural)** — eventbus retirement. 5 commits (`626b2090`,
`a99b3a35`, `af41d8c2`, `fb170dcc`, `ad28430f`). The `chronos-domain::bus`
module is deleted from the source tree; `ProbeBackend::read_since` is
retracted; `bus_capacity` is dropped from MCP wire; the ratchet reached 0.
Full proposal/tasks: `cycle-artifacts/p-3416cfb8288f8964/rec-c2.3-eventbus-removal/`.

**retire-stale-bus-doc-mentions (chore)** — editorial pass removing 5
doc-comments that still described the removed EventBus behavior as if it
existed, plus 2 leftover test stubs. Bundled pre-existing tree hygiene
(`fmt` drift + `clippy::new_without_default` on `NativeProbeBackend`).
2 commits (`23757379`, `99c0dcee`). Full proposal/tasks:
`cycle-artifacts/p-3416cfb8288f8964/retire-stale-bus-doc-mentions/`.

## Pick one for tomorrow

- **A**: archive the chore cycle via `sddk archive` (lowest cost)
- **B**: doc-comment audit round 2 — re-evaluate the kept-as-is mentions
  in `canonical_drain.rs`, `probe.rs`, `output.rs`, `probe_backend.rs`
- **C**: REC-C3 — EventBus-adjacent fan-out audit (accepted-Raw
  observer, `tracing::instrument` surface, resolver-cloned events). The
  next architectural question

A is mechanical, B is editorial, C is a real A-lite cycle.

Anything not picked stays in the queue.
