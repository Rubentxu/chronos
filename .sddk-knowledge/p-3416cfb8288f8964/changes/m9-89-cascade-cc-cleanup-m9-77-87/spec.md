# Spec — m9-89 cascade CC cleanup across m9-77..m9-88

## Requirements

### REQ-M9-89-01: Close 12 cascading CC drift lines
The cycle must close drift lines for CC#3, #7, #8, #11, #12, #14, #15, #22, #23, #29, #40, #43 across m9-77..m9-88 cycles.

**Verification**: `bash scripts/check_vault_drift.sh` reports ≤ 2 drift lines (CC#6 mid-cycle + CC#46/CC#53 deferred).

### REQ-M9-89-02: Regenerate archive-manifest SHA-256 rows
CC#4 SHA-256 drift from the previous step's edits must be regenerated via `scripts/regen_manifest_index_shas.py`.

**Verification**: `python3 scripts/regen_manifest_index_shas.py --check` exits 0.

### REQ-M9-89-03: Provide reusable vault-drift fix tooling
`scripts/audit_m9_89_cascade.py` and `scripts/fix_m9_89_cascade.py` must be checked in, documented, and idempotent.

**Verification**:
- Running `scripts/fix_m9_89_cascade.py --dry-run` on a clean tree produces 0 changes.
- Running `scripts/fix_m9_89_cascade.py` twice produces the same result (idempotent).

### REQ-M9-89-04: Zero Rust source code touched
This cycle must not modify any `.rs` files outside of the existing tools/ directory.

**Verification**: `git diff --stat main..chore/m9-89-cascade-cc-cleanup-m9-77-87 -- '*.rs'` returns empty.

### REQ-M9-89-05: Update cycles/index.md with m9-89 row
The m9-89 row must be added to `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` and Total cycles bumped to 89.

**Verification**: `grep "m9-89" cycles/index.md` returns 2 lines (table row + metadata Last archive field).

### REQ-M9-89-06: Close the cycle via SDDK ceremony
The cycle must close via:
- `git checkout main` (after merge)
- `git merge --no-ff chore/m9-89-cascade-cc-cleanup-m9-77-87` 
- `git tag v0.7.91 <merge-sha>` (after pre-creating at cascade SHA per CC#42)
- `git push origin main v0.7.91`
- Archive manifest to `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-89-cascade-cc-cleanup-m9-77-87/`
- Handoff document to `.sddk-knowledge/p-3416cfb8288f8964/handoff/`

## Scenarios

### Scenario 1: Run drift check after fix
- **Given**: `scripts/fix_m9_89_cascade.py` has been run on a clean tree.
- **When**: `bash scripts/check_vault_drift.sh` is executed.
- **Then**: Exit code is 1, with ≤ 2 drift lines (CC#6 mid-cycle + CC#46/CC#53 deferred).
- **And**: All 12 cascading CCs (3, 7, 8, 11, 12, 14, 15, 22, 23, 29, 40, 43) are clean.

### Scenario 2: Run fix tool on clean tree
- **Given**: A tree where m9-89 fixes have already been applied.
- **When**: `python3 scripts/fix_m9_89_cascade.py` is run.
- **Then**: 0 changes are produced (idempotent).
- **And**: No files are modified.

### Scenario 3: Run SHA-256 regen after archive edits
- **Given**: The cascade fix has modified archive-manifest files.
- **When**: `python3 scripts/regen_manifest_index_shas.py --check` is run.
- **Then**: Exit code is 0 (all SHAs at fixpoint).

## Out-of-scope

- Source code changes (REQ-M9-89-04 explicitly forbids them).
- Stale branch cleanup (CC#46/CC#53) — deferred to FIND-M9-71 hardening.
- FIND-M9-81 sddk CLI bug — external-deferred from m9-88.
- New vault cross-checks (would be a follow-up cycle if needed).
