# Exploration Report — m9-88-cc55-drift-remediation

## Identification

| Field | Value |
|---|---|
| Cycle | m9-88-cc55-drift-remediation |
| Path | B-direct (single vault commit, no source changes) |
| Date | 2026-09-14 |
| Inherited from | FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (deferred as out-of-scope) |
| Triggered by | vault-drift-sweep after m9-87 closure |

## Context

After m9-87 closure, `bash scripts/check_vault_drift.sh` reported
CC#55 (apply-checkpoint.json base_sha/head_sha must exist in git) with
1 drift line. Investigation revealed 3 distinct drifts:

1. **m9-66** (apply-checkpoint.json): line 35 has an invalid JSON escape
   (`'^\|'` raw bytes — Python 3.14's strict parser rejects it). This
   caused `json.load()` to raise JSONDecodeError for m9-66, which
   silently aborted any CC that loaded it (CC#3, CC#7, CC#8, CC#11,
   CC#12, CC#14, CC#15, CC#22, CC#23, CC#29, CC#40, CC#43, CC#55).
2. **m9-67** (apply-checkpoint.json + 3 others): base_sha was
   `67b3d76bb6e9...` (off-by-one from real parent `67b3d76a80ec...`).
3. **m9-85** (apply-checkpoint.json + 3 others): base_sha was
   `2c2a5cc8f837...` (non-existent commit).

## Why pre-existing

All three drifts were introduced by vault commits from earlier cycles
(m9-66, m9-67, m9-85 respectively). The previous "all CCs PASS" state
after m9-87 closure was misleading because m9-66's JSON error caused
many CCs to silently abort (returning no DRIFT lines), making the
meta-check (CC#48) report "0 drift" while multiple drift classes
existed.

This is the same failure pattern that m9-57 documented: a strict CC
that fails on malformed input silently passes if downstream CCs don't
inspect the failure.

## Scope decision

m9-88 is **scoped to CC#55 only** — the 3 drifts identified above.
The unblocked CCs (CC#3, CC#11, etc.) now surface pre-existing drift
in m9-77..m9-87 that is documented in the m9-88 commit message but
deferred to separate hardening cycles:

- CC#11 hardening: enforce `status: "CLOSED"` and add
  `findings_introduced` field to all apply-checkpoints.
- CC#3 hardening: extend era detection to accept
  `peel_match=False` for any cycle where head_sha is a merge commit
  (not just `fix-peel` cycles).
- archive-manifest backfill: add `archived_at` to apply-checkpoints
  whose archive-manifest.md exists.

## Path classification

B-direct because:

- Single vault commit on main (no source code touched).
- Scope is bounded (3 specific drift fixes).
- No architectural fork.
- No new files in chronos source.

## Verification approach

For each fix:

1. `git cat-file -t <base_sha>` and `<head_sha>` both report `commit`.
2. `json.load(open(apply-checkpoint.json))` succeeds (for m9-66).
3. CC#55 (and all CCs that load apply-checkpoints) run without
   JSONDecodeError.

After the fix:

- `bash scripts/check_vault_drift.sh` should report 0 lines from CC#55.
- Other CCs (CC#3, CC#11, etc.) may now report drift in cycles
  m9-77..m9-87 — these are pre-existing and tracked separately.

## Out of scope (not addressed by m9-88)

- FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK — `sddk cycle evaluate-gate`
  fails with `duplicate_event_id` + `ENGINE_UNREGISTERED_EVALUATOR`.
  Root cause is in the `sddk` CLI binary at
  `/home/rubentxu/.local/bin/sddk` (not chronos source). Cannot fix
  without modifying the sddk binary or filing upstream. Deferred to
  external follow-up.

- CC#3, CC#11 hardening (described above) — pre-existing drift
  classes that were always present but masked by the m9-66 JSON error.

- cc-001 god-module final slimming (extract integration tests) —
  optional, diminishing returns.

- M7 milestone — events_read merge, observe merge, lifecycle work.
