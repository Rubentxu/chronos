# Proposal — m9-92-cc34-cc42-cleanup

## Intent

Close the 3 pre-existing CC drift lines that surfaced after m9-91
closure:

1. **CC#34 A1**: `m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md`
   lacks a `## Cross-check` (or `## Verification`) section.

2. **CC#34 A2**: `m9-90-stale-branches-cleanup/change-entry.md`
   lacks a `## Cross-check` section.

3. **CC#42 A**: `m9-90-stale-branches-cleanup/release-receipt.md`
   stores `Remote tag_peel = 2184975a93b43ea1bbd2681dead79dfb4476fef7`
   but the immutable `v0.7.92` tag now points at
   `5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d`
   (the post-cascade HEAD, written over the original peel after
   m9-90's SHA-256 fixpoint cascade landed). The same drift
   propagates to `cycle-artifacts/.../m9-90-stale-branches-cleanup/release-report.md`
   and `archive-manifest.md`.

## Scope

**In scope (m9-92):**

- Add `## Cross-check` section to m9-89 + m9-90 change-entries
  (CC#34 fix; 2 files).
- Re-align `Remote tag_peel` field in m9-90 release-receipt +
  release-report + archive-manifest to `5cdb4e1` (CC#42 fix; 3
  files).
- Cycle artifacts for m9-92 itself (`apply-checkpoint.json`,
  implementation-receipt, merge-receipt, release-receipt,
  release-report, verify-findings.json, verify-report.md).
- Vault artifacts (`exploration-report.md`, `proposal.md`,
  `spec.md`, `tasks.md`, `change-entry.md`).
- Handoff + archive manifest.
- Bump `cycles/index.md`: Total cycles 91 → 92 + new m9-92 row.
- Bump `terms/index.md`: Last archive = m9-92.
- Cascade SHA-256 rows in any archive-manifest that previously
  referenced affected files.

**Out of scope (deferred to m9-93+):**

- Any Rust source code change. This is B-direct vault-only.
- `FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK` (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.
- `FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA` (opened by m9-91):
  separate cycle for adding `JsonSchema` to
  `chronos_domain::TraceEvent`.
- `cc-001-god-module` and `cc-004-implicit-io-toctou` (deferred
  to m10+): not addressable from a vault-only cycle.

## Risk and verification

- Build: no Rust touched; no rebuild required.
- CC verification: `bash scripts/check_vault_drift.sh` should
  report 0 drift after the cascade.
- Tool verification: `python3 scripts/regen_manifest_index_shas.py
  --check` should remain clean (the manifest SHA-256 rows need to
  be rewritten once more for the affected files).

## Tag plan

Tag `v0.7.94` pre-created at the cycle-artifacts commit, moved
through the SHA-cascade fixpoint commits per CC#42
fixpoint-cascade workaround (m9-83 handoff pattern).

Patch bump (v0.7.93 → v0.7.94) per AGENTS.md §5.
