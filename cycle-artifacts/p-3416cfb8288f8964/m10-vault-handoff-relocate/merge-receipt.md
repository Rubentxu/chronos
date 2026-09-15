# Merge Receipt — m10-vault-handoff-relocate

**Cycle**: `p-3416cfb8288f8964/m10-vault-handoff-relocate`
**Path**: B-direct
**Branch**: `feat/m10-vault-handoff-relocate`
**Base**: `94c1d59d7c8728bdb0bf97df1cef1e3311a57cc5` (main)
**Feature HEAD**: `b4551186` (single commit)
**Merge commit** (`--no-ff`): `340d64d969ba3df936f4c3e73720e8192d980d73`
**Merge argv**: `git merge --no-ff feat/m10-vault-handoff-relocate -m "Merge feat/m10-vault-handoff-relocate into main (m10-vault-handoff-relocate, B-direct)"`
**Merge exit code**: 0
**Merge output digest (stdout bytes)**: `sha256:` (trivial merge, no conflicts)

## Pre-merge subject

| Field | Value |
|---|---|
| Branch pre-state | main @ `94c1d59d` |
| Tags on main pre-state | v0.7.105 → fa92a6ee (m10-vault-last-updated-backfill) |
| Working tree on main pre-state | clean (per `git status --short` before checkout to feat branch) |

## Post-merge subject

| Field | Value |
|---|---|
| main HEAD | `340d64d969ba3df936f4c3e73720e8192d980d73` |
| Tag created on main | `v0.7.106` (annotated), peels to `340d64d969ba3df936f4c3e73720e8192d980d73` |
| Push argv | `git push origin main`; `git push origin v0.7.106` |
| Push exit codes | 0 / 0 |
| Push outputs | `94c1d59d..340d64d9  main -> main`; `* [new tag]  v0.7.106 -> v0.7.106` |
| Net diff vs base | +7/-0 across 7 files |

## Subject binding

All artifacts on disk reference the merge SHA `340d64d969ba3df936f4c3e73720e8192d980d73` as the published subject. `inventory.json` (cycle artifacts) hashes match the on-disk files. CC#4 `regen_manifest_index_shas.py --check` is clean.
