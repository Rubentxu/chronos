# Tasks — m9-89 cascade CC cleanup across m9-77..m9-88

## Phase: Apply

- [x] T1: Build `scripts/audit_m9_89_cascade.py` (per-cycle, per-CC fix catalog)
- [x] T2: Build `scripts/fix_m9_89_cascade.py` (mechanical backfill tool, idempotent, --dry-run)
- [x] T3: Run `python3 scripts/fix_m9_89_cascade.py` against clean tree; commit
- [x] T4: Run `python3 scripts/regen_manifest_index_shas.py` to rewrite SHA-256 cascade
- [x] T5: Update `terms/index.md` Last archive field to m9-89 (closes CC#6 mid-cycle)
- [x] T6: T0 fmt + clippy clean (verifies REQ-M9-89-04)
- [x] T7: Write m9-89 cycle artifacts (apply-checkpoint, implementation-receipt, verify-findings, verify-report, merge-receipt, release-receipt, release-report, change-entry, exploration-report, proposal, spec, tasks)
- [ ] T8: Add m9-89 row to `cycles/index.md` and bump Total cycles to 89
- [ ] T9: Merge chore branch into main with `--no-ff`
- [ ] T10: Pre-create tag `v0.7.91` at cascade SHA, move to merge SHA (CC#42 workaround)
- [ ] T11: Push to origin
- [ ] T12: Write archive-manifest.md to `changes/archive/m9-89-cascade-cc-cleanup-m9-77-87/`
- [ ] T13: Run `scripts/regen_manifest_index_shas.py` final time to capture new archive-manifest SHA
- [ ] T14: Write handoff document to `handoff/m9-89-cascade-cc-cleanup-m9-77-87-closure-2026-09-14.md`
- [ ] T15: Delete cycle branch (`chore/m9-89-cascade-cc-cleanup-m9-77-87`)

## Phase: Verify

- [ ] V1: `bash scripts/check_vault_drift.sh` reports clean (CC#6 + CC#46/CC#53 deferred are expected)
- [ ] V2: `python3 scripts/regen_manifest_index_shas.py --check` exits 0
- [ ] V3: `python3 scripts/fix_m9_89_cascade.py --dry-run` produces 0 changes (idempotent)
- [ ] V4: `cargo fmt --all -- --check` clean
- [ ] V5: `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] V6: `cargo test --workspace --lib --no-fail-fast -- --test-threads=1` matches m9-87 baseline
- [ ] V7: `git diff --stat main..HEAD -- '*.rs'` returns empty (REQ-M9-89-04)

## Phase: Release

- [ ] R1: Tag `v0.7.91` exists on origin, peel matches merge SHA
- [ ] R2: Apply-checkpoint.status = `CLOSED`, release_status = `released`, archive_status = `complete`
- [ ] R3: All cross-checks satisfied (apply-checkpoint.head == release-receipt.head == merge-receipt.head == cycle-artifacts SHA-cascade end)
- [ ] R4: Handoff document persisted to `handoff/` directory
- [ ] R5: CC sweep clean (only CC#6 + CC#46/CC#53 deferred expected)

## Out-of-scope (deferred)

- CC#46/CC#53 stale branches cleanup — FIND-M9-71 hardening cycle
- FIND-M9-81 sddk CLI bug — external-deferred from m9-88
