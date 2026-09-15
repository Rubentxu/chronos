# Release Receipt — m10-vault-handoff-relocate

**Cycle**: `p-3416cfb8288f8964/m10-vault-handoff-relocate`
**Path**: B-direct
**Published subject (main HEAD)**: `340d64d969ba3df936f4c3e73720e8192d980d73` (merge commit)
**Tag**: `v0.7.106` (annotated)
**Tag peel (`vN^{commit}`)**: `340d64d969ba3df936f4c3e73720e8192d980d73` — matches main HEAD ✓
**Tag type**: annotated (`git tag -a`)
**Tag message**: `v0.7.106 — m10-vault-handoff-relocate (CC#18 close)`

## Remote tag (verification)

| Field | Value |
|---|---|
| Remote tag | `refs/tags/v0.7.106` |
| Remote tag_peel (`v0.7.106^{commit}`) | `340d64d969ba3df936f4c3e73720e8192d980d73` |
| Match between local peel and remote peel? | YES — `git rev-parse v0.7.106^{commit}` matches both |
| Push argv (tag) | `git push origin v0.7.106` |
| Push argv (main) | `git push origin main` |
| Push exit codes | 0 / 0 |
| Push output (main) | `94c1d59d..340d64d9  main -> main` |
| Push output (tag) | `* [new tag]  v0.7.106 -> v0.7.106` |

## Diff snapshot (base → published main HEAD)

| File | Change |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/` | git mv 4 HANDOFF-*.md files to handoffs/ (preserves history) |
| `cycle-artifacts/p-3416cfb8288f8964/handoffs/README.md` | new file |
| `cycle-artifacts/p-3416cfb8288f8964/handoffs/verify-findings.json` | new file (documentation dir marker) |
| `cycle-artifacts/p-3416cfb8288f8964/m10-vault-last-updated-backfill/verify-findings.json` | new file (B-direct synthesized) |

Total: 7 files, +7/-0.

## Branch hygiene

- Branch `feat/m10-vault-handoff-relocate` retained per archive rule (do not delete before manifest is persisted).
- Local branch exists (`git branch --list feat/m10-vault-handoff-relocate` → present).

## Operational change

Default operator behaviour is **byte-identical** to v0.7.105. The change is purely vault hygiene:
- CC#18 is now clean (0 drift lines)
- 4 HANDOFF files moved to handoffs/ subdir with git mv (history preserved)
- verify-findings.json synthesized for m10-vault-last-updated-backfill
- verify-findings.json marker added for handoffs/ (documentation dir)

No code changed, no tests changed.

## Acceptance evidence

- `cargo fmt --all -- --check` — clean (untouched code).
- `cargo clippy -p chronos-mcp -p chronos-services --all-targets -- -D warnings` — clean (untouched crates).
- `python3 scripts/regen_manifest_index_shas.py --check` — clean (98 manifests verified).
- `bash scripts/check_vault_drift.sh` — CC#18 clean (0 drift lines); CC#17 and CC#26 report pre-existing schema issues (out of scope).

## Status

RELEASED. Cycle proceeds to archive phase.
