# Archive Manifest — m10-vault-handoff-relocate (initial)

**Cycle**: `p-3416cfb8288f8964/m10-vault-handoff-relocate`
**Path**: B-direct
**Status**: RELEASED → CLOSED (post `archive.complete`)
**Base SHA**: `94c1d59d7c8728bdb0bf97df1cef1e3311a57cc5`
**Merged SHA**: `340d64d969ba3df936f4c3e73720e8192d980d73`
**Tag**: `v0.7.106`
**Subject binding**: tag peel `v0.7.106^{commit}` = `340d64d969ba3df936f4c3e73720e8192d980d73` ✓ matches main HEAD

## Cycle artifacts (verified, SHA-bound)

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/merge-receipt.md` | `81800c3f6e9c0795933da5f09dc41eebf92f9fc5c47e3543bc7e3223ef1f712e` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/release-receipt.md` | `691d98ad1b9048e7f01d9c073ba445afe7956a14c00e7c6a265f2da1404aa44f` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/release-report.md` | `08a1eedb2ccc358c9902fa808f442b577833f8cc4e4c0c2c77ae15396b9fa356` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/implementation-receipt.md` | `62665ebe00e6b4c475b70fdd255723ab76c3a638eca8b480300f2edb0f1c0cf7` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/archive-report.md` | `(finalized post-transition)` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/archive-manifest.md` | `(finalized post-transition)` |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-handoff-relocate/reports/cierre.html` | `26c1e065421797099694e3753a17569a3b789050742d7951436e237bbe2b3fb2` |

## Source change (single commit)

| Path | Lines |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/` | git mv 4 HANDOFF-*.md files to handoffs/ |
| `cycle-artifacts/p-3416cfb8288f8964/handoffs/README.md` | new file |
| `cycle-artifacts/p-3416cfb8288f8964/handoffs/verify-findings.json` | new file |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-last-updated-backfill/verify-findings.json` | new file |

Total: 7 files, +7/-0. No Rust code touched.

## CC sweep evidence

| Check | Result |
|---|---|
| `python3 scripts/regen_manifest_index_shas.py --check` | clean (98 manifests) ✓ |
| `bash scripts/check_vault_drift.sh` | CC#18 clean (0 drift lines) ✓; CC#17 and CC#26 report pre-existing schema issues (out of scope) |

## Status: CLOSED

Cycle closed at `2026-09-15T09:29Z`. CC#18 drift is now resolved. All 4 HANDOFF files relocated to handoffs/ subdirectory. verify-findings.json synthesized for m10-vault-last-updated-backfill and handoffs/.
