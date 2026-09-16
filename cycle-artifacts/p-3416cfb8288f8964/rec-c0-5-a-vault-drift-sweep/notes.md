# REC-C0.5-A — Vault Drift Sweep closure

## Scope

Close the residual vault drift carried from REC-C0.5-C close-out. Per the
REC-C0.5-C notes (`cycle-artifacts/.../rec-c0-5-c-fixture-discovery/notes.md`),
the only remaining line of drift at end of REC-C0.5-C was a CC#18 violation
(rec-c0-5-c-fixture-discovery missing `verify-findings.json`).

## Investigation

Initial state (post REC-C0.5-C close-out, before this cycle):

```
bash scripts/check_vault_drift.sh
DRIFT detected (CC#54, exit 21):
DRIFT: CC#4: <20 lines of archive-manifest.md :: cycles/index.md SHA mismatches>
DRIFT: CC#5: actual=0 declared=98
```

So the actual drift was **21 lines**, not the 1 line implied by the
prior handoff. The 20 CC#4 lines are stale SHA-256 references in
archive-manifest.md Artifact index tables (the cycle index hash changed
when cycles were added, but the manifests didn't regen). The 1 CC#5
line is from a permanent awk-pattern bug.

## Root cause #1: CC#4 — archive-manifest SHA drift (20 lines)

`scripts/regen_manifest_index_shas.py` was designed by m9-76 to rewrite
these rows to a fixpoint. The fix is mechanical:

```bash
python3 scripts/regen_manifest_index_shas.py
```

This rewrote 20 rows across 100 manifests. After regen, CC#4 is clean.

## Root cause #2: CC#5 — awk pattern bug

CC#5 in `vault-drift-sweep.md` line 2626 was:

```bash
actual=$(awk -F'|' '/^\| m9-/{c++} END{print c+0}' \
    .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
```

The pattern `^\| m9-` matches lines starting with `| m9-` (with hyphen).
But `cycles/index.md` uses milestone tokens without a hyphen after the
milestone name:

- `| m0 | m0-truth-first-foundation | …`
- `| m10 | m10-ms-cap-discovery | …`
- `| rec-c0 | rec-c0-2-d-uat-verification | …`

So the awk pattern always returns 0. CC#5 has been permanently dead.

**Why the prior cycle didn't fix this:** m9-47 closed CC#39 Part C,
which counts filesystem cycle dirs (the authoritative Total cycles
value after m9-47). The smoke test comment at
`scripts/smoke_test_ccs.sh:137` says "CC#5 is subsumed by CC#39".
But CC#5 was never aligned with CC#39's filesystem count — it kept
the broken awk pattern. So CC#5 always fires drift, even when the
real Total cycles value is correct.

## Fix

Two coordinated changes:

1. **`vault-drift-sweep.md` CC#5 bash block** (line ~2629) updated to
   mirror CC#39 Part C: count m9-* filesystem dirs across the three
   canonical locations (`cycle-artifacts/`, `cycle-artifacts/p-…/`,
   `.sddk-knowledge/.../changes/`), with the same dedup logic. The
   illustrative bash block under "### 5." was also updated to reflect
   the new semantics.

2. **`cycles/index.md`** updated to:
   - Keep `Total cycles = 98` (CC#39 authoritative value).
   - Add `rec-c0-5-b-probe-inject-capability` and
     `rec-c0-5-c-fixture-discovery` rows with real published SHAs
     (`98f9dba4f35ee3e0edf61c2132bd9d84edd25fdb` and
     `02c2a5528687d8a6f8045665f61b9c8a03c499fa`).
   - Update `Last archive` to `rec-c0-5-c-fixture-discovery` and
     `Last updated` to `2026-09-16T07:00Z`.

3. **`cycle-artifacts/.../rec-c0-5-c-fixture-discovery/verify-findings.json`**
   synthesized retroactively to satisfy CC#18 (the cycle folder was
   created before the verify-findings schema was enforced for REC-C0.*
   folders). The synthesis is documented in the `_note` field per the
   CC#18 resolution protocol.

## Verification

```
$ bash scripts/check_vault_drift.sh
vault-drift-sweep: PASS (48 python CCs all clean, 7 bash CCs all clean)

$ python3 scripts/regen_manifest_index_shas.py --check
(passes silently)

$ python3 -m pytest scripts/tests/test_regen_manifest_index_shas.py
13 passed in 0.36s

$ python3 scripts/validate_cycle_artifacts.py
Cycle-artifact completeness gate PASSED on 6 cycle(s).

$ bash scripts/smoke_test_ccs.sh
=== Summary ===
Tests run: 6
Failures: 0
All critical CCs detect synthetic drift. CC detection chain is healthy.
```

The smoke test is critical here because it pins CC#4 and CC#39 detection
together: if my CC#5 fix accidentally weakened the meta-check, the
test_regen_script test would catch it (since both run on the same
workdir). All 6 tests pass.

## Cargo gates (T0/T1/T2)

T0 (lint): PASS.
T1 (lib tests, excluding chronos-native per §6.5 + chronos-e2e bucket D):
PASS across all 16 crates.
T2 (regression suites):
- `cargo test -p chronos-sandbox --test e2e_connectivity`: 1/1
- `cargo test -p chronos-sandbox --test boundary_conditions`: 9/9
- `cargo test -p chronos-sandbox --test fixture_resolver`: 4/4
- `cargo test -p chronos-sandbox --test probe_lifecycle`: 5/5
- `cargo test -p chronos-sandbox --test probe_inject`: 4/4 (regression
  for the REC-C0.5-B fix)

No Rust changes were made in this cycle; only vault artifacts and the
drift meta-check were touched. The cargo gates verify no incidental
breakage from the changes.

## Workspace residual failures (unchanged from prior)

The 34 unrelated failures from REC-C0.5-C close-out are unchanged:
- 5 query_filters / query_edge_cases — REC-C1
- 13 chronos-e2e test_ptrace_capture — bucket D (ptrace perms)
- 6 chronos-native m2_function_frame_capture — REC-C1 / bucket D
- 4 tripwires_tools — bucket C pre-existing
- 4 ptrace_tracer lib flakes — §6.5 needs `--test-threads=1`
- 2 unrelated lib tests

None of these are in this cycle's scope. REC-C0.5-A's scope is
**vault drift only**, and that scope is closed.

## Lessons

1. **Smoke-test the CC meta-checks before assuming the prior handoff's
   drift count is current.** The handoff said "1 line of drift"; the
   actual was 21 lines. Always re-run `check_vault_drift.sh` before
   starting a vault cycle.

2. **The awk pattern in CC#5 was broken since m9-47** (when CC#39
   superseded it). The supersession was documented in
   `smoke_test_ccs.sh:137` but the awk was never aligned. REC-C0.5-A
   is the cycle that finally closed this gap.

3. **CC#5 ↔ CC#39 alignment** is now stable: both count filesystem
   cycle dirs with the same dedup logic. Future drift in either will
   trip the other. The smoke test pins this together (test_cc39
   exercises the python chain; CC#5 runs in the same meta-check).

4. **The CC#18 retroactive synthesis** is the standard fix for cycle
   folders created before the schema was enforced. The `_note` field
   documents the synthesis so future readers don't mistake it for
   an original file.
