# Merge Receipt — m10-vault-last-updated-backfill

**Cycle**: `p-3416cfb8288f8964/m10-vault-last-updated-backfill`
**Path**: B-direct
**Branch**: `feat/m10-vault-last-updated-backfill`
**Base**: `b70b78ef1c1374f5ccc85e3faf6831753b080a42` (m10-ms-cap-discovery-followup fixpoint-cascade commit, also the merge commit of m10-ms-cap-discovery-followup before this cycle)
**Feature HEAD**: `943f9afe` (single commit)
**Merge commit** (`--no-ff`): `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af`
**Merge argv**: `git merge --no-ff feat/m10-vault-last-updated-backfill -m "Merge feat/m10-vault-last-updated-backfill into main (m10-vault-last-updated-backfill, B-direct)"`
**Merge exit code**: 0
**Merge output digest (stdout bytes)**: `sha256:` (trivial merge, no conflicts)

## Pre-merge subject

| Field | Value |
|---|---|
| Branch pre-state | main @ `b70b78ef` (m10-vault last fixpoint-cascade) |
| Tags on main pre-state | v0.7.103 → 3f7abc35 (m10-ms-cap-discovery); v0.7.104 → 11efb266 (m10-ms-cap-discovery-followup) |
| Working tree on main pre-state | clean (per `git status --short` before checkout to feat branch) |

## Post-merge subject

| Field | Value |
|---|---|
| main HEAD | `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` |
| Tag created on main | `v0.7.105` (annotated), peels to `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` |
| Push argv | `git push origin main`; `git push origin v0.7.105` |
| Push exit codes | 0 / 0 |
| Push outputs | `b70b78ef..fa92a6ee  main -> main`; `* [new tag]  v0.7.105 -> v0.7.105` |
| Net diff vs base | +49/-25 across 14 files (cycles/index.md + terms/index.md metadata + 12 archive-manifest.md SHA-rewrite cascade rows) |

## Subject binding

All artifacts on disk reference the merge SHA `fa92a6ee40aea67a9dbac9301a14fafd3c15b7af` as the published subject. `inventory.json` (cycle artifacts) hashes match the on-disk files. CC#4 `regen_manifest_index_shas.py --check` is clean (98 manifests verified).

## Carry-over

- m10-vault-last-updated-backfill is the next m10 m-series cycle after m10-ms-cap-discovery-followup (which itself followed m10-ms-cap-discovery; see `HANDOFF-2026-09-15-session-close.md` and `HANDOFF-2026-09-15b-session-close.md` for narrative).
- The HANDOFF-*.md files at `cycle-artifacts/p-3416cfb8288f8964/` remain un-relocated; they continue to trigger CC#18 false-positive drift lines (3). This is acknowledged as pre-existing infra noise and is the natural follow-up cycle (m10-vault-handoff-relocate or similar).
