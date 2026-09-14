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
| Head SHA | 81cec6bbcf889b114ab3317575e2b87c78d18a82 |
| Main SHA | 81cec6bbcf889b114ab3317575e2b87c78d18a82 |
| Remote tag | v0.7.85 |
| Remote tag_peel | 81cec6bbcf889b114ab3317575e2b87c78d18a82 |
| Peel match | clean peel: v0.7.85 points at a531c31 (fixpoint HEAD) |

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
