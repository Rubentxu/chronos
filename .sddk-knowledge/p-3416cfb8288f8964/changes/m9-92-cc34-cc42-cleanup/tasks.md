# Tasks — m9-92-cc34-cc42-cleanup

## Phase 1 — Vault edits (CC#34 + CC#42)

### T1.1 CC#34: add Cross-check to m9-89 change-entry

File: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md`

Insert `## Cross-check` section after the existing `## Drift delta` /
before `## Out-of-scope` (or other appropriate location). Content:

```markdown
## Cross-check

- **CC#1..CC#8 vault invariants**: pass.
- **CC#46 (no stale local feat/m9-*/fix/m9-*/chore/m9-*)**: clean
  (pre-m9-89 was 11; m9-89 deleted those merged into main).
- **CC#53 (no stale branches merged into main)**: clean (pre-m9-89
  was 11; m9-89 deleted them).
- **bash scripts/check_vault_drift.sh**: clean (post-m9-89).
- **apply-checkpoint.json head_sha = tag-peel**: verified.
- **SHA-256 evidence bindings fixpoint**: `python3 scripts/regen_manifest_index_shas.py --check` exits 0.
- **Tag immutable**: v0.7.91 at `47a10f8`; release-receipt was
  re-aligned to that peel via the CC#42 fixpoint-cascade workaround
  (per m9-89 closure handoff).
```

### T1.2 CC#34: add Cross-check to m9-90 change-entry

File: `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/change-entry.md`

Insert `## Cross-check` section with same template as T1.1,
adapted to m9-90's drift line count (9 stale branches).
Reference the `clean_m9_90_stale_branches.py` tool's idempotency.

### T1.3 CC#42: align m9-90 release-receipt tag_peel

File: `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/release-receipt.md`

- Change `Remote tag_peel | 2184975a93b43ea1bbd2681dead79dfb4476fef7`
  to `Remote tag_peel | 5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`.
- Update `Head SHA` and `Main SHA` to reflect the immutable tag
  location.
- Update `## Release notes` to document the CC#42 fixpoint-cascade
  workaround applied (mirror m9-89's wording).

### T1.4 CC#42: cascade the tag alignment to m9-90 release-report + archive-manifest

Files:

- `cycle-artifacts/p-3416cfb8288f8964/m9-90-stale-branches-cleanup/release-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-90-stale-branches-cleanup/archive-manifest.md`

Apply same SHA changes as T1.3.

## Phase 2 — Cycle artifacts

### T2.1 Write 7 m9-92 cycle artifacts

`cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/`:

- `apply-checkpoint.json`
- `implementation-receipt.md`
- `merge-receipt.md`
- `release-receipt.md`
- `release-report.md`
- `verify-findings.json`
- `verify-report.md`

### T2.2 Update root apply-checkpoint.json

Replace m9-91 with m9-92 cycle pointer at repo root.

### T2.3 Write m9-92 archive-manifest

At `.sddk-knowledge/.../changes/archive/m9-92-cc34-cc42-cleanup/`,
write a fresh archive-manifest.md with SHA-256 evidence bindings
for all 7 cycle artifacts + 4 vault files (this is the
`apply-checkpoint` to write).

## Phase 3 — Merge + cascade

### T3.1 Merge --no-ff to main

After vault phase commits:
- `git tag v0.7.94 <cycle-artifacts-commit>`
- `git checkout main && git pull --ff-only origin main && git merge --no-ff chore/m9-92-cc34-cc42-cleanup`

### T3.2 SHA-cascade fixpoint

Run regen tool:
- `python3 scripts/regen_manifest_index_shas.py`
Then iterate until cascade converges (typically 1-3 commits). Move
the v0.7.94 tag once more to the cascade-final HEAD.

## Phase 4 — Push + cleanup

### T4.1 Push main + tag

- `git push origin main`
- `git push origin v0.7.94`

### T4.2 Delete local cycle branch

- `git branch -d chore/m9-92-cc34-cc42-cleanup`
- (Remote is not pushed to, no `--delete` needed.)

## Commits (forecast)

- `m9-92: vault edits (CC#34 change-entries + CC#42 m9-90 tag alignment)`
- `m9-92: cycle artifacts (apply-checkpoint, 6 receipts/reports + vault files)`
- `Merge branch 'chore/m9-92-cc34-cc42-cleanup' into main`
- `m9-92: cascade SHAs after release + archive-manifest SHA-256 fixpoint`
- (optional) `m9-92: bump Head SHAs to fixpoint commit X`
- `m9-92: closure handoff`

## Forecast budget

- LoC: ~80 lines (vault edits are small; cycle artifacts are
  ~600 lines but are documentation, not source).
- Time: 5-10 minutes wall (writing artifacts + running regen).
- Tier: T0 only (no Rust touched; no tests required).
