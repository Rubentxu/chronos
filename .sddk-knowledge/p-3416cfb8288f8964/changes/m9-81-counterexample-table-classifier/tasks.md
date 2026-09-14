# Tasks: m9-81 chronos-store::counterexample_storage uses the canonical table_error helper

> Tasks are reviewable work units. One task = one commit = one
> acceptance check. The refactor is mechanically equivalent to m9-72's
> earlier work on `cas.rs` and `storage.rs`, so all 6 sites land in a
> single T1 commit (each site is a 4-line → 2-line substitution; doing
> them in 6 separate commits would be churn).

## T0 — Proposal + spec + exploration-report on cycle branch

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/exploration-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/proposal.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/spec.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/tasks.md`

**Acceptance:**
- All 4 files present on the cycle branch.
- The m9-81 row is added to `cycles/index.md` and `terms/index.md` with
  status `OPEN`.

**Commit message:**
> m9-81: vault (exploration-report + proposal + spec + tasks)

## T1 — Refactor 6 sites in counterexample_storage.rs

**Files:**
- `crates/chronos-store/src/counterexample_storage.rs` (add import;
  replace 6 inline `match` ladders with `classify_read_table_error(...).or_not_found(...)`)

**Acceptance:**
- `grep -n 'redb::TableError::TableDoesNotExist\|TableError::TableDoesNotExist' crates/chronos-store/src/counterexample_storage.rs` returns 0 matches.
- `grep -n 'classify_read_table_error' crates/chronos-store/src/counterexample_storage.rs` returns 6 matches.
- `cargo fmt --all -- --check` exits 0.
- `cargo clippy -p chronos-store --all-targets -- -D warnings` exits 0.
- `cargo test -p chronos-store --lib --no-fail-fast` exits 0 with the
  same passed/failed count as the cycle base.

**Commit message:**
> m9-81: route 6 counterexample_storage read paths through table_error

## T2 — Vault index update + CC sweep

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` (close m9-81
  row, bump Total cycles)
- `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` (record
  FIND-M9-72 closure; bump Last archive)
- `scripts/regen_manifest_index_shas.py` run to fixpoint
- `bash scripts/check_vault_drift.sh` to confirm CC#39 stays as the
  only known drift (pre-existing, unrelated)

**Acceptance:**
- `cycles/index.md` has m9-81 row with status `CLOSED`.
- `terms/index.md` records FIND-M9-72 closure under "Findings closed
  in m9-81".
- `scripts/check_vault_drift.sh` returns only the pre-existing
  CC#39 off-by-one (Total cycles says 81, actual folder count is
  80). This is independent of m9-81.

**Commit message:**
> m9-81: vault (cycles index + terms index + CC sweep)

## T3 — Verify phase artifacts (apply-checkpoint + receipts)

**Files:**
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/implementation-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/verify-report.md`

**Acceptance:**
- All 4 artifacts present and satisfy the schemas in CC#28
  (base_sha/head_sha), CC#30A-D (title/verdict/cycle/summary),
  CC#32 (Path field), CC#36 (lens_summary), CC#55 (Files
  Inventory).

**Commit message:**
> m9-81: verify-phase cycle-artifacts

## T4 — Release phase (merge --no-ff + tag + receipts)

**Files:**
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/release-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/T0-progress-note.md` (cycle narrative)

**Acceptance:**
- Cycle branch merged with `--no-ff` into main → merge SHA recorded
- `v0.7.83` tag created on the merge commit, peeled match verified
- `release-receipt.md` records tag, peel SHA, main SHA
- `merge-receipt.md` records merge SHA + clean-merge attestation

**Commit message:**
> m9-81: release-phase artifacts (v0.7.83)

## T5 — Archive phase (archive-manifest + change-entry)

**Files:**
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-81-counterexample-table-classifier/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-81-counterexample-table-classifier/archive-manifest.md`
- Cycle branch deleted (local + remote)

**Acceptance:**
- `change-entry.md` and `archive-manifest.md` present.
- `cycles/index.md` row `Status: OPEN` → `Status: CLOSED`.
- `terms/index.md` `Last archive` → m9-81 (or "m9-80" if there are
  more cycles in flight; m9-81 is the latest as of cycle start).
- Cycle branch deleted; reflog keeps the SHA reachable for ≥30 days.

**Commit message:**
> m9-81: archive phase (archive-manifest + change-entry)

## T6 — Apply-checkpoint status flip

**File:**
- `cycle-artifacts/p-3416cfb8288f8964/m9-81-counterexample-table-classifier/apply-checkpoint.json`

**Acceptance:**
- `status: "archived"`, `archive_status: "complete"`, paths
  populated.

**Commit message:**
> m9-81: apply-checkpoint status=archived + paths populated

## T7 — Handoff persistence

**File:**
- `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` (append new session)

**Acceptance:**
- New "Session 2026-09-14T…" section appended documenting the
  cycle's closure state and any open follow-ups.

**Commit message:**
> docs(handoff): append m9-81 closure (session 2026-09-14T…)
