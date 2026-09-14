# Proposal — m9-88-cc55-drift-remediation

## Intent

Close the 3 pre-existing CC#55 drift lines that surfaced after m9-87
closure:

1. Fix `m9-66-bash-cc-meta-check/apply-checkpoint.json` line 35:
   promote `'^\|'` (invalid JSON escape) to `'^\\|'` (the actual
   regex characters `^\|` stored as a JSON string).

2. Fix `m9-67-cc-smoke-test/*` (4 files): correct the off-by-one
   base_sha from `67b3d76bb6e9...` to `67b3d76a80ec8e766a0689fba810fc499b5cd4a4`
   (the real parent of the cycle's head_sha).

3. Fix `m9-85-cc001-god-module-impl-split/*` (4 files): correct the
   non-existent base_sha from `2c2a5cc8f837...` to
   `72e120c2e2bb9774c459ef516dcf3e7b21ef90c3` (the real parent of
   the cycle's head_sha).

4. (Bonus) Fix `m9-79-attach-capability-type/apply-checkpoint.json`:
   set `peel_match: True` (head==peel verified in git, was None).

## Scope

**In scope (m9-88):**
- The 4 fixes above (10 files total: 3 apply-checkpoint.json + 6
  companion files + 1 bonus).
- Cycle artifacts for m9-88 itself.
- Vault artifacts (exploration, proposal, spec, tasks, archive).
- Handoff documentation.

**Out of scope (deferred):**
- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK — `sddk` CLI binary bug.
- Pre-existing drift that surfaces after m9-66 fix:
  - CC#3 (peel_match=None for m9-77..m9-87) — 11 lines.
  - CC#7, CC#8, CC#11, CC#12, CC#14, CC#15, CC#22, CC#23, CC#29,
    CC#40, CC#43 — various pre-existing drifts.
- cc-001 final slimming.
- M7 milestone work.

## Approach

**Single vault commit on main:**

1. Apply each fix using `python3` (to handle JSON re-serialization
   safely and validate).
2. Verify each fix:
   - `git cat-file -t <sha>` reports `commit`.
   - `json.load(open(apply-checkpoint.json))` succeeds.
   - `bash scripts/check_vault_drift.sh` runs without
     JSONDecodeError.
3. Commit on `feat/m9-88-cc55-drift-remediation` branch.
4. `--no-ff` merge into main.
5. Tag `v0.7.90` (pre-created at the merge commit, per the CC#42
   fixpoint-cascade workaround).
6. Push.

## Risks

- **JSON re-serialization key reorder:** `python3 -c "json.dumps(j, indent=2)"`
  does not preserve key order. Manual ordering required to keep diffs
  minimal. Mitigation: read the original field order from git, then
  write back with the same order.

- **JSON re-serialization array format:** `json.dumps(j, indent=2)`
  formats inline arrays as multi-line, changing line counts. Mitigation:
  accept the formatting change since the JSON content is equivalent.

- **Cascade SHA effects:** changing m9-66 / m9-67 / m9-85 SHA values
  may affect downstream checks (CC#4 archive-manifest SHA cross-checks,
  cycles/index.md SHA column, evidence bindings). Mitigation: run
  `python3 scripts/regen_manifest_index_shas.py` after the fix to
  cascade SHAs to fixpoint.

## Estimated work

- Apply 4 fixes: ~10 min
- Verify: ~5 min
- Commit + merge + tag: ~5 min
- Cascade SHA fixpoint: ~2 min
- Cycle artifacts + handoff: ~15 min

Total: ~40 min wall time.

## Carry-forward

After m9-88 closure:

- CC#55: PASS (the original goal).
- CC#3, CC#7, CC#8, CC#11, CC#12, CC#14, CC#15, CC#22, CC#23, CC#29,
  CC#40, CC#43: drift will surface (was previously masked by m9-66
  JSON error). Recommend a follow-up hardening cycle (m9-89?) to
  address the cascade of pre-existing drift that m9-88 reveals.
