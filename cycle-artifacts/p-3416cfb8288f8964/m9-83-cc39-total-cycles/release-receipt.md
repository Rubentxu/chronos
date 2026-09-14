# Release Receipt — m9-83-cc39-total-cycles

## Identification

| Field | Value |
|---|---|
| Cycle | m9-83-cc39-total-cycles |
| Path | B-direct |
| Branch | fix/m9-83-cc39-total-cycles |
| Date | 2026-09-14 |

## SHAs

| Field | Value |
|---|---|
| Base SHA | a0f72c2a7fe36eaeb9c772505dfe563f85f42773 |
| Head SHA | e5eb0f03c64a3f4a647f3edd0a164b057db98e37 |
| Main SHA | e5eb0f03c64a3f4a647f3edd0a164b057db98e37 |
| Remote tag | — |
| Remote tag_peel | — |
| Peel match | n/a (no tag: trivial B-direct fix with no code changes) |

## Release notes

- No tag created: trivial B-direct literal fix to vault metadata
  (cycles/index.md `Total cycles` field). No code, no schemas, no
  release artefact needed.
- ff_merged=true: single commit on main directly (no merge commit).
- Cross-checks satisfied:
  - apply-checkpoint.head_sha == release-receipt.head_sha
  - apply-checkpoint.base_sha == release-receipt.base_sha
  - apply-checkpoint.main_sha == release-receipt.main_sha

## Verification

- `python3 scripts/regen_manifest_index_shas.py --check`: passes.
- `bash scripts/check_vault_drift.sh`: only CC#48 + CC#51 (pre-existing
  script-level drift) reported; CC#39 clean.
