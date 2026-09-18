# Exploration — retire-stale-bus-doc-mentions (retroactive ledger sync)

> **Status note**: this exploration-report is created retroactively to allow
> the ledger to record the chore cycle `retire-stale-bus-doc-mentions`
> that was merged to `main` on 2026-09-17 but never opened as an SDDK
> cycle at the time. No further investigation is performed; the cycle
> is closed at exploration time.

## Why this cycle exists in the ledger

On 2026-09-17 two cycles were closed via direct Git work without the
SDDK lifecycle being initiated:

1. `feat/rec-c2.3-eventbus-removal` (architectural, 5 commits, merged
   via `d435557e`, tag `rec-c2.3-eventbus-removal`)
2. `chore/rec-c2.3-retire-stale-bus-doc` (chore follow-on, 2 commits,
   merged via `ab863cf1`, tag `retire-stale-bus-doc-mentions`)

The Git history, ratchet, and code are correct. The ledger has no
record of either cycle because `sddk cycle start` was never called.
This retroactive sync registers the **chore** cycle (`retire-stale-bus-doc-mentions`)
in the ledger so the archive manifest, cycle index, and analytics can
refer to it. The **architectural** cycle (`rec-c2.3-eventbus-removal`)
is left for a separate retroactive sync if desired.

## Current state at ledger-sync time

- Branch: `main` @ `ab863cf1` (chore merge commit)
- Predecessor: `d435557e` (REC-C2.3 merge commit)
- Ratchet (`python3 scripts/check_legacy_evb.py`): PASS, baseline 0
- Working tree: clean (`git status` empty)
- Cycle artifacts on disk:
  - `cycle-artifacts/p-3416cfb8288f8964/retire-stale-bus-doc-mentions/proposal.md`
  - `cycle-artifacts/p-3416cfb8288f8964/retire-stale-bus-doc-mentions/tasks.md`
- Tags pushed: `rec-c2.3-eventbus-removal`, `retire-stale-bus-doc-mentions`

## Discovery — what the chore actually changed

Five doc-only edit points across four production files plus two residual
test stubs. No Rust semantics changed. Source diff: +24 / −25 across 6
files. Verified by:

- `git show --stat 23757379`: clean diff on expected files
- `git show --stat 99c0dcee`: cycle-artifacts commit (proposal + tasks)
- Pre-merge tree hygiene bundled: `cargo fmt` drift on
  `probe_backend.rs` and `clippy::new_without_default` on
  `NativeProbeBackend` (fixed with `impl Default { Self::new() }`)
  per AGENTS.md §4 option A.

No new exploration needed — the cycle has already shipped and is verified
by the merge to `main` plus the ratchet passing at baseline 0.

## Out of scope

- Opening the architectural `rec-c2.3-eventbus-removal` retroactively in
  this session. Same mechanics, but the cycle is large enough (5 commits,
  4 sub-cycles, ratchet work) that it deserves its own retroactive sync
  if/when desired.
- Doc-comment audit round 2 (option B in the handoff).
- REC-C3 fan-out audit (option C in the handoff).