# Handoff — m9-83 closure (session 2026-09-14T11:00Z-11:18Z)

## Cycle summary

Closed **FIND-M9-83-CC39-TOTAL-CYCLES-OFF-BY-ONE** (pre-existing on
`main` since m9-78). Single-line literal fix in `cycles/index.md`:
`Total cycles` field 84 → 83 to match the actual row count.

B-direct path. No code, no schemas, no migration. Tag `v0.7.85`
created (points at commit `4214fbfe53b912bc6c5a1e9ea0de46931db1c833`
inside the cycle's commit chain, not the final fixpoint HEAD
`0bb5f33`, per the fixpoint-cascade workaround documented below).

## Artifacts produced

- `cycle-artifacts/p-3416cfb8288f8964/m9-83-cc39-total-cycles/`:
  7 files (apply-checkpoint.json, implementation-receipt.md,
  merge-receipt.md, release-receipt.md, release-report.md,
  verify-findings.json, verify-report.md).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-83-cc39-total-cycles/`:
  exploration-report.md, proposal.md, spec.md, tasks.md (T0) +
  change-entry.md (T4).
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-83-cc39-total-cycles/archive-manifest.md`.
- Tag `v0.7.85` created and pushed.

## CC state

- CC#39 (Total cycles vs row count): clean.
- All 48 python CCs + 7 bash CCs pass per `bash scripts/check_vault_drift.sh`.
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.

## Commit chain

```
0bb5f33 (HEAD -> main, origin/main) m9-83: bump SHAs to 4214fbf (matches v0.7.85 tag)
4214fbf [tag: v0.7.85]              m9-83: bump Head SHAs to fixpoint commit 2717a63
2717a63                               chore(vault): cascade SHAs after re-tag (final fixpoint)
81cec6b                               m9-83: tag v0.7.85 + cascade SHAs to fixpoint
a531c31                               chore(vault): regen archive-manifest SHAs to fixpoint (post-v0.7.85)
31c9803                               m9-83: tag v0.7.85 + cascade SHAs to fixpoint
a21dcc2                               m9-83: bump Head SHAs to merge commit c9f8977 + regen archive-manifest Evidence bindings
c9f8977                               Merge fix/m9-83-cc39-total-cycles into main
e7167bb                               m9-83: cycle artifacts + Total cycles literal fix
dca1df3                               m9-83: vault (exploration-report + proposal + spec + tasks)
e5eb0f0                               (pre-cycle main, m9-82 closure fixpoint)
```

## Non-standard tag location: v0.7.85 at 4214fbf, not at HEAD

The cycle's commit chain shows `v0.7.85` at `4214fbf`, which is the
parent of the final HEAD `0bb5f33`. This is non-standard (normally
a release tag should point at HEAD), but was deliberate to satisfy
CC#42 (`release-receipt.Remote tag_peel` must equal
`git rev-parse v0.7.85^{commit}`).

The chicken-and-egg:

1. To pass CC#42, `release-receipt.Remote tag_peel` must equal what
   `v0.7.85` actually points at.
2. After every commit, `HEAD` advances. The tag can only be created
   at a known SHA, so it stays at the SHA that existed at tag time.
3. The `release-receipt.Remote tag_peel` field is committed into the
   repo, so it must be set BEFORE the commit that "moves HEAD past it"
   is created.

Resolution: pre-create the tag at the SHA where the SHA-bump commit
will be authored, then commit the SHA bumps (HEAD advances), then push
the tag (still at the pre-commit SHA).

Consequence: `git describe` from HEAD will report
`v0.7.85-1-g0bb5f33` (one commit past the tag). Future cycles should
either:
- Use `--amend` to keep the same SHA, or
- Update the artifacts (release-receipt, archive-manifest, cycles/index)
  in a single final commit and tag at the resulting HEAD, OR
- Live with the `vN-1-g<sha>` describe offset.

The `release-receipt.md` documents the non-standard location.

## Carry-forward unchanged from prior handoff

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (still open).
- FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL (informational;
  documented in AGENTS.md §6.5).
- cc-001-god-module (`counterexample_storage.rs` at 2,821 lines, 5
  concerns; A-lite split).

The M7 milestone (deferred from M6) is larger-scope work:
events_read merge, observe merge, session_compare + session_explain
split, session_start/stop lifecycle, deprecation sunset sweep.

## Next candidates

After m9-83, the natural next moves are:

1. **m9-84 cc-001-god-module split** (A-lite; bounded cross-crate
   refactor). Pulling `counterexample_storage.rs` apart into 5
   focused modules (read-path / write-path / bundle storage /
   chunk storage / session-event link) with no behaviour change.
   Pre-existing m9-04 P2 debt.
2. **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (medium). Investigate the
   pre-existing FOREIGN KEY constraint bug in
   `/var/home/rubentxu/.local/state/sddk/projects/p-3416cfb8288f8964/ledger.sqlite`
   that blocks `sddk cycle start` gate evaluation.
3. **M7 milestone work** (A-full). The big deferred milestone.

Recommend next cycle: m9-84 cc-001-god-module (A-lite) since the
M9+ backlog needs that split before M7 work can proceed cleanly
(counterexample storage is on the M7 critical path).
