# Spec — m9-92-cc34-cc42-cleanup

## Behavioral requirements

### REQ-M9-92-01: change-entry.md `## Cross-check` section presence (CC#34)

**Added**: every `change-entry.md` for cycles m9-19+ must contain
either a `## Cross-check` section or a `## Verification` section.

- Target files (this cycle):
  - `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md`
  - `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-90-stale-branches-cleanup/change-entry.md`

The added section is a bulleted list of vault-side cross-checks
that were performed (CC#46 branches deleted, idempotency, etc.)
plus an explicit pointer to the cycle's `verify-report.md` and
`apply-checkpoint.json`.

### REQ-M9-92-02: release-receipt / release-report / archive-manifest tag_peel alignment to immutable tag (CC#42)

**Modified**: the `Remote tag_peel` field in m9-90 release-receipt
must equal the current `git rev-parse v0.7.92^{commit}` value
(`5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`).

- The cycle-artifacts commit (`2184975a`) is preserved as a
  reference (still documents where the tag was originally created)
  but the `Remote tag_peel` field is updated to the immutable
  current tag location.
- Same fix to `release-report.md` and `archive-manifest.md`.

The m9-90 release-receipt's `## Release notes` is updated to
explicitly document the tag-push that occurred after m9-90 closure,
matching the m9-89 pattern (m9-89 handoff: "applied CC#42
fixpoint-cascade workaround: re-aligned m9-89 release-receipt back
to v0.7.91's actual peel (47a10f8)").

### REQ-M9-92-03: cycle-artifacts for m9-92 itself

**Added**: 7 cycle artifacts in
`cycle-artifacts/p-3416cfb8288f8964/m9-92-cc34-cc42-cleanup/` and
the corresponding `apply-checkpoint.json` at the repo root.

### REQ-M9-92-04: vault artifacts for m9-92

**Added**:
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-92-cc34-cc42-cleanup/`:
  exploration-report, proposal (already written), spec (this file),
  tasks, change-entry.
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-92-cc34-cc42-cleanup/`:
  archive-manifest with SHA-256 evidence bindings.
- `.sddk-knowledge/p-3416cfb8288f8964/handoff/`:
  closure handoff (2026-09-14).

### REQ-M9-92-05: cycles/index.md bump

**Modified**:
- Add m9-92 row referencing the cascade-final commit SHA.
- Bump Total cycles 91 → 92.
- Bump Last updated.

### REQ-M9-92-06: terms/index.md Last archive bump

**Modified**: Last archive = `m9-92-cc34-cc42-cleanup`.

## Scenarios

1. **CC#34 fix verified**: `bash scripts/check_vault_drift.sh`
   reports 0 `CC#34` drift lines (was 2 pre-cycle).
2. **CC#42 fix verified**: `bash scripts/check_vault_drift.sh`
   reports 0 `CC#42 A` drift lines (was 1 pre-cycle); specifically
   the m9-90 release-receipt's stored peel matches the current tag.
3. **No new drift introduced**: CC#3, CC#4, CC#8, CC#11, CC#12,
   CC#23, CC#39, CC#47 all remain clean (the m9-91 invariants).
4. **SHA-256 fixpoint preserved**: `python3 scripts/regen_manifest_index_shas.py
   --check` exits 0 with 0 manifests needing update.

## Acceptance criteria

- `bash scripts/check_vault_drift.sh` exits 0 (no drift).
- `python3 scripts/regen_manifest_index_shas.py --check` exits 0
  (no fixpoint drift).
- `git show v0.7.94` points at the cycle-artifacts commit (CC#42
  fixpoint-cascade workaround applied).
- `cycles/index.md` Total cycles = 92.
- All 7 cycle artifacts present for m9-92.
- Vault CCs m9-89, m9-90, m9-91, m9-92 all clean.

## Out of scope

- **REQ-M9-92-99 (excluded)**: any change to Rust source code.
  This is strictly a B-direct vault-only hardening cycle.
- **REQ-M9-92-100 (excluded)**: addressed-elsewhere m9-93+
  follow-ups (see proposal.md `Out of scope` section).
